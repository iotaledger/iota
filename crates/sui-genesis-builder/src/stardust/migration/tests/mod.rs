// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashMap, str::FromStr};

use iota_sdk::types::block::{
    address::AliasAddress,
    output::{
        feature::{Irc30Metadata, MetadataFeature},
        unlock_condition::ImmutableAliasAddressUnlockCondition,
        AliasId, Feature, FoundryOutput, FoundryOutputBuilder, NativeToken, Output, OutputId,
        SimpleTokenScheme, TokenId, TokenScheme,
    },
};
use move_binary_format::errors::VMError;
use move_core_types::{ident_str, identifier::IdentStr, vm_status::StatusCode};
use sui_types::{
    balance::Balance,
    base_types::{SuiAddress, TxContext},
    coin::Coin,
    digests::TransactionDigest,
    epoch_data::EpochData,
    gas_coin::GAS,
    in_memory_storage::InMemoryStorage,
    inner_temporary_store::InnerTemporaryStore,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::{Argument, CheckedInputObjects, ObjectArg},
    TypeTag, STARDUST_PACKAGE_ID, SUI_FRAMEWORK_PACKAGE_ID,
};

use super::MIGRATION_PROTOCOL_VERSION;
use crate::stardust::{
    migration::{
        executor::Executor,
        migration::{Migration, NATIVE_TOKEN_BAG_KEY_TYPE, PACKAGE_DEPS},
        verification::created_objects::CreatedObjects,
    },
    types::snapshot::OutputHeader,
};

mod alias;
mod executor;
mod foundry;
mod nft;

fn random_output_header() -> OutputHeader {
    OutputHeader::new_testing(
        rand::random(),
        rand::random(),
        rand::random(),
        rand::random(),
    )
}

fn run_migration(
    outputs: impl IntoIterator<Item = (OutputHeader, Output)>,
) -> (Executor, HashMap<OutputId, CreatedObjects>) {
    let mut migration = Migration::new(1).unwrap();
    migration.run_migration(outputs).unwrap();
    migration.into_parts()
}

fn create_foundry(
    iota_amount: u64,
    token_scheme: SimpleTokenScheme,
    irc_30_metadata: Irc30Metadata,
    alias_id: AliasId,
) -> (OutputHeader, FoundryOutput) {
    let builder =
        FoundryOutputBuilder::new_with_amount(iota_amount, 1, TokenScheme::Simple(token_scheme))
            .add_unlock_condition(ImmutableAliasAddressUnlockCondition::new(
                AliasAddress::new(alias_id),
            ))
            .add_feature(Feature::Metadata(
                MetadataFeature::new(irc_30_metadata).unwrap(),
            ));
    let foundry_output = builder.finish().unwrap();

    (random_output_header(), foundry_output)
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
    outputs: impl IntoIterator<Item = (OutputHeader, Output)>,
    output_owner_module_name: &IdentStr,
    output_owned_module_name: &IdentStr,
    unlock_condition_function: &IdentStr,
) {
    let (mut executor, objects_map) = run_migration(outputs);

    // Find the corresponding objects to the migrated outputs.
    let owner_created_objects = objects_map
        .get(&output_id_owner)
        .expect("owner output should have created objects");
    let owned_created_objects = objects_map
        .get(&output_id_owned)
        .expect("owned output should have created objects");

    let owner_output_object_ref = executor
        .store()
        .get_object(owner_created_objects.output().unwrap())
        .unwrap()
        .compute_object_reference();
    let owned_output_object_ref = executor
        .store()
        .get_object(owned_created_objects.output().unwrap())
        .unwrap()
        .compute_object_reference();

    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();
        let owner_arg = builder
            .obj(ObjectArg::ImmOrOwnedObject(owner_output_object_ref))
            .unwrap();

        let extracted_assets = builder.programmable_move_call(
            STARDUST_PACKAGE_ID,
            output_owner_module_name.into(),
            ident_str!("extract_assets").into(),
            vec![],
            vec![owner_arg],
        );

        let Argument::Result(result_idx) = extracted_assets else {
            panic!("expected Argument::Result");
        };
        let balance_arg = Argument::NestedResult(result_idx, 0);
        let bag_arg = Argument::NestedResult(result_idx, 1);
        let owned_arg = Argument::NestedResult(result_idx, 2);

        let receiving_owned_arg = builder
            .obj(ObjectArg::Receiving(owned_output_object_ref))
            .unwrap();
        let received_owned_output = builder.programmable_move_call(
            STARDUST_PACKAGE_ID,
            ident_str!("address_unlock_condition").into(),
            unlock_condition_function.into(),
            vec![],
            vec![owned_arg, receiving_owned_arg],
        );

        let coin_arg = builder.programmable_move_call(
            SUI_FRAMEWORK_PACKAGE_ID,
            ident_str!("coin").into(),
            ident_str!("from_balance").into(),
            vec![TypeTag::from_str(&format!("{SUI_FRAMEWORK_PACKAGE_ID}::sui::SUI")).unwrap()],
            vec![balance_arg],
        );

        // Destroying the bag only works if it's empty, hence asserting that it is in
        // fact empty.
        builder.programmable_move_call(
            SUI_FRAMEWORK_PACKAGE_ID,
            ident_str!("bag").into(),
            ident_str!("destroy_empty").into(),
            vec![],
            vec![bag_arg],
        );

        // Transfer the coin to the zero address since we have to move it somewhere.
        builder.transfer_arg(SuiAddress::default(), coin_arg);

        // We have to use extracted object as we cannot transfer it (since it lacks the
        // `store` ability), so we extract its assets.
        let extracted_assets = builder.programmable_move_call(
            STARDUST_PACKAGE_ID,
            output_owned_module_name.into(),
            ident_str!("extract_assets").into(),
            vec![],
            vec![received_owned_output],
        );
        let Argument::Result(result_idx) = extracted_assets else {
            panic!("expected Argument::Result");
        };
        let balance_arg = Argument::NestedResult(result_idx, 0);
        let bag_arg = Argument::NestedResult(result_idx, 1);
        let inner_owned_arg = Argument::NestedResult(result_idx, 2);

        let coin_arg = builder.programmable_move_call(
            SUI_FRAMEWORK_PACKAGE_ID,
            ident_str!("coin").into(),
            ident_str!("from_balance").into(),
            vec![TypeTag::from_str(&format!("{SUI_FRAMEWORK_PACKAGE_ID}::sui::SUI")).unwrap()],
            vec![balance_arg],
        );

        // Destroying the bag only works if it's empty, hence asserting that it is in
        // fact empty.
        builder.programmable_move_call(
            SUI_FRAMEWORK_PACKAGE_ID,
            ident_str!("bag").into(),
            ident_str!("destroy_empty").into(),
            vec![],
            vec![bag_arg],
        );

        // Transfer the coin to the zero address since we have to move it somewhere.
        builder.transfer_arg(SuiAddress::default(), coin_arg);

        // We have successfully extracted the owned objects which is what we want to
        // test. Transfer to the zero address so the PTB doesn't fail.
        builder.transfer_arg(SuiAddress::default(), owned_arg);
        builder.transfer_arg(SuiAddress::default(), inner_owned_arg);

        builder.finish()
    };

    let input_objects = CheckedInputObjects::new_for_genesis(
        executor
            .load_input_objects([owner_output_object_ref])
            .chain(executor.load_packages(PACKAGE_DEPS))
            .collect(),
    );
    executor.execute_pt_unmetered(input_objects, pt).unwrap();
}

/// Test that an Output that owns Native Tokens can extract those tokens from
/// the contained bag.
fn extract_native_token_from_bag(
    output_id: OutputId,
    outputs: impl IntoIterator<Item = (OutputHeader, Output)>,
    module_name: &IdentStr,
    native_token: NativeToken,
) {
    let native_token_id: &TokenId = native_token.token_id();

    let (mut executor, objects_map) = run_migration(outputs);

    // Find the corresponding objects to the migrated output.
    let output_created_objects = objects_map
        .get(&output_id)
        .expect("output should have created objects");

    let output_object_ref = executor
        .store()
        .get_object(output_created_objects.output().unwrap())
        .unwrap()
        .compute_object_reference();

    // Recreate the key under which the tokens are stored in the bag.
    let foundry_ledger_data = executor.native_tokens().get(native_token_id).unwrap();
    let token_type = foundry_ledger_data.canonical_coin_type();
    let token_type_tag = token_type.parse::<TypeTag>().unwrap();

    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();
        let inner_object_arg = builder
            .obj(ObjectArg::ImmOrOwnedObject(output_object_ref))
            .unwrap();

        let extracted_assets = builder.programmable_move_call(
            STARDUST_PACKAGE_ID,
            module_name.into(),
            ident_str!("extract_assets").into(),
            vec![],
            vec![inner_object_arg],
        );

        let Argument::Result(result_idx) = extracted_assets else {
            panic!("expected Argument::Result");
        };
        let balance_arg = Argument::NestedResult(result_idx, 0);
        let bag_arg = Argument::NestedResult(result_idx, 1);
        // This is the inner object, i.e. the Alias extracted from an Alias Output
        // or NFT extracted from an NFT Output.
        let inner_arg = Argument::NestedResult(result_idx, 2);

        let coin_arg = builder.programmable_move_call(
            SUI_FRAMEWORK_PACKAGE_ID,
            ident_str!("coin").into(),
            ident_str!("from_balance").into(),
            vec![GAS::type_tag()],
            vec![balance_arg],
        );

        builder.transfer_arg(SuiAddress::default(), coin_arg);
        builder.transfer_arg(SuiAddress::default(), inner_arg);

        let token_type_arg = builder.pure(token_type.clone()).unwrap();
        let balance_arg = builder.programmable_move_call(
            SUI_FRAMEWORK_PACKAGE_ID,
            ident_str!("bag").into(),
            ident_str!("remove").into(),
            vec![
                NATIVE_TOKEN_BAG_KEY_TYPE
                    .parse()
                    .expect("should be a valid type tag"),
                Balance::type_(token_type_tag.clone()).into(),
            ],
            vec![bag_arg, token_type_arg],
        );

        let coin_arg = builder.programmable_move_call(
            SUI_FRAMEWORK_PACKAGE_ID,
            ident_str!("coin").into(),
            ident_str!("from_balance").into(),
            vec![token_type_tag.clone()],
            vec![balance_arg],
        );

        // Destroying the bag only works if it's empty, hence asserting that it is in
        // fact empty.
        builder.programmable_move_call(
            SUI_FRAMEWORK_PACKAGE_ID,
            ident_str!("bag").into(),
            ident_str!("destroy_empty").into(),
            vec![],
            vec![bag_arg],
        );

        builder.transfer_arg(SuiAddress::default(), coin_arg);

        builder.finish()
    };

    let input_objects = CheckedInputObjects::new_for_genesis(
        executor
            .load_input_objects([output_object_ref])
            .chain(executor.load_packages(PACKAGE_DEPS))
            .collect(),
    );
    let InnerTemporaryStore { written, .. } =
        executor.execute_pt_unmetered(input_objects, pt).unwrap();

    let coin_token_struct_tag = Coin::type_(token_type_tag);
    let coin_token = written
        .values()
        .find(|obj| {
            obj.struct_tag()
                .map(|tag| tag == coin_token_struct_tag)
                .unwrap_or(false)
        })
        .and_then(|obj| obj.as_coin_maybe())
        .expect("coin token object should exist");

    assert_eq!(coin_token.balance.value(), native_token.amount().as_u64());
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

fn unlock_object_test(
    output_id: OutputId,
    outputs: impl IntoIterator<Item = (OutputHeader, Output)>,
    sender: &SuiAddress,
    module_name: &IdentStr,
    epoch_start_timestamp_ms: u64,
    expected_test_result: UnlockObjectTestResult,
) {
    let (migration_executor, objects_map) = run_migration(outputs);

    // Recreate the TxContext and Executor so we can set a timestamp greater than 0.
    let tx_context = TxContext::new(
        sender,
        &TransactionDigest::new(rand::random()),
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
    let mut executor = Executor::new(MIGRATION_PROTOCOL_VERSION.into())
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
            vec![],
            vec![inner_object_arg],
        );

        let Argument::Result(result_idx) = extracted_assets else {
            panic!("expected Argument::Result");
        };
        let balance_arg = Argument::NestedResult(result_idx, 0);
        let bag_arg = Argument::NestedResult(result_idx, 1);
        let object_arg = Argument::NestedResult(result_idx, 2);

        let coin_arg = builder.programmable_move_call(
            SUI_FRAMEWORK_PACKAGE_ID,
            ident_str!("coin").into(),
            ident_str!("from_balance").into(),
            vec![TypeTag::from_str(&format!("{SUI_FRAMEWORK_PACKAGE_ID}::sui::SUI")).unwrap()],
            vec![balance_arg],
        );

        // Transfer the assets to the zero address since we have to move them somewhere
        // in the test.
        builder.transfer_arg(SuiAddress::ZERO, coin_arg);
        builder.transfer_arg(SuiAddress::ZERO, bag_arg);
        builder.transfer_arg(SuiAddress::ZERO, object_arg);

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
        (Ok(_), UnlockObjectTestResult::Success) => (),
        (Ok(_), UnlockObjectTestResult::Failure(_)) => {
            panic!("expected test failure, but test suceeded")
        }
        (Err(err), UnlockObjectTestResult::Success) => {
            panic!("expected test success, but test failed: {err}")
        }
        (Err(err), UnlockObjectTestResult::Failure(expected_sub_status)) => {
            for cause in err.chain() {
                match cause.downcast_ref::<VMError>() {
                    Some(vm_error) => {
                        assert_eq!(vm_error.major_status(), StatusCode::ABORTED);
                        let actual_sub_status = vm_error
                            .sub_status()
                            .expect("sub_status should be set for aborts");
                        assert_eq!(
                            actual_sub_status, expected_sub_status,
                            "actual vm sub_status {actual_sub_status} did not match expected sub_status {expected_sub_status}"
                        );
                        // Finish test successfully.
                        return;
                    }
                    None => continue,
                }
            }
            panic!(
                "expected test failure, but failed to find expected VMError in error chain, got: {err}"
            );
        }
    }
}
