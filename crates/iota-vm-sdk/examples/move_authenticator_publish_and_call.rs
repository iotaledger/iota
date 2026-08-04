// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Publish a Move package and call into it with `MoveAuthenticator`-signed
//! transactions, entirely offline.
//!
//! The account is set up first, with two ordinary unsigned runs: publish
//! `examples/move/account` (whose `init` shares an `Account`) and call its
//! `link_auth` to attach the `#[authenticator]` function to that `Account`.
//! From then on the account object's address is the sender, and every
//! transaction is authorized by running that Move function in the VM:
//!
//! 1. publish `examples/move/view_functions` — its `init` shares a `Counter`;
//! 2. call `counter::increment` on the shared `Counter`.
//!
//! Two unsigned dev-inspect runs of the `#[view]` function `counter::value`
//! read the counter before and after the increment.
//!
//! Every state-changing run uses `ExecutionMode::Execute`, so each
//! transaction's effects are committed to the [`InMemoryStore`] and are visible
//! to the next one.
//!
//! Run with:
//!   cargo run -p iota-vm-sdk --example move_authenticator_publish_and_call

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use iota_move_build::{BuildConfig, CompiledPackage};
use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::{
    Identifier, MoveAuthenticatorV1, MoveStruct, ObjectId, Owner, SharedObjectReference,
    TransactionDigest, Version,
};
use iota_types::{
    effects::TransactionEffectsAPI,
    move_package::{ProtocolBuildConfig, derive_package_metadata_id},
    object::{MoveStructExt, OBJECT_START_VERSION, Object},
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::{CallArg, SenderSignedData, TransactionData, TransactionDataAPI},
};
use iota_vm_sdk::{
    Address, Chain, ChainContext, DebugConfig, ExecuteOptions, ExecutionResult, InMemoryStore,
    LocalVm, ProtocolVersion, Store, UserSignature,
};

const GAS_PRICE: u64 = 1000;
const GAS_BUDGET: u64 = 5_000_000_000;
const GAS_COIN_VALUE: u64 = 1_000_000_000_000;

/// The `#[authenticator]` function `link_auth` attaches to the account.
const AUTHENTICATE_FUNCTION: &str = "authenticate_no_args";

const TRACE_PATH: &str = "authenticator_gas_profile.trace.json";

fn main() -> Result<()> {
    let protocol_version = ProtocolVersion::MAX;
    let protocol_config = ProtocolConfig::get_for_version(protocol_version, Chain::Unknown);

    let owner = Address::ZERO;
    let owner_gas = gas_coin(owner);
    let owner_gas_id = owner_gas.id();

    let mut store = InMemoryStore::with_framework();
    store.insert(owner_gas);

    let ctx =
        ChainContext::new(protocol_version, Chain::Unknown).with_reference_gas_price(GAS_PRICE);
    let mut vm = LocalVm::new(ctx, store)?;

    // --- Account setup: two unsigned runs paid for by `owner` --------------

    let account_pkg = compile("examples/move/account", &protocol_config)?;
    let tx = publish_tx(&vm, owner, owner_gas_id, &account_pkg)?;
    let result = run(&mut vm, tx)?;
    let account_pkg_id = published_package_id(&result)?;
    let (account_id, account_version) = created_shared(&result)?;
    println!("Account package:  {account_pkg_id}");
    println!("Account object:   {account_id}");

    let tx = link_auth_tx(
        &vm,
        owner,
        owner_gas_id,
        account_pkg_id,
        account_id,
        account_version,
    )?;
    run(&mut vm, tx)?;
    println!("Authenticator:    {account_pkg_id}::account::{AUTHENTICATE_FUNCTION}");

    // --- From here on the account signs, via its authenticator -------------

    // The account's address is the account object's ID; fund it so it can pay
    // for its own transactions.
    let sender: Address = account_id.into();
    let sender_gas = gas_coin(sender);
    let sender_gas_id = sender_gas.id();
    vm.store_mut().insert(sender_gas);

    let view_pkg = compile("examples/move/view_functions", &protocol_config)?;
    let tx = publish_tx(&vm, sender, sender_gas_id, &view_pkg)?;
    let result = run_signed(&mut vm, tx, account_id, account_version, true)?;
    let view_pkg_id = published_package_id(&result)?;
    let (counter_id, counter_version) = created_shared(&result)?;
    println!("\nview_functions:   {view_pkg_id}");
    println!("Counter object:   {counter_id}");

    let value = counter_value(&mut vm, owner, view_pkg_id, counter_id, counter_version)?;
    println!("Counter value:    {value}");

    let tx = increment_tx(
        &vm,
        sender,
        sender_gas_id,
        view_pkg_id,
        counter_id,
        counter_version,
    )?;
    run_signed(&mut vm, tx, account_id, account_version, true)?;

    let value = counter_value(&mut vm, owner, view_pkg_id, counter_id, counter_version)?;
    println!("Counter value:    {value} (after increment)");

    Ok(())
}

// === Transactions ===

/// A `Publish` of `package`, with the returned `UpgradeCap` transferred to
/// `sender`.
fn publish_tx(
    vm: &LocalVm,
    sender: Address,
    gas_id: ObjectId,
    package: &CompiledPackage,
) -> Result<TransactionData> {
    let mut b = ProgrammableTransactionBuilder::new();
    let cap = b.publish_upgradeable(
        package.get_package_bytes(false),
        package.get_dependency_storage_package_ids(),
    );
    b.transfer_arg(sender, cap);
    transaction(vm, sender, gas_id, b)
}

/// `account::link_auth(account, package_metadata, "account",
/// AUTHENTICATE_FUNCTION)`: turns the shared `Account` into an account that
/// authorizes transactions by running the named `#[authenticator]` function.
fn link_auth_tx(
    vm: &LocalVm,
    sender: Address,
    gas_id: ObjectId,
    account_pkg_id: ObjectId,
    account_id: ObjectId,
    account_version: Version,
) -> Result<TransactionData> {
    // `create_auth_function_ref_v1` validates the function against the
    // package's `PackageMetadataV1`, a derived object created at publish time.
    let metadata_id = derive_package_metadata_id(account_pkg_id);
    let metadata = vm
        .store()
        .get_object(&metadata_id, None)?
        .context("published package has no PackageMetadataV1 — missing #[authenticator]?")?;

    let mut b = ProgrammableTransactionBuilder::new();
    // `link_auth` consumes the account by value, so it needs it mutably.
    let account = b.obj(CallArg::Shared(SharedObjectReference::new(
        account_id,
        account_version,
        true,
    )))?;
    let metadata = b.obj(CallArg::ImmutableOrOwned(metadata.object_ref()))?;
    let module = b.pure("account")?;
    let function = b.pure(AUTHENTICATE_FUNCTION)?;
    b.programmable_move_call(
        account_pkg_id,
        Identifier::from_static("account"),
        Identifier::from_static("link_auth"),
        vec![],
        vec![account, metadata, module, function],
    );
    transaction(vm, sender, gas_id, b)
}

/// `counter::increment(counter)`.
fn increment_tx(
    vm: &LocalVm,
    sender: Address,
    gas_id: ObjectId,
    view_pkg_id: ObjectId,
    counter_id: ObjectId,
    counter_version: Version,
) -> Result<TransactionData> {
    let mut b = ProgrammableTransactionBuilder::new();
    let counter = b.obj(CallArg::Shared(SharedObjectReference::new(
        counter_id,
        counter_version,
        true,
    )))?;
    b.programmable_move_call(
        view_pkg_id,
        Identifier::from_static("counter"),
        Identifier::from_static("increment"),
        vec![],
        vec![counter],
    );
    transaction(vm, sender, gas_id, b)
}

/// Finish `b` into a transaction paid for by `gas_id`, read from the store at
/// whatever version the previous run left it.
fn transaction(
    vm: &LocalVm,
    sender: Address,
    gas_id: ObjectId,
    b: ProgrammableTransactionBuilder,
) -> Result<TransactionData> {
    let gas = vm
        .store()
        .get_object(&gas_id, None)?
        .context("gas coin must be in the store")?;
    Ok(TransactionData::new_programmable(
        sender,
        vec![gas.object_ref()],
        b.finish(),
        GAS_BUDGET,
        GAS_PRICE,
    ))
}

// === Running ===

/// Run an unsigned transaction, committing its effects to the store.
fn run(vm: &mut LocalVm, tx: TransactionData) -> Result<ExecutionResult> {
    check(vm.execute(tx, ExecuteOptions::execute())?)
}

/// Run a transaction authorized by the account's `MoveAuthenticator`,
/// committing its effects to the store.
///
/// The authenticator names the account object to authenticate — the function to
/// run is resolved from that object — plus the call arguments that function
/// takes. `authenticate_no_args` takes none.
fn run_signed(
    vm: &mut LocalVm,
    tx: TransactionData,
    account_id: ObjectId,
    account_version: Version,
    with_tracing: bool,
) -> Result<ExecutionResult> {
    let authenticator = MoveAuthenticatorV1::new_with_shared_account_object(
        vec![],
        vec![],
        SharedObjectReference::new(account_id, account_version, false),
    );
    let signed = SenderSignedData::new(
        tx,
        vec![UserSignature::MoveAuthenticator(authenticator.into())],
    );
    let opts = if with_tracing {
        ExecuteOptions::execute()
            .with_debug(DebugConfig::default().with_tracing(PathBuf::from(TRACE_PATH)))
    } else {
        ExecuteOptions::execute()
    };
    let result = check(vm.execute_signed(signed, opts)?)?;

    println!("Signature status: {:?}", result.signature_status);
    Ok(result)
}

/// Read the counter through its `#[view]` function: an unsigned, gas-less
/// dev-inspect run that returns the value without touching the store.
fn counter_value(
    vm: &mut LocalVm,
    sender: Address,
    view_pkg_id: ObjectId,
    counter_id: ObjectId,
    counter_version: Version,
) -> Result<u64> {
    let mut b = ProgrammableTransactionBuilder::new();
    let counter = b.obj(CallArg::Shared(SharedObjectReference::new(
        counter_id,
        counter_version,
        false,
    )))?;
    b.programmable_move_call(
        view_pkg_id,
        Identifier::from_static("counter"),
        Identifier::from_static("value"),
        vec![],
        vec![counter],
    );
    let tx = TransactionData::new_programmable(sender, vec![], b.finish(), GAS_BUDGET, GAS_PRICE);

    let result = check(vm.execute(tx, ExecuteOptions::dev_inspect())?)?;
    let (_, return_values) = result
        .command_results
        .last()
        .context("dev-inspect returns a result per command")?;
    let (bytes, _) = return_values.first().context("`value` returns a u64")?;
    Ok(bcs::from_bytes(bytes)?)
}

/// Fail on a Move-level abort: the run itself returned `Ok`, but nothing was
/// committed and the following steps would have nothing to build on.
fn check(result: ExecutionResult) -> Result<ExecutionResult> {
    if !result.status.is_success() {
        bail!("transaction failed: {:?}", result.status);
    }
    Ok(result)
}

// === Store helpers ===

/// A fresh, well-funded gas coin owned by `owner`.
fn gas_coin(owner: Address) -> Object {
    Object::new_move(
        MoveStruct::new_gas_coin(OBJECT_START_VERSION, ObjectId::random(), GAS_COIN_VALUE),
        Owner::Address(owner),
        TransactionDigest::ZERO,
    )
}

/// The ID of the package published by a run.
fn published_package_id(result: &ExecutionResult) -> Result<ObjectId> {
    result
        .output_objects
        .iter()
        // A package is the only written object with no Move struct type.
        .find(|obj| obj.type_().is_none())
        .map(|obj| obj.id())
        .context("run published no package")
}

/// The single shared object created by a run, as `(id, initial shared
/// version)`.
fn created_shared(result: &ExecutionResult) -> Result<(ObjectId, Version)> {
    result
        .effects
        .created()
        .into_iter()
        .find_map(|(obj_ref, owner)| match owner {
            Owner::Shared(initial_shared_version) => {
                Some((obj_ref.object_id, initial_shared_version))
            }
            _ => None,
        })
        .context("run created no shared object")
}

// === Compilation ===

/// Compile a Move package from the repository, for the same protocol config the
/// VM runs.
fn compile(relative_path: &str, protocol_config: &ProtocolConfig) -> Result<CompiledPackage> {
    let path: PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", "..", relative_path]
        .iter()
        .collect();
    let mut config = BuildConfig::new_for_testing();
    config.protocol_build_config = ProtocolBuildConfig::from_protocol_config(protocol_config);
    Ok(config.build(&path)?)
}
