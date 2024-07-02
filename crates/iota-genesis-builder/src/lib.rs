// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, HashSet},
    fs::{self, File},
    io::{BufReader, BufWriter},
    path::Path,
    sync::Arc,
};

use anyhow::{bail, Context};
use camino::Utf8Path;
use fastcrypto::{hash::HashFunction, traits::KeyPair};
use iota_config::genesis::{
    Genesis, GenesisCeremonyParameters, GenesisChainParameters, TimelockAllocation,
    TokenDistributionSchedule, UnsignedGenesis,
};
use iota_execution::{self, Executor};
use iota_framework::{BuiltInFramework, SystemPackage};
use iota_protocol_config::{Chain, ProtocolConfig, ProtocolVersion};
use iota_sdk::types::block::address::Address;
use iota_types::{
    balance::Balance,
    base_types::{
        ExecutionDigests, IotaAddress, ObjectID, SequenceNumber, TransactionDigest, TxContext,
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
    transaction::{
        CallArg, CheckedInputObjects, Command, InputObjectKind, ObjectArg, ObjectReadResult,
        Transaction,
    },
    IOTA_FRAMEWORK_ADDRESS, IOTA_SYSTEM_ADDRESS, IOTA_SYSTEM_STATE_OBJECT_ID,
    IOTA_SYSTEM_STATE_OBJECT_SHARED_VERSION, TIMELOCK_ADDRESS,
};
use move_binary_format::CompiledModule;
use move_core_types::ident_str;
use shared_crypto::intent::{Intent, IntentMessage, IntentScope};
use stake::{delegate_genesis_stake, GenesisStake};
use stardust::{migration::MigrationObjects, types::stardust_to_iota_address};
use tracing::trace;
use validator_info::{GenesisValidatorInfo, GenesisValidatorMetadata, ValidatorInfo};

mod stake;
pub mod stardust;
pub mod validator_info;

// TODO: Lazy static `stardust_to_iota_address`
const IF_STARDUST_ADDRESS: &str =
    "iota1qp8h9augeh6tk3uvlxqfapuwv93atv63eqkpru029p6sgvr49eufyz7katr";

const GENESIS_BUILDER_COMMITTEE_DIR: &str = "committee";
const GENESIS_BUILDER_PARAMETERS_FILE: &str = "parameters";
const GENESIS_BUILDER_TOKEN_DISTRIBUTION_SCHEDULE_FILE: &str = "token-distribution-schedule";
const GENESIS_BUILDER_SIGNATURE_DIR: &str = "signatures";
const GENESIS_BUILDER_UNSIGNED_GENESIS_FILE: &str = "unsigned-genesis";

pub const BROTLI_COMPRESSOR_BUFFER_SIZE: usize = 4096;
pub const BROTLI_COMPRESSOR_QUALITY: u32 = 11; // Compression levels go from 0 to 11, where 11 has the highest compression ratio but requires more time
pub const BROTLI_COMPRESSOR_LG_WINDOW_SIZE: u32 = 22; // set LZ77 window size (0, 10-24) where bigger windows size improves density

pub const OBJECT_SNAPSHOT_FILE_PATH: &str = "stardust_object_snapshot.bin";

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

    pub fn with_token_distribution_schedule(
        mut self,
        token_distribution_schedule: TokenDistributionSchedule,
    ) -> Self {
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

    /// Utility to load Stardust Migration Objects into the builder from a
    /// snapshot file
    pub fn load_stardust_migration_objects(
        mut self,
        snapshot: impl AsRef<Path>,
    ) -> anyhow::Result<Self> {
        self.migration_objects = MigrationObjects::new(bcs::from_reader(
            brotli::Decompressor::new(File::open(snapshot)?, BROTLI_COMPRESSOR_BUFFER_SIZE),
        )?);
        Ok(self)
    }

    pub fn unsigned_genesis_checkpoint(&self) -> Option<UnsignedGenesis> {
        self.built_genesis.clone()
    }

    fn build_and_cache_unsigned_genesis(&mut self) {
        // Verify that all input data is valid
        self.validate_inputs().unwrap();
        let validators = self.validators.clone().into_values().collect::<Vec<_>>();

        // If not vanilla then create genesis_stake
        if !self.migration_objects.is_empty() {
            let delegator =
                stardust_to_iota_address(Address::try_from_bech32(IF_STARDUST_ADDRESS).unwrap())
                    .unwrap();
            // TODO: check whether we need to start with
            // VALIDATOR_LOW_STAKE_THRESHOLD_MICROS
            let minimum_stake = iota_types::governance::MIN_VALIDATOR_JOINING_STAKE_MICROS;
            self.genesis_stake = delegate_genesis_stake(
                &validators,
                delegator,
                &self.migration_objects,
                minimum_stake,
            )
            .unwrap();
        }

        // Verify that token distribution schedule is valid
        self.validate_token_distribution_schedule().unwrap();
        // Get the vanilla token distribution schedule or merge it with genesis stake
        let token_distribution_schedule = if self.is_vanilla() {
            if let Some(token_distribution_schedule) = &self.token_distribution_schedule {
                token_distribution_schedule.clone()
            } else {
                TokenDistributionSchedule::new_for_validators_with_default_allocation(
                    validators.iter().map(|v| v.info.iota_address()),
                )
            }
        } else if let Some(token_distribution_schedule) = &self.token_distribution_schedule {
            self.genesis_stake
                .extend_vanilla_token_distribution_schedule(token_distribution_schedule.clone())
        } else {
            self.genesis_stake.to_token_distribution_schedule()
        };

        // If the genesis stake was created, then burn gas objects that were added to
        // the token distribution schedule, because they will be created on the
        // Move side during genesis. That means we need to prevent from being
        // part of the initial genesis objects by evicting them here.
        self.migration_objects.evict(
            self.genesis_stake
                .take_gas_coins_to_burn()
                .into_iter()
                .map(|(id, _, _)| id),
        );
        let mut objects = self.migration_objects.take_objects();
        objects.extend(self.objects.values().cloned());

        // Use stake_subsidy_start_epoch to mimic the burn of newly minted IOTA coins
        if !self.is_vanilla() {
            // Because we set the `stake_subsidy_fund` as non-zero
            // we need to effectively disable subsidy rewards
            // to avoid the respective inflation effects.
            //
            // TODO: Handle properly during new tokenomics
            // implementation.
            self.parameters.stake_subsidy_start_epoch = u64::MAX;
        }

        // Finally build the genesis data
        self.built_genesis = Some(build_unsigned_genesis_data(
            &self.parameters,
            &token_distribution_schedule,
            &self.genesis_stake.take_timelock_allocations(),
            &validators,
            objects,
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

        let genesis = Genesis::new(
            checkpoint,
            checkpoint_contents,
            transaction,
            effects,
            events,
            objects,
        );

        // Verify that all on-chain state was properly created
        self.validate().unwrap();

        genesis
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
            token_distribution_schedule.check_all_stake_operations_are_for_valid_validators(
                self.validators.values().map(|v| v.info.iota_address()),
                (!self.is_vanilla())
                    .then(|| {
                        self.genesis_stake
                            .timelock_allocations()
                            .iter()
                            .map(|allocation| {
                                (allocation.staked_with_validator, allocation.amount_nanos)
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
            );
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
            stake_subsidy_start_epoch,
            stake_subsidy_initial_distribution_amount,
            stake_subsidy_period_length,
            stake_subsidy_decrease_rate,
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
            system_state.parameters.stake_subsidy_start_epoch,
            stake_subsidy_start_epoch,
        );
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

        assert_eq!(system_state.stake_subsidy.distribution_counter, 0);
        assert_eq!(
            system_state.stake_subsidy.current_distribution_amount,
            stake_subsidy_initial_distribution_amount,
        );
        assert_eq!(
            system_state.stake_subsidy.stake_subsidy_period_length,
            stake_subsidy_period_length,
        );
        assert_eq!(
            system_state.stake_subsidy.stake_subsidy_decrease_rate,
            stake_subsidy_decrease_rate,
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
        assert_eq!(
            system_state.stake_subsidy.balance.value(),
            token_distribution_schedule.stake_subsidy_fund_nanos
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

        for allocation in token_distribution_schedule.allocations {
            if let Some(staked_with_validator) = allocation.staked_with_validator {
                let staking_pool_id = *address_to_pool_id
                    .get(&staked_with_validator)
                    .expect("staking pool should exist");
                let staked_iota_object_id = staked_iota_objects
                    .iter()
                    .find(|(_k, (o, s))| {
                        let Owner::AddressOwner(owner) = &o.owner else {
                            panic!("gas object owner must be address owner");
                        };
                        *owner == allocation.recipient_address
                            && s.principal() == allocation.amount_micros
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
                assert_eq!(staked_iota_object.1.principal(), allocation.amount_micros);
                assert_eq!(staked_iota_object.1.pool_id(), staking_pool_id);
                assert_eq!(staked_iota_object.1.activation_epoch(), 0);
            } else {
                let gas_object_id = gas_objects
                    .iter()
                    .find(|(_k, (o, g))| {
                        if let Owner::AddressOwner(owner) = &o.owner {
                            *owner == allocation.recipient_address
                                && g.value() == allocation.amount_micros
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
                assert_eq!(gas_object.1.value(), allocation.amount_micros,);
            }
        }

        // All Gas and staked objects should be accounted for
        if !self.parameters.allow_insertion_of_extra_objects {
            assert!(gas_objects.is_empty());
            assert!(staked_iota_objects.is_empty());
        }

        let committee = system_state.get_current_epoch_committee().committee;
        for signature in self.signatures.values() {
            if self.validators.get(&signature.authority).is_none() {
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

    pub fn load<P: AsRef<Path>>(path: P) -> anyhow::Result<Self, anyhow::Error> {
        let path = path.as_ref();
        let path: &Utf8Path = path.try_into()?;
        trace!("Reading Genesis Builder from {}", path);

        if !path.is_dir() {
            bail!("path must be a directory");
        }

        // Load parameters
        let parameters_file = path.join(GENESIS_BUILDER_PARAMETERS_FILE);
        let parameters = serde_yaml::from_slice(
            &fs::read(parameters_file).context("unable to read genesis parameters file")?,
        )
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
                .with_context(|| format!("unable to load validator signatrue for {path}"))?;
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
        };

        let unsigned_genesis_file = path.join(GENESIS_BUILDER_UNSIGNED_GENESIS_FILE);
        if unsigned_genesis_file.exists() {
            let loaded_genesis: UnsignedGenesis =
                bcs::from_reader(BufReader::new(File::open(unsigned_genesis_file)?))?;

            // If we have a built genesis, then we must have a token_distribution_schedule
            // present as well.
            assert!(
                builder.token_distribution_schedule.is_some(),
                "If a built genesis is present, then there must also be a token-distribution-schedule present"
            );

            // Verify loaded genesis matches one build from the constituent parts
            let built = builder.get_or_build_unsigned_genesis();
            loaded_genesis.checkpoint_contents.digest(); // cache digest before compare
            assert!(
                *built == loaded_genesis,
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

fn build_unsigned_genesis_data(
    parameters: &GenesisCeremonyParameters,
    token_distribution_schedule: &TokenDistributionSchedule,
    timelock_allocations: &[TimelockAllocation],
    validators: &[GenesisValidatorInfo],
    objects: Vec<Object>,
) -> UnsignedGenesis {
    if !parameters.allow_insertion_of_extra_objects && !objects.is_empty() {
        panic!(
            "insertion of extra objects at genesis time is prohibited due to 'allow_insertion_of_extra_objects' parameter"
        );
    }

    let genesis_chain_parameters = parameters.to_genesis_chain_parameters();
    let genesis_validators = validators
        .iter()
        .cloned()
        .map(GenesisValidatorMetadata::from)
        .collect::<Vec<_>>();

    token_distribution_schedule.validate();
    token_distribution_schedule.check_all_stake_operations_are_for_valid_validators(
        genesis_validators.iter().map(|v| v.iota_address),
        (!timelock_allocations.is_empty())
            .then(|| {
                timelock_allocations
                    .iter()
                    .map(|allocation| (allocation.staked_with_validator, allocation.amount_nanos))
                    .collect()
            })
            .unwrap_or_default(),
    );

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

    let objects = create_genesis_objects(
        &mut genesis_ctx,
        objects,
        &genesis_validators,
        &genesis_chain_parameters,
        token_distribution_schedule,
        timelock_allocations,
        system_packages,
        metrics.clone(),
    );

    let protocol_config = get_genesis_protocol_config(parameters.protocol_version);

    let (genesis_transaction, genesis_effects, genesis_events, objects) =
        create_genesis_transaction(objects, &protocol_config, metrics, &epoch_data);
    let (checkpoint, checkpoint_contents) =
        create_genesis_checkpoint(parameters, &genesis_transaction, &genesis_effects);

    UnsignedGenesis {
        checkpoint,
        checkpoint_contents,
        transaction: genesis_transaction,
        effects: genesis_effects,
        events: genesis_events,
        objects,
    }
}

fn create_genesis_checkpoint(
    parameters: &GenesisCeremonyParameters,
    transaction: &Transaction,
    effects: &TransactionEffects,
) -> (CheckpointSummary, CheckpointContents) {
    let execution_digests = ExecutionDigests {
        transaction: *transaction.digest(),
        effects: effects.digest(),
    };
    let contents =
        CheckpointContents::new_with_digests_and_signatures([execution_digests], vec![vec![]]);
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

        iota_types::transaction::VerifiedTransaction::new_genesis_transaction(genesis_objects)
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
    timelock_allocations: &[TimelockAllocation],
    system_packages: Vec<SystemPackage>,
    metrics: Arc<LimitsMetrics>,
) -> Vec<Object> {
    let mut store = InMemoryStorage::new(Vec::new());
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
        process_package(
            &mut store,
            executor.as_ref(),
            genesis_ctx,
            &system_package.modules(),
            system_package.dependencies().to_vec(),
            &protocol_config,
            metrics.clone(),
        )
        .unwrap();
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
        timelock_allocations,
        metrics,
    )
    .unwrap();

    store.into_inner().into_values().collect()
}

pub(crate) fn process_package(
    store: &mut InMemoryStorage,
    executor: &dyn Executor,
    ctx: &mut TxContext,
    modules: &[CompiledModule],
    dependencies: Vec<ObjectID>,
    protocol_config: &ProtocolConfig,
    metrics: Arc<LimitsMetrics>,
) -> anyhow::Result<()> {
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
    let InnerTemporaryStore { written, .. } = executor.update_genesis_state(
        &*store,
        protocol_config,
        metrics,
        ctx,
        CheckedInputObjects::new_for_genesis(loaded_dependencies),
        pt,
    )?;

    store.finish(written);

    Ok(())
}

pub fn generate_genesis_system_object(
    store: &mut InMemoryStorage,
    executor: &dyn Executor,
    genesis_validators: &[GenesisValidatorMetadata],
    genesis_ctx: &mut TxContext,
    genesis_chain_parameters: &GenesisChainParameters,
    token_distribution_schedule: &TokenDistributionSchedule,
    timelock_allocations: &[TimelockAllocation],
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
            IOTA_FRAMEWORK_ADDRESS.into(),
            ident_str!("object").to_owned(),
            ident_str!("iota_system_state").to_owned(),
            vec![],
            vec![],
        );

        // Step 2: Create and share the Clock.
        builder.move_call(
            IOTA_FRAMEWORK_ADDRESS.into(),
            ident_str!("clock").to_owned(),
            ident_str!("create").to_owned(),
            vec![],
            vec![],
        )?;

        // Step 3: Create ProtocolConfig-controlled system objects, unless disabled
        // (which only happens in tests).
        if protocol_config.create_authenticator_state_in_genesis() {
            builder.move_call(
                IOTA_FRAMEWORK_ADDRESS.into(),
                ident_str!("authenticator_state").to_owned(),
                ident_str!("create").to_owned(),
                vec![],
                vec![],
            )?;
        }
        if protocol_config.random_beacon() {
            builder.move_call(
                IOTA_FRAMEWORK_ADDRESS.into(),
                ident_str!("random").to_owned(),
                ident_str!("create").to_owned(),
                vec![],
                vec![],
            )?;
        }
        if protocol_config.enable_coin_deny_list() {
            builder.move_call(
                IOTA_FRAMEWORK_ADDRESS.into(),
                DENY_LIST_MODULE.to_owned(),
                DENY_LIST_CREATE_FUNC.to_owned(),
                vec![],
                vec![],
            )?;
        }

        // Step 4: Mint the supply of IOTA.
        let iota_supply = builder.programmable_move_call(
            IOTA_FRAMEWORK_ADDRESS.into(),
            ident_str!("iota").to_owned(),
            ident_str!("new").to_owned(),
            vec![],
            vec![],
        );

        // Step 5: Run genesis.
        // The first argument is the system state uid we got from step 1 and the second
        // one is the IOTA supply we got from step 3.
        let mut arguments = vec![iota_system_state_uid, iota_supply];
        let mut call_arg_arguments = vec![
            CallArg::Pure(bcs::to_bytes(&genesis_chain_parameters).unwrap()),
            CallArg::Pure(bcs::to_bytes(&genesis_validators).unwrap()),
            CallArg::Pure(bcs::to_bytes(&token_distribution_schedule).unwrap()),
        ]
        .into_iter()
        .map(|a| builder.input(a))
        .collect::<anyhow::Result<_, _>>()?;
        arguments.append(&mut call_arg_arguments);
        builder.programmable_move_call(
            IOTA_SYSTEM_ADDRESS.into(),
            ident_str!("genesis").to_owned(),
            ident_str!("create").to_owned(),
            vec![],
            arguments,
        );

        // Step 6: Handle the timelock allocations.
        for allocation in timelock_allocations {
            if allocation.surplus_nanos > 0 {
                // Split the surplus amount.
                let timelock = *allocation
                    .timelock_objects
                    .last()
                    .expect("there should be at least two objects");
                let arguments = vec![
                    builder.obj(ObjectArg::ImmOrOwnedObject(timelock))?,
                    builder.pure(allocation.surplus_nanos)?,
                ];
                let surplus_timelock = builder.programmable_move_call(
                    TIMELOCK_ADDRESS.into(),
                    ident_str!("timelocked_balance").to_owned(),
                    ident_str!("split").to_owned(),
                    vec![GAS::type_tag()],
                    arguments,
                );
                builder.programmable_move_call(
                    TIMELOCK_ADDRESS.into(),
                    ident_str!("timelock").to_owned(),
                    ident_str!("self_transfer").to_owned(),
                    vec![Balance::type_tag(GAS::type_tag())],
                    vec![surplus_timelock],
                );
            }
            // Add the stake
            let arguments = vec![
                builder.obj(ObjectArg::SharedObject {
                    id: IOTA_SYSTEM_STATE_OBJECT_ID,
                    initial_shared_version: IOTA_SYSTEM_STATE_OBJECT_SHARED_VERSION,
                    mutable: true,
                })?,
                builder.make_obj_vec(
                    allocation
                        .timelock_objects
                        .iter()
                        .map(|&timelock| ObjectArg::ImmOrOwnedObject(timelock)),
                )?,
                builder.pure(allocation.staked_with_validator)?,
            ];
            let receipt_vector = builder.programmable_move_call(
                TIMELOCK_ADDRESS.into(),
                ident_str!("timelocked_staking").to_owned(),
                ident_str!("request_add_stake_mul_bal_non_entry").to_owned(),
                vec![],
                arguments,
            );
            builder.programmable_move_call(
                TIMELOCK_ADDRESS.into(),
                ident_str!("timelock").to_owned(),
                ident_str!("self_transfer_multiple").to_owned(),
                vec![],
                vec![receipt_vector],
            );
        }
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

    #[test]
    #[cfg_attr(msim, ignore)]
    fn ceremony() {
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
        Builder::load(dir.path()).unwrap();
    }
}
