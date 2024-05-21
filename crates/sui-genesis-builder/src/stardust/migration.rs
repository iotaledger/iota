//! Contains the logic for the migration process.
use move_core_types::{ident_str, language_storage::StructTag};
use std::{
    collections::{BTreeSet, HashMap},
    io::{BufWriter, Write},
    sync::Arc,
};
use sui_types::{
    balance::Balance,
    base_types::{ObjectRef, SequenceNumber},
    collection_types::Bag,
    id::UID,
    move_package::{MovePackage, TypeOrigin},
    object::Object,
    transaction::{Argument, InputObjects, ObjectArg},
    TypeTag,
};

use anyhow::Result;
use fastcrypto::hash::HashFunction;
use iota_sdk::types::block::output::{
    AliasOutput as StardustAlias, BasicOutput, FoundryOutput, NativeTokens,
    NftOutput as StardustNft, Output, TokenId, TreasuryOutput,
};
use move_vm_runtime_v2::move_vm::MoveVM;
use sui_adapter_v2::{
    adapter::new_move_vm, gas_charger::GasCharger, programmable_transactions,
    temporary_store::TemporaryStore,
};
use sui_framework::BuiltInFramework;
use sui_move_build::CompiledPackage;
use sui_move_natives_v2::all_natives;
use sui_protocol_config::{Chain, ProtocolConfig, ProtocolVersion};
use sui_types::{
    base_types::{ObjectID, SuiAddress, TxContext},
    crypto::DefaultHash,
    digests::TransactionDigest,
    epoch_data::EpochData,
    execution_mode,
    in_memory_storage::InMemoryStorage,
    inner_temporary_store::InnerTemporaryStore,
    metrics::LimitsMetrics,
    move_package::UpgradeCap,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::{
        CheckedInputObjects, Command, InputObjectKind, ObjectReadResult, ProgrammableTransaction,
    },
    MOVE_STDLIB_PACKAGE_ID, STARDUST_PACKAGE_ID, SUI_FRAMEWORK_PACKAGE_ID, SUI_SYSTEM_PACKAGE_ID,
};

use super::types::{
    snapshot::OutputHeader, stardust_to_sui_address_owner, Alias, AliasOutput, Nft, NftOutput,
};
use crate::process_package;
use crate::stardust::native_token::package_builder;
use crate::stardust::native_token::package_data::NativeTokenPackageData;

/// The dependencies of the generated packages for native tokens.
pub const PACKAGE_DEPS: [ObjectID; 4] = [
    MOVE_STDLIB_PACKAGE_ID,
    SUI_FRAMEWORK_PACKAGE_ID,
    SUI_SYSTEM_PACKAGE_ID,
    STARDUST_PACKAGE_ID,
];

/// We fix the protocol version used in the migration.
pub const MIGRATION_PROTOCOL_VERSION: u64 = 42;

/// The orchestrator of the migration process.
///
/// It is constructed by an [`Iterator`] of stardust UTXOs, and holds an inner executor
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
}

impl Migration {
    /// Try to setup the migration process by creating the inner executor
    /// and bootstraping the in-memory storage.
    pub fn new() -> Result<Self> {
        let executor = Executor::new(ProtocolVersion::new(MIGRATION_PROTOCOL_VERSION))?;
        Ok(Self { executor })
    }

    /// Create the packages, and associated objects representing foundry outputs.
    fn migrate_foundries(
        &mut self,
        foundries: impl Iterator<Item = (OutputHeader, FoundryOutput)>,
    ) -> Result<()> {
        let mut foundries: Vec<(OutputHeader, FoundryOutput)> = foundries.collect();
        // We sort the outputs to make sure the order of outputs up to
        // a certain milestone timestamp remains the same between runs.
        foundries.sort_by_key(|(header, _)| (header.ms_timestamp(), header.output_id()));
        let compiled = foundries
            .into_iter()
            .map(|(_, output)| {
                let pkg = generate_package(&output)?;
                Ok((output, pkg))
            })
            .collect::<Result<Vec<_>>>()?;
        self.executor.create_foundries(compiled.into_iter())
    }

    /// Create objects for all outputs except for foundry outputs.
    fn migrate_outputs(
        &mut self,
        outputs: impl Iterator<Item = (OutputHeader, Output)>,
    ) -> Result<()> {
        let mut outputs: Vec<(OutputHeader, Output)> = outputs.collect();
        // We sort the outputs to make sure the order of outputs up to
        // a certain milestone timestamp remains the same between runs.
        //
        // This guarantees that fresh ids created through the transaction
        // context will also map to the same objects betwen runs.
        outputs.sort_by_key(|(header, _)| (header.ms_timestamp(), header.output_id()));
        for (header, output) in outputs {
            match output {
                Output::Alias(alias) => self.executor.create_alias_objects(header, alias)?,
                Output::Basic(basic) => self.executor.create_basic_objects(header, basic)?,
                Output::Nft(nft) => self.executor.create_nft_objects(header, nft)?,
                Output::Treasury(treasury) => self.executor.create_treasury_objects(treasury)?,
                Output::Foundry(_) => {
                    continue;
                }
            };
        }
        Ok(())
    }

    /// The migration objects.
    ///
    /// The system packages and underlying `init` objects
    /// are filtered out because they will be generated
    /// in the genesis process.
    fn objects(self) -> Vec<Object> {
        self.executor
            .store
            .into_inner()
            .into_values()
            .filter(|object| {
                !self
                    .executor
                    .system_packages_and_objects
                    .contains(&object.id())
            })
            .collect()
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
        foundries: impl Iterator<Item = (OutputHeader, FoundryOutput)>,
        outputs: impl Iterator<Item = (OutputHeader, Output)>,
        writer: impl Write,
    ) -> Result<()> {
        self.migrate_foundries(foundries)?;
        self.migrate_outputs(outputs)?;
        let stardust_object_ledger = self.objects();
        verify_ledger_state(&stardust_object_ledger)?;
        create_snapshot(stardust_object_ledger, writer)
    }
}

// Build a `CompiledPackage` from a given `FoundryOutput`.
fn generate_package(foundry: &FoundryOutput) -> Result<CompiledPackage> {
    let native_token_data = NativeTokenPackageData::try_from(foundry)?;
    package_builder::build_and_compile(native_token_data)
}

/// On-chain data about the objects created while
/// publishing foundry packages
struct FoundryLedgerData {
    minted_coin_id: ObjectID,
    coin_type_origin: TypeOrigin,
    package_id: ObjectID,
}

impl FoundryLedgerData {
    /// Store the minted coin `ObjectID` and derive data from the foundry package.
    ///
    /// # Panic
    ///
    /// Panics if the package does not contain any [`TypeOrigin`].
    fn new(minted_coin_id: ObjectID, foundry_package: &MovePackage) -> Self {
        Self {
            minted_coin_id,
            // There must be only one type created in the foundry package.
            coin_type_origin: foundry_package.type_origin_table()[0].clone(),
            package_id: foundry_package.id(),
        }
    }
}

/// Creates the objects that map to the stardust UTXO ledger.
///
/// Internally uses an unmetered Move VM.
struct Executor {
    protocol_config: ProtocolConfig,
    tx_context: TxContext,
    /// Stores all the migration objects.
    store: InMemoryStorage,
    /// Caches the system packages and init objects. Useful
    /// for evicting them from the store before
    /// creating the snapshot.
    system_packages_and_objects: BTreeSet<ObjectID>,
    move_vm: Arc<MoveVM>,
    metrics: Arc<LimitsMetrics>,
    /// Map the stardust token id [`TokenId`] to the on-chain info of the
    /// published foundry objects.
    native_tokens: HashMap<TokenId, FoundryLedgerData>,
}

impl Executor {
    /// Setup the execution environment backed by an in-memory store that holds
    /// all the system packages.
    fn new(protocol_version: ProtocolVersion) -> Result<Self> {
        let mut tx_context = create_migration_context();
        // Use a throwaway metrics registry for transaction execution.
        let metrics = Arc::new(LimitsMetrics::new(&prometheus::Registry::new()));
        let mut store = InMemoryStorage::new(Vec::new());
        // We don't know the chain ID here since we haven't yet created the genesis checkpoint.
        // However since we know there are no chain specific protocol config options in genesis,
        // we use Chain::Unknown here.
        let protocol_config = ProtocolConfig::get_for_version(protocol_version, Chain::Unknown);
        // Get the correct system packages for our protocol version. If we cannot find the snapshot
        // that means that we must be at the latest version and we should use the latest version of the
        // framework.
        let mut system_packages =
            sui_framework_snapshot::load_bytecode_snapshot(protocol_version.as_u64())
                .unwrap_or_else(|_| BuiltInFramework::iter_system_packages().cloned().collect());
        // TODO: Remove when we have bumped the protocol to include the stardust packages
        // into the system packages.
        //
        // See also: https://github.com/iotaledger/kinesis/pull/149
        system_packages.extend(BuiltInFramework::iter_stardust_packages().cloned());

        let silent = true;
        let executor = sui_execution::executor(&protocol_config, silent, None)
            .expect("Creating an executor should not fail here");
        for system_package in system_packages.into_iter() {
            process_package(
                &mut store,
                executor.as_ref(),
                &mut tx_context,
                &system_package.modules(),
                system_package.dependencies().to_vec(),
                &protocol_config,
                metrics.clone(),
            )?;
        }
        let move_vm = Arc::new(new_move_vm(all_natives(silent), &protocol_config, None)?);

        let system_packages_and_objects = store.objects().keys().copied().collect();
        Ok(Self {
            protocol_config,
            tx_context,
            store,
            system_packages_and_objects,
            move_vm,
            metrics,
            native_tokens: Default::default(),
        })
    }

    /// Load input objects from the store to be used as checked
    /// input while executing a transaction
    fn load_input_objects(
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
    fn load_packages(
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

    fn execute_pt_unmetered(
        &mut self,
        input_objects: CheckedInputObjects,
        pt: ProgrammableTransaction,
    ) -> Result<InnerTemporaryStore> {
        let input_objects = input_objects.into_inner();
        let mut temporary_store = TemporaryStore::new(
            &self.store,
            input_objects,
            vec![],
            self.tx_context.digest(),
            &self.protocol_config,
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
    fn create_foundries(
        &mut self,
        foundries: impl Iterator<Item = (FoundryOutput, CompiledPackage)>,
    ) -> Result<()> {
        for (foundry, pkg) in foundries {
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
            let mut minted_coin_id = None::<ObjectID>;
            let mut foundry_package = None::<&MovePackage>;
            for object in written.values() {
                if object.is_coin() {
                    minted_coin_id = Some(object.id());
                } else if object.is_package() {
                    foundry_package = Some(
                        object
                            .data
                            .try_as_package()
                            .expect("already verified this is a package"),
                    );
                }
            }
            let (minted_coin_id, foundry_package) = (
                minted_coin_id.expect("a coin must have been minted"),
                foundry_package.expect("there should be a published package"),
            );
            self.native_tokens.insert(
                foundry.token_id(),
                FoundryLedgerData::new(minted_coin_id, foundry_package),
            );
            self.store.finish(
                written
                    .into_iter()
                    // We ignore the [`UpgradeCap`] objects.
                    .filter(|(_, object)| object.struct_tag() != Some(UpgradeCap::type_()))
                    .collect(),
            );
        }
        Ok(())
    }

    fn create_alias_objects(&mut self, header: OutputHeader, alias: StardustAlias) -> Result<()> {
        // Take the Alias ID set in the output or, if its zeroized, compute it from the Output ID.
        let alias_id = ObjectID::new(*alias.alias_id().or_from_output_id(&header.output_id()));
        let move_alias = Alias::try_from_stardust(alias_id, &alias)?;

        // TODO: We should ensure that no circular ownership exists.
        let alias_output_owner = stardust_to_sui_address_owner(alias.governor_address())?;

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

        let (bag, version) = self.create_bag_with_pt(alias.native_tokens())?;
        let move_alias_output =
            AliasOutput::try_from_stardust(self.tx_context.fresh_id(), &alias, bag)?;

        // The bag will be wrapped into the alias output object, so
        // by equating their versions we emulate a ptb.
        let move_alias_output_object = move_alias_output.to_genesis_object(
            alias_output_owner,
            &self.protocol_config,
            &self.tx_context,
            version,
        )?;
        let move_alias_output_object_ref = move_alias_output_object.compute_object_reference();
        self.store.insert_object(move_alias_output_object);

        // Attach the Alias to the Alias Output as a dynamic object field via the attach_alias convenience method.
        let pt = {
            let mut builder = ProgrammableTransactionBuilder::new();

            let alias_output_arg =
                builder.obj(ObjectArg::ImmOrOwnedObject(move_alias_output_object_ref))?;
            let alias_arg = builder.obj(ObjectArg::ImmOrOwnedObject(move_alias_object_ref))?;
            builder.programmable_move_call(
                STARDUST_PACKAGE_ID,
                ident_str!("alias_output").into(),
                ident_str!("attach_alias").into(),
                vec![],
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

        Ok(())
    }

    /// Create a [`Bag`] of balances of native tokens executing a programmable transaction block.
    fn create_bag_with_pt(
        &mut self,
        native_tokens: &NativeTokens,
    ) -> Result<(Bag, SequenceNumber)> {
        let mut object_deps = Vec::with_capacity(native_tokens.len());
        let mut foundry_package_deps = Vec::with_capacity(native_tokens.len());
        let pt = {
            let mut builder = ProgrammableTransactionBuilder::new();
            let bag = pt::bag_new(&mut builder);
            for token in native_tokens.iter() {
                if token.amount().bits() > 64 {
                    anyhow::bail!("unsupported number of tokens");
                }

                let Some(foundry_ledger_data) = self.native_tokens.get(token.token_id()) else {
                    anyhow::bail!("foundry for native token has not been published");
                };

                let Some(foundry_coin) = self.store.get_object(&foundry_ledger_data.minted_coin_id)
                else {
                    anyhow::bail!("foundry coin should exist");
                };
                let object_ref = foundry_coin.compute_object_reference();

                object_deps.push(object_ref);
                foundry_package_deps.push(foundry_ledger_data.package_id);

                let token_type = format!(
                    "{}::{}::{}",
                    foundry_ledger_data.coin_type_origin.package,
                    foundry_ledger_data.coin_type_origin.module_name,
                    foundry_ledger_data.coin_type_origin.struct_name
                );
                let balance = pt::coin_balance_split(
                    &mut builder,
                    object_ref,
                    token_type.parse()?,
                    token.amount().as_u64(),
                )?;
                pt::bag_add(&mut builder, bag, balance, token_type)?;
            }

            // The `Bag` object does not have the `drop` ability so we have to use it
            // in the transaction block. Therefore we transfer it to the `0x0` address.
            //
            // Nevertheless, we only store the contents of the object, and thus the ownership
            // metadata are irrelevant to us. This is a dummy transfer then to satisfy
            // the VM.
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
            .find_map(|(id, object)| (!input_objects.contains_key(id)).then_some(object.clone()))
            .expect("the bag should have been created");
        written.remove(&bag_object.id());
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
        Ok((bag, bag_object.version()))
    }

    /// Create [`Coin`] objects representing native tokens in the ledger.
    fn create_native_token_coins(
        &mut self,
        native_tokens: &NativeTokens,
        owner: SuiAddress,
    ) -> Result<()> {
        let mut object_deps = Vec::with_capacity(native_tokens.len());
        let mut foundry_package_deps = Vec::with_capacity(native_tokens.len());
        let pt = {
            let mut builder = ProgrammableTransactionBuilder::new();
            for token in native_tokens.iter() {
                if token.amount().bits() > 64 {
                    anyhow::bail!("unsupported number of tokens");
                }

                let Some(foundry_ledger_data) = self.native_tokens.get(token.token_id()) else {
                    anyhow::bail!("foundry for native token has not been published");
                };

                let Some(foundry_coin) = self.store.get_object(&foundry_ledger_data.minted_coin_id)
                else {
                    anyhow::bail!("foundry coin should exist");
                };
                let object_ref = foundry_coin.compute_object_reference();

                object_deps.push(object_ref);
                foundry_package_deps.push(foundry_ledger_data.package_id);

                // Pay using that object
                builder.pay(vec![object_ref], vec![owner], vec![token.amount().as_u64()])?;
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

        // Save the modified coin
        self.store.finish(written);
        Ok(())
    }

    /// This implements the control flow in
    /// crates/sui-framework/packages/stardust/basic_migration_graph.svg
    fn create_basic_objects(
        &mut self,
        header: OutputHeader,
        basic_output: BasicOutput,
    ) -> Result<()> {
        let mut data = super::types::output::BasicOutput::new(header.clone(), &basic_output);
        let owner: SuiAddress = basic_output.address().to_string().parse()?;

        // The minimum version of the manually created objects
        let package_deps = InputObjects::new(self.load_packages(PACKAGE_DEPS).collect());
        let mut version = package_deps.lamport_timestamp(&[]);
        let object = if data.has_empty_bag() {
            if !basic_output.native_tokens().is_empty() {
                self.create_native_token_coins(basic_output.native_tokens(), owner)?;
            }
            // Overwrite the default 0 UID of `Bag::default()`, since we won't be creating a new bag in this code path.
            data.native_tokens.id = UID::new(self.tx_context.fresh_id());
            data.into_genesis_coin_object(owner, &self.protocol_config, &self.tx_context, version)?
        } else {
            if !basic_output.native_tokens().is_empty() {
                // The bag will be wrapped into the basic output object, so
                // by equating their versions we emulate a ptb.
                (data.native_tokens, version) =
                    self.create_bag_with_pt(basic_output.native_tokens())?;
            }
            data.to_genesis_object(owner, &self.protocol_config, &self.tx_context, version)?
        };
        self.store.insert_object(object);
        Ok(())
    }

    fn create_nft_objects(&mut self, header: OutputHeader, nft: StardustNft) -> Result<()> {
        // Take the Nft ID set in the output or, if its zeroized, compute it from the Output ID.
        let nft_id = ObjectID::new(*nft.nft_id().or_from_output_id(&header.output_id()));
        let move_nft = Nft::try_from_stardust(nft_id, &nft)?;

        // TODO: We should ensure that no circular ownership exists.
        let nft_output_owner = stardust_to_sui_address_owner(nft.address())?;

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

        let (bag, version) = self.create_bag_with_pt(nft.native_tokens())?;
        let move_nft_output = NftOutput::try_from_stardust(self.tx_context.fresh_id(), &nft, bag)?;

        // The bag will be wrapped into the nft output object, so
        // by equating their versions we emulate a ptb.
        let move_nft_output_object = move_nft_output.to_genesis_object(
            nft_output_owner,
            &self.protocol_config,
            &self.tx_context,
            version,
        )?;
        let move_nft_output_object_ref = move_nft_output_object.compute_object_reference();
        self.store.insert_object(move_nft_output_object);

        // Attach the Nft to the Nft Output as a dynamic object field via the attach_nft convenience method.
        let pt = {
            let mut builder = ProgrammableTransactionBuilder::new();

            let nft_output_arg =
                builder.obj(ObjectArg::ImmOrOwnedObject(move_nft_output_object_ref))?;
            let nft_arg = builder.obj(ObjectArg::ImmOrOwnedObject(move_nft_object_ref))?;
            builder.programmable_move_call(
                STARDUST_PACKAGE_ID,
                ident_str!("nft_output").into(),
                ident_str!("attach_nft").into(),
                vec![],
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

        Ok(())
    }

    fn create_treasury_objects(&mut self, _treasury: TreasuryOutput) -> Result<()> {
        todo!();
    }
}

/// Verify the ledger state represented by the objects in [`InMemoryStorage`].
fn verify_ledger_state(_store: &[Object]) -> Result<()> {
    // TODO: Implementation. Returns Ok for now so the migration can be tested.
    Ok(())
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
fn package_module_bytes(pkg: &CompiledPackage) -> Result<Vec<Vec<u8>>> {
    pkg.get_modules()
        .map(|module| {
            let mut buf = Vec::new();
            module.serialize(&mut buf)?;
            Ok(buf)
        })
        .collect::<Result<_>>()
}

/// Create a [`TxContext]` that remains the same across invocations.
fn create_migration_context() -> TxContext {
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

mod pt {
    use super::*;

    pub fn coin_balance_split(
        builder: &mut ProgrammableTransactionBuilder,
        foundry_coin_ref: ObjectRef,
        token_type_tag: TypeTag,
        amount: u64,
    ) -> Result<Argument> {
        let foundry_coin_ref = builder.obj(ObjectArg::ImmOrOwnedObject(foundry_coin_ref))?;
        let balance = builder.programmable_move_call(
            SUI_FRAMEWORK_PACKAGE_ID,
            ident_str!("coin").into(),
            ident_str!("balance_mut").into(),
            vec![token_type_tag.clone()],
            vec![foundry_coin_ref],
        );
        let amount = builder.pure(amount)?;
        Ok(builder.programmable_move_call(
            SUI_FRAMEWORK_PACKAGE_ID,
            ident_str!("balance").into(),
            ident_str!("split").into(),
            vec![token_type_tag],
            vec![balance, amount],
        ))
    }

    pub fn bag_add(
        builder: &mut ProgrammableTransactionBuilder,
        bag: Argument,
        balance: Argument,
        token_type: String,
    ) -> Result<()> {
        let key_type: StructTag = "0x01::ascii::String".parse()?;
        let value_type = Balance::type_(token_type.parse::<TypeTag>()?);
        let token_name = builder.pure(token_type)?;
        builder.programmable_move_call(
            SUI_FRAMEWORK_PACKAGE_ID,
            ident_str!("bag").into(),
            ident_str!("add").into(),
            vec![key_type.into(), value_type.into()],
            vec![bag, token_name, balance],
        );
        Ok(())
    }

    pub fn bag_new(builder: &mut ProgrammableTransactionBuilder) -> Argument {
        builder.programmable_move_call(
            SUI_FRAMEWORK_PACKAGE_ID,
            ident_str!("bag").into(),
            ident_str!("new").into(),
            vec![],
            vec![],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn migration_create_and_deserialize_snapshot() {
        let mut persisted: Vec<u8> = Vec::new();
        let objects = (0..4)
            .map(|_| Object::new_gas_for_testing())
            .collect::<Vec<_>>();
        create_snapshot(objects.clone(), &mut persisted).unwrap();
        let snapshot_objects: Vec<Object> = bcs::from_bytes(&persisted).unwrap();
        assert_eq!(objects, snapshot_objects);
    }
}
