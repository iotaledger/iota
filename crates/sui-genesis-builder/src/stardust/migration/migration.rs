// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Contains the logic for the migration process.

use anyhow::Result;
use fastcrypto::hash::HashFunction;
use sui_move_build::CompiledPackage;
use sui_protocol_config::ProtocolVersion;
use sui_types::{
    base_types::{ObjectID, SuiAddress, TxContext},
    crypto::DefaultHash,
    digests::TransactionDigest,
    epoch_data::EpochData,
    object::Object,
    MOVE_STDLIB_PACKAGE_ID, STARDUST_PACKAGE_ID, SUI_FRAMEWORK_PACKAGE_ID, SUI_SYSTEM_PACKAGE_ID,
    TIMELOCK_PACKAGE_ID,
};

use std::{
    collections::HashMap,
    io::{prelude::Write, BufWriter},
};

use iota_sdk::types::block::output::{FoundryOutput, Output, OutputId};

use crate::stardust::{
    migration::{
        executor::Executor,
        verification::{created_objects::CreatedObjects, verify_output},
    },
    native_token::package_data::NativeTokenPackageData,
    types::snapshot::OutputHeader,
};

/// We fix the protocol version used in the migration.
pub const MIGRATION_PROTOCOL_VERSION: u64 = 42;

/// The dependencies of the generated packages for native tokens.
pub const PACKAGE_DEPS: [ObjectID; 5] = [
    MOVE_STDLIB_PACKAGE_ID,
    SUI_FRAMEWORK_PACKAGE_ID,
    SUI_SYSTEM_PACKAGE_ID,
    STARDUST_PACKAGE_ID,
    TIMELOCK_PACKAGE_ID,
];

pub(crate) const NATIVE_TOKEN_BAG_KEY_TYPE: &str = "0x01::ascii::String";

/// The orchestrator of the migration process.
///
/// It is run by providing an [`Iterator`] of stardust UTXOs, and holds an inner executor
/// and in-memory object storage for their conversion into objects.
///
/// It guarantees the following:
///
/// * That foundry UTXOs are sorted by `(milestone_timestamp, output_id)`.
/// * That the foundry packages and total supplies are created first
/// * That all other outputs are created in a second iteration over the original UTXOs.
/// * That the resulting ledger state is valid.
///
/// The migration process results in the generation of a snapshot file with the generated
/// objects serialized.
pub struct Migration {
    executor: Executor,
    output_objects_map: HashMap<OutputId, CreatedObjects>,
}

impl Migration {
    /// Try to setup the migration process by creating the inner executor
    /// and bootstraping the in-memory storage.
    pub fn new() -> Result<Self> {
        let executor = Executor::new(ProtocolVersion::new(MIGRATION_PROTOCOL_VERSION))?;
        Ok(Self {
            executor,
            output_objects_map: Default::default(),
        })
    }

    /// Run all stages of the migration.
    ///
    /// * Generate and build the foundry packages
    /// * Create the foundry packages, and associated objects.
    /// * Create all other objects.
    /// * Validate the resulting object-based ledger state.
    /// * Create the snapshot file.
    pub fn run(
        mut self,
        outputs: impl IntoIterator<Item = (OutputHeader, Output)>,
        writer: impl Write,
    ) -> Result<()> {
        let (mut foundries, mut outputs) = outputs.into_iter().fold(
            (Vec::new(), Vec::new()),
            |(mut foundries, mut outputs), (header, output)| {
                if let Output::Foundry(foundry) = output {
                    foundries.push((header, foundry));
                } else {
                    outputs.push((header, output));
                }
                (foundries, outputs)
            },
        );
        // We sort the outputs to make sure the order of outputs up to
        // a certain milestone timestamp remains the same between runs.
        //
        // This guarantees that fresh ids created through the transaction
        // context will also map to the same objects betwen runs.
        outputs.sort_by_key(|(header, _)| (header.ms_timestamp(), header.output_id()));
        foundries.sort_by_key(|(header, _)| (header.ms_timestamp(), header.output_id()));
        self.migrate_foundries(&foundries)?;
        self.migrate_outputs(&outputs)?;
        self.verify_ledger_state(&outputs)?;
        create_snapshot(&self.into_objects(), writer)
    }

    /// The migration objects.
    ///
    /// The system packages and underlying `init` objects
    /// are filtered out because they will be generated
    /// in the genesis process.
    fn into_objects(self) -> Vec<Object> {
        self.executor.into_objects()
    }

    /// Create the packages, and associated objects representing foundry outputs.
    fn migrate_foundries<'a>(
        &mut self,
        foundries: impl IntoIterator<Item = &'a (OutputHeader, FoundryOutput)>,
    ) -> Result<()> {
        let compiled = foundries
            .into_iter()
            .map(|(header, output)| {
                let pkg = generate_package(&output)?;
                Ok((header, output, pkg))
            })
            .collect::<Result<Vec<_>>>()?;
        self.output_objects_map
            .extend(self.executor.create_foundries(compiled.into_iter())?);
        Ok(())
    }

    /// Create objects for all outputs except for foundry outputs.
    fn migrate_outputs<'a>(
        &mut self,
        outputs: impl IntoIterator<Item = &'a (OutputHeader, Output)>,
    ) -> Result<()> {
        for (header, output) in outputs {
            let created = match output {
                Output::Alias(alias) => self.executor.create_alias_objects(header, alias)?,
                Output::Basic(basic) => self.executor.create_basic_objects(header, basic)?,
                Output::Nft(nft) => self.executor.create_nft_objects(nft)?,
                Output::Treasury(_) | Output::Foundry(_) => continue,
            };
            self.output_objects_map.insert(header.output_id(), created);
        }
        Ok(())
    }

    /// Verify the ledger state represented by the objects in [`InMemoryStorage`].
    pub fn verify_ledger_state<'a>(
        &self,
        outputs: impl IntoIterator<Item = &'a (OutputHeader, Output)>,
    ) -> Result<()> {
        for (header, output) in outputs {
            let objects = self
                .output_objects_map
                .get(&header.output_id())
                .ok_or_else(|| {
                    anyhow::anyhow!("missing created objects for output {}", header.output_id())
                })?;
            verify_output(header, output, objects, self.executor.store())?;
        }
        Ok(())
    }
}

// Build a `CompiledPackage` from a given `FoundryOutput`.
fn generate_package(foundry: &FoundryOutput) -> Result<CompiledPackage> {
    let native_token_data = NativeTokenPackageData::try_from(foundry)?;
    crate::stardust::native_token::package_builder::build_and_compile(native_token_data)
}

/// Serialize the objects stored in [`InMemoryStorage`] into a file using
/// [`bcs`] encoding.
fn create_snapshot(ledger: &[Object], writer: impl Write) -> Result<()> {
    let mut writer = BufWriter::new(writer);
    writer.write_all(&bcs::to_bytes(&ledger)?)?;
    Ok(writer.flush()?)
}

/// Get the bytes of all bytecode modules (not including direct or transitive
/// dependencies) of [`CompiledPackage`].
pub(super) fn package_module_bytes(pkg: &CompiledPackage) -> Result<Vec<Vec<u8>>> {
    pkg.get_modules()
        .map(|module| {
            let mut buf = Vec::new();
            module.serialize(&mut buf)?;
            Ok(buf)
        })
        .collect::<Result<_>>()
}

/// Create a [`TxContext]` that remains the same across invocations.
pub(super) fn create_migration_context() -> TxContext {
    let mut hasher = DefaultHash::default();
    hasher.update(b"stardust-migration");
    let hash = hasher.finalize();
    let stardust_migration_transaction_digest = TransactionDigest::new(hash.into());

    TxContext::new(
        &SuiAddress::default(),
        &stardust_migration_transaction_digest,
        &EpochData::new_genesis(0),
    )
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::stardust::{
        migration::{Migration, MIGRATION_PROTOCOL_VERSION},
        types::{snapshot::OutputHeader, Alias, AliasOutput, ALIAS_OUTPUT_MODULE_NAME},
    };
    use iota_sdk::types::block::{
        address::{Address, AliasAddress, Ed25519Address},
        output::{
            feature::{IssuerFeature, MetadataFeature, SenderFeature},
            unlock_condition::{
                GovernorAddressUnlockCondition, ImmutableAliasAddressUnlockCondition,
                StateControllerAddressUnlockCondition,
            },
            AliasId, AliasOutput as StardustAlias, AliasOutputBuilder, Feature,
            FoundryOutputBuilder, NativeToken, NativeTokens, SimpleTokenScheme, UnlockCondition,
        },
    };
    use move_core_types::{ident_str, language_storage::StructTag};
    use sui_types::{
        balance::Balance,
        base_types::ObjectID,
        object::Object,
        programmable_transaction_builder::ProgrammableTransactionBuilder,
        transaction::{Argument, CheckedInputObjects, ObjectArg},
        STARDUST_PACKAGE_ID, SUI_FRAMEWORK_PACKAGE_ID,
    };
    use sui_types::{
        dynamic_field::{derive_dynamic_field_id, Field},
        object::Owner,
    };

    use crate::stardust::native_token::{
        package_builder,
        package_data::{NativeTokenModuleData, NativeTokenPackageData},
    };

    use super::*;

    fn random_output_header() -> OutputHeader {
        OutputHeader::new_testing(
            rand::random(),
            rand::random(),
            rand::random(),
            rand::random(),
        )
    }

    fn run_migration(outputs: impl IntoIterator<Item = (OutputHeader, Output)>) -> Vec<Object> {
        let mut snapshot_buffer = Vec::new();

        Migration::new()
            .unwrap()
            .run(outputs, &mut snapshot_buffer)
            .unwrap();

        bcs::from_bytes(&snapshot_buffer).unwrap()
    }

    fn migrate_alias(
        header: OutputHeader,
        stardust_alias: StardustAlias,
    ) -> (ObjectID, Alias, AliasOutput) {
        let alias_id: AliasId = stardust_alias
            .alias_id()
            .or_from_output_id(&header.output_id())
            .to_owned();
        let mut snapshot_buffer = Vec::new();
        Migration::new()
            .unwrap()
            .run([(header, stardust_alias.into())], &mut snapshot_buffer)
            .unwrap();

        let migrated_objects: Vec<Object> = bcs::from_bytes(&snapshot_buffer).unwrap();

        // Ensure the migrated objects exist under the expected identifiers.
        let alias_object_id = ObjectID::new(*alias_id);
        let alias_object = migrated_objects
            .iter()
            .find(|obj| obj.id() == alias_object_id)
            .expect("alias object should be present in the migrated snapshot");
        assert_eq!(alias_object.struct_tag().unwrap(), Alias::tag(),);
        let alias_output_object = migrated_objects
            .iter()
            .find(|obj| match obj.struct_tag() {
                Some(tag) => tag == AliasOutput::tag(),
                None => false,
            })
            .expect("alias object should be present in the migrated snapshot");

        // Version is set to 1 when the alias is created based on the computed lamport timestamp.
        // When the alias is attached to the alias output, the version should be incremented.
        assert!(
            alias_object.version().value() > 1,
            "alias object version should have been incremented"
        );
        assert!(
            alias_output_object.version().value() > 1,
            "alias output object version should have been incremented"
        );

        let alias_output: AliasOutput =
            bcs::from_bytes(alias_output_object.data.try_as_move().unwrap().contents()).unwrap();
        let alias: Alias =
            bcs::from_bytes(alias_object.data.try_as_move().unwrap().contents()).unwrap();

        (alias_object_id, alias, alias_output)
    }

    /// Test that the migrated alias objects in the snapshot contain the expected data.
    #[test]
    fn test_alias_migration() {
        let alias_id = AliasId::new(rand::random());
        let random_address = Ed25519Address::from(rand::random::<[u8; Ed25519Address::LENGTH]>());
        let header = random_output_header();

        let stardust_alias = AliasOutputBuilder::new_with_amount(1_000_000, alias_id)
            .add_unlock_condition(StateControllerAddressUnlockCondition::new(random_address))
            .add_unlock_condition(GovernorAddressUnlockCondition::new(random_address))
            .with_state_metadata([0xff; 1])
            .with_features(vec![
                Feature::Metadata(MetadataFeature::new([0xdd; 1]).unwrap()),
                Feature::Sender(SenderFeature::new(random_address)),
            ])
            .with_immutable_features(vec![
                Feature::Metadata(MetadataFeature::new([0xaa; 1]).unwrap()),
                Feature::Issuer(IssuerFeature::new(random_address)),
            ])
            .with_state_index(3)
            .finish()
            .unwrap();

        let (alias_object_id, alias, alias_output) = migrate_alias(header, stardust_alias.clone());
        let expected_alias = Alias::try_from_stardust(alias_object_id, &stardust_alias).unwrap();

        // Compare only the balance. The ID is newly generated and the bag is tested separately.
        assert_eq!(stardust_alias.amount(), alias_output.iota.value());

        assert_eq!(expected_alias, alias);
    }

    /// Test that an Alias with a zeroed ID is migrated to an Alias Object with its UID set to the hashed Output ID.
    #[test]
    fn test_alias_migration_with_zeroed_id() {
        let random_address = Ed25519Address::from(rand::random::<[u8; Ed25519Address::LENGTH]>());
        let header = random_output_header();

        let stardust_alias = AliasOutputBuilder::new_with_amount(1_000_000, AliasId::null())
            .add_unlock_condition(StateControllerAddressUnlockCondition::new(random_address))
            .add_unlock_condition(GovernorAddressUnlockCondition::new(random_address))
            .finish()
            .unwrap();

        // If this function does not panic, then the created aliases
        // were found at the correct non-zeroed Alias ID.
        migrate_alias(header, stardust_alias);
    }

    /// Test that an Alias owned by another Alias can be received by the owning object.
    ///
    /// The PTB sends the extracted assets to the null address since it must be used in the transaction.
    #[test]
    fn test_alias_migration_with_alias_owner() {
        let random_address = Ed25519Address::from(rand::random::<[u8; Ed25519Address::LENGTH]>());

        let alias1_amount = 1_000_000;
        let stardust_alias1 =
            AliasOutputBuilder::new_with_amount(alias1_amount, AliasId::new(rand::random()))
                .add_unlock_condition(StateControllerAddressUnlockCondition::new(random_address))
                .add_unlock_condition(GovernorAddressUnlockCondition::new(random_address))
                .finish()
                .unwrap();

        let alias2_amount = 2_000_000;
        // stardust_alias1 is the owner of stardust_alias2.
        let stardust_alias2 =
            AliasOutputBuilder::new_with_amount(alias2_amount, AliasId::new(rand::random()))
                .add_unlock_condition(StateControllerAddressUnlockCondition::new(Address::from(
                    stardust_alias1.alias_id().clone(),
                )))
                .add_unlock_condition(GovernorAddressUnlockCondition::new(Address::from(
                    stardust_alias1.alias_id().clone(),
                )))
                .finish()
                .unwrap();

        let migrated_objects = run_migration([
            (random_output_header(), stardust_alias1.into()),
            (random_output_header(), stardust_alias2.into()),
        ]);

        // Find the corresponding objects to the migrated aliases, uniquely identified by their amounts.
        // Should be adapted to use the tags from issue 239 to make this much easier.
        let alias_output1_id = migrated_objects
            .iter()
            .find(|obj| {
                obj.struct_tag()
                    .map(|tag| tag == AliasOutput::tag())
                    .unwrap_or(false)
                    && bcs::from_bytes::<AliasOutput>(obj.data.try_as_move().unwrap().contents())
                        .unwrap()
                        .iota
                        .value()
                        == alias1_amount
            })
            .expect("alias1 should exist")
            .id();

        let alias_output2_id = migrated_objects
            .iter()
            .find(|obj| {
                obj.struct_tag()
                    .map(|tag| tag == AliasOutput::tag())
                    .unwrap_or(false)
                    && bcs::from_bytes::<AliasOutput>(obj.data.try_as_move().unwrap().contents())
                        .unwrap()
                        .iota
                        .value()
                        == alias2_amount
            })
            .expect("alias2 should exist")
            .id();

        let mut executor = Executor::new(MIGRATION_PROTOCOL_VERSION.into()).unwrap();
        for object in migrated_objects {
            executor.store.insert_object(object);
        }

        let alias_output1_object_ref = executor
            .store()
            .get_object(&alias_output1_id)
            .unwrap()
            .compute_object_reference();

        let alias_output2_object_ref = executor
            .store()
            .get_object(&alias_output2_id)
            .unwrap()
            .compute_object_reference();

        let pt = {
            let mut builder = ProgrammableTransactionBuilder::new();
            let alias1_arg = builder
                .obj(ObjectArg::ImmOrOwnedObject(alias_output1_object_ref))
                .unwrap();

            let extracted_assets = builder.programmable_move_call(
                STARDUST_PACKAGE_ID,
                ALIAS_OUTPUT_MODULE_NAME.into(),
                ident_str!("extract_assets").into(),
                vec![],
                vec![alias1_arg],
            );

            let Argument::Result(result_idx) = extracted_assets else {
                panic!("expected Argument::Result");
            };
            let balance_arg = Argument::NestedResult(result_idx, 0);
            let bag_arg = Argument::NestedResult(result_idx, 1);
            let alias1_arg = Argument::NestedResult(result_idx, 2);

            let receiving_alias2_arg = builder
                .obj(ObjectArg::Receiving(alias_output2_object_ref))
                .unwrap();
            let received_alias_output2 = builder.programmable_move_call(
                STARDUST_PACKAGE_ID,
                ident_str!("address_unlock_condition").into(),
                ident_str!("unlock_alias_address_owned_alias").into(),
                vec![],
                vec![alias1_arg, receiving_alias2_arg],
            );

            let coin_arg = builder.programmable_move_call(
                SUI_FRAMEWORK_PACKAGE_ID,
                ident_str!("coin").into(),
                ident_str!("from_balance").into(),
                vec![
                    StructTag::from_str(&format!("{}::sui::SUI", SUI_FRAMEWORK_PACKAGE_ID))
                        .unwrap()
                        .into(),
                ],
                vec![balance_arg],
            );

            builder.transfer_arg(SuiAddress::default(), bag_arg);
            builder.transfer_arg(SuiAddress::default(), coin_arg);

            // We have to use Alias Output as we cannot transfer it (since it lacks the `store` ability),
            // so we extract its assets.
            let extracted_assets = builder.programmable_move_call(
                STARDUST_PACKAGE_ID,
                ALIAS_OUTPUT_MODULE_NAME.into(),
                ident_str!("extract_assets").into(),
                vec![],
                vec![received_alias_output2],
            );
            let Argument::Result(result_idx) = extracted_assets else {
                panic!("expected Argument::Result");
            };
            let balance_arg = Argument::NestedResult(result_idx, 0);
            let bag_arg = Argument::NestedResult(result_idx, 1);
            let alias2_arg = Argument::NestedResult(result_idx, 2);

            let coin_arg = builder.programmable_move_call(
                SUI_FRAMEWORK_PACKAGE_ID,
                ident_str!("coin").into(),
                ident_str!("from_balance").into(),
                vec![
                    StructTag::from_str(&format!("{}::sui::SUI", SUI_FRAMEWORK_PACKAGE_ID))
                        .unwrap()
                        .into(),
                ],
                vec![balance_arg],
            );

            builder.transfer_arg(SuiAddress::default(), coin_arg);
            builder.transfer_arg(SuiAddress::default(), bag_arg);

            builder.transfer_arg(SuiAddress::default(), alias1_arg);
            builder.transfer_arg(SuiAddress::default(), alias2_arg);

            builder.finish()
        };

        let input_objects = CheckedInputObjects::new_for_genesis(
            executor
                .load_input_objects([alias_output1_object_ref])
                .chain(executor.load_packages(PACKAGE_DEPS))
                .collect(),
        );
        executor.execute_pt_unmetered(input_objects, pt).unwrap();
    }

    #[test]
    fn create_bag_with_pt() {
        // Mock the foundry
        let owner = AliasAddress::new(AliasId::new([0; AliasId::LENGTH]));
        let supply = 1_000_000;
        let token_scheme = SimpleTokenScheme::new(supply, 0, supply).unwrap();
        let header = OutputHeader::new_testing(
            rand::random(),
            rand::random(),
            rand::random(),
            rand::random(),
        );
        let foundry = FoundryOutputBuilder::new_with_amount(1000, 1, token_scheme.into())
            .with_unlock_conditions([UnlockCondition::from(
                ImmutableAliasAddressUnlockCondition::new(owner),
            )])
            .finish_with_params(supply)
            .unwrap();
        let foundry_id = foundry.id();
        let foundry_package_data = NativeTokenPackageData::new(
            "wat",
            NativeTokenModuleData::new(
                foundry_id, "wat", "WAT", 0, "WAT", supply, supply, "wat", "wat", None, owner,
            ),
        );
        let foundry_package = package_builder::build_and_compile(foundry_package_data).unwrap();

        // Execution
        let mut executor = Executor::new(ProtocolVersion::MAX).unwrap();
        let object_count = executor.store().objects().len();
        executor
            .create_foundries([(&header, &foundry, foundry_package)])
            .unwrap();
        // Foundry package publication creates four objects
        //
        // * The package
        // * Coin metadata
        // * MaxSupplyPolicy
        // * The total supply coin
        assert_eq!(executor.store().objects().len() - object_count, 4);
        assert!(executor.native_tokens.get(&foundry_id.into()).is_some());
        let initial_supply_coin_object = executor
            .store()
            .objects()
            .values()
            .find_map(|object| object.is_coin().then_some(object))
            .expect("there should be only a single coin: the total supply of native tokens");
        let coin_type_tag = initial_supply_coin_object.coin_type_maybe().unwrap();
        let initial_supply_coin_data = initial_supply_coin_object.as_coin_maybe().unwrap();

        // Mock the native token
        let token_amount = 10_000;
        let native_token = NativeToken::new(foundry_id.into(), token_amount).unwrap();

        // Create the bag
        let (bag, _, _) = executor
            .create_bag_with_pt(&NativeTokens::from_vec(vec![native_token]).unwrap())
            .unwrap();
        assert!(executor.store().get_object(bag.id.object_id()).is_none());

        // Verify the mutation of the foundry coin with the total supply
        let mutated_supply_coin = executor
            .store()
            .get_object(initial_supply_coin_data.id())
            .unwrap()
            .as_coin_maybe()
            .unwrap();
        assert_eq!(mutated_supply_coin.value(), supply - token_amount);

        // Get the dynamic fields (df)
        let tokens = executor
            .store()
            .objects()
            .values()
            .filter_map(|object| object.is_child_object().then_some(object))
            .collect::<Vec<_>>();
        assert_eq!(tokens.len(), 1);
        assert_eq!(
            tokens[0].owner,
            Owner::ObjectOwner((*bag.id.object_id()).into())
        );
        let token_as_df = tokens[0].to_rust::<Field<String, Balance>>().unwrap();
        // Verify name
        let expected_name = coin_type_tag.to_canonical_string(true);
        assert_eq!(token_as_df.name, expected_name);
        // Verify value
        let expected_balance = Balance::new(token_amount);
        assert_eq!(token_as_df.value, expected_balance);
        // Verify df id
        let expected_id = derive_dynamic_field_id(
            *bag.id.object_id(),
            &NATIVE_TOKEN_BAG_KEY_TYPE.parse().unwrap(),
            &bcs::to_bytes(&expected_name).unwrap(),
        )
        .unwrap();
        assert_eq!(*token_as_df.id.object_id(), expected_id);
    }

    #[test]
    fn migration_create_and_deserialize_snapshot() {
        let mut persisted: Vec<u8> = Vec::new();
        let objects = (0..4)
            .map(|_| Object::new_gas_for_testing())
            .collect::<Vec<_>>();
        create_snapshot(&objects, &mut persisted).unwrap();
        let snapshot_objects: Vec<Object> = bcs::from_bytes(&persisted).unwrap();
        assert_eq!(objects, snapshot_objects);
    }
}
