// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashMap, str::FromStr};

use anyhow::{anyhow, bail, ensure};
use iota_sdk::types::block::{
    address::AliasAddress,
    output::{
        AliasId, Feature, FoundryOutput, FoundryOutputBuilder, NativeTokens, Output, OutputId,
        SimpleTokenScheme, TokenScheme,
        feature::{Irc30Metadata, MetadataFeature},
        unlock_condition::ImmutableAliasAddressUnlockCondition,
    },
};
use iota_types::{
    IOTA_FRAMEWORK_PACKAGE_ID, STARDUST_PACKAGE_ID, TypeTag,
    balance::Balance,
    base_types::{IotaAddress, TxContext},
    coin::Coin,
    digests::TransactionDigest,
    epoch_data::EpochData,
    in_memory_storage::InMemoryStorage,
    inner_temporary_store::InnerTemporaryStore,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    stardust::coin_type::CoinType,
    transaction::{Argument, CheckedInputObjects, ObjectArg},
};
use move_binary_format::errors::VMError;
use move_core_types::{ident_str, identifier::IdentStr, vm_status::StatusCode};
use rand::random;

use crate::stardust::{
    migration::{
        MigrationTargetNetwork,
        executor::Executor,
        migration::{
            MIGRATION_PROTOCOL_VERSION, Migration, NATIVE_TOKEN_BAG_KEY_TYPE, PACKAGE_DEPS,
        },
        verification::created_objects::CreatedObjects,
    },
    types::{
        address_swap_map::AddressSwapMap, output_header::OutputHeader,
        output_index::random_output_index,
    },
};

mod alias;
mod basic;
mod executor;
mod foundry;
mod nft;

fn random_output_header() -> OutputHeader {
    OutputHeader::new(
        random(),
        random_output_index(),
        random(),
        random(),
        random(),
    )
}

fn run_migration(
    total_supply: u64,
    outputs: impl IntoIterator<Item = (OutputHeader, Output)>,
    coin_type: CoinType,
) -> anyhow::Result<(Executor, HashMap<OutputId, CreatedObjects>)> {
    let mut migration = Migration::new(
        1,
        total_supply,
        MigrationTargetNetwork::Mainnet,
        coin_type,
        AddressSwapMap::default(),
    )?;
    migration.run_migration(outputs)?;
    Ok(migration.into_parts())
}

fn create_foundry(
    amount: u64,
    token_scheme: SimpleTokenScheme,
    irc_30_metadata: Irc30Metadata,
    alias_id: AliasId,
) -> anyhow::Result<(OutputHeader, FoundryOutput)> {
    let builder =
        FoundryOutputBuilder::new_with_amount(amount, 1, TokenScheme::Simple(token_scheme))
            .add_unlock_condition(ImmutableAliasAddressUnlockCondition::new(
                AliasAddress::new(alias_id),
            ))
            .add_immutable_feature(Feature::Metadata(
                MetadataFeature::new(irc_30_metadata).unwrap(),
            ));
    let foundry_output = builder.finish()?;

    Ok((random_output_header(), foundry_output))
}

/// Test that an Object owned by another Object (not to be confused with
/// Owner::ObjectOwner) can be received by the owning object. This means aliases
/// owned by aliases, aliases owned by NFTs, etc.
///
/// The PTB sends the extracted assets to the null address since they must be
/// used in the transaction.
fn object_migration_with_object_owner(
    output_id_owner: OutputId,
    output_id_owned: OutputId,
    total_supply: u64,
    outputs: impl IntoIterator<Item = (OutputHeader, Output)>,
    output_owner_module_name: &IdentStr,
    output_owned_module_name: &IdentStr,
    unlock_condition_function: &IdentStr,
    coin_type: CoinType,
) -> anyhow::Result<()> {
    let (mut executor, objects_map) = run_migration(total_supply, outputs, coin_type)?;

    // Find the corresponding objects to the migrated outputs.
    let owner_created_objects = objects_map
        .get(&output_id_owner)
        .expect("owner output should have created objects");
    let owned_created_objects = objects_map
        .get(&output_id_owned)
        .expect("owned output should have created objects");

    let owner_output_object_ref = executor
        .store()
        .get_object(owner_created_objects.output()?)
        .ok_or_else(|| anyhow!("missing owner-created output"))?
        .compute_object_reference();
    let owned_output_object_ref = executor
        .store()
        .get_object(owned_created_objects.output()?)
        .ok_or_else(|| anyhow!("missing owned created output"))?
        .compute_object_reference();

    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();
        let owner_arg = builder.obj(ObjectArg::ImmOrOwnedObject(owner_output_object_ref))?;

        let extracted_assets = builder.programmable_move_call(
            STARDUST_PACKAGE_ID,
            output_owner_module_name.into(),
            ident_str!("extract_assets").into(),
            vec![coin_type.to_type_tag()],
            vec![owner_arg],
        );

        let Argument::Result(result_idx) = extracted_assets else {
            bail!("expected Argument::Result");
        };
        let balance_arg = Argument::NestedResult(result_idx, 0);
        let bag_arg = Argument::NestedResult(result_idx, 1);
        let owned_arg = Argument::NestedResult(result_idx, 2);

        let receiving_owned_arg = builder.obj(ObjectArg::Receiving(owned_output_object_ref))?;
        let received_owned_output = builder.programmable_move_call(
            STARDUST_PACKAGE_ID,
            ident_str!("address_unlock_condition").into(),
            unlock_condition_function.into(),
            vec![coin_type.to_type_tag()],
            vec![owned_arg, receiving_owned_arg],
        );

        let coin_arg = builder.programmable_move_call(
            IOTA_FRAMEWORK_PACKAGE_ID,
            ident_str!("coin").into(),
            ident_str!("from_balance").into(),
            vec![TypeTag::from_str(&format!(
                "{IOTA_FRAMEWORK_PACKAGE_ID}::iota::IOTA"
            ))?],
            vec![balance_arg],
        );

        // Destroying the bag only works if it's empty, hence asserting that it is in
        // fact empty.
        builder.programmable_move_call(
            IOTA_FRAMEWORK_PACKAGE_ID,
            ident_str!("bag").into(),
            ident_str!("destroy_empty").into(),
            vec![],
            vec![bag_arg],
        );

        // Transfer the coin to the zero address since we have to move it somewhere.
        builder.transfer_arg(IotaAddress::default(), coin_arg);

        // We have to use extracted object as we cannot transfer it (since it lacks the
        // `store` ability), so we extract its assets.
        let extracted_assets = builder.programmable_move_call(
            STARDUST_PACKAGE_ID,
            output_owned_module_name.into(),
            ident_str!("extract_assets").into(),
            vec![coin_type.to_type_tag()],
            vec![received_owned_output],
        );
        let Argument::Result(result_idx) = extracted_assets else {
            bail!("expected Argument::Result");
        };
        let balance_arg = Argument::NestedResult(result_idx, 0);
        let bag_arg = Argument::NestedResult(result_idx, 1);
        let inner_owned_arg = Argument::NestedResult(result_idx, 2);

        let coin_arg = builder.programmable_move_call(
            IOTA_FRAMEWORK_PACKAGE_ID,
            ident_str!("coin").into(),
            ident_str!("from_balance").into(),
            vec![TypeTag::from_str(&format!(
                "{IOTA_FRAMEWORK_PACKAGE_ID}::iota::IOTA"
            ))?],
            vec![balance_arg],
        );

        // Destroying the bag only works if it's empty, hence asserting that it is in
        // fact empty.
        builder.programmable_move_call(
            IOTA_FRAMEWORK_PACKAGE_ID,
            ident_str!("bag").into(),
            ident_str!("destroy_empty").into(),
            vec![],
            vec![bag_arg],
        );

        // Transfer the coin to the zero address since we have to move it somewhere.
        builder.transfer_arg(IotaAddress::default(), coin_arg);

        // We have successfully extracted the owned objects which is what we want to
        // test. Transfer to the zero address so the PTB doesn't fail.
        builder.transfer_arg(IotaAddress::default(), owned_arg);
        builder.transfer_arg(IotaAddress::default(), inner_owned_arg);

        builder.finish()
    };

    let input_objects = CheckedInputObjects::new_for_genesis(
        executor
            .load_input_objects([owner_output_object_ref])
            .chain(executor.load_packages(PACKAGE_DEPS))
            .collect(),
    );
    executor.execute_pt_unmetered(input_objects, pt)?;
    Ok(())
}

/// Test that an Output that owns Native Tokens can extract those tokens from
/// the contained bag.
fn extract_native_tokens_from_bag(
    output_id: OutputId,
    total_supply: u64,
    outputs: impl IntoIterator<Item = (OutputHeader, Output)>,
    module_name: &IdentStr,
    native_tokens: NativeTokens,
    expected_assets: ExpectedAssets,
    coin_type: CoinType,
) -> anyhow::Result<()> {
    let (mut executor, objects_map) = run_migration(total_supply, outputs, coin_type)?;

    // Find the corresponding objects to the migrated output.
    let output_created_objects = objects_map
        .get(&output_id)
        .expect("output should have created objects");

    // Get the corresponding output object from the object store.
    let output_object_ref = executor
        .store()
        .get_object(output_created_objects.output()?)
        .ok_or_else(|| anyhow!("missing output-created output"))?
        .compute_object_reference();

    // Recreate the keys under which the tokens are stored in the bag.
    let native_tokens = native_tokens
        .into_iter()
        .map(|native_token| {
            let native_token_id = native_token.token_id();
            let foundry_ledger_data = executor
                .native_tokens()
                .get(native_token_id)
                .ok_or_else(|| anyhow!("missing native token {native_token_id}"))?;
            let bag_key = foundry_ledger_data.to_canonical_string(/* with_prefix */ false);
            let token_type_tag = foundry_ledger_data
                .to_canonical_string(/* with_prefix */ true)
                .parse::<TypeTag>()?;
            Ok((native_token, bag_key, token_type_tag))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();
        let inner_object_arg = builder.obj(ObjectArg::ImmOrOwnedObject(output_object_ref))?;

        let extracted_assets = builder.programmable_move_call(
            STARDUST_PACKAGE_ID,
            module_name.into(),
            ident_str!("extract_assets").into(),
            vec![coin_type.to_type_tag()],
            vec![inner_object_arg],
        );

        let Argument::Result(result_idx) = extracted_assets else {
            bail!("expected Argument::Result");
        };
        let balance_arg = Argument::NestedResult(result_idx, 0);
        let bag_arg = Argument::NestedResult(result_idx, 1);
        if matches!(expected_assets, ExpectedAssets::BalanceBagObject) {
            // This is the inner object, i.e. the Alias extracted from an Alias Output
            // or NFT extracted from an NFT Output.
            let object_arg = Argument::NestedResult(result_idx, 2);
            builder.transfer_arg(IotaAddress::default(), object_arg);
        }

        let gas_coin_arg = builder.programmable_move_call(
            IOTA_FRAMEWORK_PACKAGE_ID,
            ident_str!("coin").into(),
            ident_str!("from_balance").into(),
            vec![coin_type.to_type_tag()],
            vec![balance_arg],
        );

        builder.transfer_arg(IotaAddress::default(), gas_coin_arg);

        for (_, bag_key, token_type_tag) in &native_tokens {
            let bag_key_arg = builder.pure(bag_key.clone())?;
            let token_balance_arg = builder.programmable_move_call(
                IOTA_FRAMEWORK_PACKAGE_ID,
                ident_str!("bag").into(),
                ident_str!("remove").into(),
                vec![
                    NATIVE_TOKEN_BAG_KEY_TYPE
                        .parse()
                        .expect("should be a valid type tag"),
                    Balance::type_(token_type_tag.clone()).into(),
                ],
                vec![bag_arg, bag_key_arg],
            );

            let minted_coin_arg = builder.programmable_move_call(
                IOTA_FRAMEWORK_PACKAGE_ID,
                ident_str!("coin").into(),
                ident_str!("from_balance").into(),
                vec![token_type_tag.clone()],
                vec![token_balance_arg],
            );

            builder.transfer_arg(IotaAddress::default(), minted_coin_arg);
        }

        // Destroying the bag only works if it's empty, hence asserting that it is in
        // fact empty.
        builder.programmable_move_call(
            IOTA_FRAMEWORK_PACKAGE_ID,
            ident_str!("bag").into(),
            ident_str!("destroy_empty").into(),
            vec![],
            vec![bag_arg],
        );

        builder.finish()
    };

    let input_objects = CheckedInputObjects::new_for_genesis(
        executor
            .load_input_objects([output_object_ref])
            .chain(executor.load_packages(PACKAGE_DEPS))
            .collect(),
    );
    let InnerTemporaryStore { written, .. } = executor.execute_pt_unmetered(input_objects, pt)?;

    for (native_token, _, token_type_tag) in native_tokens {
        let coin_token_struct_tag = Coin::type_(token_type_tag);
        let coin_token = written
            .values()
            .find(|obj| {
                obj.struct_tag()
                    .map(|tag| tag == coin_token_struct_tag)
                    .unwrap_or(false)
            })
            .ok_or_else(|| anyhow!("missing coin object"))
            .and_then(|obj| {
                obj.as_coin_maybe()
                    .ok_or_else(|| anyhow!("object is not a coin"))
            })?;

        ensure!(
            coin_token.balance.value() == native_token.amount().as_u64(),
            "coin token balance does not match original native token amount"
        );
    }

    Ok(())
}

enum UnlockObjectTestResult {
    /// The test should succeed.
    Success,
    /// The test should fail with the given sub_status.
    Failure(u64),
}

impl UnlockObjectTestResult {
    /// A copy of `EWrongSender` in the expiration unlock condition smart
    /// contract.
    pub(crate) const ERROR_WRONG_SENDER_FAILURE: Self = Self::Failure(0);
    /// A copy of `ETimelockNotExpired` in the timelock unlock condition smart
    /// contract.
    pub(crate) const ERROR_TIMELOCK_NOT_EXPIRED_FAILURE: Self = Self::Failure(0);
}

enum ExpectedAssets {
    BalanceBag,
    BalanceBagObject,
}

fn unlock_object(
    output_id: OutputId,
    total_supply: u64,
    outputs: impl IntoIterator<Item = (OutputHeader, Output)>,
    sender: &IotaAddress,
    module_name: &IdentStr,
    epoch_start_timestamp_ms: u64,
    expected_test_result: UnlockObjectTestResult,
    expected_assets: ExpectedAssets,
    coin_type: CoinType,
) -> anyhow::Result<()> {
    let (migration_executor, objects_map) = run_migration(total_supply, outputs, coin_type)?;

    // Recreate the TxContext and Executor so we can set a timestamp greater than 0.
    let tx_context = TxContext::new(
        sender,
        &TransactionDigest::new(random()),
        &EpochData::new(0, epoch_start_timestamp_ms, Default::default()),
    );
    let store = InMemoryStorage::new(
        // Cloning all objects in the store includes the system packages we need for executing
        // tests.
        migration_executor
            .store()
            .objects()
            .values()
            .cloned()
            .collect(),
    );
    let mut executor = Executor::new(
        MIGRATION_PROTOCOL_VERSION.into(),
        MigrationTargetNetwork::Mainnet,
        coin_type,
    )
    .unwrap()
    .with_tx_context(tx_context)
    .with_store(store);

    // Find the corresponding objects to the migrated output.
    let output_created_objects = objects_map
        .get(&output_id)
        .expect("output should have created objects");

    let output_object_ref = executor
        .store()
        .get_object(output_created_objects.output().unwrap())
        .unwrap()
        .compute_object_reference();

    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();
        let inner_object_arg = builder
            .obj(ObjectArg::ImmOrOwnedObject(output_object_ref))
            .unwrap();

        let extracted_assets = builder.programmable_move_call(
            STARDUST_PACKAGE_ID,
            module_name.into(),
            ident_str!("extract_assets").into(),
            vec![coin_type.to_type_tag()],
            vec![inner_object_arg],
        );

        let Argument::Result(result_idx) = extracted_assets else {
            bail!("expected Argument::Result");
        };
        let balance_arg = Argument::NestedResult(result_idx, 0);
        let bag_arg = Argument::NestedResult(result_idx, 1);

        let coin_arg = builder.programmable_move_call(
            IOTA_FRAMEWORK_PACKAGE_ID,
            ident_str!("coin").into(),
            ident_str!("from_balance").into(),
            vec![coin_type.to_type_tag()],
            vec![balance_arg],
        );

        // Transfer the assets to the zero address since we have to move them somewhere
        // in the test.
        builder.transfer_arg(IotaAddress::ZERO, coin_arg);
        builder.transfer_arg(IotaAddress::ZERO, bag_arg);

        if matches!(expected_assets, ExpectedAssets::BalanceBagObject) {
            let object_arg = Argument::NestedResult(result_idx, 2);
            builder.transfer_arg(IotaAddress::ZERO, object_arg);
        }

        builder.finish()
    };

    let input_objects = CheckedInputObjects::new_for_genesis(
        executor
            .load_input_objects([output_object_ref])
            .chain(executor.load_packages(PACKAGE_DEPS))
            .collect(),
    );
    let result = executor.execute_pt_unmetered(input_objects, pt);

    match (result, expected_test_result) {
        (Ok(_), UnlockObjectTestResult::Success) => Ok(()),
        (Ok(_), UnlockObjectTestResult::Failure(_)) => {
            bail!("expected test failure, but test succeeded")
        }
        (Err(err), UnlockObjectTestResult::Success) => {
            bail!("expected test success, but test failed: {err}")
        }
        (Err(err), UnlockObjectTestResult::Failure(expected_sub_status)) => {
            for cause in err.chain() {
                match cause.downcast_ref::<VMError>() {
                    Some(vm_error) => {
                        ensure!(vm_error.major_status() == StatusCode::ABORTED);
                        let actual_sub_status = vm_error
                            .sub_status()
                            .expect("sub_status should be set for aborts");
                        ensure!(
                            actual_sub_status == expected_sub_status,
                            "actual vm sub_status {actual_sub_status} did not match expected sub_status {expected_sub_status}"
                        );
                        // Finish test successfully.
                        return Ok(());
                    }
                    None => continue,
                }
            }
            bail!(
                "expected test failure, but failed to find expected VMError in error chain, got: {err}"
            );
        }
    }
}
