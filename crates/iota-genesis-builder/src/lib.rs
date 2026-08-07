// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    cell::RefCell,
    collections::BTreeMap,
    fs::{self, File},
    io::{BufReader, BufWriter},
    path::Path,
    rc::Rc,
    sync::Arc,
};

use anyhow::{Context, bail};
use camino::Utf8Path;
use fastcrypto::{hash::HashFunction, traits::KeyPair};
use iota_config::genesis::{
    Genesis, GenesisCeremonyParameters, GenesisChainParameters, TokenDistributionSchedule,
    UnsignedGenesis,
};
use iota_execution::{self, Executor};
use iota_framework::{BuiltInFramework, SystemPackage};
use iota_genesis_common::{execute_genesis_transaction, get_genesis_protocol_config};
use iota_protocol_config::{Chain, ProtocolConfig, ProtocolVersion};
use iota_sdk_types::{
    Address, Command, Event, GenesisObject, Identifier, ObjectId, Owner, TransactionDigest,
    TransactionEffects, TransactionEvents, Version,
    checkpoint::{CheckpointContents, CheckpointSummary},
    crypto::{Intent, IntentMessage, IntentScope},
};
use iota_types::{
    base_types::{ExecutionDigests, TxContext},
    committee::Committee,
    crypto::{
        AuthorityKeyPair, AuthorityPublicKeyBytes, AuthoritySignInfo, AuthoritySignInfoTrait,
        AuthoritySignature, DefaultHash, IotaAuthoritySignature,
    },
    deny_list_v1::DENY_LIST_CREATE_FUNC,
    digests::ChainIdentifier,
    epoch_data::EpochData,
    gas_coin::GasCoin,
    governance::StakedIota,
    in_memory_storage::InMemoryStorage,
    inner_temporary_store::InnerTemporaryStore,
    iota_system_state::{IotaSystemState, IotaSystemStateTrait, get_iota_system_state},
    messages_checkpoint::{
        CertifiedCheckpointSummary, CheckpointContentsExt, CheckpointVersionSpecificData,
        CheckpointVersionSpecificDataV1,
    },
    metrics::LimitsMetrics,
    object::{MoveStructExt, Object},
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    randomness_state::RANDOMNESS_STATE_CREATE_FUNCTION_NAME,
    transaction::{
        CallArg, CheckedInputObjects, InputObjectKind, ObjectReadResult, TransactionEnvelope,
    },
};
use move_binary_format::CompiledModule;
use tracing::trace;
use validator_info::{GenesisValidatorInfo, GenesisValidatorMetadata, ValidatorInfo};

pub mod validator_info;

const GENESIS_BUILDER_COMMITTEE_DIR: &str = "committee";
pub const GENESIS_BUILDER_PARAMETERS_FILE: &str = "parameters";
const GENESIS_BUILDER_TOKEN_DISTRIBUTION_SCHEDULE_FILE: &str = "token-distribution-schedule";
const GENESIS_BUILDER_SIGNATURE_DIR: &str = "signatures";
const GENESIS_BUILDER_UNSIGNED_GENESIS_FILE: &str = "unsigned-genesis";
const GENESIS_BUILDER_MIGRATION_LOGIC_REMOVAL_PROTOCOL_VERSION: u64 = 32;

pub struct Builder {
    parameters: GenesisCeremonyParameters,
    token_distribution_schedule: Option<TokenDistributionSchedule>,
    objects: BTreeMap<ObjectId, Object>,
    validators: BTreeMap<AuthorityPublicKeyBytes, GenesisValidatorInfo>,
    // Validator signatures over checkpoint
    signatures: BTreeMap<AuthorityPublicKeyBytes, AuthoritySignInfo>,
    built_genesis: Option<UnsignedGenesis>,
}

impl Default for Builder {
    fn default() -> Self {
        Self::new()
    }
}

impl Builder {
    pub fn new() -> Self {
        Self {
            parameters: Default::default(),
            token_distribution_schedule: None,
            objects: Default::default(),
            validators: Default::default(),
            signatures: Default::default(),
            built_genesis: None,
        }
    }

    pub fn with_parameters(mut self, parameters: GenesisCeremonyParameters) -> Self {
        self.parameters = parameters;
        self
    }

    /// Set the [`TokenDistributionSchedule`].
    ///
    /// # Panics
    ///
    /// Panics if the schedule is invalid, e.g. it contains timelocked stake,
    /// which is not supported at genesis.
    pub fn with_token_distribution_schedule(
        mut self,
        token_distribution_schedule: TokenDistributionSchedule,
    ) -> Self {
        token_distribution_schedule.validate();
        self.token_distribution_schedule = Some(token_distribution_schedule);
        self
    }

    pub fn with_protocol_version(mut self, v: ProtocolVersion) -> Self {
        self.parameters.protocol_version = v;
        self
    }

    pub fn add_object(mut self, object: Object) -> Self {
        self.objects.insert(object.id(), object);
        self
    }

    pub fn add_objects(mut self, objects: Vec<Object>) -> Self {
        for object in objects {
            self.objects.insert(object.id(), object);
        }
        self
    }

    pub fn add_validator(
        mut self,
        validator: ValidatorInfo,
        proof_of_possession: AuthoritySignature,
    ) -> Self {
        self.validators.insert(
            validator.authority_key(),
            GenesisValidatorInfo {
                info: validator,
                proof_of_possession,
            },
        );
        self
    }

    pub fn validators(&self) -> &BTreeMap<AuthorityPublicKeyBytes, GenesisValidatorInfo> {
        &self.validators
    }

    pub fn add_validator_signature(mut self, keypair: &AuthorityKeyPair) -> Self {
        let name = keypair.public().into();
        assert!(
            self.validators.contains_key(&name),
            "provided keypair does not correspond to a validator in the validator set"
        );

        let UnsignedGenesis { checkpoint, .. } = self.get_or_build_unsigned_genesis();

        let checkpoint_signature = {
            let intent_msg = IntentMessage::new(
                Intent::iota_app(IntentScope::CheckpointSummary),
                checkpoint.clone(),
            );
            let signature = AuthoritySignature::new_secure(&intent_msg, &checkpoint.epoch, keypair);
            AuthoritySignInfo {
                epoch: checkpoint.epoch,
                authority: name,
                signature,
            }
        };

        self.signatures.insert(name, checkpoint_signature);

        self
    }

    pub fn unsigned_genesis_checkpoint(&self) -> Option<UnsignedGenesis> {
        self.built_genesis.clone()
    }

    /// Evaluate the genesis [`TokenDistributionSchedule`]: use the schedule
    /// given as input, if any, or instantiate a default schedule for the
    /// validators otherwise.
    fn resolve_token_distribution_schedule(&mut self) -> TokenDistributionSchedule {
        self.token_distribution_schedule.take().unwrap_or_else(|| {
            TokenDistributionSchedule::new_for_validators_with_default_allocation(
                self.validators.values().map(|v| v.info.iota_address()),
                self.parameters.protocol_version,
            )
        })
    }

    fn build_and_cache_unsigned_genesis(&mut self) {
        // Verify that all input data is valid.
        // Check that if extra objects are present then it is allowed by the parameters
        // to add extra objects and it also validates the validator info
        self.validate_inputs().unwrap();

        let token_distribution_schedule = self.resolve_token_distribution_schedule();

        // Verify that token distribution schedule is valid
        token_distribution_schedule.validate();
        token_distribution_schedule
            .check_minimum_stake_for_validators(
                self.validators.values().map(|v| v.info.iota_address()),
                self.parameters.protocol_version,
            )
            .expect("all validators should have the required stake");

        let unsigned_genesis = build_unsigned_genesis_data(
            &self.parameters,
            &token_distribution_schedule,
            self.validators.values(),
            self.objects.clone().into_values().collect::<Vec<_>>(),
        );

        self.built_genesis = Some(unsigned_genesis);
        self.token_distribution_schedule = Some(token_distribution_schedule);
    }

    pub fn get_or_build_unsigned_genesis(&mut self) -> &UnsignedGenesis {
        if self.built_genesis.is_none() {
            self.build_and_cache_unsigned_genesis();
        }
        self.built_genesis
            .as_ref()
            .expect("genesis should have been built and cached")
    }

    fn committee(objects: &[Object]) -> Committee {
        let iota_system_object =
            get_iota_system_state(&objects).expect("IOTA System State object must always exist");
        iota_system_object
            .get_current_epoch_committee()
            .committee()
            .clone()
    }

    pub fn protocol_version(&self) -> ProtocolVersion {
        self.parameters.protocol_version
    }

    pub fn build(mut self) -> Genesis {
        if self.built_genesis.is_none() {
            self.build_and_cache_unsigned_genesis();
        }

        // Verify that all on-chain state was properly created
        self.validate().unwrap();

        let UnsignedGenesis {
            checkpoint,
            checkpoint_contents,
            transaction,
            effects,
            events,
            objects,
        } = self
            .built_genesis
            .take()
            .expect("genesis should have been built");

        let committee = Self::committee(&objects);

        let checkpoint = {
            let signatures = self.signatures.clone().into_values().collect();

            CertifiedCheckpointSummary::new(checkpoint, signatures, &committee).unwrap()
        };

        Genesis::new(
            checkpoint,
            checkpoint_contents,
            transaction,
            effects,
            events,
            objects,
        )
    }

    /// Validates the entire state of the build, no matter what the internal
    /// state is (input collection phase or output phase)
    pub fn validate(&self) -> anyhow::Result<(), anyhow::Error> {
        self.validate_inputs()?;
        self.validate_token_distribution_schedule()?;
        self.validate_output();
        Ok(())
    }

    /// Runs through validation checks on the input values present in the
    /// builder
    fn validate_inputs(&self) -> anyhow::Result<(), anyhow::Error> {
        if !self.parameters.allow_insertion_of_extra_objects && !self.objects.is_empty() {
            bail!("extra objects are disallowed");
        }

        for validator in self.validators.values() {
            validator.validate().with_context(|| {
                format!(
                    "metadata for validator {} is invalid",
                    validator.info.name()
                )
            })?;
        }

        Ok(())
    }

    /// Runs through validation checks on the input token distribution schedule
    fn validate_token_distribution_schedule(&self) -> anyhow::Result<(), anyhow::Error> {
        if let Some(token_distribution_schedule) = &self.token_distribution_schedule {
            token_distribution_schedule.validate();
            token_distribution_schedule.check_minimum_stake_for_validators(
                self.validators.values().map(|v| v.info.iota_address()),
                self.parameters.protocol_version,
            )?;
        }

        Ok(())
    }

    /// Runs through validation checks on the generated output (the initial
    /// chain state) based on the input values present in the builder
    fn validate_output(&self) {
        // If genesis hasn't been built yet, just early return as there is nothing to
        // validate yet
        let Some(unsigned_genesis) = self.unsigned_genesis_checkpoint() else {
            return;
        };

        let GenesisChainParameters {
            protocol_version,
            chain_start_timestamp_ms,
            epoch_duration_ms,
            max_validator_count,
            min_validator_joining_stake,
            validator_low_stake_threshold,
            validator_very_low_stake_threshold,
            validator_low_stake_grace_period,
        } = self.parameters.to_genesis_chain_parameters();

        // In non-testing code, genesis type must always be V1.
        let system_state = match unsigned_genesis.iota_system_object() {
            IotaSystemState::V1(inner) => inner,
            IotaSystemState::V2(_) => unreachable!(),
            #[cfg(msim)]
            _ => {
                // Types other than V1 used in simtests do not need to be validated.
                return;
            }
        };

        assert!(unsigned_genesis.has_randomness_state_object());

        assert!(unsigned_genesis.has_coin_deny_list_object());

        assert_eq!(
            self.validators.len(),
            system_state.validators.active_validators.len()
        );
        let mut address_to_pool_id = BTreeMap::new();
        for (validator, onchain_validator) in self
            .validators
            .values()
            .zip(system_state.validators.active_validators.iter())
        {
            let metadata = onchain_validator.verified_metadata();

            // Validators should not have duplicate addresses so the result of insertion
            // should be None.
            assert!(
                address_to_pool_id
                    .insert(metadata.iota_address, onchain_validator.staking_pool.id)
                    .is_none()
            );
            assert_eq!(validator.info.iota_address(), metadata.iota_address);
            assert_eq!(validator.info.authority_key(), metadata.iota_pubkey_bytes());
            assert_eq!(validator.info.network_key, metadata.network_pubkey);
            assert_eq!(validator.info.protocol_key, metadata.protocol_pubkey);
            assert_eq!(
                validator.proof_of_possession.as_ref().to_vec(),
                metadata.proof_of_possession_bytes
            );
            assert_eq!(validator.info.name(), &metadata.name);
            assert_eq!(validator.info.description, metadata.description);
            assert_eq!(validator.info.image_url, metadata.image_url);
            assert_eq!(validator.info.project_url, metadata.project_url);
            assert_eq!(validator.info.network_address(), &metadata.net_address);
            assert_eq!(validator.info.p2p_address, metadata.p2p_address);
            assert_eq!(validator.info.primary_address, metadata.primary_address);

            assert_eq!(validator.info.gas_price, onchain_validator.gas_price);
            assert_eq!(
                validator.info.commission_rate,
                onchain_validator.commission_rate
            );
        }

        assert_eq!(system_state.epoch, 0);
        assert_eq!(system_state.protocol_version, protocol_version);
        assert_eq!(system_state.storage_fund.non_refundable_balance.value(), 0);
        assert_eq!(
            system_state
                .storage_fund
                .total_object_storage_rebates
                .value(),
            0
        );

        assert_eq!(system_state.parameters.epoch_duration_ms, epoch_duration_ms);
        assert_eq!(
            system_state.parameters.max_validator_count,
            max_validator_count,
        );
        assert_eq!(
            system_state.parameters.min_validator_joining_stake,
            min_validator_joining_stake,
        );
        assert_eq!(
            system_state.parameters.validator_low_stake_threshold,
            validator_low_stake_threshold,
        );
        assert_eq!(
            system_state.parameters.validator_very_low_stake_threshold,
            validator_very_low_stake_threshold,
        );
        assert_eq!(
            system_state.parameters.validator_low_stake_grace_period,
            validator_low_stake_grace_period,
        );

        assert!(!system_state.safe_mode);
        assert_eq!(
            system_state.epoch_start_timestamp_ms,
            chain_start_timestamp_ms,
        );
        assert_eq!(system_state.validators.pending_removals.len(), 0);
        assert_eq!(
            system_state
                .validators
                .pending_active_validators
                .contents
                .size,
            0
        );
        assert_eq!(system_state.validators.inactive_validators.size, 0);
        assert_eq!(system_state.validators.validator_candidates.size, 0);

        // Check distribution is correct
        let token_distribution_schedule = self.token_distribution_schedule.clone().unwrap();

        let allocations_amount: u64 = token_distribution_schedule
            .allocations
            .iter()
            .map(|allocation| allocation.amount_nanos)
            .sum();

        assert_eq!(
            system_state.iota_treasury_cap.total_supply().value,
            token_distribution_schedule.pre_minted_supply + allocations_amount
        );

        let mut gas_objects: BTreeMap<ObjectId, (&Object, GasCoin)> = unsigned_genesis
            .objects()
            .iter()
            .filter_map(|o| GasCoin::try_from(o).ok().map(|g| (o.id(), (o, g))))
            .collect();
        let mut staked_iota_objects: BTreeMap<ObjectId, (&Object, StakedIota)> = unsigned_genesis
            .objects()
            .iter()
            .filter_map(|o| StakedIota::try_from(o).ok().map(|s| (o.id(), (o, s))))
            .collect();

        for allocation in token_distribution_schedule.allocations {
            if let Some(staked_with_validator) = allocation.staked_with_validator {
                let staking_pool_id = *address_to_pool_id
                    .get(&staked_with_validator)
                    .expect("staking pool should exist");
                let staked_iota_object_id = staked_iota_objects
                    .iter()
                    .find(|(_k, (o, s))| {
                        let Owner::Address(owner) = &o.owner else {
                            panic!("gas object owner must be address owner");
                        };
                        *owner == allocation.recipient_address
                            && s.principal() == allocation.amount_nanos
                            && s.pool_id() == staking_pool_id
                    })
                    .map(|(k, _)| *k)
                    .expect("all allocations should be present");
                let staked_iota_object =
                    staked_iota_objects.remove(&staked_iota_object_id).unwrap();
                assert_eq!(
                    staked_iota_object.0.owner,
                    Owner::Address(allocation.recipient_address)
                );
                assert_eq!(staked_iota_object.1.principal(), allocation.amount_nanos);
                assert_eq!(staked_iota_object.1.pool_id(), staking_pool_id);
                assert_eq!(staked_iota_object.1.activation_epoch(), 0);
            } else {
                let gas_object_id = gas_objects
                    .iter()
                    .find(|(_k, (o, g))| {
                        if let Owner::Address(owner) = &o.owner {
                            *owner == allocation.recipient_address
                                && g.value() == allocation.amount_nanos
                        } else {
                            false
                        }
                    })
                    .map(|(k, _)| *k)
                    .expect("all allocations should be present");
                let gas_object = gas_objects.remove(&gas_object_id).unwrap();
                assert_eq!(
                    gas_object.0.owner,
                    Owner::Address(allocation.recipient_address)
                );
                assert_eq!(gas_object.1.value(), allocation.amount_nanos,);
            }
        }

        // All Gas and staked objects should be accounted for
        if !self.parameters.allow_insertion_of_extra_objects {
            assert!(gas_objects.is_empty());
            assert!(staked_iota_objects.is_empty());
        }

        let committee = system_state.get_current_epoch_committee();
        for signature in self.signatures.values() {
            if !self.validators.contains_key(&signature.authority) {
                panic!("found signature for unknown validator: {signature:#?}");
            }

            signature
                .verify_secure(
                    unsigned_genesis.checkpoint(),
                    Intent::iota_app(IntentScope::CheckpointSummary),
                    committee.committee(),
                )
                .expect("signature should be valid");
        }
    }

    pub fn load<P: AsRef<Path>>(path: P) -> anyhow::Result<Self, anyhow::Error> {
        let path = path.as_ref();
        let path: &Utf8Path = path.try_into()?;
        trace!("Reading Genesis Builder from {}", path);

        if !path.is_dir() {
            bail!("path must be a directory");
        }

        // Load parameters
        let parameters_file = path.join(GENESIS_BUILDER_PARAMETERS_FILE);
        let parameters = serde_yaml::from_slice(&fs::read(&parameters_file).context(format!(
            "unable to read genesis parameters file {parameters_file}"
        ))?)
        .context("unable to deserialize genesis parameters")?;

        let token_distribution_schedule_file =
            path.join(GENESIS_BUILDER_TOKEN_DISTRIBUTION_SCHEDULE_FILE);
        let token_distribution_schedule = if token_distribution_schedule_file.exists() {
            Some(TokenDistributionSchedule::from_csv(fs::File::open(
                token_distribution_schedule_file,
            )?)?)
        } else {
            None
        };

        // Load validator infos
        let mut committee = BTreeMap::new();
        for entry in path.join(GENESIS_BUILDER_COMMITTEE_DIR).read_dir_utf8()? {
            let entry = entry?;
            if entry.file_name().starts_with('.') {
                continue;
            }

            let path = entry.path();
            let validator_info: GenesisValidatorInfo = serde_yaml::from_slice(&fs::read(path)?)
                .with_context(|| format!("unable to load validator info for {path}"))?;
            committee.insert(validator_info.info.authority_key(), validator_info);
        }

        // Load Signatures
        let mut signatures = BTreeMap::new();
        for entry in path.join(GENESIS_BUILDER_SIGNATURE_DIR).read_dir_utf8()? {
            let entry = entry?;
            if entry.file_name().starts_with('.') {
                continue;
            }

            let path = entry.path();
            let sigs: AuthoritySignInfo = bcs::from_bytes(&fs::read(path)?)
                .with_context(|| format!("unable to load validator signature for {path}"))?;
            signatures.insert(sigs.authority, sigs);
        }

        let mut builder = Self {
            parameters,
            token_distribution_schedule,
            objects: Default::default(),
            validators: committee,
            signatures,
            built_genesis: None, // Leave this as none, will build and compare below
        };

        let unsigned_genesis_file = path.join(GENESIS_BUILDER_UNSIGNED_GENESIS_FILE);
        if unsigned_genesis_file.exists() {
            let reader = BufReader::new(File::open(unsigned_genesis_file)?);
            let loaded_genesis: UnsignedGenesis = bcs::from_reader(reader)?;

            // If we have a built genesis, then we must have a token_distribution_schedule
            // present as well.
            assert!(
                builder.token_distribution_schedule.is_some(),
                "If a built genesis is present, then there must also be a token-distribution-schedule present"
            );

            // Verify loaded genesis matches one build from the constituent parts
            loaded_genesis.checkpoint_contents.digest(); // cache digest before compare
            assert!(
                *builder.get_or_build_unsigned_genesis() == loaded_genesis,
                "loaded genesis does not match built genesis"
            );

            // Just to double check that its set after building above
            assert!(builder.unsigned_genesis_checkpoint().is_some());
        }

        Ok(builder)
    }

    pub fn save<P: AsRef<Path>>(self, path: P) -> anyhow::Result<(), anyhow::Error> {
        let path = path.as_ref();
        trace!("Writing Genesis Builder to {}", path.display());

        fs::create_dir_all(path)?;

        // Write parameters
        let parameters_file = path.join(GENESIS_BUILDER_PARAMETERS_FILE);
        fs::write(parameters_file, serde_yaml::to_string(&self.parameters)?)?;

        if let Some(token_distribution_schedule) = &self.token_distribution_schedule {
            token_distribution_schedule.to_csv(fs::File::create(
                path.join(GENESIS_BUILDER_TOKEN_DISTRIBUTION_SCHEDULE_FILE),
            )?)?;
        }

        // Write Signatures
        let signature_dir = path.join(GENESIS_BUILDER_SIGNATURE_DIR);
        std::fs::create_dir_all(&signature_dir)?;
        for (pubkey, sigs) in self.signatures {
            let name = self.validators.get(&pubkey).unwrap().info.name();
            fs::write(signature_dir.join(name), &bcs::to_bytes(&sigs)?)?;
        }

        // Write validator infos
        let committee_dir = path.join(GENESIS_BUILDER_COMMITTEE_DIR);
        fs::create_dir_all(&committee_dir)?;

        for (_pubkey, validator) in self.validators {
            fs::write(
                committee_dir.join(validator.info.name()),
                &serde_yaml::to_string(&validator)?,
            )?;
        }

        if let Some(genesis) = &self.built_genesis {
            let mut write = BufWriter::new(File::create(
                path.join(GENESIS_BUILDER_UNSIGNED_GENESIS_FILE),
            )?);
            bcs::serialize_into(&mut write, &genesis)?;
        }

        Ok(())
    }
}

// Create a Genesis Txn Context to be used when generating genesis objects by
// hashing all of the inputs into genesis ans using that as our "Txn Digest".
// This is done to ensure that coin objects created between chains are unique
fn create_genesis_context(
    epoch_data: &EpochData,
    genesis_chain_parameters: &GenesisChainParameters,
    genesis_validators: &[GenesisValidatorMetadata],
    token_distribution_schedule: &TokenDistributionSchedule,
    system_packages: &[SystemPackage],
    protocol_config: &ProtocolConfig,
) -> Rc<RefCell<TxContext>> {
    let mut hasher = DefaultHash::default();
    hasher.update(b"iota-genesis");
    hasher.update(bcs::to_bytes(genesis_chain_parameters).unwrap());
    hasher.update(bcs::to_bytes(genesis_validators).unwrap());
    hasher.update(bcs::to_bytes(token_distribution_schedule).unwrap());
    for system_package in system_packages {
        hasher.update(bcs::to_bytes(&system_package.bytes).unwrap());
    }

    let hash = hasher.finalize();
    let genesis_transaction_digest = TransactionDigest::new(hash.into());

    let tx_context = TxContext::new(
        &Address::ZERO,
        &genesis_transaction_digest,
        epoch_data,
        0,
        0,
        0,
        None,
        protocol_config,
    );

    Rc::new(RefCell::new(tx_context))
}

fn build_unsigned_genesis_data<'info>(
    parameters: &GenesisCeremonyParameters,
    token_distribution_schedule: &TokenDistributionSchedule,
    validators: impl Iterator<Item = &'info GenesisValidatorInfo>,
    objects: Vec<Object>,
) -> UnsignedGenesis {
    if !parameters.allow_insertion_of_extra_objects && !objects.is_empty() {
        panic!(
            "insertion of extra objects at genesis time is prohibited due to 'allow_insertion_of_extra_objects' parameter"
        );
    }

    let genesis_chain_parameters = parameters.to_genesis_chain_parameters();
    let genesis_validators = validators
        .cloned()
        .map(GenesisValidatorMetadata::from)
        .collect::<Vec<_>>();

    let epoch_data = EpochData::new_genesis(genesis_chain_parameters.chain_start_timestamp_ms);

    // Get the correct system packages for our protocol version. If we cannot find
    // the snapshot that means that we must be at the latest version and we
    // should use the latest version of the framework.
    let mut system_packages =
        iota_framework_snapshot::load_bytecode_snapshot(parameters.protocol_version.as_u64())
            .unwrap_or_else(|_| BuiltInFramework::iter_system_packages().cloned().collect());

    // if system packages are provided in `objects`, update them with the provided
    // bytes. This is a no-op under normal conditions and only an issue with
    // certain tests.
    update_system_packages_from_objects(&mut system_packages, &objects);

    let protocol_config = get_genesis_protocol_config(parameters.protocol_version);

    let genesis_ctx = create_genesis_context(
        &epoch_data,
        &genesis_chain_parameters,
        &genesis_validators,
        token_distribution_schedule,
        &system_packages,
        &protocol_config,
    );

    // Use a throwaway metrics registry for genesis transaction execution.
    let registry = prometheus_filtered::Registry::new();
    let metrics = Arc::new(LimitsMetrics::new(&registry));

    // In here the main genesis objects are created. This means the main system
    // objects and the ones that are created at genesis like the network coin.
    let (genesis_objects, events) = create_genesis_objects(
        genesis_ctx,
        objects,
        &genesis_validators,
        &genesis_chain_parameters,
        token_distribution_schedule,
        system_packages,
        metrics.clone(),
    );

    // Create the main genesis transaction of kind `GenesisTransaction`
    let (genesis_transaction, genesis_effects, genesis_events, genesis_objects) =
        create_genesis_transaction(
            genesis_objects,
            events,
            &protocol_config,
            metrics,
            &epoch_data,
        );

    let (checkpoint, checkpoint_contents) = create_genesis_checkpoint(
        &protocol_config,
        parameters,
        &genesis_transaction,
        &genesis_effects,
    );

    UnsignedGenesis {
        checkpoint,
        checkpoint_contents,
        transaction: genesis_transaction,
        effects: genesis_effects,
        events: genesis_events,
        objects: genesis_objects,
    }
}

// Some tests provide an override of the system packages via objects to the
// genesis builder. When that happens we need to update the system packages with
// the new bytes provided. Mock system packages in protocol config tests are an
// example of that (today the only example).
// The problem here arises from the fact that if regular system packages are
// pushed first *AND* if any of them is loaded in the loader cache, there is no
// way to override them with the provided object (no way to mock properly).
// System packages are loaded only from internal dependencies (a system package
// depending on some other), and in that case they would be loaded in the
// VM/loader cache. The Bridge is an example of that and what led to this code.
// The bridge depends on `iota_system` which is mocked in some tests, but would
// be in the loader cache courtesy of the Bridge, thus causing the problem.
fn update_system_packages_from_objects(
    system_packages: &mut Vec<SystemPackage>,
    objects: &[Object],
) {
    // Filter `objects` for system packages, and make `SystemPackage`s out of them.
    let system_package_overrides: BTreeMap<ObjectId, Vec<Vec<u8>>> = objects
        .iter()
        .filter_map(|obj| {
            let pkg = obj.data.as_opt_package()?;
            pkg.id().is_system_package().then(|| {
                (
                    pkg.id(),
                    pkg.serialized_module_map().values().cloned().collect(),
                )
            })
        })
        .collect();

    // Replace packages in `system_packages` that are present in `objects` with
    // their counterparts from the previous step.
    for package in system_packages {
        if let Some(overrides) = system_package_overrides.get(&package.id).cloned() {
            package.bytes = overrides;
        }
    }
}

fn create_genesis_checkpoint(
    protocol_config: &ProtocolConfig,
    parameters: &GenesisCeremonyParameters,
    system_genesis_transaction: &TransactionEnvelope,
    system_genesis_tx_effects: &TransactionEffects,
) -> (CheckpointSummary, CheckpointContents) {
    let genesis_execution_digests = ExecutionDigests {
        transaction: *system_genesis_transaction.digest(),
        effects: system_genesis_tx_effects.digest(),
    };

    let contents = CheckpointContents::new_with_digests_and_signatures(
        vec![genesis_execution_digests],
        vec![vec![]],
    );
    let version_specific_data =
        match protocol_config.checkpoint_summary_version_specific_data_as_option() {
            None | Some(0) => Vec::new(),
            Some(1) => bcs::to_bytes(&CheckpointVersionSpecificData::V1(
                CheckpointVersionSpecificDataV1::default(),
            ))
            .unwrap(),
            _ => unimplemented!("unrecognized version_specific_data version for CheckpointSummary"),
        };
    let checkpoint = CheckpointSummary {
        epoch: 0,
        sequence_number: 0,
        network_total_transactions: contents.len().try_into().unwrap(),
        contents_digest: contents.digest(),
        previous_digest: None,
        epoch_rolling_gas_cost_summary: Default::default(),
        end_of_epoch_data: None,
        timestamp_ms: parameters.chain_start_timestamp_ms,
        version_specific_data,
        checkpoint_commitments: Default::default(),
    };

    (checkpoint, contents)
}

fn create_genesis_transaction(
    objects: Vec<Object>,
    events: Vec<Event>,
    protocol_config: &ProtocolConfig,
    metrics: Arc<LimitsMetrics>,
    epoch_data: &EpochData,
) -> (
    TransactionEnvelope,
    TransactionEffects,
    TransactionEvents,
    Vec<Object>,
) {
    let genesis_transaction = {
        let genesis_objects = objects
            .into_iter()
            .map(|mut object| {
                if let Some(o) = object.data.as_opt_mut_struct() {
                    o.decrement_version_to(Version::MIN_VALID_INCL);
                }

                if let Owner::Shared(initial_shared_version) = &mut object.owner {
                    *initial_shared_version = Version::MIN_VALID_INCL;
                }

                let object = object.into_inner();
                GenesisObject::new(object.data, object.owner)
            })
            .collect();

        iota_types::transaction::VerifiedTransaction::new_genesis_transaction(
            genesis_objects,
            events,
        )
        .into_inner()
    };

    // execute txn to effects
    let (effects, events, objects) =
        execute_genesis_transaction(epoch_data, protocol_config, metrics, &genesis_transaction);

    (genesis_transaction, effects, events, objects)
}

fn create_genesis_objects(
    genesis_ctx: Rc<RefCell<TxContext>>,
    input_objects: Vec<Object>,
    validators: &[GenesisValidatorMetadata],
    parameters: &GenesisChainParameters,
    token_distribution_schedule: &TokenDistributionSchedule,
    system_packages: Vec<SystemPackage>,
    metrics: Arc<LimitsMetrics>,
) -> (Vec<Object>, Vec<Event>) {
    let mut store = InMemoryStorage::new(Vec::new());
    let mut events = Vec::new();
    // We don't know the chain ID here since we haven't yet created the genesis
    // checkpoint. However since we know there are no chain specific protocol
    // config options in genesis, we use Chain::Unknown here.
    let protocol_config = ProtocolConfig::get_for_version(
        ProtocolVersion::new(parameters.protocol_version),
        Chain::Unknown,
    );

    let silent = true;
    let executor = iota_execution::executor(&protocol_config, silent, None)
        .expect("Creating an executor should not fail here");

    for system_package in system_packages.into_iter() {
        let tx_events = process_package(
            &mut store,
            executor.as_ref(),
            genesis_ctx.clone(),
            &system_package.modules(),
            system_package.dependencies,
            &protocol_config,
            metrics.clone(),
        )
        .expect("Processing a package should not fail here");

        events.extend(tx_events.0);
    }

    for object in input_objects {
        store.insert_object(object);
    }

    generate_genesis_system_object(
        &mut store,
        executor.as_ref(),
        validators,
        genesis_ctx,
        parameters,
        token_distribution_schedule,
        metrics,
    )
    .expect("Genesis creation should not fail here");

    (store.into_inner().into_values().collect(), events)
}

pub(crate) fn process_package(
    store: &mut InMemoryStorage,
    executor: &dyn Executor,
    ctx: Rc<RefCell<TxContext>>,
    modules: &[CompiledModule],
    dependencies: Vec<ObjectId>,
    protocol_config: &ProtocolConfig,
    metrics: Arc<LimitsMetrics>,
) -> anyhow::Result<TransactionEvents> {
    let dependency_objects = store.get_objects(&dependencies);
    // When publishing genesis packages, since the std framework packages all have
    // non-zero addresses, they will be considered as dependencies even though they
    // are not. Hence input_objects contain objects that don't exist on-chain
    // because they are yet to be published.
    #[cfg(debug_assertions)]
    {
        use std::collections::HashSet;

        use move_core_types::account_address::AccountAddress;

        let to_be_published_addresses: HashSet<_> = modules
            .iter()
            .map(|module| *module.self_id().address())
            .collect();
        assert!(
            // An object either exists on-chain, or is one of the packages to be published.
            dependencies
                .iter()
                .zip(dependency_objects.iter())
                .all(|(dependency, obj_opt)| obj_opt.is_some()
                    || to_be_published_addresses
                        .contains(&AccountAddress::new(dependency.into_bytes())))
        );
    }
    let loaded_dependencies: Vec<_> = dependencies
        .iter()
        .zip(dependency_objects)
        .filter_map(|(dependency, object)| {
            Some(ObjectReadResult::new(
                InputObjectKind::MovePackage(*dependency),
                object?.clone().into(),
            ))
        })
        .collect();

    let module_bytes = modules
        .iter()
        .map(|m| {
            let mut buf = vec![];
            m.serialize_with_version(m.version, &mut buf).unwrap();
            buf
        })
        .collect();
    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();
        // executing in Genesis mode does not create an `UpgradeCap`.
        builder.command(Command::new_publish(module_bytes, dependencies));
        builder.finish()
    };
    let InnerTemporaryStore {
        written, events, ..
    } = executor.update_genesis_state(
        &*store,
        protocol_config,
        metrics,
        ctx,
        CheckedInputObjects::new_for_genesis(loaded_dependencies),
        pt,
    )?;

    store.finish(written);

    Ok(events)
}

pub fn generate_genesis_system_object(
    store: &mut InMemoryStorage,
    executor: &dyn Executor,
    genesis_validators: &[GenesisValidatorMetadata],
    genesis_ctx: Rc<RefCell<TxContext>>,
    genesis_chain_parameters: &GenesisChainParameters,
    token_distribution_schedule: &TokenDistributionSchedule,
    metrics: Arc<LimitsMetrics>,
) -> anyhow::Result<()> {
    let protocol_config = ProtocolConfig::get_for_version(
        ProtocolVersion::new(genesis_chain_parameters.protocol_version),
        ChainIdentifier::default().chain(),
    );

    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();
        // Step 1: Create the IotaSystemState UID
        let iota_system_state_uid = builder.programmable_move_call(
            ObjectId::FRAMEWORK,
            Identifier::OBJECT_MODULE,
            Identifier::from_static("iota_system_state"),
            vec![],
            vec![],
        );

        // Step 2: Create and share the Clock.
        builder.move_call(
            ObjectId::FRAMEWORK,
            Identifier::CLOCK_MODULE,
            Identifier::from_static("create"),
            vec![],
            vec![],
        )?;

        // Create the randomness state_object
        builder.move_call(
            ObjectId::FRAMEWORK,
            Identifier::RANDOM_MODULE,
            RANDOMNESS_STATE_CREATE_FUNCTION_NAME,
            vec![],
            vec![],
        )?;

        // Create the deny list
        builder.move_call(
            ObjectId::FRAMEWORK,
            Identifier::DENY_LIST_MODULE,
            DENY_LIST_CREATE_FUNC,
            vec![],
            vec![],
        )?;

        // Step 4: Create the IOTA Coin Treasury Cap.
        let iota_treasury_cap = builder.programmable_move_call(
            ObjectId::FRAMEWORK,
            Identifier::IOTA_MODULE,
            Identifier::from_static("new"),
            vec![],
            vec![],
        );

        // Step 5: Create System Admin Cap.
        let system_admin_cap = builder.programmable_move_call(
            ObjectId::FRAMEWORK,
            Identifier::SYSTEM_ADMIN_CAP_MODULE,
            Identifier::from_static("new_system_admin_cap"),
            vec![],
            vec![],
        );

        // Step 6: Run genesis.
        // The first argument is the system state uid we got from step 1 and the second
        // one is the IOTA `TreasuryCap` we got from step 4.
        let mut arguments = vec![iota_system_state_uid, iota_treasury_cap];
        let mut call_arg_arguments = vec![
            CallArg::pure(&genesis_chain_parameters),
            CallArg::pure(&genesis_validators),
            CallArg::pure(&token_distribution_schedule),
        ]
        .into_iter()
        .map(|a| builder.input(a))
        .collect::<anyhow::Result<_, _>>()?;
        arguments.append(&mut call_arg_arguments);
        if genesis_chain_parameters.protocol_version
            < GENESIS_BUILDER_MIGRATION_LOGIC_REMOVAL_PROTOCOL_VERSION
        {
            // For older protocol versions, e.g., for running some specific tests, we need
            // to pass the timelock genesis label as an argument, but as a None value.
            arguments.push(builder.input(CallArg::pure(&None::<String>))?);
        }
        arguments.push(system_admin_cap);
        builder.programmable_move_call(
            ObjectId::SYSTEM,
            Identifier::from_static("genesis"),
            Identifier::from_static("create"),
            vec![],
            arguments,
        );

        builder.finish()
    };

    let InnerTemporaryStore { mut written, .. } = executor.update_genesis_state(
        &*store,
        &protocol_config,
        metrics,
        genesis_ctx,
        CheckedInputObjects::new_for_genesis(vec![]),
        pt,
    )?;

    // update the value of the clock to match the chain start time
    {
        let object = written.get_mut(&ObjectId::CLOCK).unwrap();
        object
            .data
            .as_opt_mut_struct()
            .unwrap()
            .set_clock_timestamp_ms_unchecked(genesis_chain_parameters.chain_start_timestamp_ms);
    }

    store.finish(written);

    Ok(())
}

#[cfg(test)]
mod test {
    use fastcrypto::traits::KeyPair;
    use iota_config::{
        genesis::*,
        local_ip_utils,
        node::{DEFAULT_COMMISSION_RATE, DEFAULT_VALIDATOR_GAS_PRICE},
    };
    use iota_protocol_config::ProtocolVersion;
    use iota_sdk_types::Address;
    use iota_types::crypto::{
        AccountKeyPair, AuthorityKeyPair, NetworkKeyPair, generate_proof_of_possession,
        get_key_pair_from_rng,
    };

    use crate::{Builder, validator_info::ValidatorInfo};

    #[test]
    fn allocation_csv() {
        // No genesis is being built in this test, so there is no protocol version to
        // thread through; use the current version.
        let schedule = TokenDistributionSchedule::new_for_validators_with_default_allocation(
            [Address::random(), Address::random()],
            ProtocolVersion::MAX,
        );
        let mut output = Vec::new();

        schedule.to_csv(&mut output).unwrap();

        let parsed_schedule = TokenDistributionSchedule::from_csv(output.as_slice()).unwrap();

        assert_eq!(schedule, parsed_schedule);

        std::io::Write::write_all(&mut std::io::stdout(), &output).unwrap();
    }

    #[test]
    #[cfg_attr(msim, ignore)]
    fn ceremony() {
        let dir = tempfile::TempDir::new().unwrap();

        let authority_key: AuthorityKeyPair = get_key_pair_from_rng(&mut rand::rngs::OsRng).1;
        let protocol_key: NetworkKeyPair = get_key_pair_from_rng(&mut rand::rngs::OsRng).1;
        let account_key: AccountKeyPair = get_key_pair_from_rng(&mut rand::rngs::OsRng).1;
        let account_address = account_key.public_key().derive_address();
        let network_key: NetworkKeyPair = get_key_pair_from_rng(&mut rand::rngs::OsRng).1;
        let validator = ValidatorInfo {
            name: "0".into(),
            authority_key: authority_key.public().into(),
            protocol_key: protocol_key.public().clone(),
            account_address,
            network_key: network_key.public().clone(),
            gas_price: DEFAULT_VALIDATOR_GAS_PRICE,
            commission_rate: DEFAULT_COMMISSION_RATE,
            network_address: local_ip_utils::new_local_tcp_address_for_testing(),
            p2p_address: local_ip_utils::new_local_udp_address_for_testing(),
            primary_address: local_ip_utils::new_local_udp_address_for_testing(),
            description: String::new(),
            image_url: String::new(),
            project_url: String::new(),
        };
        let pop = generate_proof_of_possession(&authority_key, account_address);
        let mut builder = Builder::new().add_validator(validator, pop);

        let genesis = builder.get_or_build_unsigned_genesis();
        for object in genesis.objects() {
            println!("ObjectId: {} Type: {:?}", object.id(), object.type_());
        }
        builder.save(dir.path()).unwrap();
        Builder::load(dir.path()).unwrap();
    }
}
