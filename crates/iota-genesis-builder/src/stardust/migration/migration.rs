// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Contains the logic for the migration process.

use std::{
    collections::HashMap,
    io::{prelude::Write, BufWriter},
};

use anyhow::Result;
use iota_move_build::CompiledPackage;
use iota_protocol_config::ProtocolVersion;
use iota_sdk::types::block::output::{FoundryOutput, Output, OutputId};
use iota_types::{
    base_types::{IotaAddress, ObjectID, TxContext},
    epoch_data::EpochData,
    gas_coin::GAS,
    object::Object,
    smr_coin::SMR,
    IOTA_FRAMEWORK_PACKAGE_ID, IOTA_SYSTEM_PACKAGE_ID, MOVE_STDLIB_PACKAGE_ID, STARDUST_PACKAGE_ID,
    TIMELOCK_PACKAGE_ID,
};
use move_core_types::language_storage::TypeTag;
use tracing::info;

use crate::stardust::{
    migration::{
        executor::Executor,
        verification::{created_objects::CreatedObjects, verify_outputs},
        MigrationTargetNetwork,
    },
    native_token::package_data::NativeTokenPackageData,
    types::{snapshot::OutputHeader, timelock},
};

/// We fix the protocol version used in the migration.
pub const MIGRATION_PROTOCOL_VERSION: u64 = 1;

/// The dependencies of the generated packages for native tokens.
pub const PACKAGE_DEPS: [ObjectID; 5] = [
    MOVE_STDLIB_PACKAGE_ID,
    IOTA_FRAMEWORK_PACKAGE_ID,
    IOTA_SYSTEM_PACKAGE_ID,
    STARDUST_PACKAGE_ID,
    TIMELOCK_PACKAGE_ID,
];

pub(crate) const NATIVE_TOKEN_BAG_KEY_TYPE: &str = "0x01::ascii::String";

/// The orchestrator of the migration process.
///
/// It is run by providing an [`Iterator`] of stardust UTXOs, and holds an inner
/// executor and in-memory object storage for their conversion into objects.
///
/// It guarantees the following:
///
/// * That foundry UTXOs are sorted by `(milestone_timestamp, output_id)`.
/// * That the foundry packages and total supplies are created first
/// * That all other outputs are created in a second iteration over the original
///   UTXOs.
/// * That the resulting ledger state is valid.
///
/// The migration process results in the generation of a snapshot file with the
/// generated objects serialized.
pub struct Migration {
    target_milestone_timestamp_sec: u32,
    total_supply: u64,
    executor: Executor,
    pub(super) output_objects_map: HashMap<OutputId, CreatedObjects>,
    /// The coin type to use in order to migrate outputs. Can be either `Iota`
    /// or `Shimmer`. Is fixed for the entire migration process.
    type_tag: CoinType,
}

impl Migration {
    /// Try to setup the migration process by creating the inner executor
    /// and bootstraping the in-memory storage.
    pub fn new(
        target_milestone_timestamp_sec: u32,
        total_supply: u64,
        target_network: MigrationTargetNetwork,
        type_tag: CoinType,
    ) -> Result<Self> {
        let executor = Executor::new(
            ProtocolVersion::new(MIGRATION_PROTOCOL_VERSION),
            target_network,
            type_tag.clone(),
        )?;
        Ok(Self {
            target_milestone_timestamp_sec,
            total_supply,
            executor,
            output_objects_map: Default::default(),
            type_tag,
        })
    }

    /// Run all stages of the migration except snapshot migration.
    /// Factored out to faciliate testing.
    ///
    /// See also `Self::run`.
    pub(crate) fn run_migration(
        &mut self,
        outputs: impl IntoIterator<Item = (OutputHeader, Output)>,
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
        info!("Migrating foundries...");
        self.migrate_foundries(&foundries)?;
        info!("Migrating the rest of outputs...");
        self.migrate_outputs(&outputs)?;
        let outputs = outputs
            .into_iter()
            .chain(foundries.into_iter().map(|(h, f)| (h, Output::Foundry(f))))
            .collect::<Vec<_>>();
        info!("Verifying ledger state...");
        self.verify_ledger_state(&outputs)?;

        Ok(())
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
        info!("Starting the migration...");
        self.run_migration(outputs)?;
        info!("Migration ended.");
        info!("Writing snapshot file...");
        create_snapshot(&self.into_objects(), writer)?;
        info!("Snapshot file written.");
        Ok(())
    }

    /// The migration objects.
    ///
    /// The system packages and underlying `init` objects
    /// are filtered out because they will be generated
    /// in the genesis process.
    fn into_objects(self) -> Vec<Object> {
        self.executor.into_objects()
    }

    /// Create the packages, and associated objects representing foundry
    /// outputs.
    fn migrate_foundries<'a>(
        &mut self,
        foundries: impl IntoIterator<Item = &'a (OutputHeader, FoundryOutput)>,
    ) -> Result<()> {
        let compiled = foundries
            .into_iter()
            .map(|(header, output)| {
                let pkg = generate_package(output)?;
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
                Output::Alias(alias) => {
                    self.executor
                        .create_alias_objects(header, alias, &self.type_tag)?
                }
                Output::Nft(nft) => {
                    self.executor
                        .create_nft_objects(header, nft, &self.type_tag)?
                }
                Output::Basic(basic) => {
                    // All timelocked vested rewards(basic outputs with the specific ID format)
                    // should be migrated as TimeLock<Balance<IOTA>> objects.
                    if timelock::is_timelocked_vested_reward(
                        header.output_id(),
                        basic,
                        self.target_milestone_timestamp_sec,
                    ) {
                        self.executor.create_timelock_object(
                            header.output_id(),
                            basic,
                            self.target_milestone_timestamp_sec,
                        )?
                    } else {
                        self.executor.create_basic_objects(
                            header,
                            basic,
                            self.target_milestone_timestamp_sec,
                            &self.type_tag,
                        )?
                    }
                }
                Output::Treasury(_) | Output::Foundry(_) => continue,
            };
            self.output_objects_map.insert(header.output_id(), created);
        }
        Ok(())
    }

    /// Verify the ledger state represented by the objects in
    /// [`InMemoryStorage`].
    pub fn verify_ledger_state<'a>(
        &self,
        outputs: impl IntoIterator<Item = &'a (OutputHeader, Output)>,
    ) -> Result<()> {
        verify_outputs(
            outputs,
            &self.output_objects_map,
            self.executor.native_tokens(),
            self.target_milestone_timestamp_sec,
            self.total_supply,
            self.executor.store(),
        )?;
        Ok(())
    }

    /// Consumes the `Migration` and returns the underlying `Executor` and
    /// created objects map, so tests can continue to work in the same
    /// environment as the migration.
    #[cfg(test)]
    pub(super) fn into_parts(self) -> (Executor, HashMap<OutputId, CreatedObjects>) {
        (self.executor, self.output_objects_map)
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
pub(super) fn create_migration_context(target_network: MigrationTargetNetwork) -> TxContext {
    TxContext::new(
        &IotaAddress::default(),
        &target_network.migration_transaction_digest(),
        &EpochData::new_genesis(0),
    )
}

/// The type tag for the outputs used in the migration.
#[derive(Clone, Debug)]
pub enum CoinType {
    Iota,
    Shimmer,
}

impl CoinType {
    pub fn to_type_tag(&self) -> TypeTag {
        match self {
            Self::Iota => GAS::type_tag(),
            Self::Shimmer => SMR::type_tag(),
        }
    }
}
