// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeSet, HashMap},
    sync::Arc,
};

use anyhow::Result;
use iota_adapter_latest::{
    adapter::new_move_vm, execution_mode, gas_charger::GasCharger, programmable_transactions,
    temporary_store::TemporaryStore,
};
use iota_framework::BuiltInFramework;
use iota_move_build::CompiledPackage;
use iota_move_natives_latest::all_natives;
use iota_protocol_config::{Chain, ProtocolConfig, ProtocolVersion};
use iota_sdk::types::block::output::{
    AliasOutput, BasicOutput, FoundryOutput, NativeTokens, NftOutput, OutputId, TokenId,
};
use iota_types::{
    IOTA_FRAMEWORK_PACKAGE_ID, STARDUST_PACKAGE_ID, TypeTag,
    balance::Balance,
    base_types::{IotaAddress, ObjectID, ObjectRef, SequenceNumber, TxContext},
    coin_manager::{CoinManager, CoinManagerTreasuryCap},
    collection_types::Bag,
    dynamic_field::Field,
    id::UID,
    in_memory_storage::InMemoryStorage,
    inner_temporary_store::InnerTemporaryStore,
    metrics::LimitsMetrics,
    move_package::{MovePackage, TypeOrigin, UpgradeCap},
    object::Object,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    stardust::{
        coin_type::CoinType,
        output::{Nft, foundry::create_foundry_amount_coin},
    },
    timelock::timelock,
    transaction::{
        Argument, CheckedInputObjects, Command, InputObjectKind, InputObjects, ObjectArg,
        ObjectReadResult, ProgrammableTransaction,
    },
};
use move_core_types::{ident_str, language_storage::StructTag};
use move_vm_runtime_latest::move_vm::MoveVM;

use crate::{
    process_package,
    stardust::{
        migration::{
            MigrationTargetNetwork, PACKAGE_DEPS, create_migration_context, package_module_bytes,
            verification::created_objects::CreatedObjects,
        },
        types::{
            address_swap_map::AddressSwapMap, output_header::OutputHeader,
            token_scheme::SimpleTokenSchemeU64,
        },
    },
};

/// Creates the objects that map to the stardust UTXO ledger.
///
/// Internally uses an unmetered Move VM.
pub(super) struct Executor {
    protocol_config: ProtocolConfig,
    tx_context: TxContext,
    /// Stores all the migration objects.
    store: InMemoryStorage,
    /// Caches the system packages and init objects. Useful for evicting
    /// them from the store before creating the snapshot.
    system_packages_and_objects: BTreeSet<ObjectID>,
    move_vm: Arc<MoveVM>,
    metrics: Arc<LimitsMetrics>,
    /// Map the stardust token id [`TokenId`] to the on-chain info of the
    /// published foundry objects.
    native_tokens: HashMap<TokenId, FoundryLedgerData>,
    /// The coin type to use in order to migrate outputs. Can only be equal to
    /// `Iota` at the moment. Is fixed for the entire migration process.
    coin_type: CoinType,
}

impl Executor {
    /// Setup the execution environment backed by an in-memory store that holds
    /// all the system packages.
    pub(super) fn new(
        protocol_version: ProtocolVersion,
        target_network: MigrationTargetNetwork,
        coin_type: CoinType,
    ) -> Result<Self> {
        let mut tx_context = create_migration_context(&coin_type, target_network);
        // Use a throwaway metrics registry for transaction execution.
        let metrics = Arc::new(LimitsMetrics::new(&prometheus::Registry::new()));
        let mut store = InMemoryStorage::new(Vec::new());
        // We don't know the chain ID here since we haven't yet created the genesis
        // checkpoint. However since we know there are no chain specific
        // protocol config options in genesis, we use Chain::Unknown here.
        let protocol_config = ProtocolConfig::get_for_version(protocol_version, Chain::Unknown);
        // Get the correct system packages for our protocol version. If we cannot find
        // the snapshot that means that we must be at the latest version and we
        // should use the latest version of the framework.
        let system_packages =
            iota_framework_snapshot::load_bytecode_snapshot(protocol_version.as_u64())
                .unwrap_or_else(|_| BuiltInFramework::iter_system_packages().cloned().collect());

        let silent = true;
        let executor = iota_execution::executor(&protocol_config, silent, None)
            .expect("Creating an executor should not fail here");
        for system_package in system_packages.into_iter() {
            process_package(
                &mut store,
                executor.as_ref(),
                &mut tx_context,
                &system_package.modules(),
                system_package.dependencies,
                &protocol_config,
                metrics.clone(),
            )?;
        }
        let move_vm = Arc::new(new_move_vm(
            all_natives(silent, &protocol_config),
            &protocol_config,
            None,
        )?);

        let system_packages_and_objects = store.objects().keys().copied().collect();
        Ok(Self {
            protocol_config,
            tx_context,
            store,
            system_packages_and_objects,
            move_vm,
            metrics,
            native_tokens: Default::default(),
            coin_type,
        })
    }

    pub fn store(&self) -> &InMemoryStorage {
        &self.store
    }

    pub(crate) fn native_tokens(&self) -> &HashMap<TokenId, FoundryLedgerData> {
        &self.native_tokens
    }

    /// The migration objects.
    ///
    /// The system packages and underlying `init` objects
    /// are filtered out because they will be generated
    /// in the genesis process.
    pub(super) fn into_objects(self) -> Vec<Object> {
        self.store
            .into_inner()
            .into_values()
            .filter(|object| !self.system_packages_and_objects.contains(&object.id()))
            .collect()
    }

    /// Load input objects from the store to be used as checked
    /// input while executing a transaction
    pub(crate) fn load_input_objects(
        &self,
        object_refs: impl IntoIterator<Item = ObjectRef> + 'static,
    ) -> impl Iterator<Item = ObjectReadResult> + '_ {
        object_refs.into_iter().filter_map(|object_ref| {
            Some(ObjectReadResult::new(
                InputObjectKind::ImmOrOwnedMoveObject(object_ref),
                self.store.get_object(&object_ref.0)?.clone().into(),
            ))
        })
    }

    /// Load packages from the store to be used as checked
    /// input while executing a transaction
    pub(crate) fn load_packages(
        &self,
        object_ids: impl IntoIterator<Item = ObjectID> + 'static,
    ) -> impl Iterator<Item = ObjectReadResult> + '_ {
        object_ids.into_iter().filter_map(|object_id| {
            Some(ObjectReadResult::new(
                InputObjectKind::MovePackage(object_id),
                self.store.get_object(&object_id)?.clone().into(),
            ))
        })
    }

    fn checked_system_packages(&self) -> CheckedInputObjects {
        CheckedInputObjects::new_for_genesis(self.load_packages(PACKAGE_DEPS).collect())
    }

    pub(crate) fn execute_pt_unmetered(
        &mut self,
        input_objects: CheckedInputObjects,
        pt: ProgrammableTransaction,
    ) -> Result<InnerTemporaryStore> {
        let input_objects = input_objects.into_inner();
        let epoch_id = 0; // Genesis
        let mut temporary_store = TemporaryStore::new(
            &self.store,
            input_objects,
            vec![],
            self.tx_context.digest(),
            &self.protocol_config,
            epoch_id,
        );
        let mut gas_charger = GasCharger::new_unmetered(self.tx_context.digest());
        programmable_transactions::execution::execute::<execution_mode::Normal>(
            &self.protocol_config,
            self.metrics.clone(),
            &self.move_vm,
            &mut temporary_store,
            &mut self.tx_context,
            &mut gas_charger,
            pt,
            &mut None,
        )?;
        temporary_store.update_object_version_and_prev_tx();
        Ok(temporary_store.into_inner())
    }

    /// Process the foundry outputs as follows:
    ///
    /// * Publish the generated packages using a tailored unmetered executor.
    /// * For each native token, map the [`TokenId`] to the [`ObjectID`] of the
    ///   coin that holds its total supply.
    /// * Update the inner store with the created objects.
    pub(super) fn create_foundries<'a>(
        &mut self,
        foundries: impl IntoIterator<Item = (&'a OutputHeader, &'a FoundryOutput, CompiledPackage)>,
    ) -> Result<Vec<(OutputId, CreatedObjects)>> {
        let mut res = Vec::new();
        for (header, foundry, pkg) in foundries {
            let mut created_objects = CreatedObjects::default();
            let modules = package_module_bytes(&pkg)?;
            let deps = self.checked_system_packages();
            let pt = {
                let mut builder = ProgrammableTransactionBuilder::new();
                let upgrade_cap = builder.command(Command::Publish(modules, PACKAGE_DEPS.into()));
                // We make a dummy transfer because the `UpgradeCap` does
                // not have the drop ability.
                //
                // We ignore it in the genesis, to render the package immutable.
                builder.transfer_arg(Default::default(), upgrade_cap);
                builder.finish()
            };
            let InnerTemporaryStore { written, .. } = self.execute_pt_unmetered(deps, pt)?;
            // Get on-chain info
            let mut native_token_coin_id = None::<ObjectID>;
            let mut foundry_package = None::<&MovePackage>;
            for object in written.values() {
                if object.is_package() {
                    foundry_package = Some(
                        object
                            .data
                            .try_as_package()
                            .expect("already verified this is a package"),
                    );
                    created_objects.set_package(object.id())?;
                } else if object.is_coin() {
                    native_token_coin_id = Some(object.id());
                    created_objects.set_native_token_coin(object.id())?;
                } else if let Some(tag) = object.struct_tag() {
                    if CoinManager::is_coin_manager(&tag) {
                        created_objects.set_coin_manager(object.id())?;
                    } else if CoinManagerTreasuryCap::is_coin_manager_treasury_cap(&tag) {
                        created_objects.set_coin_manager_treasury_cap(object.id())?;
                    }
                }
            }
            let (native_token_coin_id, foundry_package) = (
                native_token_coin_id.expect("a native token coin must have been minted"),
                foundry_package.expect("there should be a published package"),
            );
            self.native_tokens.insert(
                foundry.token_id(),
                FoundryLedgerData::new(
                    native_token_coin_id,
                    foundry_package,
                    SimpleTokenSchemeU64::try_from(foundry.token_scheme().as_simple())?,
                ),
            );

            // Create the amount coin object.
            let amount_coin = create_foundry_amount_coin(
                &header.output_id(),
                foundry,
                &self.tx_context,
                foundry_package.version(),
                &self.protocol_config,
                &self.coin_type,
            )?;
            created_objects.set_coin(amount_coin.id())?;
            self.store.insert_object(amount_coin);

            self.store.finish(
                written
                    .into_iter()
                    // We ignore the [`UpgradeCap`] objects.
                    .filter(|(_, object)| object.struct_tag() != Some(UpgradeCap::type_()))
                    .collect(),
            );
            res.push((header.output_id(), created_objects));
        }
        Ok(res)
    }

    pub(super) fn create_alias_objects(
        &mut self,
        header: &OutputHeader,
        alias: &AliasOutput,
        coin_type: CoinType,
        address_swap_map: &mut AddressSwapMap,
    ) -> Result<CreatedObjects> {
        let mut created_objects = CreatedObjects::default();

        // Take the Alias ID set in the output or, if its zeroized, compute it from the
        // Output ID.
        let alias_id = ObjectID::new(*alias.alias_id().or_from_output_id(&header.output_id()));
        let move_alias = iota_types::stardust::output::Alias::try_from_stardust(alias_id, alias)?;

        // TODO: We should ensure that no circular ownership exists.
        let alias_output_owner =
            address_swap_map.swap_stardust_to_iota_address_owner(alias.governor_address())?;

        let package_deps = InputObjects::new(self.load_packages(PACKAGE_DEPS).collect());
        let version = package_deps.lamport_timestamp(&[]);

        let move_alias_object = move_alias.to_genesis_object(
            alias_output_owner,
            &self.protocol_config,
            &self.tx_context,
            version,
        )?;
        let move_alias_object_ref = move_alias_object.compute_object_reference();

        self.store.insert_object(move_alias_object);

        let (bag, version, fields) = self.create_bag_with_pt(alias.native_tokens())?;
        created_objects.set_native_tokens(fields)?;

        let move_alias_output = iota_types::stardust::output::AliasOutput::try_from_stardust(
            self.tx_context.fresh_id(),
            alias,
            bag,
        )?;

        // The bag will be wrapped into the alias output object, so
        // by equating their versions we emulate a ptb.
        let move_alias_output_object = move_alias_output.to_genesis_object(
            alias_output_owner,
            &self.protocol_config,
            &self.tx_context,
            version,
            coin_type,
        )?;
        let move_alias_output_object_ref = move_alias_output_object.compute_object_reference();

        created_objects.set_output(move_alias_output_object.id())?;
        self.store.insert_object(move_alias_output_object);

        // Attach the Alias to the Alias Output as a dynamic object field via the
        // attach_alias convenience method.
        let pt = {
            let mut builder = ProgrammableTransactionBuilder::new();

            let alias_output_arg =
                builder.obj(ObjectArg::ImmOrOwnedObject(move_alias_output_object_ref))?;
            let alias_arg = builder.obj(ObjectArg::ImmOrOwnedObject(move_alias_object_ref))?;

            builder.programmable_move_call(
                STARDUST_PACKAGE_ID,
                ident_str!("alias_output").into(),
                ident_str!("attach_alias").into(),
                vec![coin_type.to_type_tag()],
                vec![alias_output_arg, alias_arg],
            );

            builder.finish()
        };

        let input_objects = CheckedInputObjects::new_for_genesis(
            self.load_input_objects([move_alias_object_ref, move_alias_output_object_ref])
                .chain(self.load_packages(PACKAGE_DEPS))
                .collect(),
        );

        let InnerTemporaryStore { written, .. } = self.execute_pt_unmetered(input_objects, pt)?;
        self.store.finish(written);

        Ok(created_objects)
    }

    /// Create a [`Bag`] of balances of native tokens executing a programmable
    /// transaction block.
    pub(crate) fn create_bag_with_pt(
        &mut self,
        native_tokens: &NativeTokens,
    ) -> Result<(Bag, SequenceNumber, Vec<ObjectID>)> {
        let mut object_deps = Vec::with_capacity(native_tokens.len());
        let mut foundry_package_deps = Vec::with_capacity(native_tokens.len());
        let pt = {
            let mut builder = ProgrammableTransactionBuilder::new();
            let bag = pt::bag_new(&mut builder);
            for token in native_tokens.iter() {
                let Some(foundry_ledger_data) = self.native_tokens.get_mut(token.token_id()) else {
                    anyhow::bail!("foundry for native token has not been published");
                };

                let Some(foundry_coin) = self
                    .store
                    .get_object(&foundry_ledger_data.native_token_coin_id)
                else {
                    anyhow::bail!("foundry coin should exist");
                };
                let object_ref = foundry_coin.compute_object_reference();

                object_deps.push(object_ref);
                foundry_package_deps.push(foundry_ledger_data.package_id);

                let token_type =
                    foundry_ledger_data.to_canonical_string(/* with_prefix */ true);
                let bag_key = foundry_ledger_data.to_canonical_string(/* with_prefix */ false);

                let adjusted_amount = foundry_ledger_data
                    .token_scheme_u64
                    .adjust_tokens(token.amount());

                foundry_ledger_data.minted_value = foundry_ledger_data
                    .minted_value
                    .checked_sub(adjusted_amount)
                    .ok_or_else(|| anyhow::anyhow!("underflow splitting native token balance"))?;

                let balance = pt::coin_balance_split(
                    &mut builder,
                    object_ref,
                    token_type.parse()?,
                    adjusted_amount,
                )?;
                pt::bag_add(&mut builder, bag, bag_key, balance, token_type)?;
            }

            // The `Bag` object does not have the `drop` ability so we have to use it
            // in the transaction block. Therefore we transfer it to the `0x0` address.
            //
            // Nevertheless, we only store the contents of the object, and thus the
            // ownership metadata are irrelevant to us. This is a dummy transfer
            // then to satisfy the VM.
            builder.transfer_arg(Default::default(), bag);
            builder.finish()
        };
        let checked_input_objects = CheckedInputObjects::new_for_genesis(
            self.load_packages(PACKAGE_DEPS)
                .chain(self.load_packages(foundry_package_deps))
                .chain(self.load_input_objects(object_deps))
                .collect(),
        );
        let InnerTemporaryStore {
            mut written,
            input_objects,
            ..
        } = self.execute_pt_unmetered(checked_input_objects, pt)?;
        let bag_object = written
            .iter()
            // We filter out the dynamic-field objects that are owned by the bag
            // and we should be left with only the bag
            .find_map(|(id, object)| {
                (!input_objects.contains_key(id) && !object.is_child_object()).then_some(id)
            })
            .copied()
            .and_then(|id| written.remove(&id))
            .ok_or_else(|| anyhow::anyhow!("the bag should have been created"))?;
        written.remove(&bag_object.id());
        let field_ids = written
            .iter()
            .filter_map(|(id, object)| object.to_rust::<Field<String, Balance>>().map(|_| *id))
            .collect();
        // Save the modified coins
        self.store.finish(written);
        // Return bag
        let bag = bcs::from_bytes(
            bag_object
                .data
                .try_as_move()
                .expect("this should be a move object")
                .contents(),
        )
        .expect("this should be a valid Bag Move object");
        Ok((bag, bag_object.version(), field_ids))
    }

    /// Create [`Coin`] objects representing native tokens in the ledger.
    fn create_native_token_coins(
        &mut self,
        native_tokens: &NativeTokens,
        owner: IotaAddress,
    ) -> Result<Vec<ObjectID>> {
        let mut object_deps = Vec::with_capacity(native_tokens.len());
        let mut foundry_package_deps = Vec::with_capacity(native_tokens.len());
        let mut foundry_coins = Vec::with_capacity(native_tokens.len());
        let pt = {
            let mut builder = ProgrammableTransactionBuilder::new();
            for token in native_tokens.iter() {
                let Some(foundry_ledger_data) = self.native_tokens.get_mut(token.token_id()) else {
                    anyhow::bail!("foundry for native token has not been published");
                };

                let Some(foundry_coin) = self
                    .store
                    .get_object(&foundry_ledger_data.native_token_coin_id)
                else {
                    anyhow::bail!("foundry coin should exist");
                };
                let object_ref = foundry_coin.compute_object_reference();
                foundry_coins.push(foundry_coin.id());

                object_deps.push(object_ref);
                foundry_package_deps.push(foundry_ledger_data.package_id);

                // Pay using that object
                let adjusted_amount = foundry_ledger_data
                    .token_scheme_u64
                    .adjust_tokens(token.amount());

                foundry_ledger_data.minted_value = foundry_ledger_data
                    .minted_value
                    .checked_sub(adjusted_amount)
                    .ok_or_else(|| anyhow::anyhow!("underflow splitting native token balance"))?;

                builder.pay(vec![object_ref], vec![owner], vec![adjusted_amount])?;
            }

            builder.finish()
        };
        let checked_input_objects = CheckedInputObjects::new_for_genesis(
            self.load_packages(PACKAGE_DEPS)
                .chain(self.load_packages(foundry_package_deps))
                .chain(self.load_input_objects(object_deps))
                .collect(),
        );
        // Execute
        let InnerTemporaryStore { written, .. } =
            self.execute_pt_unmetered(checked_input_objects, pt)?;

        let coin_ids = written
            .keys()
            // Linear search is ok due to the expected very small size of
            // `foundry_coins`
            .filter(|id| !foundry_coins.contains(id))
            .copied()
            .collect();

        // Save the modified coin
        self.store.finish(written);
        Ok(coin_ids)
    }

    /// This implements the control flow in
    /// crates/iota-framework/packages/stardust/basic_migration_graph.svg
    pub(super) fn create_basic_objects(
        &mut self,
        header: &OutputHeader,
        basic_output: &BasicOutput,
        target_milestone_timestamp_sec: u32,
        coin_type: &CoinType,
        address_swap_map: &mut AddressSwapMap,
    ) -> Result<CreatedObjects> {
        let mut basic =
            iota_types::stardust::output::BasicOutput::new(header.new_object_id(), basic_output)?;

        let basic_objects_owner =
            address_swap_map.swap_stardust_to_iota_address(basic_output.address())?;

        let mut created_objects = CreatedObjects::default();

        // The minimum version of the manually created objects
        let package_deps = InputObjects::new(self.load_packages(PACKAGE_DEPS).collect());
        let mut version = package_deps.lamport_timestamp(&[]);

        let object = if basic.is_simple_coin(target_milestone_timestamp_sec) {
            if !basic_output.native_tokens().is_empty() {
                let coins = self
                    .create_native_token_coins(basic_output.native_tokens(), basic_objects_owner)?;
                created_objects.set_native_tokens(coins)?;
            }
            let amount_coin = basic.into_genesis_coin_object(
                basic_objects_owner,
                &self.protocol_config,
                &self.tx_context,
                version,
                coin_type,
            )?;
            created_objects.set_coin(amount_coin.id())?;
            amount_coin
        } else {
            if !basic_output.native_tokens().is_empty() {
                let fields;
                // The bag will be wrapped into the basic output object, so
                // by equating their versions we emulate a ptb.
                (basic.native_tokens, version, fields) =
                    self.create_bag_with_pt(basic_output.native_tokens())?;
                created_objects.set_native_tokens(fields)?;
            } else {
                // Overwrite the default 0 UID of `Bag::default()`, since we won't
                // be creating a new bag in this code path.
                basic.native_tokens.id = UID::new(self.tx_context.fresh_id());
            }
            let object = basic.to_genesis_object(
                basic_objects_owner,
                &self.protocol_config,
                &self.tx_context,
                version,
                coin_type,
            )?;
            created_objects.set_output(object.id())?;
            object
        };

        self.store.insert_object(object);
        Ok(created_objects)
    }

    /// Creates [`TimeLock<Balance<IOTA>>`] objects which represent vested
    /// rewards that were created during the stardust upgrade on IOTA
    /// mainnet.
    pub(super) fn create_timelock_object(
        &mut self,
        output_id: OutputId,
        basic_output: &BasicOutput,
        target_milestone_timestamp: u32,
        address_swap_map: &mut AddressSwapMap,
    ) -> Result<CreatedObjects> {
        let mut created_objects = CreatedObjects::default();

        let basic_output_owner =
            address_swap_map.swap_stardust_to_iota_address(basic_output.address())?;

        let package_deps = InputObjects::new(self.load_packages(PACKAGE_DEPS).collect());
        let version = package_deps.lamport_timestamp(&[]);

        let timelock =
            timelock::try_from_stardust(output_id, basic_output, target_milestone_timestamp)?;

        let object = timelock::to_genesis_object(
            timelock,
            basic_output_owner,
            &self.protocol_config,
            &self.tx_context,
            version,
        )?;

        created_objects.set_output(object.id())?;

        self.store.insert_object(object);
        Ok(created_objects)
    }

    pub(super) fn create_nft_objects(
        &mut self,
        header: &OutputHeader,
        nft: &NftOutput,
        coin_type: CoinType,
        address_swap_map: &mut AddressSwapMap,
    ) -> Result<CreatedObjects> {
        let mut created_objects = CreatedObjects::default();

        // Take the Nft ID set in the output or, if its zeroized, compute it from the
        // Output ID.
        let nft_id = ObjectID::new(*nft.nft_id().or_from_output_id(&header.output_id()));
        let move_nft = Nft::try_from_stardust(nft_id, nft)?;

        // TODO: We should ensure that no circular ownership exists.
        let nft_output_owner_address =
            address_swap_map.swap_stardust_to_iota_address(nft.address())?;

        let nft_output_owner =
            address_swap_map.swap_stardust_to_iota_address_owner(nft.address())?;

        let package_deps = InputObjects::new(self.load_packages(PACKAGE_DEPS).collect());
        let version = package_deps.lamport_timestamp(&[]);
        let move_nft_object = move_nft.to_genesis_object(
            nft_output_owner,
            &self.protocol_config,
            &self.tx_context,
            version,
        )?;

        let move_nft_object_ref = move_nft_object.compute_object_reference();
        self.store.insert_object(move_nft_object);

        let (bag, version, fields) = self.create_bag_with_pt(nft.native_tokens())?;
        created_objects.set_native_tokens(fields)?;
        let move_nft_output = iota_types::stardust::output::NftOutput::try_from_stardust(
            self.tx_context.fresh_id(),
            nft,
            bag,
        )?;

        // The bag will be wrapped into the nft output object, so
        // by equating their versions we emulate a ptb.
        let move_nft_output_object = move_nft_output.to_genesis_object(
            nft_output_owner_address,
            &self.protocol_config,
            &self.tx_context,
            version,
            coin_type,
        )?;
        let move_nft_output_object_ref = move_nft_output_object.compute_object_reference();
        created_objects.set_output(move_nft_output_object.id())?;
        self.store.insert_object(move_nft_output_object);

        // Attach the Nft to the Nft Output as a dynamic object field via the attach_nft
        // convenience method.
        let pt = {
            let mut builder = ProgrammableTransactionBuilder::new();

            let nft_output_arg =
                builder.obj(ObjectArg::ImmOrOwnedObject(move_nft_output_object_ref))?;
            let nft_arg = builder.obj(ObjectArg::ImmOrOwnedObject(move_nft_object_ref))?;
            builder.programmable_move_call(
                STARDUST_PACKAGE_ID,
                ident_str!("nft_output").into(),
                ident_str!("attach_nft").into(),
                vec![coin_type.to_type_tag()],
                vec![nft_output_arg, nft_arg],
            );

            builder.finish()
        };

        let input_objects = CheckedInputObjects::new_for_genesis(
            self.load_input_objects([move_nft_object_ref, move_nft_output_object_ref])
                .chain(self.load_packages(PACKAGE_DEPS))
                .collect(),
        );

        let InnerTemporaryStore { written, .. } = self.execute_pt_unmetered(input_objects, pt)?;
        self.store.finish(written);

        Ok(created_objects)
    }
}

#[cfg(test)]
impl Executor {
    /// Set the [`TxContext`] of the [`Executor`].
    pub(crate) fn with_tx_context(mut self, tx_context: TxContext) -> Self {
        self.tx_context = tx_context;
        self
    }

    /// Set the [`InMemoryStorage`] of the [`Executor`].
    pub(crate) fn with_store(mut self, store: InMemoryStorage) -> Self {
        self.store = store;
        self
    }
}

mod pt {
    use super::*;
    use crate::stardust::migration::NATIVE_TOKEN_BAG_KEY_TYPE;

    pub fn coin_balance_split(
        builder: &mut ProgrammableTransactionBuilder,
        foundry_coin_ref: ObjectRef,
        token_type_tag: TypeTag,
        amount: u64,
    ) -> Result<Argument> {
        let foundry_coin_ref = builder.obj(ObjectArg::ImmOrOwnedObject(foundry_coin_ref))?;
        let amount = builder.pure(amount)?;
        let coin = builder.programmable_move_call(
            IOTA_FRAMEWORK_PACKAGE_ID,
            ident_str!("coin").into(),
            ident_str!("split").into(),
            vec![token_type_tag.clone()],
            vec![foundry_coin_ref, amount],
        );
        Ok(builder.programmable_move_call(
            IOTA_FRAMEWORK_PACKAGE_ID,
            ident_str!("coin").into(),
            ident_str!("into_balance").into(),
            vec![token_type_tag],
            vec![coin],
        ))
    }

    pub fn bag_add(
        builder: &mut ProgrammableTransactionBuilder,
        bag: Argument,
        bag_key: String,
        balance: Argument,
        token_type: String,
    ) -> Result<()> {
        let key_type: StructTag = NATIVE_TOKEN_BAG_KEY_TYPE.parse()?;
        let value_type = Balance::type_(token_type.parse::<TypeTag>()?);
        let bag_key_arg = builder.pure(bag_key)?;
        builder.programmable_move_call(
            IOTA_FRAMEWORK_PACKAGE_ID,
            ident_str!("bag").into(),
            ident_str!("add").into(),
            vec![key_type.into(), value_type.into()],
            vec![bag, bag_key_arg, balance],
        );
        Ok(())
    }

    pub fn bag_new(builder: &mut ProgrammableTransactionBuilder) -> Argument {
        builder.programmable_move_call(
            IOTA_FRAMEWORK_PACKAGE_ID,
            ident_str!("bag").into(),
            ident_str!("new").into(),
            vec![],
            vec![],
        )
    }
}

/// On-chain data about the objects created while
/// publishing foundry packages
pub(crate) struct FoundryLedgerData {
    pub(crate) native_token_coin_id: ObjectID,
    pub(crate) coin_type_origin: TypeOrigin,
    pub(crate) package_id: ObjectID,
    pub(crate) token_scheme_u64: SimpleTokenSchemeU64,
    pub(crate) minted_value: u64,
}

impl FoundryLedgerData {
    /// Store the minted coin `ObjectID` and derive data from the foundry
    /// package.
    ///
    /// # Panic
    ///
    /// Panics if the package does not contain any [`TypeOrigin`].
    fn new(
        native_token_coin_id: ObjectID,
        foundry_package: &MovePackage,
        token_scheme_u64: SimpleTokenSchemeU64,
    ) -> Self {
        Self {
            native_token_coin_id,
            // There must be only one type created in the foundry package.
            coin_type_origin: foundry_package.type_origin_table()[0].clone(),
            package_id: foundry_package.id(),
            minted_value: token_scheme_u64.circulating_supply(),
            token_scheme_u64,
        }
    }

    pub(crate) fn to_canonical_string(&self, with_prefix: bool) -> String {
        format!(
            "{}::{}::{}",
            self.coin_type_origin
                .package
                .to_canonical_string(with_prefix),
            self.coin_type_origin.module_name,
            self.coin_type_origin.datatype_name
        )
    }
}
