// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, HashSet},
    fs::{self, File},
    io::{prelude::Read, BufReader, BufWriter},
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
};

use anyhow::{bail, Context};
use camino::Utf8Path;
use fastcrypto::{hash::HashFunction, traits::KeyPair};
use flate2::bufread::GzDecoder;
use iota_config::genesis::{
    Genesis, GenesisCeremonyParameters, GenesisChainParameters, TokenDistributionSchedule,
    UnsignedGenesis,
};
use iota_execution::{self, Executor};
use iota_framework::{BuiltInFramework, SystemPackage};
use iota_protocol_config::{Chain, ProtocolConfig, ProtocolVersion};
use iota_sdk::{types::block::address::Address, Url};
use iota_types::{
    balance::{Balance, BALANCE_MODULE_NAME},
    base_types::{
        ExecutionDigests, IotaAddress, ObjectID, ObjectRef, SequenceNumber, TransactionDigest,
        TxContext,
    },
    committee::Committee,
    crypto::{
        AuthorityKeyPair, AuthorityPublicKeyBytes, AuthoritySignInfo, AuthoritySignInfoTrait,
        AuthoritySignature, DefaultHash, IotaAuthoritySignature,
    },
    deny_list::{DENY_LIST_CREATE_FUNC, DENY_LIST_MODULE},
    digests::ChainIdentifier,
    effects::{TransactionEffects, TransactionEffectsAPI, TransactionEvents},
    epoch_data::EpochData,
    event::Event,
    gas::IotaGasStatus,
    gas_coin::{GasCoin, GAS},
    governance::StakedIota,
    in_memory_storage::InMemoryStorage,
    inner_temporary_store::InnerTemporaryStore,
    iota_system_state::{get_iota_system_state, IotaSystemState, IotaSystemStateTrait},
    message_envelope::Message,
    messages_checkpoint::{CertifiedCheckpointSummary, CheckpointContents, CheckpointSummary},
    metrics::LimitsMetrics,
    object::{Object, Owner},
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    stardust::stardust_to_iota_address,
    timelock::{
        stardust_upgrade_label::STARDUST_UPGRADE_LABEL_VALUE,
        timelocked_staked_iota::TimelockedStakedIota,
    },
    transaction::{
        CallArg, CheckedInputObjects, Command, InputObjectKind, ObjectArg, ObjectReadResult,
        Transaction,
    },
    IOTA_FRAMEWORK_PACKAGE_ID, IOTA_SYSTEM_ADDRESS,
};
use move_binary_format::CompiledModule;
use move_core_types::ident_str;
use serde::{Deserialize, Serialize};
use shared_crypto::intent::{Intent, IntentMessage, IntentScope};
use stake::{delegate_genesis_stake, GenesisStake};
use stardust::migration::MigrationObjects;
use tracing::trace;
use validator_info::{GenesisValidatorInfo, GenesisValidatorMetadata, ValidatorInfo};

mod stake;
pub mod stardust;
pub mod validator_info;

// TODO: Lazy static `stardust_to_iota_address`
pub const IF_STARDUST_ADDRESS: &str =
    "iota1qp8h9augeh6tk3uvlxqfapuwv93atv63eqkpru029p6sgvr49eufyz7katr";

const GENESIS_BUILDER_COMMITTEE_DIR: &str = "committee";
pub const GENESIS_BUILDER_PARAMETERS_FILE: &str = "parameters";
const GENESIS_BUILDER_TOKEN_DISTRIBUTION_SCHEDULE_FILE: &str = "token-distribution-schedule";
const GENESIS_BUILDER_SIGNATURE_DIR: &str = "signatures";
const GENESIS_BUILDER_UNSIGNED_GENESIS_FILE: &str = "unsigned-genesis";
const GENESIS_BUILDER_MIGRATION_SOURCES_FILE: &str = "migration-sources";

pub const OBJECT_SNAPSHOT_FILE_PATH: &str = "stardust_object_snapshot.bin";
pub const IOTA_OBJECT_SNAPSHOT_URL: &str = "https://stardust-objects.s3.eu-central-1.amazonaws.com/iota/alphanet/latest/stardust_object_snapshot.bin.gz";
pub const SHIMMER_OBJECT_SNAPSHOT_URL: &str = "https://stardust-objects.s3.eu-central-1.amazonaws.com/shimmer/alphanet/latest/stardust_object_snapshot.bin.gz";

pub struct Builder {
    parameters: GenesisCeremonyParameters,
    token_distribution_schedule: Option<TokenDistributionSchedule>,
    objects: BTreeMap<ObjectID, Object>,
    validators: BTreeMap<AuthorityPublicKeyBytes, GenesisValidatorInfo>,
    // Validator signatures over checkpoint
    signatures: BTreeMap<AuthorityPublicKeyBytes, AuthoritySignInfo>,
    built_genesis: Option<UnsignedGenesis>,
    migration_objects: MigrationObjects,
    genesis_stake: GenesisStake,
    migration_sources: Vec<SnapshotSource>,
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
            migration_objects: Default::default(),
            genesis_stake: Default::default(),
            migration_sources: Default::default(),
        }
    }

    /// Checks if the genesis to be built is vanilla or if it includes Stardust
    /// migration stakes
    pub fn is_vanilla(&self) -> bool {
        self.genesis_stake.is_empty()
    }

    pub fn with_parameters(mut self, parameters: GenesisCeremonyParameters) -> Self {
        self.parameters = parameters;
        self
    }

    /// Set the [`TokenDistributionSchedule`].
    ///
    /// # Panic
    ///
    /// This method fails if the passed schedule contains timelocked stake.
    /// This is to avoid conflicts with the genesis stake, that delegates
    /// timelocked stake based on the migrated state.
    pub fn with_token_distribution_schedule(
        mut self,
        token_distribution_schedule: TokenDistributionSchedule,
    ) -> Self {
        assert!(
            !token_distribution_schedule.contains_timelocked_stake(),
            "timelocked stake should be generated only from migrated stake"
        );
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
            validator.protocol_key(),
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

    pub fn add_migration_source(mut self, source: SnapshotSource) -> Self {
        self.migration_sources.push(source);
        self
    }

    pub fn unsigned_genesis_checkpoint(&self) -> Option<UnsignedGenesis> {
        self.built_genesis.clone()
    }

    fn load_migration_sources(&mut self) -> anyhow::Result<()> {
        for source in &self.migration_sources {
            tracing::info!("Adding migration objects from {:?}", source);
            self.migration_objects
                .extend(bcs::from_reader::<Vec<_>>(source.to_reader()?)?);
        }
        Ok(())
    }

    /// Create and cache the [`GenesisStake`] if the builder
    /// contains migrated objects.
    fn create_and_cache_genesis_stake(&mut self) -> anyhow::Result<()> {
        if !self.migration_objects.is_empty() {
            let delegator =
                stardust_to_iota_address(Address::try_from_bech32(IF_STARDUST_ADDRESS).unwrap())
                    .unwrap();
            // TODO: check whether we need to start with
            // VALIDATOR_LOW_STAKE_THRESHOLD_NANOS
            let minimum_stake = iota_types::governance::MIN_VALIDATOR_JOINING_STAKE_NANOS;
            self.genesis_stake = delegate_genesis_stake(
                self.validators.values(),
                delegator,
                &self.migration_objects,
                minimum_stake,
            )?;
        }
        Ok(())
    }

    /// Evaluate the genesis [`TokenDistributionSchedule`].
    ///
    /// This merges conditionally the cached token distribution
    /// (i.e. `self.token_distribution_schedule`)  with the genesis stake
    /// resulting from the migrated state.
    ///
    /// If the cached token distribution schedule contains timelocked stake, it
    /// is assumed that the genesis stake is already merged and no operation
    /// is performed. This is the case where we load a [`Builder`] from disk
    /// that has already built genesis with the migrated state.
    fn resolve_token_distribution_schedule(&mut self) -> TokenDistributionSchedule {
        let validator_addresses = self.validators.values().map(|v| v.info.iota_address());
        let token_distribution_schedule = self.token_distribution_schedule.take();
        if self.genesis_stake.is_empty() {
            token_distribution_schedule.unwrap_or_else(|| {
                TokenDistributionSchedule::new_for_validators_with_default_allocation(
                    validator_addresses,
                )
            })
        } else if let Some(schedule) = token_distribution_schedule {
            if schedule.contains_timelocked_stake() {
                // Genesis stake is already included
                schedule
            } else {
                self.genesis_stake
                    .extend_vanilla_token_distribution_schedule(schedule)
            }
        } else {
            self.genesis_stake.to_token_distribution_schedule()
        }
    }

    fn build_and_cache_unsigned_genesis(&mut self) {
        // Verify that all input data is valid
        self.validate_inputs().unwrap();

        self.load_migration_sources()
            .expect("migration sources should be loaded without errors");

        self.create_and_cache_genesis_stake()
            .expect("genesis stake should be created without errors");

        // Get the vanilla token distribution schedule or merge it with genesis stake
        let token_distribution_schedule = self.resolve_token_distribution_schedule();
        // Verify that token distribution schedule is valid
        token_distribution_schedule.validate();
        token_distribution_schedule
            .check_minimum_stake_for_validators(
                self.validators.values().map(|v| v.info.iota_address()),
            )
            .expect("all validators should have the required stake");

        let objects = self.objects.clone().into_values().collect::<Vec<_>>();

        // Finally build the genesis data
        self.built_genesis = Some(build_unsigned_genesis_data(
            &self.parameters,
            &token_distribution_schedule,
            self.validators.values(),
            objects,
            &mut self.genesis_stake,
            &mut self.migration_objects,
        ));

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
            get_iota_system_state(&objects).expect("Iota System State object must always exist");
        iota_system_object.get_current_epoch_committee().committee
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
            migration_txs_effects,
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
            migration_txs_effects,
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

        let protocol_config = get_genesis_protocol_config(ProtocolVersion::new(protocol_version));

        if protocol_config.create_authenticator_state_in_genesis() {
            let authenticator_state = unsigned_genesis.authenticator_state_object().unwrap();
            assert!(authenticator_state.active_jwks.is_empty());
        } else {
            assert!(unsigned_genesis.authenticator_state_object().is_none());
        }
        assert_eq!(
            protocol_config.random_beacon(),
            unsigned_genesis.has_randomness_state_object()
        );

        assert_eq!(
            protocol_config.enable_coin_deny_list(),
            unsigned_genesis.coin_deny_list_state().is_some(),
        );

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
            assert_eq!(validator.info.protocol_key(), metadata.iota_pubkey_bytes());
            assert_eq!(validator.info.network_key, metadata.network_pubkey);
            assert_eq!(validator.info.worker_key, metadata.worker_pubkey);
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
            assert_eq!(
                validator.info.narwhal_primary_address,
                metadata.primary_address
            );
            assert_eq!(
                validator.info.narwhal_worker_address,
                metadata.worker_address
            );

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

        let mut gas_objects: BTreeMap<ObjectID, (&Object, GasCoin)> = unsigned_genesis
            .objects()
            .iter()
            .filter_map(|o| GasCoin::try_from(o).ok().map(|g| (o.id(), (o, g))))
            .collect();
        let mut staked_iota_objects: BTreeMap<ObjectID, (&Object, StakedIota)> = unsigned_genesis
            .objects()
            .iter()
            .filter_map(|o| StakedIota::try_from(o).ok().map(|s| (o.id(), (o, s))))
            .collect();
        let mut timelock_staked_iota_objects: BTreeMap<ObjectID, (&Object, TimelockedStakedIota)> =
            unsigned_genesis
                .objects()
                .iter()
                .filter_map(|o| {
                    TimelockedStakedIota::try_from(o)
                        .ok()
                        .map(|s| (o.id(), (o, s)))
                })
                .collect();

        for allocation in token_distribution_schedule.allocations {
            if let Some(staked_with_validator) = allocation.staked_with_validator {
                let staking_pool_id = *address_to_pool_id
                    .get(&staked_with_validator)
                    .expect("staking pool should exist");
                if let Some(expiration) = allocation.staked_with_timelock_expiration {
                    let timelock_staked_iota_object_id = timelock_staked_iota_objects
                        .iter()
                        .find(|(_k, (o, s))| {
                            let Owner::AddressOwner(owner) = &o.owner else {
                                panic!("gas object owner must be address owner");
                            };
                            *owner == allocation.recipient_address
                                && s.principal() == allocation.amount_nanos
                                && s.pool_id() == staking_pool_id
                                && s.expiration_timestamp_ms() == expiration
                        })
                        .map(|(k, _)| *k)
                        .expect("all allocations should be present");
                    let timelock_staked_iota_object = timelock_staked_iota_objects
                        .remove(&timelock_staked_iota_object_id)
                        .unwrap();
                    assert_eq!(
                        timelock_staked_iota_object.0.owner,
                        Owner::AddressOwner(allocation.recipient_address)
                    );
                    assert_eq!(
                        timelock_staked_iota_object.1.principal(),
                        allocation.amount_nanos
                    );
                    assert_eq!(timelock_staked_iota_object.1.pool_id(), staking_pool_id);
                    assert_eq!(timelock_staked_iota_object.1.activation_epoch(), 0);
                } else {
                    let staked_iota_object_id = staked_iota_objects
                        .iter()
                        .find(|(_k, (o, s))| {
                            let Owner::AddressOwner(owner) = &o.owner else {
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
                        Owner::AddressOwner(allocation.recipient_address)
                    );
                    assert_eq!(staked_iota_object.1.principal(), allocation.amount_nanos);
                    assert_eq!(staked_iota_object.1.pool_id(), staking_pool_id);
                    assert_eq!(staked_iota_object.1.activation_epoch(), 0);
                }
            } else {
                let gas_object_id = gas_objects
                    .iter()
                    .find(|(_k, (o, g))| {
                        if let Owner::AddressOwner(owner) = &o.owner {
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
                    Owner::AddressOwner(allocation.recipient_address)
                );
                assert_eq!(gas_object.1.value(), allocation.amount_nanos,);
            }
        }

        // All Gas and staked objects should be accounted for
        if !self.parameters.allow_insertion_of_extra_objects {
            assert!(gas_objects.is_empty());
            assert!(staked_iota_objects.is_empty());
        }

        let committee = system_state.get_current_epoch_committee().committee;
        for signature in self.signatures.values() {
            if !self.validators.contains_key(&signature.authority) {
                panic!("found signature for unknown validator: {:#?}", signature);
            }

            signature
                .verify_secure(
                    unsigned_genesis.checkpoint(),
                    Intent::iota_app(IntentScope::CheckpointSummary),
                    &committee,
                )
                .expect("signature should be valid");
        }
    }

    pub async fn load<P: AsRef<Path>>(path: P) -> anyhow::Result<Self, anyhow::Error> {
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

        // Load migration objects if any
        let migration_sources_file = path.join(GENESIS_BUILDER_MIGRATION_SOURCES_FILE);
        let migration_sources: Vec<SnapshotSource> = if migration_sources_file.exists() {
            serde_json::from_slice(
                &fs::read(migration_sources_file)
                    .context("unable to read migration sources file")?,
            )
            .context("unable to deserialize migration sources")?
        } else {
            Default::default()
        };

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
            committee.insert(validator_info.info.protocol_key(), validator_info);
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
            migration_objects: Default::default(),
            genesis_stake: Default::default(),
            migration_sources,
        };

        let unsigned_genesis_file = path.join(GENESIS_BUILDER_UNSIGNED_GENESIS_FILE);
        if unsigned_genesis_file.exists() {
            let reader = BufReader::new(File::open(unsigned_genesis_file)?);
            let loaded_genesis: UnsignedGenesis =
                tokio::task::spawn_blocking(move || bcs::from_reader(reader)).await??;

            // If we have a built genesis, then we must have a token_distribution_schedule
            // present as well.
            assert!(
                builder.token_distribution_schedule.is_some(),
                "If a built genesis is present, then there must also be a token-distribution-schedule present"
            );

            // Verify loaded genesis matches one build from the constituent parts
            builder = tokio::task::spawn_blocking(move || {
                builder.get_or_build_unsigned_genesis();
                builder
            })
            .await?;
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

        if !self.migration_sources.is_empty() {
            let file = path.join(GENESIS_BUILDER_MIGRATION_SOURCES_FILE);
            fs::write(file, serde_json::to_string(&self.migration_sources)?)?;
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
) -> TxContext {
    let mut hasher = DefaultHash::default();
    hasher.update(b"iota-genesis");
    hasher.update(&bcs::to_bytes(genesis_chain_parameters).unwrap());
    hasher.update(&bcs::to_bytes(genesis_validators).unwrap());
    hasher.update(&bcs::to_bytes(token_distribution_schedule).unwrap());
    for system_package in system_packages {
        hasher.update(&bcs::to_bytes(system_package.bytes()).unwrap());
    }

    let hash = hasher.finalize();
    let genesis_transaction_digest = TransactionDigest::new(hash.into());

    TxContext::new(
        &IotaAddress::default(),
        &genesis_transaction_digest,
        epoch_data,
    )
}

fn get_genesis_protocol_config(version: ProtocolVersion) -> ProtocolConfig {
    // We have a circular dependency here. Protocol config depends on chain ID,
    // which depends on genesis checkpoint (digest), which depends on genesis
    // transaction, which depends on protocol config.
    //
    // ChainIdentifier::default().chain() which can be overridden by the
    // IOTA_PROTOCOL_CONFIG_CHAIN_OVERRIDE if necessary
    ProtocolConfig::get_for_version(version, ChainIdentifier::default().chain())
}

fn build_unsigned_genesis_data<'info>(
    parameters: &GenesisCeremonyParameters,
    token_distribution_schedule: &TokenDistributionSchedule,
    validators: impl Iterator<Item = &'info GenesisValidatorInfo>,
    objects: Vec<Object>,
    genesis_stake: &mut GenesisStake,
    migration_objects: &mut MigrationObjects,
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
    let system_packages =
        iota_framework_snapshot::load_bytecode_snapshot(parameters.protocol_version.as_u64())
            .unwrap_or_else(|_| BuiltInFramework::iter_system_packages().cloned().collect());

    let mut genesis_ctx = create_genesis_context(
        &epoch_data,
        &genesis_chain_parameters,
        &genesis_validators,
        token_distribution_schedule,
        &system_packages,
    );

    // Use a throwaway metrics registry for genesis transaction execution.
    let registry = prometheus::Registry::new();
    let metrics = Arc::new(LimitsMetrics::new(&registry));
    let mut migration_txs_effects = vec![];
    let protocol_config = get_genesis_protocol_config(parameters.protocol_version);

    let (genesis_objects, events) = create_genesis_objects(
        &mut genesis_ctx,
        objects,
        &genesis_validators,
        &genesis_chain_parameters,
        token_distribution_schedule,
        system_packages,
        metrics.clone(),
    );

    if !genesis_stake.is_empty() {
        let migration_objects = create_migration_objects(
            &mut genesis_ctx,
            migration_objects.take_objects(),
            &genesis_objects,
            &genesis_chain_parameters,
            genesis_stake,
            metrics.clone(),
        );

        // Migration transactions covered by genesis tx type
        let (_migration_transaction, migration_effects, _migration_events, _objects) =
            create_genesis_transaction(
                migration_objects,
                vec![],
                &protocol_config,
                metrics.clone(),
                &epoch_data,
            );

        migration_txs_effects.push(migration_effects);
    }

    let (genesis_transaction, genesis_effects, genesis_events, genesis_objects) =
        create_genesis_transaction(
            genesis_objects,
            events,
            &protocol_config,
            metrics,
            &epoch_data,
        );

    let (checkpoint, checkpoint_contents) = create_genesis_checkpoint(
        parameters,
        &genesis_transaction,
        &genesis_effects,
        &migration_txs_effects,
    );

    UnsignedGenesis {
        checkpoint,
        checkpoint_contents,
        transaction: genesis_transaction,
        effects: genesis_effects,
        events: genesis_events,
        objects: genesis_objects,
        migration_txs_effects,
    }
}

fn create_genesis_checkpoint(
    parameters: &GenesisCeremonyParameters,
    transaction: &Transaction,
    effects: &TransactionEffects,
    migration_txs_effects: &[TransactionEffects],
) -> (CheckpointSummary, CheckpointContents) {
    let execution_digests = ExecutionDigests {
        transaction: *transaction.digest(),
        effects: effects.digest(),
    };

    let mut effects_digests = vec![execution_digests];

    let migration_effects_exec_digests: Vec<ExecutionDigests> = migration_txs_effects
        .into_iter()
        .map(|effect| effect.execution_digests())
        .collect();

    effects_digests.extend(migration_effects_exec_digests);
    let effects_digests_len = effects_digests.len();

    let contents = CheckpointContents::new_with_digests_and_signatures(
        effects_digests,
        vec![vec![]; effects_digests_len],
    );
    let checkpoint = CheckpointSummary {
        epoch: 0,
        sequence_number: 0,
        network_total_transactions: contents.size().try_into().unwrap(),
        content_digest: *contents.digest(),
        previous_digest: None,
        epoch_rolling_gas_cost_summary: Default::default(),
        end_of_epoch_data: None,
        timestamp_ms: parameters.chain_start_timestamp_ms,
        version_specific_data: Vec::new(),
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
    Transaction,
    TransactionEffects,
    TransactionEvents,
    Vec<Object>,
) {
    let genesis_transaction = {
        let genesis_objects = objects
            .into_iter()
            .map(|mut object| {
                if let Some(o) = object.data.try_as_move_mut() {
                    o.decrement_version_to(SequenceNumber::MIN);
                }

                if let Owner::Shared {
                    initial_shared_version,
                } = &mut object.owner
                {
                    *initial_shared_version = SequenceNumber::MIN;
                }

                let object = object.into_inner();
                iota_types::transaction::GenesisObject::RawObject {
                    data: object.data,
                    owner: object.owner,
                }
            })
            .collect();

        iota_types::transaction::VerifiedTransaction::new_genesis_transaction(
            genesis_objects,
            events,
        )
        .into_inner()
    };

    let genesis_digest = *genesis_transaction.digest();
    // execute txn to effects
    let (effects, events, objects) = {
        let silent = true;

        let executor = iota_execution::executor(protocol_config, silent, None)
            .expect("Creating an executor should not fail here");

        let expensive_checks = false;
        let certificate_deny_set = HashSet::new();
        let transaction_data = &genesis_transaction.data().intent_message().value;
        let (kind, signer, _) = transaction_data.execution_parts();
        let input_objects = CheckedInputObjects::new_for_genesis(vec![]);
        let (inner_temp_store, _, effects, _execution_error) = executor
            .execute_transaction_to_effects(
                &InMemoryStorage::new(Vec::new()),
                protocol_config,
                metrics,
                expensive_checks,
                &certificate_deny_set,
                &epoch_data.epoch_id(),
                epoch_data.epoch_start_timestamp(),
                input_objects,
                vec![],
                IotaGasStatus::new_unmetered(),
                kind,
                signer,
                genesis_digest,
            );
        assert!(inner_temp_store.input_objects.is_empty());
        assert!(inner_temp_store.mutable_inputs.is_empty());
        assert!(effects.mutated().is_empty());
        assert!(effects.unwrapped().is_empty());
        assert!(effects.deleted().is_empty());
        assert!(effects.wrapped().is_empty());
        assert!(effects.unwrapped_then_deleted().is_empty());

        let objects = inner_temp_store.written.into_values().collect();
        (effects, inner_temp_store.events, objects)
    };

    (genesis_transaction, effects, events, objects)
}

fn create_genesis_objects(
    genesis_ctx: &mut TxContext,
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
    // checkpoint. However since we know there are no chain specific protool
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
            genesis_ctx,
            &system_package.modules(),
            system_package.dependencies().to_vec(),
            &protocol_config,
            metrics.clone(),
        )
        .expect("Processing a package should not fail here");

        events.extend(tx_events.data.into_iter());
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
    ctx: &mut TxContext,
    modules: &[CompiledModule],
    dependencies: Vec<ObjectID>,
    protocol_config: &ProtocolConfig,
    metrics: Arc<LimitsMetrics>,
) -> anyhow::Result<TransactionEvents> {
    let dependency_objects = store.get_objects(&dependencies);
    // When publishing genesis packages, since the std framework packages all have
    // non-zero addresses, [`Transaction::input_objects_in_compiled_modules`] will
    // consider them as dependencies even though they are not. Hence
    // input_objects contain objects that don't exist on-chain because they are
    // yet to be published.
    #[cfg(debug_assertions)]
    {
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
                    || to_be_published_addresses.contains(&AccountAddress::from(*dependency)))
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
            m.serialize(&mut buf).unwrap();
            buf
        })
        .collect();
    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();
        // executing in Genesis mode does not create an `UpgradeCap`.
        builder.command(Command::Publish(module_bytes, dependencies));
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
    genesis_ctx: &mut TxContext,
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
            IOTA_FRAMEWORK_PACKAGE_ID,
            ident_str!("object").to_owned(),
            ident_str!("iota_system_state").to_owned(),
            vec![],
            vec![],
        );

        // Step 2: Create and share the Clock.
        builder.move_call(
            IOTA_FRAMEWORK_PACKAGE_ID,
            ident_str!("clock").to_owned(),
            ident_str!("create").to_owned(),
            vec![],
            vec![],
        )?;

        // Step 3: Create ProtocolConfig-controlled system objects, unless disabled
        // (which only happens in tests).
        if protocol_config.create_authenticator_state_in_genesis() {
            builder.move_call(
                IOTA_FRAMEWORK_PACKAGE_ID,
                ident_str!("authenticator_state").to_owned(),
                ident_str!("create").to_owned(),
                vec![],
                vec![],
            )?;
        }
        if protocol_config.random_beacon() {
            builder.move_call(
                IOTA_FRAMEWORK_PACKAGE_ID,
                ident_str!("random").to_owned(),
                ident_str!("create").to_owned(),
                vec![],
                vec![],
            )?;
        }
        if protocol_config.enable_coin_deny_list() {
            builder.move_call(
                IOTA_FRAMEWORK_PACKAGE_ID,
                DENY_LIST_MODULE.to_owned(),
                DENY_LIST_CREATE_FUNC.to_owned(),
                vec![],
                vec![],
            )?;
        }

        // Step 4: Create the IOTA Coin Treasury Cap.
        let iota_treasury_cap = builder.programmable_move_call(
            IOTA_FRAMEWORK_PACKAGE_ID,
            ident_str!("iota").to_owned(),
            ident_str!("new").to_owned(),
            vec![],
            vec![],
        );

        let pre_minted_supply_amount = builder
            .pure(token_distribution_schedule.pre_minted_supply)
            .expect("serialization of u64 should succeed");
        let pre_minted_supply = builder.programmable_move_call(
            IOTA_FRAMEWORK_PACKAGE_ID,
            ident_str!("iota").to_owned(),
            ident_str!("mint_balance").to_owned(),
            vec![],
            vec![iota_treasury_cap, pre_minted_supply_amount],
        );

        builder.programmable_move_call(
            IOTA_FRAMEWORK_PACKAGE_ID,
            BALANCE_MODULE_NAME.to_owned(),
            ident_str!("destroy_genesis_supply").to_owned(),
            vec![GAS::type_tag()],
            vec![pre_minted_supply],
        );

        // Step 5: Create System Timelock Cap.
        let system_timelock_cap = builder.programmable_move_call(
            IOTA_FRAMEWORK_PACKAGE_ID,
            ident_str!("timelock").to_owned(),
            ident_str!("new_system_timelock_cap").to_owned(),
            vec![],
            vec![],
        );

        // Step 6: Run genesis.
        // The first argument is the system state uid we got from step 1 and the second
        // one is the IOTA `TreasuryCap` we got from step 4.
        let mut arguments = vec![iota_system_state_uid, iota_treasury_cap];
        let mut call_arg_arguments = vec![
            CallArg::Pure(bcs::to_bytes(&genesis_chain_parameters).unwrap()),
            CallArg::Pure(bcs::to_bytes(&genesis_validators).unwrap()),
            CallArg::Pure(bcs::to_bytes(&token_distribution_schedule).unwrap()),
            CallArg::Pure(bcs::to_bytes(&Some(STARDUST_UPGRADE_LABEL_VALUE)).unwrap()),
        ]
        .into_iter()
        .map(|a| builder.input(a))
        .collect::<anyhow::Result<_, _>>()?;
        arguments.append(&mut call_arg_arguments);
        arguments.push(system_timelock_cap);
        builder.programmable_move_call(
            IOTA_SYSTEM_ADDRESS.into(),
            ident_str!("genesis").to_owned(),
            ident_str!("create").to_owned(),
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
        let object = written.get_mut(&iota_types::IOTA_CLOCK_OBJECT_ID).unwrap();
        object
            .data
            .try_as_move_mut()
            .unwrap()
            .set_clock_timestamp_ms_unsafe(genesis_chain_parameters.chain_start_timestamp_ms);
    }

    store.finish(written);

    Ok(())
}

fn create_migration_objects(
    genesis_ctx: &mut TxContext,
    input_objects: Vec<Object>,
    genesis_objects: &Vec<Object>,
    parameters: &GenesisChainParameters,
    genesis_stake: &mut GenesisStake,
    metrics: Arc<LimitsMetrics>,
) -> Vec<Object> {
    // create the temporary store and the executor
    let mut store = InMemoryStorage::new(genesis_objects.clone());
    let protocol_config = ProtocolConfig::get_for_version(
        ProtocolVersion::new(parameters.protocol_version),
        Chain::Unknown,
    );
    let silent = true;
    let executor = iota_execution::executor(&protocol_config, silent, None)
        .expect("Creating an executor should not fail here");

    // inputs are all the migration objects
    for object in input_objects {
        store.insert_object(object);
    }

    // First operation: split timelock objects that needs it
    split_timelocks(
        &mut store,
        executor.as_ref(),
        genesis_ctx,
        parameters,
        &genesis_stake.take_timelocks_to_split(),
        metrics.clone(),
    )
    .expect("Splitting timelocks should not fail here");

    // Extract objects from the store
    let mut intermediate_store = store.into_inner();

    // Second operation: burn gas and timelocks objects.
    // If the genesis stake was created, then burn gas and timelock objects that
    // were added to the token distribution schedule, because they will be
    // created on the Move side during genesis. That means we need to prevent
    // cloning value by evicting these here.
    for (id, _, _) in genesis_stake.take_gas_coins_to_burn() {
        intermediate_store.remove(&id);
    }
    for (id, _, _) in genesis_stake.take_timelocks_to_burn() {
        intermediate_store.remove(&id);
    }

    // Clean the intermediate store from objects already present in genesis_objects
    for genesis_object in genesis_objects.into_iter() {
        intermediate_store.remove(&genesis_object.id());
    }

    intermediate_store.into_values().collect()
}

pub fn split_timelocks(
    store: &mut InMemoryStorage,
    executor: &dyn Executor,
    genesis_ctx: &mut TxContext,
    genesis_chain_parameters: &GenesisChainParameters,
    timelocks_to_split: &[(ObjectRef, u64, IotaAddress)],
    metrics: Arc<LimitsMetrics>,
) -> anyhow::Result<()> {
    let protocol_config = ProtocolConfig::get_for_version(
        ProtocolVersion::new(genesis_chain_parameters.protocol_version),
        ChainIdentifier::default().chain(),
    );

    // Timelock split
    let mut timelock_split_input_objects: Vec<ObjectReadResult> = vec![];
    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();
        for (timelock, surplus_amount, recipient) in timelocks_to_split {
            timelock_split_input_objects.push(ObjectReadResult::new(
                InputObjectKind::ImmOrOwnedMoveObject(*timelock),
                store.get_object(&timelock.0).unwrap().clone().into(),
            ));
            let arguments = vec![
                builder.obj(ObjectArg::ImmOrOwnedObject(*timelock))?,
                builder.pure(surplus_amount)?,
            ];
            let surplus_timelock = builder.programmable_move_call(
                IOTA_FRAMEWORK_PACKAGE_ID,
                ident_str!("timelock").to_owned(),
                ident_str!("split").to_owned(),
                vec![GAS::type_tag()],
                arguments,
            );
            let arguments = vec![surplus_timelock, builder.pure(*recipient)?];
            builder.programmable_move_call(
                IOTA_FRAMEWORK_PACKAGE_ID,
                ident_str!("timelock").to_owned(),
                ident_str!("transfer").to_owned(),
                vec![Balance::type_tag(GAS::type_tag())],
                arguments,
            );
        }
        builder.finish()
    };

    let InnerTemporaryStore { written, .. } = executor.update_genesis_state(
        &*store,
        &protocol_config,
        metrics,
        genesis_ctx,
        CheckedInputObjects::new_for_genesis(timelock_split_input_objects),
        pt,
    )?;

    store.finish(written);

    Ok(())
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum SnapshotSource {
    /// Local uncompressed file.
    Local(PathBuf),
    /// Remote file (S3) with gzip compressed file
    S3(SnapshotUrl),
}

impl SnapshotSource {
    /// Convert to a reader.
    pub fn to_reader(&self) -> anyhow::Result<Box<dyn Read>> {
        Ok(match self {
            SnapshotSource::Local(path) => Box::new(BufReader::new(File::open(path)?)),
            SnapshotSource::S3(snapshot_url) => Box::new(snapshot_url.to_reader()?),
        })
    }
}

impl From<SnapshotUrl> for SnapshotSource {
    fn from(value: SnapshotUrl) -> Self {
        Self::S3(value)
    }
}

/// The URLs to download Iota or Shimmer object snapshots.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum SnapshotUrl {
    Iota,
    Shimmer,
    /// Custom migration snapshot for testing purposes.
    Test(Url),
}

impl std::fmt::Display for SnapshotUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SnapshotUrl::Iota => "iota".fmt(f),
            SnapshotUrl::Shimmer => "smr".fmt(f),
            SnapshotUrl::Test(url) => url.as_str().fmt(f),
        }
    }
}

impl FromStr for SnapshotUrl {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(url) = reqwest::Url::from_str(s) {
            return Ok(Self::Test(url));
        }
        Ok(match s.to_lowercase().as_str() {
            "iota" => Self::Iota,
            "smr" | "shimmer" => Self::Shimmer,
            e => bail!("unsupported snapshot url: {e}"),
        })
    }
}

impl SnapshotUrl {
    /// Returns the Iota or Shimmer object snapshot download URL.
    pub fn to_url(&self) -> Url {
        match self {
            Self::Iota => Url::parse(IOTA_OBJECT_SNAPSHOT_URL).expect("should be valid URL"),
            Self::Shimmer => Url::parse(SHIMMER_OBJECT_SNAPSHOT_URL).expect("should be valid URL"),
            Self::Test(url) => url.clone(),
        }
    }

    /// Convert a gzip decoder to read the compressed object snapshot from S3.
    pub fn to_reader(&self) -> anyhow::Result<impl Read> {
        Ok(GzDecoder::new(BufReader::new(reqwest::blocking::get(
            self.to_url(),
        )?)))
    }
}

#[cfg(test)]
mod test {
    use fastcrypto::traits::KeyPair;
    use iota_config::{
        genesis::*,
        local_ip_utils,
        node::{DEFAULT_COMMISSION_RATE, DEFAULT_VALIDATOR_GAS_PRICE},
    };
    use iota_types::{
        base_types::IotaAddress,
        crypto::{
            generate_proof_of_possession, get_key_pair_from_rng, AccountKeyPair, AuthorityKeyPair,
            NetworkKeyPair,
        },
    };

    use crate::{validator_info::ValidatorInfo, Builder};

    #[test]
    fn allocation_csv() {
        let schedule = TokenDistributionSchedule::new_for_validators_with_default_allocation([
            IotaAddress::random_for_testing_only(),
            IotaAddress::random_for_testing_only(),
        ]);
        let mut output = Vec::new();

        schedule.to_csv(&mut output).unwrap();

        let parsed_schedule = TokenDistributionSchedule::from_csv(output.as_slice()).unwrap();

        assert_eq!(schedule, parsed_schedule);

        std::io::Write::write_all(&mut std::io::stdout(), &output).unwrap();
    }

    #[tokio::test]
    #[cfg_attr(msim, ignore)]
    async fn ceremony() {
        let dir = tempfile::TempDir::new().unwrap();

        let key: AuthorityKeyPair = get_key_pair_from_rng(&mut rand::rngs::OsRng).1;
        let worker_key: NetworkKeyPair = get_key_pair_from_rng(&mut rand::rngs::OsRng).1;
        let account_key: AccountKeyPair = get_key_pair_from_rng(&mut rand::rngs::OsRng).1;
        let network_key: NetworkKeyPair = get_key_pair_from_rng(&mut rand::rngs::OsRng).1;
        let validator = ValidatorInfo {
            name: "0".into(),
            protocol_key: key.public().into(),
            worker_key: worker_key.public().clone(),
            account_address: IotaAddress::from(account_key.public()),
            network_key: network_key.public().clone(),
            gas_price: DEFAULT_VALIDATOR_GAS_PRICE,
            commission_rate: DEFAULT_COMMISSION_RATE,
            network_address: local_ip_utils::new_local_tcp_address_for_testing(),
            p2p_address: local_ip_utils::new_local_udp_address_for_testing(),
            narwhal_primary_address: local_ip_utils::new_local_udp_address_for_testing(),
            narwhal_worker_address: local_ip_utils::new_local_udp_address_for_testing(),
            description: String::new(),
            image_url: String::new(),
            project_url: String::new(),
        };
        let pop = generate_proof_of_possession(&key, account_key.public().into());
        let mut builder = Builder::new().add_validator(validator, pop);

        let genesis = builder.get_or_build_unsigned_genesis();
        for object in genesis.objects() {
            println!("ObjectID: {} Type: {:?}", object.id(), object.type_());
        }
        builder.save(dir.path()).unwrap();
        Builder::load(dir.path()).await.unwrap();
    }
}
