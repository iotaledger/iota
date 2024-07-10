// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::HashMap,
    fs::File,
    io::{BufReader, BufWriter},
    path::Path,
};

use anyhow::{Context, Result};
use fastcrypto::{
    encoding::{Base64, Encoding},
    hash::HashFunction,
};
use iota_types::{
    authenticator_state::{get_authenticator_state, AuthenticatorStateInner},
    base_types::{IotaAddress, ObjectID},
    clock::Clock,
    committee::{Committee, CommitteeWithNetworkMetadata, EpochId, ProtocolVersion},
    crypto::DefaultHash,
    deny_list::{get_coin_deny_list, PerTypeDenyList},
    effects::{TransactionEffects, TransactionEvents},
    error::IotaResult,
    gas_coin::TOTAL_SUPPLY_NANOS,
    iota_system_state::{
        get_iota_system_state, get_iota_system_state_wrapper, IotaSystemState,
        IotaSystemStateTrait, IotaSystemStateWrapper, IotaValidatorGenesis,
    },
    messages_checkpoint::{
        CertifiedCheckpointSummary, CheckpointContents, CheckpointSummary, VerifiedCheckpoint,
    },
    object::Object,
    storage::ObjectStore,
    transaction::Transaction,
    IOTA_RANDOMNESS_STATE_OBJECT_ID,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use tracing::trace;

#[derive(Clone, Debug)]
pub struct Genesis {
    checkpoint: CertifiedCheckpointSummary,
    checkpoint_contents: CheckpointContents,
    transaction: Transaction,
    effects: TransactionEffects,
    events: TransactionEvents,
    objects: Vec<Object>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct UnsignedGenesis {
    pub checkpoint: CheckpointSummary,
    pub checkpoint_contents: CheckpointContents,
    pub transaction: Transaction,
    pub effects: TransactionEffects,
    pub events: TransactionEvents,
    pub objects: Vec<Object>,
}

// Hand implement PartialEq in order to get around the fact that AuthSigs don't
// impl Eq
impl PartialEq for Genesis {
    fn eq(&self, other: &Self) -> bool {
        self.checkpoint.data() == other.checkpoint.data()
            && {
                let this = self.checkpoint.auth_sig();
                let other = other.checkpoint.auth_sig();

                this.epoch == other.epoch
                    && this.signature.as_ref() == other.signature.as_ref()
                    && this.signers_map == other.signers_map
            }
            && self.checkpoint_contents == other.checkpoint_contents
            && self.transaction == other.transaction
            && self.effects == other.effects
            && self.objects == other.objects
    }
}

impl Eq for Genesis {}

impl Genesis {
    pub fn new(
        checkpoint: CertifiedCheckpointSummary,
        checkpoint_contents: CheckpointContents,
        transaction: Transaction,
        effects: TransactionEffects,
        events: TransactionEvents,
        objects: Vec<Object>,
    ) -> Self {
        Self {
            checkpoint,
            checkpoint_contents,
            transaction,
            effects,
            events,
            objects,
        }
    }

    pub fn objects(&self) -> &[Object] {
        &self.objects
    }

    pub fn object(&self, id: ObjectID) -> Option<Object> {
        self.objects.iter().find(|o| o.id() == id).cloned()
    }

    pub fn transaction(&self) -> &Transaction {
        &self.transaction
    }

    pub fn effects(&self) -> &TransactionEffects {
        &self.effects
    }
    pub fn events(&self) -> &TransactionEvents {
        &self.events
    }

    pub fn checkpoint(&self) -> VerifiedCheckpoint {
        self.checkpoint
            .clone()
            .verify(&self.committee().unwrap())
            .unwrap()
    }

    pub fn checkpoint_contents(&self) -> &CheckpointContents {
        &self.checkpoint_contents
    }

    pub fn epoch(&self) -> EpochId {
        0
    }

    pub fn validator_set_for_tooling(&self) -> Vec<IotaValidatorGenesis> {
        self.iota_system_object()
            .into_genesis_version_for_tooling()
            .validators
            .active_validators
    }

    pub fn committee_with_network(&self) -> CommitteeWithNetworkMetadata {
        self.iota_system_object().get_current_epoch_committee()
    }

    pub fn reference_gas_price(&self) -> u64 {
        self.iota_system_object().reference_gas_price()
    }

    // TODO: No need to return IotaResult.
    pub fn committee(&self) -> IotaResult<Committee> {
        Ok(self.committee_with_network().committee)
    }

    pub fn iota_system_wrapper_object(&self) -> IotaSystemStateWrapper {
        get_iota_system_state_wrapper(&self.objects())
            .expect("Iota System State Wrapper object must always exist")
    }

    pub fn iota_system_object(&self) -> IotaSystemState {
        get_iota_system_state(&self.objects()).expect("Iota System State object must always exist")
    }

    pub fn clock(&self) -> Clock {
        let clock = self
            .objects()
            .iter()
            .find(|o| o.id() == iota_types::IOTA_CLOCK_OBJECT_ID)
            .expect("Clock must always exist")
            .data
            .try_as_move()
            .expect("Clock must be a Move object");
        bcs::from_bytes::<Clock>(clock.contents())
            .expect("Clock object deserialization cannot fail")
    }

    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, anyhow::Error> {
        let path = path.as_ref();
        trace!("Reading Genesis from {}", path.display());
        let read = File::open(path)
            .with_context(|| format!("Unable to load Genesis from {}", path.display()))?;
        bcs::from_reader(BufReader::new(read))
            .with_context(|| format!("Unable to parse Genesis from {}", path.display()))
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), anyhow::Error> {
        let path = path.as_ref();
        trace!("Writing Genesis to {}", path.display());
        let mut write = BufWriter::new(File::create(path)?);
        bcs::serialize_into(&mut write, &self)
            .with_context(|| format!("Unable to save Genesis to {}", path.display()))?;
        Ok(())
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        bcs::to_bytes(self).expect("failed to serialize genesis")
    }

    pub fn hash(&self) -> [u8; 32] {
        use std::io::Write;

        let mut digest = DefaultHash::default();
        digest.write_all(&self.to_bytes()).unwrap();
        let hash = digest.finalize();
        hash.into()
    }
}

impl Serialize for Genesis {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::Error;

        #[derive(Serialize)]
        struct RawGenesis<'a> {
            checkpoint: &'a CertifiedCheckpointSummary,
            checkpoint_contents: &'a CheckpointContents,
            transaction: &'a Transaction,
            effects: &'a TransactionEffects,
            events: &'a TransactionEvents,
            objects: &'a [Object],
        }

        let raw_genesis = RawGenesis {
            checkpoint: &self.checkpoint,
            checkpoint_contents: &self.checkpoint_contents,
            transaction: &self.transaction,
            effects: &self.effects,
            events: &self.events,
            objects: &self.objects,
        };

        if serializer.is_human_readable() {
            let bytes = bcs::to_bytes(&raw_genesis).map_err(|e| Error::custom(e.to_string()))?;
            let s = Base64::encode(bytes);
            serializer.serialize_str(&s)
        } else {
            raw_genesis.serialize(serializer)
        }
    }
}

impl<'de> Deserialize<'de> for Genesis {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error;

        #[derive(Deserialize)]
        struct RawGenesis {
            checkpoint: CertifiedCheckpointSummary,
            checkpoint_contents: CheckpointContents,
            transaction: Transaction,
            effects: TransactionEffects,
            events: TransactionEvents,
            objects: Vec<Object>,
        }

        let raw_genesis = if deserializer.is_human_readable() {
            let s = String::deserialize(deserializer)?;
            let bytes = Base64::decode(&s).map_err(|e| Error::custom(e.to_string()))?;
            bcs::from_bytes(&bytes).map_err(|e| Error::custom(e.to_string()))?
        } else {
            RawGenesis::deserialize(deserializer)?
        };

        Ok(Genesis {
            checkpoint: raw_genesis.checkpoint,
            checkpoint_contents: raw_genesis.checkpoint_contents,
            transaction: raw_genesis.transaction,
            effects: raw_genesis.effects,
            events: raw_genesis.events,
            objects: raw_genesis.objects,
        })
    }
}

impl UnsignedGenesis {
    pub fn objects(&self) -> &[Object] {
        &self.objects
    }

    pub fn object(&self, id: ObjectID) -> Option<Object> {
        self.objects.iter().find(|o| o.id() == id).cloned()
    }

    pub fn transaction(&self) -> &Transaction {
        &self.transaction
    }

    pub fn effects(&self) -> &TransactionEffects {
        &self.effects
    }
    pub fn events(&self) -> &TransactionEvents {
        &self.events
    }

    pub fn checkpoint(&self) -> &CheckpointSummary {
        &self.checkpoint
    }

    pub fn checkpoint_contents(&self) -> &CheckpointContents {
        &self.checkpoint_contents
    }

    pub fn epoch(&self) -> EpochId {
        0
    }

    pub fn iota_system_wrapper_object(&self) -> IotaSystemStateWrapper {
        get_iota_system_state_wrapper(&self.objects())
            .expect("Iota System State Wrapper object must always exist")
    }

    pub fn iota_system_object(&self) -> IotaSystemState {
        get_iota_system_state(&self.objects()).expect("Iota System State object must always exist")
    }

    pub fn authenticator_state_object(&self) -> Option<AuthenticatorStateInner> {
        get_authenticator_state(&self.objects()).expect("read from genesis cannot fail")
    }

    pub fn has_randomness_state_object(&self) -> bool {
        self.objects()
            .get_object(&IOTA_RANDOMNESS_STATE_OBJECT_ID)
            .expect("read from genesis cannot fail")
            .is_some()
    }

    pub fn coin_deny_list_state(&self) -> Option<PerTypeDenyList> {
        get_coin_deny_list(&self.objects())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct GenesisChainParameters {
    pub protocol_version: u64,
    pub chain_start_timestamp_ms: u64,
    pub epoch_duration_ms: u64,

    // Stake Subsidy parameters
    pub stake_subsidy_start_epoch: u64,
    pub stake_subsidy_initial_distribution_amount: u64,
    pub stake_subsidy_period_length: u64,
    pub stake_subsidy_decrease_rate: u16,

    // Validator committee parameters
    pub max_validator_count: u64,
    pub min_validator_joining_stake: u64,
    pub validator_low_stake_threshold: u64,
    pub validator_very_low_stake_threshold: u64,
    pub validator_low_stake_grace_period: u64,
}

/// Initial set of parameters for a chain.
#[derive(Serialize, Deserialize)]
pub struct GenesisCeremonyParameters {
    #[serde(default = "GenesisCeremonyParameters::default_timestamp_ms")]
    pub chain_start_timestamp_ms: u64,

    /// protocol version that the chain starts at.
    #[serde(default = "ProtocolVersion::max")]
    pub protocol_version: ProtocolVersion,

    #[serde(default = "GenesisCeremonyParameters::default_allow_insertion_of_extra_objects")]
    pub allow_insertion_of_extra_objects: bool,

    /// The duration of an epoch, in milliseconds.
    #[serde(default = "GenesisCeremonyParameters::default_epoch_duration_ms")]
    pub epoch_duration_ms: u64,

    /// The starting epoch in which stake subsidies start being paid out.
    #[serde(default)]
    pub stake_subsidy_start_epoch: u64,

    /// The amount of stake subsidy to be drawn down per distribution.
    /// This amount decays and decreases over time.
    #[serde(
        default = "GenesisCeremonyParameters::default_initial_stake_subsidy_distribution_amount"
    )]
    pub stake_subsidy_initial_distribution_amount: u64,

    /// Number of distributions to occur before the distribution amount decays.
    #[serde(default = "GenesisCeremonyParameters::default_stake_subsidy_period_length")]
    pub stake_subsidy_period_length: u64,

    /// The rate at which the distribution amount decays at the end of each
    /// period. Expressed in basis points.
    #[serde(default = "GenesisCeremonyParameters::default_stake_subsidy_decrease_rate")]
    pub stake_subsidy_decrease_rate: u16,
    // Most other parameters (e.g. initial gas schedule) should be derived from protocol_version.
}

impl GenesisCeremonyParameters {
    pub fn new() -> Self {
        Self {
            chain_start_timestamp_ms: Self::default_timestamp_ms(),
            protocol_version: ProtocolVersion::MAX,
            allow_insertion_of_extra_objects: true,
            stake_subsidy_start_epoch: 0,
            epoch_duration_ms: Self::default_epoch_duration_ms(),
            stake_subsidy_initial_distribution_amount:
                Self::default_initial_stake_subsidy_distribution_amount(),
            stake_subsidy_period_length: Self::default_stake_subsidy_period_length(),
            stake_subsidy_decrease_rate: Self::default_stake_subsidy_decrease_rate(),
        }
    }

    fn default_timestamp_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }

    fn default_allow_insertion_of_extra_objects() -> bool {
        true
    }

    fn default_epoch_duration_ms() -> u64 {
        // 24 hrs
        24 * 60 * 60 * 1000
    }

    fn default_initial_stake_subsidy_distribution_amount() -> u64 {
        // 1M Iota
        1_000_000 * iota_types::gas_coin::NANOS_PER_IOTA
    }

    fn default_stake_subsidy_period_length() -> u64 {
        // 10 distributions or epochs
        10
    }

    fn default_stake_subsidy_decrease_rate() -> u16 {
        // 10% in basis points
        1000
    }

    pub fn to_genesis_chain_parameters(&self) -> GenesisChainParameters {
        GenesisChainParameters {
            protocol_version: self.protocol_version.as_u64(),
            stake_subsidy_start_epoch: self.stake_subsidy_start_epoch,
            chain_start_timestamp_ms: self.chain_start_timestamp_ms,
            epoch_duration_ms: self.epoch_duration_ms,
            stake_subsidy_initial_distribution_amount: self
                .stake_subsidy_initial_distribution_amount,
            stake_subsidy_period_length: self.stake_subsidy_period_length,
            stake_subsidy_decrease_rate: self.stake_subsidy_decrease_rate,
            max_validator_count: iota_types::governance::MAX_VALIDATOR_COUNT,
            min_validator_joining_stake: iota_types::governance::MIN_VALIDATOR_JOINING_STAKE_NANOS,
            validator_low_stake_threshold:
                iota_types::governance::VALIDATOR_LOW_STAKE_THRESHOLD_NANOS,
            validator_very_low_stake_threshold:
                iota_types::governance::VALIDATOR_VERY_LOW_STAKE_THRESHOLD_NANOS,
            validator_low_stake_grace_period:
                iota_types::governance::VALIDATOR_LOW_STAKE_GRACE_PERIOD,
        }
    }
}

impl Default for GenesisCeremonyParameters {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct TokenDistributionSchedule {
    pub stake_subsidy_fund_nanos: u64,
    pub allocations: Vec<TokenAllocation>,
}

impl TokenDistributionSchedule {
    pub fn validate(&self) {
        let mut total_nanos = self.stake_subsidy_fund_nanos;

        for allocation in &self.allocations {
            total_nanos += allocation.amount_nanos;
        }

        if total_nanos != TOTAL_SUPPLY_NANOS {
            panic!(
                "TokenDistributionSchedule adds up to {total_nanos} and not expected {TOTAL_SUPPLY_NANOS}"
            );
        }
    }

    pub fn check_all_stake_operations_are_for_valid_validators<
        I: IntoIterator<Item = IotaAddress>,
    >(
        &self,
        validators: I,
    ) {
        let mut validators: HashMap<IotaAddress, u64> =
            validators.into_iter().map(|a| (a, 0)).collect();

        // Check that all allocations are for valid validators, while summing up all
        // allocations for each validator
        for allocation in &self.allocations {
            if let Some(staked_with_validator) = &allocation.staked_with_validator {
                *validators
                    .get_mut(staked_with_validator)
                    .expect("allocation must be staked with valid validator") +=
                    allocation.amount_nanos;
            }
        }

        // Check that all validators have sufficient stake allocated to ensure they meet
        // the minimum stake threshold
        let minimum_required_stake = iota_types::governance::VALIDATOR_LOW_STAKE_THRESHOLD_NANOS;
        for (validator, stake) in validators {
            if stake < minimum_required_stake {
                panic!(
                    "validator {validator} has '{stake}' stake and does not meet the minimum required stake threshold of '{minimum_required_stake}'"
                );
            }
        }
    }

    pub fn new_for_validators_with_default_allocation<I: IntoIterator<Item = IotaAddress>>(
        validators: I,
    ) -> Self {
        let mut supply = TOTAL_SUPPLY_NANOS;
        let default_allocation = iota_types::governance::VALIDATOR_LOW_STAKE_THRESHOLD_NANOS;

        let allocations = validators
            .into_iter()
            .map(|a| {
                supply -= default_allocation;
                TokenAllocation {
                    recipient_address: a,
                    amount_nanos: default_allocation,
                    staked_with_validator: Some(a),
                    staked_with_timelock: vec![],
                }
            })
            .collect();

        let schedule = Self {
            stake_subsidy_fund_nanos: supply,
            allocations,
        };

        schedule.validate();
        schedule
    }

    /// Helper to read a TokenDistributionSchedule from a csv file.
    ///
    /// The file is encoded such that the final entry in the CSV file is used to
    /// denote the allocation to the stake subsidy fund. It must be in the
    /// following format:
    /// `0x0000000000000000000000000000000000000000000000000000000000000000,
    /// <amount to stake subsidy fund>,`
    ///
    /// All entries in a token distribution schedule must add up to 10B Iota.
    pub fn from_csv<R: std::io::Read>(reader: R) -> Result<Self> {
        let mut reader = csv::Reader::from_reader(reader);
        let mut allocations: Vec<TokenAllocation> =
            reader.deserialize().collect::<Result<_, _>>()?;
        assert_eq!(
            TOTAL_SUPPLY_NANOS,
            allocations.iter().map(|a| a.amount_nanos).sum::<u64>(),
            "Token Distribution Schedule must add up to 10B Iota",
        );
        let stake_subsidy_fund_allocation = allocations.pop().unwrap();
        assert_eq!(
            IotaAddress::default(),
            stake_subsidy_fund_allocation.recipient_address,
            "Final allocation must be for stake subsidy fund",
        );
        assert!(
            stake_subsidy_fund_allocation
                .staked_with_validator
                .is_none(),
            "Can't stake the stake subsidy fund",
        );

        let schedule = Self {
            stake_subsidy_fund_nanos: stake_subsidy_fund_allocation.amount_nanos,
            allocations,
        };

        schedule.validate();
        Ok(schedule)
    }

    pub fn to_csv<W: std::io::Write>(&self, writer: W) -> Result<()> {
        let mut writer = csv::Writer::from_writer(writer);

        for allocation in &self.allocations {
            writer.serialize(allocation)?;
        }

        writer.serialize(TokenAllocation {
            recipient_address: IotaAddress::default(),
            amount_nanos: self.stake_subsidy_fund_nanos,
            staked_with_validator: None,
            staked_with_timelock: vec![],
        })?;

        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct TokenAllocation {
    pub recipient_address: IotaAddress,
    pub amount_nanos: u64,

    /// Indicates if this allocation should be staked at genesis and with which
    /// validator
    pub staked_with_validator: Option<IotaAddress>,
    /// Indicates and containe the (amount, timelock_expiration) pairs if this
    /// allocation should be staked with timelock at genesis
    pub staked_with_timelock: Vec<(u64, u64)>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenDistributionScheduleBuilder {
    pool: u64,
    allocations: Vec<TokenAllocation>,
}

impl TokenDistributionScheduleBuilder {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            pool: TOTAL_SUPPLY_NANOS,
            allocations: vec![],
        }
    }

    pub fn default_allocation_for_validators<I: IntoIterator<Item = IotaAddress>>(
        &mut self,
        validators: I,
    ) {
        let default_allocation = iota_types::governance::VALIDATOR_LOW_STAKE_THRESHOLD_NANOS;

        for validator in validators {
            self.add_allocation(TokenAllocation {
                recipient_address: validator,
                amount_nanos: default_allocation,
                staked_with_validator: Some(validator),
                staked_with_timelock: vec![],
            });
        }
    }

    pub fn add_allocation(&mut self, allocation: TokenAllocation) {
        self.pool = self.pool.checked_sub(allocation.amount_nanos).unwrap();
        self.allocations.push(allocation);
    }

    pub fn build(&self) -> TokenDistributionSchedule {
        let schedule = TokenDistributionSchedule {
            stake_subsidy_fund_nanos: self.pool,
            allocations: self.allocations.clone(),
        };

        schedule.validate();
        schedule
    }
}
