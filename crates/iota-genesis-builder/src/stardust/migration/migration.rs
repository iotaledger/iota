// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Contains the logic for the migration process.

use std::{
    cmp::Reverse,
    collections::{HashMap, HashSet},
    io::{BufWriter, prelude::Write},
};

use anyhow::Result;
use iota_move_build::CompiledPackage;
use iota_protocol_config::ProtocolVersion;
use iota_sdk::types::block::output::{FoundryOutput, Output, OutputId};
use iota_types::{
    IOTA_FRAMEWORK_PACKAGE_ID, IOTA_SYSTEM_PACKAGE_ID, MOVE_STDLIB_PACKAGE_ID, STARDUST_PACKAGE_ID,
    balance::Balance,
    base_types::{IotaAddress, ObjectID, TxContext},
    epoch_data::EpochData,
    object::Object,
    stardust::coin_type::CoinType,
    timelock::timelock::{self, TimeLock, is_timelocked_balance},
};
use move_binary_format::file_format_common::VERSION_MAX;
use tracing::info;

use crate::stardust::{
    migration::{
        MigrationTargetNetwork,
        executor::Executor,
        verification::{created_objects::CreatedObjects, verify_outputs},
    },
    native_token::package_data::NativeTokenPackageData,
    process_outputs::process_outputs_for_iota,
    types::{
        address_swap_map::AddressSwapMap, address_swap_split_map::AddressSwapSplitMap,
        output_header::OutputHeader,
    },
};

/// We fix the protocol version used in the migration.
pub const MIGRATION_PROTOCOL_VERSION: u64 = 1;

/// The dependencies of the generated packages for native tokens.
pub const PACKAGE_DEPS: [ObjectID; 4] = [
    MOVE_STDLIB_PACKAGE_ID,
    IOTA_FRAMEWORK_PACKAGE_ID,
    IOTA_SYSTEM_PACKAGE_ID,
    STARDUST_PACKAGE_ID,
];

pub(crate) const NATIVE_TOKEN_BAG_KEY_TYPE: &str = "0x01::ascii::String";

/// An alias for representing the timestamp of a Timelock
pub type ExpirationTimestamp = u64;

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
    /// The coin type to use in order to migrate outputs. Can only be equal to
    /// `Iota` at the moment. Is fixed for the entire migration process.
    coin_type: CoinType,
    address_swap_map: AddressSwapMap,
}

impl Migration {
    /// Try to setup the migration process by creating the inner executor
    /// and bootstraping the in-memory storage.
    pub fn new(
        target_milestone_timestamp_sec: u32,
        total_supply: u64,
        target_network: MigrationTargetNetwork,
        coin_type: CoinType,
        address_swap_map: AddressSwapMap,
    ) -> Result<Self> {
        let executor = Executor::new(
            ProtocolVersion::new(MIGRATION_PROTOCOL_VERSION),
            target_network,
            coin_type,
        )?;
        Ok(Self {
            target_milestone_timestamp_sec,
            total_supply,
            executor,
            output_objects_map: Default::default(),
            coin_type,
            address_swap_map,
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
        // context will also map to the same objects between runs.
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
        self.address_swap_map.verify_all_addresses_swapped()?;
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
        create_snapshot(self.into_objects(), writer)?;
        info!("Snapshot file written.");
        Ok(())
    }

    /// Run all stages of the migration coming from a Hornet snapshot with IOTA
    /// coin type.
    pub fn run_for_iota<'a>(
        self,
        target_milestone_timestamp: u32,
        swap_split_map: AddressSwapSplitMap,
        outputs: impl Iterator<Item = Result<(OutputHeader, Output)>> + 'a,
        writer: impl Write,
    ) -> Result<()> {
        itertools::process_results(
            process_outputs_for_iota(target_milestone_timestamp, swap_split_map, outputs),
            |outputs| self.run(outputs, writer),
        )?
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
                Output::Alias(alias) => self.executor.create_alias_objects(
                    header,
                    alias,
                    self.coin_type,
                    &mut self.address_swap_map,
                )?,
                Output::Nft(nft) => self.executor.create_nft_objects(
                    header,
                    nft,
                    self.coin_type,
                    &mut self.address_swap_map,
                )?,
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
                            &mut self.address_swap_map,
                        )?
                    } else {
                        self.executor.create_basic_objects(
                            header,
                            basic,
                            self.target_milestone_timestamp_sec,
                            &self.coin_type,
                            &mut self.address_swap_map,
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
    /// [`InMemoryStorage`](iota_types::in_memory_storage::InMemoryStorage).
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
            &self.address_swap_map,
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

/// All the objects created during the migration.
///
/// Internally it maintains indexes of [`TimeLock`] and
/// [`iota_types::gas_coin::GasCoin`] objects grouped by their owners to
/// accommodate queries of this sort.
#[derive(Debug, Clone, Default)]
pub struct MigrationObjects {
    inner: Vec<Object>,
    owner_timelock: HashMap<IotaAddress, Vec<usize>>,
    owner_gas_coin: HashMap<IotaAddress, Vec<usize>>,
}

impl Extend<Object> for MigrationObjects {
    fn extend<T: IntoIterator<Item = Object>>(&mut self, iter: T) {
        let last_current_ix = self.inner.len();
        self.inner.extend(iter);
        for (i, tag, object) in self.inner[last_current_ix..]
            .iter()
            .zip(last_current_ix..)
            .filter_map(|(object, i)| {
                let tag = object.struct_tag()?;
                Some((i, tag, object))
            })
        {
            let owner_object_map = if is_timelocked_balance(&tag) {
                &mut self.owner_timelock
            } else if object.is_gas_coin() {
                &mut self.owner_gas_coin
            } else {
                continue;
            };
            let owner = object
                .owner
                .get_owner_address()
                .expect("timelocks should have an address owner");
            owner_object_map
                .entry(owner)
                .and_modify(|object_ixs| object_ixs.push(i))
                .or_insert_with(|| vec![i]);
        }
    }
}

impl MigrationObjects {
    pub fn new(objects: Vec<Object>) -> Self {
        let mut migration_objects = Self::default();
        migration_objects.extend(objects);
        migration_objects
    }

    /// Evict the objects with the specified ids
    pub fn evict(&mut self, objects: impl IntoIterator<Item = ObjectID>) {
        let eviction_set = objects.into_iter().collect::<HashSet<_>>();
        let inner = std::mem::take(&mut self.inner);
        self.inner = inner
            .into_iter()
            .filter(|object| !eviction_set.contains(&object.id()))
            .collect();
    }

    /// Take the inner migration objects.
    ///
    /// This follows the semantics of [`std::mem::take`].
    pub fn take_objects(&mut self) -> Vec<Object> {
        std::mem::take(&mut self.inner)
    }

    /// Checks if inner is empty.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Get [`TimeLock`] objects created during the migration together with
    /// their expiration timestamp.
    ///
    /// The query is filtered by the object owner.
    ///
    /// The returned objects are ordered by expiration timestamp, in descending
    /// order.
    pub fn get_sorted_timelocks_and_expiration_by_owner(
        &self,
        address: IotaAddress,
    ) -> Option<Vec<(&Object, ExpirationTimestamp)>> {
        self.get_timelocks_and_expiration_by_owner(address)
            .map(|mut timelocks| {
                timelocks.sort_by_key(|&(_, timestamp)| Reverse(timestamp));
                timelocks
            })
    }

    /// Get [`TimeLock`] objects created during the migration together with
    /// their expiration timestamp.
    ///
    /// The query is filtered by the object owner.
    pub fn get_timelocks_and_expiration_by_owner(
        &self,
        address: IotaAddress,
    ) -> Option<Vec<(&Object, ExpirationTimestamp)>> {
        Some(
            self.owner_timelock
                .get(&address)?
                .iter()
                .map(|i| {
                    (
                        &self.inner[*i],
                        self.inner[*i]
                            .to_rust::<TimeLock<Balance>>()
                            .expect("this should be a TimeLock object")
                            .expiration_timestamp_ms(),
                    )
                })
                .collect(),
        )
    }

    /// Get [`iota_types::gas_coin::GasCoin`] objects created during the
    /// migration.
    ///
    /// The query is filtered by the object owner.
    pub fn get_gas_coins_by_owner(&self, address: IotaAddress) -> Option<Vec<&Object>> {
        Some(
            self.owner_gas_coin
                .get(&address)?
                .iter()
                .map(|i| &self.inner[*i])
                .collect(),
        )
    }
}

// Build a `CompiledPackage` from a given `FoundryOutput`.
fn generate_package(foundry: &FoundryOutput) -> Result<CompiledPackage> {
    let native_token_data = NativeTokenPackageData::try_from(foundry)?;
    crate::stardust::native_token::package_builder::build_and_compile(native_token_data)
}

/// Serialize the objects stored in [`InMemoryStorage`] into a file using
/// [`bcs`] encoding.
fn create_snapshot(ledger: Vec<Object>, writer: impl Write) -> Result<()> {
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
            module.serialize_with_version(VERSION_MAX, &mut buf)?;
            Ok(buf)
        })
        .collect::<Result<_>>()
}

/// Create a [`TxContext]` that remains the same across invocations.
pub(super) fn create_migration_context(
    coin_type: &CoinType,
    target_network: MigrationTargetNetwork,
) -> TxContext {
    TxContext::new(
        &IotaAddress::default(),
        &target_network.migration_transaction_digest(coin_type),
        &EpochData::new_genesis(0),
    )
}

#[cfg(test)]
mod tests {
    use iota_protocol_config::ProtocolConfig;
    use iota_types::{
        balance::Balance,
        base_types::SequenceNumber,
        gas_coin::GasCoin,
        id::UID,
        object::{Data, Owner},
        timelock::timelock::{TimeLock, to_genesis_object},
    };

    use super::*;

    #[test]
    fn migration_objects_get_timelocks() {
        let owner = IotaAddress::random_for_testing_only();
        let address = IotaAddress::random_for_testing_only();
        let tx_context = TxContext::random_for_testing_only();
        let expected_timelocks = (0..4)
            .map(|_| TimeLock::new(UID::new(ObjectID::random()), Balance::new(0), 0, None))
            .map(|timelock| {
                to_genesis_object(
                    timelock,
                    owner,
                    &ProtocolConfig::get_for_min_version(),
                    &tx_context,
                    SequenceNumber::MIN,
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        let non_matching_timelocks = (0..8)
            .map(|_| TimeLock::new(UID::new(ObjectID::random()), Balance::new(0), 0, None))
            .map(|timelock| {
                to_genesis_object(
                    timelock,
                    address,
                    &ProtocolConfig::get_for_min_version(),
                    &tx_context,
                    SequenceNumber::MIN,
                )
                .unwrap()
            });
        let non_matching_objects = (0..8)
            .map(|_| GasCoin::new_for_testing(0).to_object(SequenceNumber::MIN))
            .map(|move_object| {
                Object::new_from_genesis(
                    Data::Move(move_object),
                    Owner::AddressOwner(address),
                    tx_context.digest(),
                )
            });
        let migration_objects = MigrationObjects::new(
            non_matching_objects
                .chain(non_matching_timelocks)
                .chain(expected_timelocks.clone())
                .collect(),
        );
        let matching_objects = migration_objects
            .get_timelocks_and_expiration_by_owner(owner)
            .unwrap();
        assert_eq!(
            expected_timelocks,
            matching_objects
                .into_iter()
                .map(|(timelock, _)| timelock.clone())
                .collect::<Vec<Object>>()
        );
    }

    #[test]
    fn migration_objects_get_gas_coins() {
        let owner = IotaAddress::random_for_testing_only();
        let address = IotaAddress::random_for_testing_only();
        let tx_context = TxContext::random_for_testing_only();
        let non_matching_timelocks = (0..8)
            .map(|_| TimeLock::new(UID::new(ObjectID::random()), Balance::new(0), 0, None))
            .map(|timelock| {
                to_genesis_object(
                    timelock,
                    address,
                    &ProtocolConfig::get_for_min_version(),
                    &tx_context,
                    SequenceNumber::MIN,
                )
                .unwrap()
            });
        let expected_gas_coins = (0..8)
            .map(|_| GasCoin::new_for_testing(0).to_object(SequenceNumber::MIN))
            .map(|move_object| {
                Object::new_from_genesis(
                    Data::Move(move_object),
                    Owner::AddressOwner(owner),
                    tx_context.digest(),
                )
            })
            .collect::<Vec<_>>();
        let migration_objects = MigrationObjects::new(
            non_matching_timelocks
                .chain(expected_gas_coins.clone())
                .collect(),
        );
        let matching_objects = migration_objects.get_gas_coins_by_owner(owner).unwrap();
        assert_eq!(
            expected_gas_coins,
            matching_objects
                .into_iter()
                .cloned()
                .collect::<Vec<Object>>()
        );
    }
}
