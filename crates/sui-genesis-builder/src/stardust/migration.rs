//! Contains the logic for the migration process.
use move_core_types::{ident_str, language_storage::StructTag};
use std::{
    collections::HashMap,
    io::{BufWriter, Write},
    sync::Arc,
};
use sui_types::{
    balance::Balance,
    base_types::ObjectRef,
    coin::Coin,
    collection_types::Bag,
    id::UID,
    move_package::TypeOrigin,
    object::{Data, MoveObject, Owner},
    transaction::{Argument, ObjectArg},
    TypeTag,
};

use anyhow::Result;
use fastcrypto::hash::{Blake2b256, HashFunction};
use iota_sdk::types::block::output::{
    AliasOutput as StardustAlias, BasicOutput, FoundryOutput, NativeTokens, NftOutput, Output,
    TokenId, TreasuryOutput,
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
    object::Object,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::{
        CheckedInputObjects, Command, InputObjectKind, ObjectReadResult, ProgrammableTransaction,
    },
    MOVE_STDLIB_PACKAGE_ID, STARDUST_PACKAGE_ID, SUI_FRAMEWORK_PACKAGE_ID, SUI_SYSTEM_PACKAGE_ID,
};

use super::types::{snapshot::OutputHeader, stardust_to_sui_address_owner, Alias, AliasOutput};
use crate::process_package;

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
        for (header, output) in outputs {
            match output {
                Output::Alias(alias) => self.executor.create_alias_objects(header, alias)?,
                Output::Basic(basic) => self.executor.create_basic_objects(header, basic)?,
                Output::Nft(nft) => self.executor.create_nft_objects(nft)?,
                Output::Treasury(treasury) => self.executor.create_treasury_objects(treasury)?,
                Output::Foundry(_) => {
                    continue;
                }
            };
        }
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
        foundries: impl Iterator<Item = (OutputHeader, FoundryOutput)>,
        outputs: impl Iterator<Item = (OutputHeader, Output)>,
        writer: impl Write,
    ) -> Result<()> {
        self.migrate_foundries(foundries)?;
        self.migrate_outputs(outputs)?;
        let stardust_object_ledger = self.executor.store;
        verify_ledger_state(&stardust_object_ledger)?;
        create_snapshot(stardust_object_ledger, writer)
    }
}

// stub of package generation and build logic
fn generate_package(_foundry: &FoundryOutput) -> Result<CompiledPackage> {
    todo!()
}

/// Creates the objects that map to the stardust UTXO ledger.
///
/// Internally uses an unmetered Move VM.
struct Executor {
    protocol_config: ProtocolConfig,
    tx_context: TxContext,
    store: InMemoryStorage,
    move_vm: Arc<MoveVM>,
    metrics: Arc<LimitsMetrics>,
    /// Map the stardust token id [`TokenId`] to the [`ObjectID`] and of the
    /// coin minted by the foundry and its [`TypeOrigin`].
    native_tokens: HashMap<TokenId, (ObjectRef, TypeOrigin)>,
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

        Ok(Self {
            protocol_config,
            tx_context,
            store,
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
                builder.command(Command::Publish(modules, PACKAGE_DEPS.into()));
                builder.finish()
            };
            let InnerTemporaryStore { written, .. } = self.execute_pt_unmetered(deps, pt)?;
            // Get on-chain info
            let mut minted_coin_ref = None::<ObjectRef>;
            let mut coin_type_origin = None::<TypeOrigin>;
            for object in written.values() {
                if object.is_coin() {
                    minted_coin_ref = Some(object.compute_object_reference());
                } else if object.is_package() {
                    coin_type_origin = Some(
                        object
                            .data
                            .try_as_package()
                            .expect("already verified this is a package")
                            // there must be only one type created in the package
                            .type_origin_table()[0]
                            .clone(),
                    );
                }
            }
            let (minted_coin_ref, coin_type_origin) = (
                minted_coin_ref.expect("a coin must have been minted"),
                coin_type_origin.expect("the published package should include a type for the coin"),
            );
            self.native_tokens.insert(
                *foundry.native_tokens()[0].token_id(),
                (minted_coin_ref, coin_type_origin),
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

        // Construct the Alias object.
        let move_alias_object = unsafe {
            // Safety: we know from the definition of `Alias` in the stardust package
            // that it has public transfer (`store` ability is present).
            MoveObject::new_from_execution(
                super::types::Alias::tag().into(),
                true,
                0.into(),
                bcs::to_bytes(&move_alias)?,
                &self.protocol_config,
            )?
        };

        // SAFETY: We should ensure that no circular ownership exists first.
        let alias_output_owner = stardust_to_sui_address_owner(alias.governor_address())?;

        let move_alias_object = Object::new_from_genesis(
            Data::Move(move_alias_object),
            // We will later overwrite the owner we set here since this object will be added
            // as a dynamic field on the alias output object.
            alias_output_owner,
            self.tx_context.digest(),
        );
        let move_alias_object_ref = move_alias_object.compute_object_reference();
        self.store.insert_object(move_alias_object);

        let move_alias_output = AliasOutput::try_from_stardust(
            alias_id,
            &alias,
            self.create_bag(alias.native_tokens())?,
        )?;

        // Construct the Alias Output object.
        let move_alias_output_object = unsafe {
            // Safety: we know from the definition of `AliasOutput` in the stardust package
            // that it does not have public transfer (`store` ability is absent).
            MoveObject::new_from_execution(
                AliasOutput::tag().into(),
                false,
                0.into(),
                bcs::to_bytes(&move_alias_output)?,
                &self.protocol_config,
            )?
        };

        let move_alias_output_object = Object::new_from_genesis(
            Data::Move(move_alias_output_object),
            alias_output_owner,
            self.tx_context.digest(),
        );
        let move_alias_output_object_ref = move_alias_output_object.compute_object_reference();
        self.store.insert_object(move_alias_output_object);

        // Attach the Alias to the Alias Output as a dynamic field via the attach_alias convenience method.
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

    /// Create a [`Bag`] of balances of native tokens.
    fn create_bag(&mut self, native_tokens: &NativeTokens) -> Result<Bag> {
        let mut dependencies = Vec::with_capacity(native_tokens.len());
        let pt = {
            let mut builder = ProgrammableTransactionBuilder::new();
            let bag = pt::bag_new(&mut builder);
            for token in native_tokens.iter() {
                if token.amount().bits() > 64 {
                    anyhow::bail!("unsupported number of tokens");
                }

                let Some((object_ref, type_origin)) = self.native_tokens.get(token.token_id())
                else {
                    anyhow::bail!("foundry for native token has not been published");
                };

                dependencies.push(*object_ref);

                let token_type = format!(
                    "{}::{}::{}",
                    type_origin.package, type_origin.module_name, type_origin.struct_name
                );
                let balance = pt::coin_balance_split(
                    &mut builder,
                    *object_ref,
                    &token_type,
                    token.amount().as_u64(),
                )?;
                pt::bag_add(&mut builder, bag, balance, &token_type)?;
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
                .chain(self.load_input_objects(dependencies))
                .collect(),
        );
        let InnerTemporaryStore { mut written, .. } =
            self.execute_pt_unmetered(checked_input_objects, pt)?;
        let (_, bag_object) = written
            .pop_first()
            .expect("the bag should have been created");
        Ok(bcs::from_bytes(
            bag_object
                .data
                .try_as_move()
                .expect("this should be a move object")
                .contents(),
        )
        .expect("this should be a valid Bag Move object"))
    }

    /// Create [`Coin`] objects representing native tokens in the ledger.
    ///
    /// We set the [`ObjectID`] to the `hash(hash(OutputId) || TokenId)`
    /// so that we avoid generation based on the [`TxContext`]. The latter
    /// depends on the order of generation, and implies that the outputs
    /// should be sorted to attain idempotence.
    fn create_native_token_coins(
        &mut self,
        header: OutputHeader,
        native_tokens: &NativeTokens,
        owner: SuiAddress,
    ) -> Result<()> {
        let mut dependencies = Vec::with_capacity(native_tokens.len());
        let mut coins = Vec::with_capacity(native_tokens.len());
        let native_tokens_with_ids = native_tokens.iter().map(|token| {
            let mut coin_id = header.output_id().hash().to_vec();
            coin_id.extend(token.token_id().as_slice());
            // we set the `ObjectID` of native token objects to
            // `hash(hash(OutputId) || TokenId)`
            (ObjectID::new(Blake2b256::digest(coin_id).into()), token)
        });
        let pt = {
            let mut builder = ProgrammableTransactionBuilder::new();
            for (coin_id, token) in native_tokens_with_ids {
                if token.amount().bits() > 64 {
                    anyhow::bail!("unsupported number of tokens");
                }

                let Some((object_ref, type_origin)) = self.native_tokens.get(token.token_id())
                else {
                    anyhow::bail!("foundry for native token has not been published");
                };

                dependencies.push(*object_ref);

                let token_type = format!(
                    "{}::{}::{}",
                    type_origin.package, type_origin.module_name, type_origin.struct_name
                );
                pt::coin_balance_split(
                    &mut builder,
                    *object_ref,
                    &token_type,
                    token.amount().as_u64(),
                )?;
                let token_type_tag: TypeTag = token_type.parse()?;
                coins.push(self.coin_from_balance_with_id(
                    coin_id,
                    token_type_tag,
                    token.amount().as_u64(),
                    owner,
                )?);
            }

            builder.finish()
        };
        let checked_input_objects = CheckedInputObjects::new_for_genesis(
            self.load_packages(PACKAGE_DEPS)
                .chain(self.load_input_objects(dependencies))
                .collect(),
        );
        self.execute_pt_unmetered(checked_input_objects, pt)?;
        // now that the transaction has been executed successfully we store the
        // create coins
        coins
            .into_iter()
            .for_each(|coin| self.store.insert_object(coin));
        Ok(())
    }

    fn coin_from_balance_with_id(
        &self,
        coin_id: ObjectID,
        type_tag: TypeTag,
        amount: u64,
        owner: SuiAddress,
    ) -> Result<Object> {
        let coin = Coin::new(UID::new(coin_id), amount);
        let data = unsafe {
            // Safety: we know from the definition of `Coin`
            // that it has public transfer (`store` ability is present).
            MoveObject::new_from_execution(
                Coin::type_(type_tag).into(),
                true,
                0.into(),
                bcs::to_bytes(&coin)?,
                &self.protocol_config,
            )?
        };
        let owner = Owner::AddressOwner(owner);
        Ok(Object::new_from_genesis(
            Data::Move(data),
            owner,
            self.tx_context.digest(),
        ))
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

        // Handle native tokens
        if !basic_output.native_tokens().is_empty() {
            if data.has_empty_bag() {
                self.create_native_token_coins(header, basic_output.native_tokens(), owner)?;
            } else {
                data.native_tokens = self.create_bag(basic_output.native_tokens())?;
            }
        }
        // Construct the basic output object
        let move_object = unsafe {
            // Safety: we know from the definition of `BasicOutput` in the stardust package
            // that it has not public transfer (`store` ability is absent).
            MoveObject::new_from_execution(
                super::types::output::BasicOutput::type_().into(),
                false,
                0.into(),
                bcs::to_bytes(&data)?,
                &self.protocol_config,
            )?
        };
        // Resolve ownership
        let owner = if data.expiration.is_some() {
            Owner::Shared {
                initial_shared_version: 0.into(),
            }
        } else {
            Owner::AddressOwner(owner)
        };
        self.store.insert_object(Object::new_from_genesis(
            Data::Move(move_object),
            owner,
            self.tx_context.digest(),
        ));
        Ok(())
    }

    fn create_nft_objects(&mut self, _nft: NftOutput) -> Result<()> {
        todo!();
    }

    fn create_treasury_objects(&mut self, _treasury: TreasuryOutput) -> Result<()> {
        todo!();
    }
}

/// Verify the ledger state represented by the objects in [`InMemoryStorage`].
fn verify_ledger_state(_store: &InMemoryStorage) -> Result<()> {
    todo!();
}

/// Serialize the objects stored in [`InMemoryStorage`] into a file using
/// [`bcs`] encoding.
fn create_snapshot(store: InMemoryStorage, writer: impl Write) -> Result<()> {
    let mut writer = BufWriter::new(writer);
    let objects = store
        .into_inner()
        .into_values()
        .filter(|object| !object.is_system_package())
        .collect::<Vec<_>>();
    writer.write_all(&bcs::to_bytes(&objects)?)?;
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
    use std::str::FromStr;

    use super::*;

    pub fn coin_balance_split(
        builder: &mut ProgrammableTransactionBuilder,
        foundry_coin_ref: ObjectRef,
        token_type: &str,
        amount: u64,
    ) -> Result<Argument> {
        let token_type_tag: TypeTag = token_type.parse()?;
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
            vec![token_type_tag.clone()],
            vec![balance, amount],
        ))
    }

    pub fn bag_add(
        builder: &mut ProgrammableTransactionBuilder,
        bag: Argument,
        balance: Argument,
        token_type: &str,
    ) -> Result<()> {
        let token_struct_tag = builder.pure(token_type.parse::<StructTag>()?)?;
        let key_type: StructTag = "0x01::ascii::String".parse()?;
        let value_type = Balance::type_(token_type.parse::<TypeTag>()?);
        builder.programmable_move_call(
            SUI_FRAMEWORK_PACKAGE_ID,
            ident_str!("bag").into(),
            ident_str!("add").into(),
            vec![key_type.into(), value_type.into()],
            vec![bag, token_struct_tag, balance],
        );
        builder.transfer_arg(
            SuiAddress::from_str(
                "0x8fac4aefdc1ae21c00f745605297041e0f39667844068e3757d587c8039d1e3f",
            )
            .unwrap(),
            bag,
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
    use std::str::FromStr;

    use iota_sdk::types::block::{
        address::Ed25519Address,
        output::{
            feature::{IssuerFeature, MetadataFeature, SenderFeature},
            unlock_condition::{
                GovernorAddressUnlockCondition, StateControllerAddressUnlockCondition,
            },
            AliasId, AliasOutputBuilder, Feature,
        },
    };
    use sui_types::{inner_temporary_store::WrittenObjects, object::Object};

    use super::*;
    #[test]
    fn migration_create_and_deserialize_snapshot() {
        let mut persisted: Vec<u8> = Vec::new();
        let mut store = InMemoryStorage::default();
        let objects: WrittenObjects = (0..4)
            .map(|_| {
                let object = Object::new_gas_for_testing();
                (object.id(), object)
            })
            .collect();
        store.finish(objects.clone());
        create_snapshot(store, &mut persisted).unwrap();
        let snapshot_objects: Vec<Object> = bcs::from_bytes(&persisted).unwrap();
        assert_eq!(objects.into_values().collect::<Vec<_>>(), snapshot_objects);
    }

    #[test]
    fn alias_migration_test() {
        let random_bytes: [u8; 32] = rand::random();
        let alias_id =
            AliasId::from_str("0x8fac4aefdc1ae21c00f745605297041e0f39667844068e3757d587c8039d1e3f")
                .unwrap();

        // Some random address.
        let address = Ed25519Address::from_str(
            "0xbeefbeefbeefbeefbeefbeefbeefbeefbeefbeefbeefbeefbeefbeefbeefbeef",
        )
        .unwrap();
        let mut exec = Executor::new(MIGRATION_PROTOCOL_VERSION.into()).unwrap();

        let header =
            OutputHeader::new_testing(random_bytes, random_bytes, rand::random(), rand::random());

        let stardust_alias = AliasOutputBuilder::new_with_amount(1_000_000, alias_id)
            .add_unlock_condition(StateControllerAddressUnlockCondition::new(address))
            .add_unlock_condition(GovernorAddressUnlockCondition::new(address))
            .with_state_metadata([0xff; 1])
            .with_features(vec![
                Feature::Metadata(MetadataFeature::new([0xdd; 1]).unwrap()),
                Feature::Sender(SenderFeature::new(address)),
            ])
            .with_immutable_features(vec![
                Feature::Metadata(MetadataFeature::new([0xaa; 1]).unwrap()),
                Feature::Issuer(IssuerFeature::new(address)),
            ])
            .with_state_index(3)
            .finish()
            .unwrap();

        exec.create_alias_objects(header, stardust_alias.clone())
            .unwrap();

        // Ensure the migrated objects exist under the expected identifiers.
        // We know the alias_id is non-zero here, so we don't need to use `or_from_output_id`.
        let alias_object_id = ObjectID::new(*alias_id);
        let alias_output_object_id = ObjectID::new(Blake2b256::digest(*alias_id).into());
        let alias_object = exec.store.get_object(&alias_object_id).unwrap();
        assert_eq!(alias_object.struct_tag().unwrap(), Alias::tag(),);
        let alias_output_object = exec.store.get_object(&alias_output_object_id).unwrap();
        assert_eq!(
            alias_output_object.struct_tag().unwrap(),
            AliasOutput::tag(),
        );

        let alias_output: AliasOutput =
            bcs::from_bytes(alias_output_object.data.try_as_move().unwrap().contents()).unwrap();
        let alias: Alias =
            bcs::from_bytes(alias_object.data.try_as_move().unwrap().contents()).unwrap();

        let expected_alias_output = AliasOutput::try_from_stardust(
            alias_object_id,
            &stardust_alias,
            exec.create_bag(stardust_alias.native_tokens()).unwrap(),
        )
        .unwrap();
        let expected_alias = Alias::try_from_stardust(alias_object_id, &stardust_alias).unwrap();

        // Compare only ID and iota and not the bag as it will be different.
        assert_eq!(expected_alias_output.id, alias_output.id);
        assert_eq!(expected_alias_output.iota, alias_output.iota);

        assert_eq!(expected_alias, alias);
    }
}
