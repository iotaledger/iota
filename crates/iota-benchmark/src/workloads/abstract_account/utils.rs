// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use anyhow::{Context, Result, anyhow, bail, ensure};
use iota_core::test_utils::make_pay_iota_transaction;
use iota_sdk::types::transaction::{Argument, ObjectArg};
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    Identifier,
    base_types::{IotaAddress, ObjectID, ObjectRef},
    crypto::{AccountKeyPair, KeypairTraits},
    move_package::{
        PACKAGE_METADATA_MODULE_NAME, PACKAGE_METADATA_V1_STRUCT_NAME, PACKAGE_MODULE_NAME,
        UPGRADECAP_STRUCT_NAME,
    },
    object::{Object, Owner},
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::{Transaction, TransactionData},
};
use move_core_types::ident_str;
use tracing::info;

use crate::{
    ExecutionEffects, ValidatorProxy,
    workloads::{
        Gas,
        abstract_account::{
            AA_MODULE_NAME, ABSTRACT_ACCOUNT_TY, AuthenticatorKind, GAS_BUDGET, PAY_CHUNK_SIZE,
            WORKLOAD_LABEL, WORKLOAD_PATH,
        },
    },
};

// ------------------------------
// AA init helpers
// ------------------------------

/// Publish AA package and return:
/// - package_id (ObjectID)
/// - package_metadata_ref (ObjectRef) required by abstract_account::create
pub async fn publish_aa_package_and_find_metadata(
    proxy: Arc<dyn ValidatorProxy + Sync + Send>,
    init_coin: &mut Gas,
    owner: &(IotaAddress, Arc<AccountKeyPair>),
    gas_price: u64,
) -> Result<(ObjectID, ObjectRef)> {
    info!("[{WORKLOAD_LABEL}] publishing Move package: {WORKLOAD_PATH}");

    let package_metadata_ty = format!(
        "::{}::{}",
        PACKAGE_METADATA_MODULE_NAME, PACKAGE_METADATA_V1_STRUCT_NAME
    );
    let upgrade_cap_ty = format!("::{}::{}", PACKAGE_MODULE_NAME, UPGRADECAP_STRUCT_NAME);

    let tx = TestTransactionBuilder::new(owner.0, init_coin.0, gas_price)
        .publish_examples(WORKLOAD_PATH)
        .build_and_sign(owner.1.as_ref());

    let effects = proxy
        .execute_transaction_block(tx)
        .await
        .context("execute publish tx")?;

    ensure!(effects.is_ok(), "publish failed: {}", effects.status());

    // Update init gas ref (publish consumed/mutated it).
    *init_coin = update_gas_from_effects(init_coin, &effects)
        .context("update init gas from publish effects")?;

    let created = effects.created();
    ensure!(
        !created.is_empty(),
        "publish succeeded but effects.created() is empty"
    );

    // Strategy to find package object and PackageMetadataV1:
    // - First, find package ref: either by inspecting object data (Data::Package).
    // - Then, find metadata ref by strict type check == PackageMetadataV1.

    let mut package_ref: Option<ObjectRef> = None;
    let mut metadata_ref: Option<ObjectRef> = None;

    let mut diag: Vec<String> = Vec::new();

    // Helper closure - load object and get printable (ty, is_package).
    async fn describe_created(
        proxy: &Arc<dyn ValidatorProxy + Sync + Send>,
        r: ObjectRef,
        owner: iota_types::object::Owner,
    ) -> Result<(bool, String, String)> {
        let obj = proxy
            .get_object(r.0)
            .await
            .with_context(|| format!("get_object({:?})", r.0))?;

        let (is_package, ty) = match &obj.data {
            iota_types::object::Data::Package(_) => (true, "<package>".to_string()),
            iota_types::object::Data::Move(m) => (false, m.type_().to_string()),
        };

        Ok((
            is_package,
            ty.clone(),
            format!("id={:?} owner={:?} type={}", r.0, owner, ty),
        ))
    }

    // Attempt to find package and metadata among created objects.
    for (r, o) in created.iter().copied() {
        let (is_package, ty, line) = describe_created(&proxy, r, o).await?;

        // We only store diag if we end up failing
        diag.push(line);

        if is_package && package_ref.is_none() {
            package_ref = Some(r);
            continue;
        }

        if !is_package {
            // Ignore UpgradeCap explicitly.
            if ty.contains(&upgrade_cap_ty) {
                continue;
            }

            if ty.contains(&package_metadata_ty) {
                metadata_ref = Some(r);
            }
        }

        if package_ref.is_some() && metadata_ref.is_some() {
            break;
        }
    }

    let package_ref = package_ref.ok_or_else(|| {
        anyhow!(
            "publish: created package object not found\ncreated objects:\n{}",
            diag.join("\n")
        )
    })?;
    let package_id = package_ref.0;

    let metadata_ref = metadata_ref.ok_or_else(|| {
        anyhow!(
            "publish: PackageMetadataV1 not found among created objects\ncreated objects:\n{}",
            diag.join("\n")
        )
    })?;

    info!(
        "[{WORKLOAD_LABEL}] publish done: package_id={:?}, package_metadata_ref={:?}",
        package_id, metadata_ref
    );

    Ok((package_id, metadata_ref))
}

/// Create AbstractAccount shared object via `abstract_account::create`.
pub async fn create_abstract_account(
    proxy: Arc<dyn ValidatorProxy + Sync + Send>,
    init_coin: &mut Gas,
    owner: &(IotaAddress, Arc<AccountKeyPair>),
    gas_price: u64,
    aa_package_id: ObjectID,
    aa_package_metadata_ref: ObjectRef,
    authenticator: AuthenticatorKind,
) -> Result<ObjectRef> {
    info!(
        "[{WORKLOAD_LABEL}] creating AbstractAccount via {}::{}::create ...",
        aa_package_id,
        authenticator.module_name()
    );

    let owner_pk = owner.1.public();
    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();

        let args = vec![
            builder.obj(ObjectArg::ImmOrOwnedObject(aa_package_metadata_ref))?,
            builder.pure(authenticator.module_name())?,
            builder.pure(authenticator.function_name())?,
            builder.pure(owner_pk.as_ref())?,
        ];

        builder.programmable_move_call(
            aa_package_id,
            Identifier::new(authenticator.module_name())?,
            ident_str!("create").into(),
            vec![],
            args,
        );

        builder.finish()
    };
    let tx_data =
        TransactionData::new_programmable(owner.0, vec![init_coin.0], pt, GAS_BUDGET, gas_price);

    let tx = Transaction::from_data_and_signer(tx_data, vec![owner.1.as_ref()]);
    let effects = proxy
        .execute_transaction_block(tx)
        .await
        .context("execute create AbstractAccount tx")?;

    if !effects.is_ok() {
        effects.print_gas_summary();
        bail!("create AbstractAccount failed");
    }

    *init_coin = update_gas_from_effects(init_coin, &effects)?;

    // Find created aa shared object
    let abstract_account_ref: Vec<ObjectRef> = effects
        .created()
        .into_iter()
        .filter_map(|(r, o)| {
            if matches!(o, Owner::Shared { .. }) {
                Some(r)
            } else {
                None
            }
        })
        .collect();

    if abstract_account_ref.is_empty() {
        bail!("create AbstractAccount: no shared objects created");
    }
    if abstract_account_ref.len() == 1 {
        return Ok(abstract_account_ref[0]);
    }

    for r in abstract_account_ref.iter().copied() {
        let obj = proxy.get_object(r.0).await?;
        let ty = object_type_string(&obj).unwrap_or_default();
        if ty.contains(ABSTRACT_ACCOUNT_TY) {
            return Ok(r);
        }
    }

    Ok(abstract_account_ref[0])
}

/// Mint `count` owned coins (objects) to `recipient` with `amount` each.
/// Returns object refs of minted coins.
pub async fn mint_owned_coins_to_address(
    proxy: Arc<dyn ValidatorProxy + Sync + Send>,
    init_coin: &mut Gas,
    owner: &(IotaAddress, Arc<AccountKeyPair>),
    gas_price: u64,
    recipient: IotaAddress,
    count: u64,
    amount: u64,
) -> Result<Vec<ObjectRef>> {
    info!(
        "[{WORKLOAD_LABEL}] minting {} coins to AA address {:?}, amount={} each ...",
        count, recipient, amount
    );

    let mut remaining = count as usize;
    let mut minted: Vec<ObjectRef> = Vec::with_capacity(count as usize);

    while remaining > 0 {
        let batch = remaining.min(PAY_CHUNK_SIZE);
        remaining -= batch;

        let recipients: Vec<IotaAddress> = vec![recipient; batch];
        let amounts: Vec<u64> = vec![amount; batch];

        let tx = make_pay_iota_transaction(
            init_coin.0,
            vec![],
            recipients,
            amounts,
            owner.0,
            owner.1.as_ref(),
            gas_price,
            GAS_BUDGET,
        );

        let effects = proxy.execute_transaction_block(tx).await?;

        if !effects.is_ok() {
            effects.print_gas_summary();
            bail!("mint pay tx failed");
        }

        // update init_coin to the mutated ref
        *init_coin = update_gas_from_effects(init_coin, &effects)?;

        for (r, o) in effects.created().into_iter() {
            if matches!(o, Owner::AddressOwner(a) if a == recipient) {
                minted.push(r);
            }
        }
    }

    info!("[{WORKLOAD_LABEL}] minted coins: {}", minted.len());

    Ok(minted)
}

/// Initialize bench objects for MaxArgs authenticators.
pub async fn init_bench_objects(
    proxy: Arc<dyn ValidatorProxy + Sync + Send>,
    init_coin: &mut Gas,
    owner: &(IotaAddress, Arc<AccountKeyPair>),
    gas_price: u64,
    aa_package_id: ObjectID,
    amount: u64,
    is_shared: bool,
) -> Result<Vec<ObjectRef>> {
    let module = ident_str!(AA_MODULE_NAME).to_owned();
    let function = Identifier::new("create_bench_objects")?;

    let pt = {
        let mut b = ProgrammableTransactionBuilder::new();
        let amount_arg: Argument = b.pure(amount)?;
        let is_shared_arg: Argument = b.pure(is_shared)?;
        b.programmable_move_call(
            aa_package_id,
            module,
            function,
            vec![],
            vec![amount_arg, is_shared_arg],
        );
        b.finish()
    };

    // Take a gas object to pay for this init transaction
    let gas_obj = init_coin.0;

    // Standard GAS_BUDGET is not enough for this transaction since it creates many
    // objects. We set a higher gas budget here since we know this
    // transaction will create many objects and we don't want to run out of gas.
    let gas_budget = 2_000_000_000u64;

    let sender = owner.0;
    let signer = &*owner.1;

    let tx_data =
        TransactionData::new_programmable(sender, vec![gas_obj], pt, gas_budget, gas_price);

    // Sign + execute via proxy
    let tx = Transaction::from_data_and_signer(tx_data, vec![signer]);

    let effects = proxy
        .execute_transaction_block(tx)
        .await
        .context("execute_transaction(create bench objects) failed")?;

    let bench_refs = effects
        .created()
        .into_iter()
        .map(|(r, _adapter)| r)
        .collect::<Vec<_>>();

    ensure!(
        bench_refs.len() == amount as usize,
        "Expected {amount} BenchObject, got {}",
        bench_refs.len()
    );

    *init_coin = update_gas_from_effects(init_coin, &effects)?;

    Ok(bench_refs)
}

/// Update init gas object ref from effects.
pub fn update_gas_from_effects(current: &Gas, effects: &ExecutionEffects) -> Result<Gas> {
    let updated = effects
        .mutated()
        .into_iter()
        .find(|(r, _)| r.0 == current.0.0)
        .ok_or_else(|| anyhow::anyhow!("init coin not found in mutated effects"))?;

    Ok((updated.0, updated.1.get_owner_address()?, current.2.clone()))
}

/// If the object is not a Move object — returns None.
fn object_type_string(obj: &Object) -> Option<String> {
    obj.type_().map(|t| t.to_string())
}
