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
    IOTA_BRIDGE_OBJECT_ID, IOTA_RANDOMNESS_STATE_OBJECT_ID,
    authenticator_state::{AuthenticatorStateInner, get_authenticator_state},
    base_types::{IotaAddress, ObjectID},
    clock::Clock,
    committee::{Committee, CommitteeWithNetworkMetadata, EpochId, ProtocolVersion},
    crypto::DefaultHash,
    deny_list_v1::get_deny_list_root_object,
    effects::{TransactionEffects, TransactionEvents},
    error::IotaResult,
    iota_system_state::{
        IotaSystemState, IotaSystemStateTrait, IotaSystemStateWrapper, IotaValidatorGenesis,
        get_iota_system_state, get_iota_system_state_wrapper,
    },
    messages_checkpoint::{
        CertifiedCheckpointSummary, CheckpointContents, CheckpointSummary, VerifiedCheckpoint,
    },
    object::Object,
    storage::ObjectStore,
    transaction::Transaction,
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

    pub fn into_objects(self) -> Vec<Object> {
        self.objects
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
            .try_into_verified(&self.committee().unwrap())
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

    // TODO: No need to return IotaResult. Also consider return &.
    pub fn committee(&self) -> IotaResult<Committee> {
        Ok(self.committee_with_network().committee().clone())
    }

    pub fn iota_system_wrapper_object(&self) -> IotaSystemStateWrapper {
        get_iota_system_state_wrapper(&self.objects())
            .expect("IOTA System State Wrapper object must always exist")
    }

    pub fn contains_migrations(&self) -> bool {
        self.checkpoint_contents.size() > 1
    }

    pub fn iota_system_object(&self) -> IotaSystemState {
        get_iota_system_state(&self.objects()).expect("IOTA System State object must always exist")
    }

    pub fn clock(&self) -> Clock {
        let clock = self
            .objects()
            .iter()
            .find(|o| o.id() == iota_types::IOTA_CLOCK_OBJECT_ID)
            .expect("clock must always exist")
            .data
            .try_as_move()
            .expect("clock must be a Move object");
        bcs::from_bytes::<Clock>(clock.contents())
            .expect("clock object deserialization cannot fail")
    }

    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, anyhow::Error> {
        let path = path.as_ref();
        trace!("reading Genesis from {}", path.display());
        let read = File::open(path)
            .with_context(|| format!("unable to load Genesis from {}", path.display()))?;
        bcs::from_reader(BufReader::new(read))
            .with_context(|| format!("unable to parse Genesis from {}", path.display()))
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), anyhow::Error> {
        let path = path.as_ref();
        trace!("writing Genesis to {}", path.display());
        let mut write = BufWriter::new(File::create(path)?);
        bcs::serialize_into(&mut write, &self)
            .with_context(|| format!("unable to save Genesis to {}", path.display()))?;
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
            .expect("IOTA System State Wrapper object must always exist")
    }

    pub fn iota_system_object(&self) -> IotaSystemState {
        get_iota_system_state(&self.objects()).expect("IOTA System State object must always exist")
    }

    pub fn authenticator_state_object(&self) -> Option<AuthenticatorStateInner> {
        get_authenticator_state(self.objects()).expect("read from genesis cannot fail")
    }

    pub fn has_randomness_state_object(&self) -> bool {
        self.objects()
            .get_object(&IOTA_RANDOMNESS_STATE_OBJECT_ID)
            .expect("read from genesis cannot fail")
            .is_some()
    }

    pub fn has_bridge_object(&self) -> bool {
        self.objects()
            .get_object(&IOTA_BRIDGE_OBJECT_ID)
            .expect("read from genesis cannot fail")
            .is_some()
    }

    pub fn has_coin_deny_list_object(&self) -> bool {
        get_deny_list_root_object(&self.objects()).is_ok()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct GenesisChainParameters {
    pub protocol_version: u64,
    pub chain_start_timestamp_ms: u64,
    pub epoch_duration_ms: u64,

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
}

impl GenesisCeremonyParameters {
    pub fn new() -> Self {
        Self {
            chain_start_timestamp_ms: Self::default_timestamp_ms(),
            protocol_version: ProtocolVersion::MAX,
            allow_insertion_of_extra_objects: true,
            epoch_duration_ms: Self::default_epoch_duration_ms(),
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

    pub fn to_genesis_chain_parameters(&self) -> GenesisChainParameters {
        GenesisChainParameters {
            protocol_version: self.protocol_version.as_u64(),
            chain_start_timestamp_ms: self.chain_start_timestamp_ms,
            epoch_duration_ms: self.epoch_duration_ms,
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
    pub pre_minted_supply: u64,
    pub allocations: Vec<TokenAllocation>,
}

impl TokenDistributionSchedule {
    pub fn contains_timelocked_stake(&self) -> bool {
        self.allocations
            .iter()
            .find_map(|allocation| allocation.staked_with_timelock_expiration)
            .is_some()
    }

    pub fn validate(&self) {
        let mut total_nanos = self.pre_minted_supply;

        for allocation in &self.allocations {
            total_nanos = total_nanos
                .checked_add(allocation.amount_nanos)
                .expect("TokenDistributionSchedule allocates more than the maximum supply which equals u64::MAX");
        }
    }

    pub fn check_minimum_stake_for_validators<I: IntoIterator<Item = IotaAddress>>(
        &self,
        validators: I,
    ) -> Result<()> {
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
                anyhow::bail!(
                    "validator {validator} has '{stake}' stake and does not meet the minimum required stake threshold of '{minimum_required_stake}'"
                );
            }
        }
        Ok(())
    }

    pub fn new_for_validators_with_default_allocation<I: IntoIterator<Item = IotaAddress>>(
        validators: I,
    ) -> Self {
        let default_allocation = iota_types::governance::VALIDATOR_LOW_STAKE_THRESHOLD_NANOS;

        let allocations = validators
            .into_iter()
            .map(|a| TokenAllocation {
                recipient_address: a,
                amount_nanos: default_allocation,
                staked_with_validator: Some(a),
                staked_with_timelock_expiration: None,
            })
            .collect();

        let schedule = Self {
            pre_minted_supply: 0,
            allocations,
        };

        schedule.validate();
        schedule
    }

    /// Helper to read a TokenDistributionSchedule from a csv file.
    ///
    /// The file is encoded such that the final entry in the CSV file is used to
    /// denote the allocation to the stake subsidy fund.
    pub fn from_csv<R: std::io::Read>(reader: R) -> Result<Self> {
        let mut reader = csv::Reader::from_reader(reader);
        let mut allocations: Vec<TokenAllocation> =
            reader.deserialize().collect::<Result<_, _>>()?;

        let pre_minted_supply = allocations.pop().unwrap();
        assert_eq!(
            IotaAddress::default(),
            pre_minted_supply.recipient_address,
            "final allocation must be for the pre-minted supply amount",
        );
        assert!(
            pre_minted_supply.staked_with_validator.is_none(),
            "cannot stake the pre-minted supply amount",
        );

        let schedule = Self {
            pre_minted_supply: pre_minted_supply.amount_nanos,
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
            amount_nanos: self.pre_minted_supply,
            staked_with_validator: None,
            staked_with_timelock_expiration: None,
        })?;

        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct TokenAllocation {
    /// Indicates the address that owns the tokens. It means that this
    /// `TokenAllocation` can serve to stake some funds to the
    /// `staked_with_validator` during genesis, but it's the `recipient_address`
    /// which will receive the associated StakedIota (or TimelockedStakedIota)
    /// object.
    pub recipient_address: IotaAddress,
    /// Indicates an amount of nanos that is:
    /// - minted for the `recipient_address` and staked to a validator, only in
    ///   the case `staked_with_validator` is Some
    /// - minted for the `recipient_address` and transferred that address,
    ///   otherwise.
    pub amount_nanos: u64,

    /// Indicates if this allocation should be staked at genesis and with which
    /// validator
    pub staked_with_validator: Option<IotaAddress>,
    /// Indicates if this allocation should be staked with timelock at genesis
    /// and contains its timelock_expiration
    pub staked_with_timelock_expiration: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenDistributionScheduleBuilder {
    pre_minted_supply: u64,
    allocations: Vec<TokenAllocation>,
}

impl TokenDistributionScheduleBuilder {
    #[expect(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            pre_minted_supply: 0,
            allocations: vec![],
        }
    }

    pub fn set_pre_minted_supply(&mut self, pre_minted_supply: u64) {
        self.pre_minted_supply = pre_minted_supply;
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
                staked_with_timelock_expiration: None,
            });
        }
    }

    pub fn add_allocation(&mut self, allocation: TokenAllocation) {
        self.allocations.push(allocation);
    }

    pub fn build(&self) -> TokenDistributionSchedule {
        let schedule = TokenDistributionSchedule {
            pre_minted_supply: self.pre_minted_supply,
            allocations: self.allocations.clone(),
        };

        schedule.validate();
        schedule
    }
}

/// Represents the allocation of stake and gas payment to a validator.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ValidatorAllocation {
    /// The validator address receiving the stake and/or gas payment
    pub validator: IotaAddress,
    /// The amount of nanos to stake to the validator
    pub amount_nanos_to_stake: u64,
    /// The amount of nanos to transfer as gas payment to the validator
    pub amount_nanos_to_pay_gas: u64,
}

/// Represents a delegation of stake and gas payment to a validator,
/// coming from a delegator. This struct is used to serialize and deserialize
/// delegations to and from a csv file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Delegation {
    /// The address from which to take the nanos for staking/gas
    pub delegator: IotaAddress,
    /// The allocation to a validator receiving a stake and/or a gas payment
    #[serde(flatten)]
    pub validator_allocation: ValidatorAllocation,
}

/// Represents genesis delegations to validators.
///
/// This struct maps a delegator address to a list of validators and their
/// stake and gas allocations. Each ValidatorAllocation contains the address of
/// a validator that will receive an amount of nanos to stake and an amount as
/// gas payment.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Delegations {
    pub allocations: HashMap<IotaAddress, Vec<ValidatorAllocation>>,
}

impl Delegations {
    pub fn new_for_validators_with_default_allocation(
        validators: impl IntoIterator<Item = IotaAddress>,
        delegator: IotaAddress,
    ) -> Self {
        let validator_allocations = validators
            .into_iter()
            .map(|address| ValidatorAllocation {
                validator: address,
                amount_nanos_to_stake: iota_types::governance::MIN_VALIDATOR_JOINING_STAKE_NANOS,
                amount_nanos_to_pay_gas: 0,
            })
            .collect();

        let mut allocations = HashMap::new();
        allocations.insert(delegator, validator_allocations);

        Self { allocations }
    }

    /// Helper to read a Delegations struct from a csv file.
    ///
    /// The file is encoded such that the final entry in the CSV file is used to
    /// denote the allocation coming from a delegator. It must be in the
    /// following format:
    /// `delegator,validator,amount-nanos-to-stake,amount-nanos-to-pay-gas
    /// <delegator1-address>,<validator-1-address>,2000000000000000,5000000000
    /// <delegator1-address>,<validator-2-address>,3000000000000000,5000000000
    /// <delegator2-address>,<validator-3-address>,4500000000000000,5000000000`
    pub fn from_csv<R: std::io::Read>(reader: R) -> Result<Self> {
        let mut reader = csv::Reader::from_reader(reader);

        let mut delegations = Self::default();
        for delegation in reader.deserialize::<Delegation>() {
            let delegation = delegation?;
            delegations
                .allocations
                .entry(delegation.delegator)
                .or_default()
                .push(delegation.validator_allocation);
        }

        Ok(delegations)
    }

    /// Helper to write a Delegations struct into a csv file.
    ///
    /// It writes in the following format:
    /// `delegator,validator,amount-nanos-to-stake,amount-nanos-to-pay-gas
    /// <delegator1-address>,<validator-1-address>,2000000000000000,5000000000
    /// <delegator1-address>,<validator-2-address>,3000000000000000,5000000000
    /// <delegator2-address>,<validator-3-address>,4500000000000000,5000000000`
    pub fn to_csv<W: std::io::Write>(&self, writer: W) -> Result<()> {
        let mut writer = csv::Writer::from_writer(writer);

        writer.write_record([
            "delegator",
            "validator",
            "amount-nanos-to-stake",
            "amount-nanos-to-pay-gas",
        ])?;

        for (&delegator, validator_allocations) in &self.allocations {
            for validator_allocation in validator_allocations {
                writer.write_record(&[
                    delegator.to_string(),
                    validator_allocation.validator.to_string(),
                    validator_allocation.amount_nanos_to_stake.to_string(),
                    validator_allocation.amount_nanos_to_pay_gas.to_string(),
                ])?;
            }
        }

        Ok(())
    }
}
