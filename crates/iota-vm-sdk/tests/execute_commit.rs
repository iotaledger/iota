// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Store-commit semantics of the three `ExecutionMode`s against the public
//! `iota-vm-sdk` API: `Execute` commits writes and deletions on success;
//! `DevInspect` / `DryRun` leave the store untouched. Self-contained — uses
//! only the built-in framework, no Move compiler.

use iota_sdk_types::{
    Identifier, MoveStruct, ObjectId, Owner, StructTag, Transaction, TransactionDigest,
    transaction::{GenesisTransaction, TransactionKind},
};
use iota_types::{
    effects::TransactionEffectsAPI,
    error::{IotaError, UserInputError},
    object::{MoveStructExt, OBJECT_START_VERSION, Object},
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::{TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE, TransactionAPI},
};
use iota_vm_sdk::{
    Address, Chain, ChainContext, ExecuteOptions, InMemoryStore, LocalVm, ProtocolVersion, Store,
    TransactionDenyConfigBuilder, VmSdkError,
};

const GAS_PRICE: u64 = 1000;
/// Mirrors `ProtocolConfig::base_tx_cost_fixed`, the floor an unset budget is
/// raised to when the gas coins hold less.
const BASE_TX_COST_FIXED: u64 = 1000;
const GAS_COIN_VALUE: u64 = 1_000_000_000_000;
const TRANSFER_AMOUNT: u64 = 1000;

fn chain_context() -> ChainContext {
    ChainContext::new(ProtocolVersion::MAX, Chain::Unknown).with_reference_gas_price(GAS_PRICE)
}

/// A fresh, well-funded gas coin owned by `owner`.
fn gas_coin(owner: Address) -> Object {
    Object::new_move(
        MoveStruct::new_gas_coin(OBJECT_START_VERSION, ObjectId::random(), GAS_COIN_VALUE),
        Owner::Address(owner),
        TransactionDigest::ZERO,
    )
}

/// `transfer_iota(recipient, Some(amount))`: splits a fresh coin off gas and
/// transfers it. Mutates the gas coin and creates one new coin.
fn transfer_tx(sender: Address, gas: &Object, recipient: Address, amount: u64) -> Transaction {
    let mut b = ProgrammableTransactionBuilder::new();
    b.transfer_iota(recipient, Some(amount));
    Transaction::new_programmable(
        sender,
        vec![gas.object_ref()],
        b.finish(),
        TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE * GAS_PRICE,
        GAS_PRICE,
    )
}

/// The same transfer PTB without any gas payment.
fn gasless_transfer_tx(sender: Address, recipient: Address, amount: u64) -> Transaction {
    let mut b = ProgrammableTransactionBuilder::new();
    b.transfer_iota(recipient, Some(amount));
    Transaction::new_programmable(
        sender,
        vec![],
        b.finish(),
        TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE * GAS_PRICE,
        GAS_PRICE,
    )
}

#[test]
fn execute_commits_writes_to_store() {
    let sender = Address::ZERO;
    let recipient = Address::from(ObjectId::random());

    let gas = gas_coin(sender);
    let gas_id = gas.id();
    let gas_version_before = gas.version();

    let mut store = InMemoryStore::with_framework();
    store.insert(gas.clone());

    let mut vm = LocalVm::new(chain_context(), store).expect("build LocalVm");

    let result = vm
        .execute(
            transfer_tx(sender, &gas, recipient, TRANSFER_AMOUNT),
            ExecuteOptions::execute(),
        )
        .expect("execute must succeed");

    assert!(
        result.status.is_success(),
        "transfer must succeed, got {:?}",
        result.status
    );
    assert!(
        result.committed,
        "successful Execute must set committed = true"
    );

    // A new coin was created and is now committed to the store.
    let created = result.effects.created();
    assert_eq!(created.len(), 1, "transfer_iota creates exactly one coin");
    let new_coin_id = created[0].reference.object_id;
    assert!(
        vm.store()
            .get_object(&new_coin_id, None)
            .expect("store lookup")
            .is_some(),
        "created coin must be committed to the store"
    );

    // The gas coin was mutated: it is still present, at a higher version.
    let gas_after = vm
        .store()
        .get_object(&gas_id, None)
        .expect("store lookup")
        .expect("gas coin must remain in the store");
    assert!(
        gas_after.version() > gas_version_before,
        "gas coin version must advance after a committed mutation"
    );
}

/// `Execute` mode must also commit deletions: `pay` merges the second input
/// coin into the first, consuming (deleting) it — it must be removed from the
/// store.
#[test]
fn execute_commits_deletions_to_store() {
    let sender = Address::ZERO;
    let recipient = Address::from(ObjectId::random());

    let gas = gas_coin(sender);
    // Two owned coins; `pay` merges the second into the first, deleting it.
    let primary = gas_coin(sender);
    let merged = gas_coin(sender);
    let merged_id = merged.id();

    let mut store = InMemoryStore::with_framework();
    store.insert(gas.clone());
    store.insert(primary.clone());
    store.insert(merged.clone());

    let mut vm = LocalVm::new(chain_context(), store).expect("build LocalVm");

    let mut b = ProgrammableTransactionBuilder::new();
    b.pay(
        vec![primary.object_ref(), merged.object_ref()],
        vec![recipient],
        vec![TRANSFER_AMOUNT],
    )
    .expect("build pay PTB");
    let tx = Transaction::new_programmable(
        sender,
        vec![gas.object_ref()],
        b.finish(),
        TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE * GAS_PRICE,
        GAS_PRICE,
    );

    let result = vm
        .execute(tx, ExecuteOptions::execute())
        .expect("execute must succeed");
    assert!(
        result.status.is_success(),
        "pay must succeed, got {:?}",
        result.status
    );
    assert!(
        result.committed,
        "successful Execute must set committed = true"
    );

    // The merged-in coin is reported deleted and is gone from the store.
    assert!(
        result
            .effects
            .deleted()
            .iter()
            .any(|o| o.object_id == merged_id),
        "merged coin must appear in effects.deleted()"
    );
    assert!(
        vm.store()
            .get_object(&merged_id, None)
            .expect("store lookup")
            .is_none(),
        "deleted coin must be removed from the store"
    );
}

#[test]
fn dev_inspect_and_dry_run_leave_store_unchanged() {
    let sender = Address::ZERO;
    let recipient = Address::from(ObjectId::random());

    for opts in [ExecuteOptions::dev_inspect(), ExecuteOptions::dry_run()] {
        let mode = opts.mode;

        let gas = gas_coin(sender);
        let gas_id = gas.id();
        let gas_version_before = gas.version();

        let mut store = InMemoryStore::with_framework();
        store.insert(gas.clone());

        let mut vm = LocalVm::new(chain_context(), store).expect("build LocalVm");

        let result = vm
            .execute(transfer_tx(sender, &gas, recipient, TRANSFER_AMOUNT), opts)
            .unwrap_or_else(|e| panic!("{mode:?} must succeed: {e}"));

        assert!(result.status.is_success(), "{mode:?} run must succeed");
        assert!(!result.committed, "{mode:?} must not commit");

        // The effects still report the created coin; it must not leak into the
        // store, and the gas coin is unchanged.
        let created = result.effects.created();
        assert_eq!(
            created.len(),
            1,
            "{mode:?}: effects must report the created coin"
        );
        assert!(
            vm.store()
                .get_object(&created[0].reference.object_id, None)
                .expect("store lookup")
                .is_none(),
            "{mode:?}: created object must NOT be committed"
        );
        let gas_after = vm
            .store()
            .get_object(&gas_id, None)
            .expect("store lookup")
            .expect("gas coin still present");
        assert_eq!(
            gas_after.version(),
            gas_version_before,
            "{mode:?}: gas coin version must be unchanged"
        );
    }
}

/// `Execute` requires a real gas payment: a gasless transaction is rejected
/// instead of being funded with the mock simulation coin and committed.
#[test]
fn execute_rejects_gasless_transaction() {
    let sender = Address::ZERO;
    let recipient = Address::from(ObjectId::random());

    let mut vm =
        LocalVm::new(chain_context(), InMemoryStore::with_framework()).expect("build LocalVm");

    let err = vm
        .execute(
            gasless_transfer_tx(sender, recipient, TRANSFER_AMOUNT),
            ExecuteOptions::execute(),
        )
        .expect_err("gasless Execute must be rejected");
    assert!(matches!(err, VmSdkError::Validation(_)), "got {err:?}");
}

/// Gasless dev-inspect and dry-run runs are funded with the one-shot mock
/// coin, which is never persisted.
#[test]
fn dev_inspect_and_dry_run_fund_gasless_transactions_with_mock_coin() {
    let sender = Address::ZERO;
    let recipient = Address::from(ObjectId::random());

    for opts in [ExecuteOptions::dev_inspect(), ExecuteOptions::dry_run()] {
        let mode = opts.mode;
        let mut vm =
            LocalVm::new(chain_context(), InMemoryStore::with_framework()).expect("build LocalVm");

        let result = vm
            .execute(
                gasless_transfer_tx(sender, recipient, TRANSFER_AMOUNT),
                opts,
            )
            .unwrap_or_else(|e| panic!("{mode:?} gasless run must succeed: {e}"));

        assert!(
            result.status.is_success(),
            "{mode:?} gasless run must succeed, got {:?}",
            result.status
        );
        assert!(!result.committed, "{mode:?} must not commit");
        let mock_gas_id = result.mock_gas_id.expect("mock gas coin must be minted");
        assert!(
            vm.store()
                .get_object(&mock_gas_id, None)
                .expect("store lookup")
                .is_none(),
            "{mode:?}: mock gas coin must not be persisted"
        );
    }
}

/// A zero budget is filled in from the epoch even when a real gas coin is
/// supplied, so a dev inspect whose gas is not yet settled still runs. The coin
/// is used as-is rather than being replaced by a mock one.
#[test]
fn dev_inspect_with_real_gas_coin_fills_in_a_zero_budget() {
    let sender = Address::ZERO;
    let recipient = Address::from(ObjectId::random());

    let gas = gas_coin(sender);
    let mut store = InMemoryStore::with_framework();
    store.insert(gas.clone());
    let mut vm = LocalVm::new(chain_context(), store).expect("build LocalVm");

    let mut tx = transfer_tx(sender, &gas, recipient, TRANSFER_AMOUNT);
    tx.gas_data_mut().budget = 0;

    let result = vm
        .execute(tx, ExecuteOptions::dev_inspect())
        .expect("dev-inspect must not error");
    assert!(
        result.status.is_success(),
        "zero-budget dev-inspect with a real gas coin must succeed, got {:?}",
        result.status
    );
    assert!(
        result.mock_gas_id.is_none(),
        "a supplied gas coin must be used as-is"
    );
}

/// A budget the caller does declare is metered against, rather than being
/// replaced by `max_tx_gas`: a budget too small for the transfer runs out of
/// gas.
#[test]
fn dev_inspect_meters_against_a_declared_budget() {
    let sender = Address::ZERO;
    let recipient = Address::from(ObjectId::random());

    let gas = gas_coin(sender);
    let mut store = InMemoryStore::with_framework();
    store.insert(gas.clone());
    let mut vm = LocalVm::new(chain_context(), store).expect("build LocalVm");

    let mut tx = transfer_tx(sender, &gas, recipient, TRANSFER_AMOUNT);
    tx.gas_data_mut().budget = GAS_PRICE;

    let result = vm
        .execute(tx, ExecuteOptions::dev_inspect())
        .expect("dev-inspect must not error");
    assert!(
        !result.status.is_success(),
        "a budget too small for the transfer should not succeed, got {:?}",
        result.status
    );
}

/// A gas coin that cannot back the budget is rejected up front, the same way
/// the node rejects it.
///
/// An unset budget is capped at what the coins hold, so it takes a coin holding
/// less than the smallest budget a transaction may declare to get here: the cap
/// is raised back to that minimum, and the coin cannot cover even that.
#[test]
fn dev_inspect_rejects_a_gas_coin_that_cannot_back_the_budget() {
    let sender = Address::ZERO;
    let recipient = Address::from(ObjectId::random());

    let underfunded_balance = GAS_PRICE;
    let gas = Object::new_move(
        MoveStruct::new_gas_coin(
            OBJECT_START_VERSION,
            ObjectId::random(),
            underfunded_balance,
        ),
        Owner::Address(sender),
        TransactionDigest::ZERO,
    );
    let mut store = InMemoryStore::with_framework();
    store.insert(gas.clone());
    let mut vm = LocalVm::new(chain_context(), store).expect("build LocalVm");

    // Left at zero, so the budget is filled in: capped at the coin's balance,
    // then raised to `base_tx_cost_fixed * gas_price`, which the coin above still
    // cannot cover.
    let mut tx = transfer_tx(sender, &gas, recipient, TRANSFER_AMOUNT);
    tx.gas_data_mut().budget = 0;
    let min_gas_budget = u128::from(BASE_TX_COST_FIXED) * u128::from(GAS_PRICE);
    assert!(u128::from(underfunded_balance) < min_gas_budget);

    let err = vm
        .execute(tx, ExecuteOptions::dev_inspect())
        .expect_err("a gas coin that cannot back the budget must be rejected");
    assert!(
        matches!(
            &err,
            VmSdkError::Validation(e)
                if matches!(
                    &e.source,
                    IotaError::UserInput {
                        error: UserInputError::GasBalanceTooLow { gas_balance, needed_gas_amount },
                    } if *gas_balance == u128::from(underfunded_balance)
                        && *needed_gas_amount == min_gas_budget
                )
        ),
        "got {err:?}"
    );
}

/// A gas payment naming an object that is not a gas coin is rejected, even when
/// another coin in the payment covers the budget on its own.
#[test]
fn dev_inspect_rejects_a_gas_payment_that_is_not_a_gas_coin() {
    let sender = Address::ZERO;
    let recipient = Address::from(ObjectId::random());

    let funded_coin = gas_coin(sender);
    let not_a_coin = Object::new_move(
        MoveStruct::new(
            StructTag::new(
                Address::FRAMEWORK,
                Identifier::from_static("object_basics"),
                Identifier::from_static("Object"),
                vec![],
            )
            .into(),
            OBJECT_START_VERSION,
            // A Move object's contents lead with its own id.
            bcs::to_bytes(&(ObjectId::random(), 7u64)).unwrap(),
        )
        .unwrap(),
        Owner::Address(sender),
        TransactionDigest::ZERO,
    );

    let mut store = InMemoryStore::with_framework();
    store.insert(funded_coin.clone());
    store.insert(not_a_coin.clone());
    let mut vm = LocalVm::new(chain_context(), store).expect("build LocalVm");

    let mut tx = transfer_tx(sender, &funded_coin, recipient, TRANSFER_AMOUNT);
    tx.gas_data_mut().objects = vec![funded_coin.object_ref(), not_a_coin.object_ref()];

    let err = vm
        .execute(tx, ExecuteOptions::dev_inspect())
        .expect_err("a gas payment that is not a gas coin must be rejected");
    assert!(matches!(err, VmSdkError::Validation(_)), "got {err:?}");
}

#[test]
fn dev_inspect_rejects_more_gas_coins_than_the_protocol_allows() {
    let sender = Address::ZERO;
    let recipient = Address::from(ObjectId::random());

    let funded_coin = gas_coin(sender);
    let mut store = InMemoryStore::with_framework();
    store.insert(funded_coin.clone());

    let max_gas_payment_objects =
        iota_protocol_config::ProtocolConfig::get_for_version(ProtocolVersion::MAX, Chain::Unknown)
            .max_gas_payment_objects() as usize;
    let extra_coins: Vec<Object> = (0..max_gas_payment_objects)
        .map(|_| gas_coin(sender))
        .collect();
    for coin in &extra_coins {
        store.insert(coin.clone());
    }
    let mut vm = LocalVm::new(chain_context(), store).expect("build LocalVm");

    let mut tx = transfer_tx(sender, &funded_coin, recipient, TRANSFER_AMOUNT);
    tx.gas_data_mut().objects = std::iter::once(funded_coin.object_ref())
        .chain(extra_coins.iter().map(|coin| coin.object_ref()))
        .collect();

    let err = vm
        .execute(tx, ExecuteOptions::dev_inspect())
        .expect_err("a gas payment over the protocol cap must be rejected");
    assert!(matches!(err, VmSdkError::Validation(_)), "got {err:?}");
}

/// System transactions are rejected in every mode.
#[test]
fn system_transactions_are_rejected() {
    let sender = Address::ZERO;

    for opts in [
        ExecuteOptions::dev_inspect(),
        ExecuteOptions::dry_run(),
        ExecuteOptions::execute(),
    ] {
        let mode = opts.mode;
        let mut vm =
            LocalVm::new(chain_context(), InMemoryStore::with_framework()).expect("build LocalVm");

        let tx = Transaction::new_with_gas_coins(
            TransactionKind::Genesis(GenesisTransaction {
                objects: vec![],
                events: vec![],
            }),
            sender,
            vec![],
            TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE * GAS_PRICE,
            GAS_PRICE,
        );

        let err = vm
            .execute(tx, opts)
            .expect_err("system transaction must be rejected");
        assert!(
            matches!(err, VmSdkError::Validation(_)),
            "{mode:?}: got {err:?}"
        );
    }
}

/// The deny-list configuration on `ExecuteOptions` is enforced.
#[test]
fn deny_config_rejects_denied_sender() {
    let sender = Address::ZERO;
    let recipient = Address::from(ObjectId::random());

    let gas = gas_coin(sender);
    let mut store = InMemoryStore::with_framework();
    store.insert(gas.clone());
    let mut vm = LocalVm::new(chain_context(), store).expect("build LocalVm");

    let deny_config = TransactionDenyConfigBuilder::new()
        .add_denied_address(sender)
        .build();
    let err = vm
        .execute(
            transfer_tx(sender, &gas, recipient, TRANSFER_AMOUNT),
            ExecuteOptions::dry_run().with_deny_config(deny_config),
        )
        .expect_err("a denied sender must be rejected");
    assert!(matches!(err, VmSdkError::Validation(_)), "got {err:?}");
}
