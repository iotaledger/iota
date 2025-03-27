// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[allow(unused_const)]
module iota_system::validator {
    use std::bcs;

    use iota::balance::Balance;
    use iota::iota::IOTA;
    use iota_system::validator_cap::{Self};
    use iota_system::staking_pool::{Self, PoolTokenExchangeRate, StakedIota, StakingPoolV1};
    use std::string::String;
    use iota::url::Url;
    use iota::url;
    use iota::event;
    use iota::bag::Bag;
    use iota::bag;

    /// Invalid proof_of_possession field in ValidatorMetadata
    const EInvalidProofOfPossession: u64 = 0;

    /// Invalid authority_pubkey_bytes field in ValidatorMetadata
    const EMetadataInvalidAuthorityPubkey: u64 = 1;

    /// Invalid network_pubkey_bytes field in ValidatorMetadata
    const EMetadataInvalidNetPubkey: u64 = 2;

    /// Invalid protocol_pubkey_bytes field in ValidatorMetadata
    const EMetadataInvalidProtocolPubkey: u64 = 3;

    /// Invalid net_address field in ValidatorMetadata
    const EMetadataInvalidNetAddr: u64 = 4;

    /// Invalid p2p_address field in ValidatorMetadata
    const EMetadataInvalidP2pAddr: u64 = 5;

    /// Invalid primary_address field in ValidatorMetadata
    const EMetadataInvalidPrimaryAddr: u64 = 6;

    /// Commission rate set by the validator is higher than the threshold
    const ECommissionRateTooHigh: u64 = 7;

    /// Validator Metadata is too long
    const EValidatorMetadataExceedingLengthLimit: u64 = 8;

    /// Intended validator is not a candidate one.
    const ENotValidatorCandidate: u64 = 9;

    /// Stake amount is invalid or wrong.
    const EInvalidStakeAmount: u64 = 10;

    /// Function called during non-genesis times.
    const ECalledDuringNonGenesis: u64 = 11;

    /// New Capability is not created by the validator itself
    const ENewCapNotCreatedByValidatorItself: u64 = 100;

    /// Capability code is not valid
    const EInvalidCap: u64 = 101;

    /// Validator trying to set gas price higher than threshold.
    const EGasPriceHigherThanThreshold: u64 = 102;

    // The committee member index is not within the range of validators number
    const ECommitteeMembersOutOfRange: u64 = 103;

    // TODO: potentially move this value to onchain config.
    const MAX_COMMISSION_RATE: u64 = 2_000; // Max rate is 20%, which is 2000 base points

    const MAX_VALIDATOR_METADATA_LENGTH: u64 = 256;

    // TODO: Move this to onchain config when we have a good way to do it.
    /// Max gas price a validator can set is 100K NANOS.
    const MAX_VALIDATOR_GAS_PRICE: u64 = 100_000;

    public struct ValidatorMetadataV1 has store {
        /// The IOTA Address of the validator. This is the sender that created the ValidatorV1 object,
        /// and also the address to send validator/coins to during withdraws.
        iota_address: address,
        /// The public key bytes corresponding to the private key that the validator
        /// holds to sign transactions. For now, this is the same as AuthorityName.
        authority_pubkey_bytes: vector<u8>,
        /// The public key bytes corresponding to the private key that the validator
        /// uses to establish TLS connections.
        network_pubkey_bytes: vector<u8>,
        /// The public key bytes corresponding to the private key that the validator
        /// holds to sign consensus blocks.
        protocol_pubkey_bytes: vector<u8>,
        /// This is a proof that the validator has ownership of the private key.
        proof_of_possession: vector<u8>,
        /// A unique human-readable name of this validator.
        name: String,
        description: String,
        image_url: Url,
        project_url: Url,
        /// The network address of the validator (could also contain extra info such as port, DNS and etc.).
        net_address: String,
        /// The address of the validator used for p2p activities such as state sync (could also contain extra info such as port, DNS and etc.).
        p2p_address: String,
        /// The primary address of the consensus
        primary_address: String,

        /// "next_epoch" metadata only takes effects in the next epoch.
        /// If none, current value will stay unchanged.
        next_epoch_authority_pubkey_bytes: Option<vector<u8>>,
        next_epoch_proof_of_possession: Option<vector<u8>>,
        next_epoch_network_pubkey_bytes: Option<vector<u8>>,
        next_epoch_protocol_pubkey_bytes: Option<vector<u8>>,
        next_epoch_net_address: Option<String>,
        next_epoch_p2p_address: Option<String>,
        next_epoch_primary_address: Option<String>,

        /// Any extra fields that's not defined statically.
        extra_fields: Bag,
    }

    public struct ValidatorV1 has store {
        /// Summary of the validator.
        metadata: ValidatorMetadataV1,
        /// The voting power of this validator, which might be different from its
        /// stake amount.
        voting_power: u64,
        /// The ID of this validator's current valid `UnverifiedValidatorOperationCap`
        operation_cap_id: ID,
        /// Gas price quote, updated only at end of epoch.
        gas_price: u64,
        /// Staking pool for this validator.
        staking_pool: StakingPoolV1,
        /// Commission rate of the validator, in basis point.
        commission_rate: u64,
        /// Total amount of stake that would be active in the next epoch.
        next_epoch_stake: u64,
        /// This validator's gas price quote for the next epoch.
        next_epoch_gas_price: u64,
        /// The commission rate of the validator starting the next epoch, in basis point.
        next_epoch_commission_rate: u64,
        /// Any extra fields that's not defined statically.
        extra_fields: Bag,
    }

    /// Event emitted when a new stake request is received.
    public struct StakingRequestEvent has copy, drop {
        pool_id: ID,
        validator_address: address,
        staker_address: address,
        epoch: u64,
        amount: u64,
    }

    /// Event emitted when a new unstake request is received.
    public struct UnstakingRequestEvent has copy, drop {
        pool_id: ID,
        validator_address: address,
        staker_address: address,
        stake_activation_epoch: u64,
        unstaking_epoch: u64,
        principal_amount: u64,
        reward_amount: u64,
    }

    public(package) fun new_metadata(
        iota_address: address,
        authority_pubkey_bytes: vector<u8>,
        network_pubkey_bytes: vector<u8>,
        protocol_pubkey_bytes: vector<u8>,
        proof_of_possession: vector<u8>,
        name: String,
        description: String,
        image_url: Url,
        project_url: Url,
        net_address: String,
        p2p_address: String,
        primary_address: String,
        extra_fields: Bag,
    ): ValidatorMetadataV1 {
        let metadata = ValidatorMetadataV1 {
            iota_address,
            authority_pubkey_bytes,
            network_pubkey_bytes,
            protocol_pubkey_bytes,
            proof_of_possession,
            name,
            description,
            image_url,
            project_url,
            net_address,
            p2p_address,
            primary_address,
            next_epoch_authority_pubkey_bytes: option::none(),
            next_epoch_network_pubkey_bytes: option::none(),
            next_epoch_protocol_pubkey_bytes: option::none(),
            next_epoch_proof_of_possession: option::none(),
            next_epoch_net_address: option::none(),
            next_epoch_p2p_address: option::none(),
            next_epoch_primary_address: option::none(),
            extra_fields,
        };
        metadata
    }

    public(package) fun new(
        iota_address: address,
        authority_pubkey_bytes: vector<u8>,
        network_pubkey_bytes: vector<u8>,
        protocol_pubkey_bytes: vector<u8>,
        proof_of_possession: vector<u8>,
        name: vector<u8>,
        description: vector<u8>,
        image_url: vector<u8>,
        project_url: vector<u8>,
        net_address: vector<u8>,
        p2p_address: vector<u8>,
        primary_address: vector<u8>,
        gas_price: u64,
        commission_rate: u64,
        ctx: &mut TxContext
    ): ValidatorV1 {
        assert!(
            net_address.length() <= MAX_VALIDATOR_METADATA_LENGTH
                && p2p_address.length() <= MAX_VALIDATOR_METADATA_LENGTH
                && primary_address.length() <= MAX_VALIDATOR_METADATA_LENGTH
                && name.length() <= MAX_VALIDATOR_METADATA_LENGTH
                && description.length() <= MAX_VALIDATOR_METADATA_LENGTH
                && image_url.length() <= MAX_VALIDATOR_METADATA_LENGTH
                && project_url.length() <= MAX_VALIDATOR_METADATA_LENGTH,
            EValidatorMetadataExceedingLengthLimit
        );
        assert!(commission_rate <= MAX_COMMISSION_RATE, ECommissionRateTooHigh);
        assert!(gas_price <= MAX_VALIDATOR_GAS_PRICE, EGasPriceHigherThanThreshold);

        let metadata = new_metadata(
            iota_address,
            authority_pubkey_bytes,
            network_pubkey_bytes,
            protocol_pubkey_bytes,
            proof_of_possession,
            name.to_ascii_string().to_string(),
            description.to_ascii_string().to_string(),
            url::new_unsafe_from_bytes(image_url),
            url::new_unsafe_from_bytes(project_url),
            net_address.to_ascii_string().to_string(),
            p2p_address.to_ascii_string().to_string(),
            primary_address.to_ascii_string().to_string(),
            bag::new(ctx),
        );

        // Checks that the keys & addresses & PoP are valid.
        validate_metadata(&metadata);

        new_from_metadata(
            metadata,
            gas_price,
            commission_rate,
            ctx
        )
    }

    /// Deactivate this validator's staking pool
    public(package) fun deactivate(self: &mut ValidatorV1, deactivation_epoch: u64) {
        self.staking_pool.deactivate_staking_pool(deactivation_epoch)
    }

    public(package) fun activate(self: &mut ValidatorV1, activation_epoch: u64) {
        self.staking_pool.activate_staking_pool(activation_epoch);
    }

    /// Update the commission rate.
    public(package) fun adjust_next_epoch_commission_rate(self: &mut ValidatorV1) {
        self.commission_rate = self.next_epoch_commission_rate;
    }

    /// Request to add stake to the validator's staking pool, processed at the end of the epoch.
    public(package) fun request_add_stake(
        self: &mut ValidatorV1,
        stake: Balance<IOTA>,
        staker_address: address,
        ctx: &mut TxContext,
    ) : StakedIota {
        let stake_amount = stake.value();
        assert!(stake_amount > 0, EInvalidStakeAmount);
        let stake_epoch = ctx.epoch() + 1;
        let staked_iota = self.staking_pool.request_add_stake(stake, stake_epoch, ctx);
        // Process stake right away if staking pool is preactive.
        if (self.staking_pool.is_preactive()) {
            self.staking_pool.process_pending_stake();
        };
        self.next_epoch_stake = self.next_epoch_stake + stake_amount;
        event::emit(
            StakingRequestEvent {
                pool_id: staking_pool_id(self),
                validator_address: self.metadata.iota_address,
                staker_address,
                epoch: ctx.epoch(),
                amount: stake_amount,
            }
        );
        staked_iota
    }

    /// Request to add stake to the validator's staking pool at genesis
    public(package) fun request_add_stake_at_genesis(
        self: &mut ValidatorV1,
        stake: Balance<IOTA>,
        staker_address: address,
        ctx: &mut TxContext,
    ) {
        let staked_iota = request_add_stake_at_genesis_with_receipt(
            self,
            stake,
            ctx
        );
        transfer::public_transfer(staked_iota, staker_address);
    }

    /// Internal request to add stake to the validator's staking pool at genesis.
    /// Returns a StakedIota
    public(package) fun request_add_stake_at_genesis_with_receipt(
        self: &mut ValidatorV1,
        stake: Balance<IOTA>,
        ctx: &mut TxContext,
    ) : StakedIota {
        assert!(ctx.epoch() == 0, ECalledDuringNonGenesis);
        let stake_amount = stake.value();
        assert!(stake_amount > 0, EInvalidStakeAmount);

        let staked_iota = self.staking_pool.request_add_stake(
            stake,
            0, // epoch 0 -- genesis
            ctx
        );

        // Process stake right away
        self.staking_pool.process_pending_stake();
        self.next_epoch_stake = self.next_epoch_stake + stake_amount;
        
        staked_iota
    }

    /// Request to withdraw stake from the validator's staking pool, processed at the end of the epoch.
    public(package) fun request_withdraw_stake(
        self: &mut ValidatorV1,
        staked_iota: StakedIota,
        ctx: &TxContext,
    ) : Balance<IOTA> {
        let principal_amount = staked_iota.staked_iota_amount();
        let stake_activation_epoch = staked_iota.stake_activation_epoch();
        let withdrawn_stake = self.staking_pool.request_withdraw_stake(staked_iota, ctx);
        let withdraw_amount = withdrawn_stake.value();
        let reward_amount = withdraw_amount - principal_amount;
        self.next_epoch_stake = self.next_epoch_stake - withdraw_amount;
        event::emit(
            UnstakingRequestEvent {
                pool_id: staking_pool_id(self),
                validator_address: self.metadata.iota_address,
                staker_address: ctx.sender(),
                stake_activation_epoch,
                unstaking_epoch: ctx.epoch(),
                principal_amount,
                reward_amount,
            }
        );
        withdrawn_stake
    }

    /// Request to set new commission rate for the next epoch.
    public(package) fun request_set_commission_rate(self: &mut ValidatorV1, new_commission_rate: u64) {
        assert!(new_commission_rate <= MAX_COMMISSION_RATE, ECommissionRateTooHigh);
        self.next_epoch_commission_rate = new_commission_rate;
    }

    /// Set new commission rate for the candidate validator.
    public(package) fun set_candidate_commission_rate(self: &mut ValidatorV1, new_commission_rate: u64) {
        assert!(is_preactive(self), ENotValidatorCandidate);
        assert!(new_commission_rate <= MAX_COMMISSION_RATE, ECommissionRateTooHigh);
        self.commission_rate = new_commission_rate;
    }

    /// Deposit stakes rewards into the validator's staking pool, called at the end of the epoch.
    public(package) fun deposit_stake_rewards(self: &mut ValidatorV1, reward: Balance<IOTA>) {
        self.next_epoch_stake = self.next_epoch_stake + reward.value();
        self.staking_pool.deposit_rewards(reward);
    }

    /// Process pending stakes and withdraws, called at the end of the epoch.
    public(package) fun process_pending_stakes_and_withdraws(self: &mut ValidatorV1, ctx: &TxContext) {
        self.staking_pool.process_pending_stakes_and_withdraws(ctx);
        assert!(stake_amount(self) == self.next_epoch_stake, EInvalidStakeAmount);
    }

    /// Returns true if the validator is preactive.
    public fun is_preactive(self: &ValidatorV1): bool {
        self.staking_pool.is_preactive()
    }

    public fun metadata(self: &ValidatorV1): &ValidatorMetadataV1 {
        &self.metadata
    }

    public fun iota_address(self: &ValidatorV1): address {
        self.metadata.iota_address
    }

    public fun name(self: &ValidatorV1): &String {
        &self.metadata.name
    }

    public fun description(self: &ValidatorV1): &String {
        &self.metadata.description
    }

    public fun image_url(self: &ValidatorV1): &Url {
        &self.metadata.image_url
    }

    public fun project_url(self: &ValidatorV1): &Url {
        &self.metadata.project_url
    }

    public fun network_address(self: &ValidatorV1): &String {
        &self.metadata.net_address
    }

    public fun p2p_address(self: &ValidatorV1): &String {
        &self.metadata.p2p_address
    }

    public fun primary_address(self: &ValidatorV1): &String {
        &self.metadata.primary_address
    }

    public fun authority_pubkey_bytes(self: &ValidatorV1): &vector<u8> {
        &self.metadata.authority_pubkey_bytes
    }

    public fun proof_of_possession(self: &ValidatorV1): &vector<u8> {
        &self.metadata.proof_of_possession
    }

    public fun network_pubkey_bytes(self: &ValidatorV1): &vector<u8> {
        &self.metadata.network_pubkey_bytes
    }

    public fun protocol_pubkey_bytes(self: &ValidatorV1): &vector<u8> {
        &self.metadata.protocol_pubkey_bytes
    }

    public fun next_epoch_network_address(self: &ValidatorV1): &Option<String> {
        &self.metadata.next_epoch_net_address
    }

    public fun next_epoch_p2p_address(self: &ValidatorV1): &Option<String> {
        &self.metadata.next_epoch_p2p_address
    }

    public fun next_epoch_primary_address(self: &ValidatorV1): &Option<String> {
        &self.metadata.next_epoch_primary_address
    }

    public fun next_epoch_authority_pubkey_bytes(self: &ValidatorV1): &Option<vector<u8>> {
        &self.metadata.next_epoch_authority_pubkey_bytes
    }

    public fun next_epoch_proof_of_possession(self: &ValidatorV1): &Option<vector<u8>> {
        &self.metadata.next_epoch_proof_of_possession
    }

    public fun next_epoch_network_pubkey_bytes(self: &ValidatorV1): &Option<vector<u8>> {
        &self.metadata.next_epoch_network_pubkey_bytes
    }

    public fun next_epoch_protocol_pubkey_bytes(self: &ValidatorV1): &Option<vector<u8>> {
        &self.metadata.next_epoch_protocol_pubkey_bytes
    }

    public fun operation_cap_id(self: &ValidatorV1): &ID {
        &self.operation_cap_id
    }

    #[deprecated]
    public fun next_epoch_gas_price(self: &ValidatorV1): u64 {
        self.next_epoch_gas_price
    }

    // TODO: this and `stake_amount` and `total_stake` all seem to return the same value?
    // two of the functions can probably be removed.
    public fun total_stake_amount(self: &ValidatorV1): u64 {
        self.staking_pool.iota_balance()
    }

    public fun stake_amount(self: &ValidatorV1): u64 {
        self.staking_pool.iota_balance()
    }

    /// Return the total amount staked with this validator
    public fun total_stake(self: &ValidatorV1): u64 {
        stake_amount(self)
    }

    /// Return the voting power of this validator.
    public fun voting_power(self: &ValidatorV1): u64 {
        self.voting_power
    }

    /// Set the voting power of this validator, called only from validator_set.
    public(package) fun set_voting_power(self: &mut ValidatorV1, new_voting_power: u64) {
        self.voting_power = new_voting_power;
    }

    public fun pending_stake_amount(self: &ValidatorV1): u64 {
        self.staking_pool.pending_stake_amount()
    }

    public fun pending_stake_withdraw_amount(self: &ValidatorV1): u64 {
        self.staking_pool.pending_stake_withdraw_amount()
    }

    public fun gas_price(self: &ValidatorV1): u64 {
        self.gas_price
    }

    public fun commission_rate(self: &ValidatorV1): u64 {
        self.commission_rate
    }

    public fun pool_token_exchange_rate_at_epoch(self: &ValidatorV1, epoch: u64): PoolTokenExchangeRate {
        self.staking_pool.pool_token_exchange_rate_at_epoch(epoch)
    }

    public fun staking_pool_id(self: &ValidatorV1): ID {
        object::id(&self.staking_pool)
    }

    // MUSTFIX: We need to check this when updating metadata as well.
    public fun is_duplicate(self: &ValidatorV1, other: &ValidatorV1): bool {
        self.metadata.iota_address == other.metadata.iota_address
            || self.metadata.name == other.metadata.name
            || self.metadata.net_address == other.metadata.net_address
            || self.metadata.p2p_address == other.metadata.p2p_address
            || self.metadata.authority_pubkey_bytes == other.metadata.authority_pubkey_bytes
            || self.metadata.network_pubkey_bytes == other.metadata.network_pubkey_bytes
            || self.metadata.network_pubkey_bytes == other.metadata.protocol_pubkey_bytes
            || self.metadata.protocol_pubkey_bytes == other.metadata.protocol_pubkey_bytes
            || self.metadata.protocol_pubkey_bytes == other.metadata.network_pubkey_bytes
            // All next epoch parameters.
            || is_equal_some(&self.metadata.next_epoch_net_address, &other.metadata.next_epoch_net_address)
            || is_equal_some(&self.metadata.next_epoch_p2p_address, &other.metadata.next_epoch_p2p_address)
            || is_equal_some(&self.metadata.next_epoch_authority_pubkey_bytes, &other.metadata.next_epoch_authority_pubkey_bytes)
            || is_equal_some(&self.metadata.next_epoch_network_pubkey_bytes, &other.metadata.next_epoch_network_pubkey_bytes)
            || is_equal_some(&self.metadata.next_epoch_network_pubkey_bytes, &other.metadata.next_epoch_protocol_pubkey_bytes)
            || is_equal_some(&self.metadata.next_epoch_protocol_pubkey_bytes, &other.metadata.next_epoch_protocol_pubkey_bytes)
            || is_equal_some(&self.metadata.next_epoch_protocol_pubkey_bytes, &other.metadata.next_epoch_network_pubkey_bytes)
            // My next epoch parameters with other current epoch parameters.
            || is_equal_some_and_value(&self.metadata.next_epoch_net_address, &other.metadata.net_address)
            || is_equal_some_and_value(&self.metadata.next_epoch_p2p_address, &other.metadata.p2p_address)
            || is_equal_some_and_value(&self.metadata.next_epoch_authority_pubkey_bytes, &other.metadata.authority_pubkey_bytes)
            || is_equal_some_and_value(&self.metadata.next_epoch_network_pubkey_bytes, &other.metadata.network_pubkey_bytes)
            || is_equal_some_and_value(&self.metadata.next_epoch_network_pubkey_bytes, &other.metadata.protocol_pubkey_bytes)
            || is_equal_some_and_value(&self.metadata.next_epoch_protocol_pubkey_bytes, &other.metadata.protocol_pubkey_bytes)
            || is_equal_some_and_value(&self.metadata.next_epoch_protocol_pubkey_bytes, &other.metadata.network_pubkey_bytes)
            // Other next epoch parameters with my current epoch parameters.
            || is_equal_some_and_value(&other.metadata.next_epoch_net_address, &self.metadata.net_address)
            || is_equal_some_and_value(&other.metadata.next_epoch_p2p_address, &self.metadata.p2p_address)
            || is_equal_some_and_value(&other.metadata.next_epoch_authority_pubkey_bytes, &self.metadata.authority_pubkey_bytes)
            || is_equal_some_and_value(&other.metadata.next_epoch_network_pubkey_bytes, &self.metadata.network_pubkey_bytes)
            || is_equal_some_and_value(&other.metadata.next_epoch_network_pubkey_bytes, &self.metadata.protocol_pubkey_bytes)
            || is_equal_some_and_value(&other.metadata.next_epoch_protocol_pubkey_bytes, &self.metadata.protocol_pubkey_bytes)
            || is_equal_some_and_value(&other.metadata.next_epoch_protocol_pubkey_bytes, &self.metadata.network_pubkey_bytes)
    }

    fun is_equal_some_and_value<T>(a: &Option<T>, b: &T): bool {
        if (a.is_none()) {
            false
        } else {
            a.borrow() == b
        }
    }

    fun is_equal_some<T>(a: &Option<T>, b: &Option<T>): bool {
        if (a.is_none() || b.is_none()) {
            false
        } else {
            a.borrow() == b.borrow()
        }
    }

    public(package) fun smaller_than(self: &ValidatorV1, other: &ValidatorV1): bool{
        if (self.total_stake() != other.total_stake()) {
            return self.total_stake() < other.total_stake()
        };

        let self_pubkey = self.authority_pubkey_bytes();
        let other_pubkey = other.authority_pubkey_bytes();

        // Compare the two pubkeys lexicographically, assuming equal lengths
        'smaller_than: {
            self_pubkey.zip_do_ref!(other_pubkey, |a, b| if (a != b) return 'smaller_than *a < *b);
            false // Should never end up here
        }
    }

    // ==== Validator Metadata Management Functions ====

    /// Create a new `UnverifiedValidatorOperationCap`, transfer to the validator,
    /// and registers it, thus revoking the previous cap's permission.
    public(package) fun new_unverified_validator_operation_cap_and_transfer(self: &mut ValidatorV1, ctx: &mut TxContext) {
        let address = ctx.sender();
        assert!(address == self.metadata.iota_address, ENewCapNotCreatedByValidatorItself);
        let new_id = validator_cap::new_unverified_validator_operation_cap_and_transfer(address, ctx);
        self.operation_cap_id = new_id;
    }

    /// Update name of the validator.
    public(package) fun update_name(self: &mut ValidatorV1, name: vector<u8>) {
        assert!(
            name.length() <= MAX_VALIDATOR_METADATA_LENGTH,
            EValidatorMetadataExceedingLengthLimit
        );
        self.metadata.name = name.to_ascii_string().to_string();
    }

    /// Update description of the validator.
    public(package) fun update_description(self: &mut ValidatorV1, description: vector<u8>) {
        assert!(
            description.length() <= MAX_VALIDATOR_METADATA_LENGTH,
            EValidatorMetadataExceedingLengthLimit
        );
        self.metadata.description = description.to_ascii_string().to_string();
    }

    /// Update image url of the validator.
    public(package) fun update_image_url(self: &mut ValidatorV1, image_url: vector<u8>) {
        assert!(
            image_url.length() <= MAX_VALIDATOR_METADATA_LENGTH,
            EValidatorMetadataExceedingLengthLimit
        );
        self.metadata.image_url = url::new_unsafe_from_bytes(image_url);
    }

    /// Update project url of the validator.
    public(package) fun update_project_url(self: &mut ValidatorV1, project_url: vector<u8>) {
        assert!(
            project_url.length() <= MAX_VALIDATOR_METADATA_LENGTH,
            EValidatorMetadataExceedingLengthLimit
        );
        self.metadata.project_url = url::new_unsafe_from_bytes(project_url);
    }

    /// Update network address of this validator, taking effects from next epoch
    public(package) fun update_next_epoch_network_address(self: &mut ValidatorV1, net_address: vector<u8>) {
        assert!(
            net_address.length() <= MAX_VALIDATOR_METADATA_LENGTH,
            EValidatorMetadataExceedingLengthLimit
        );
        let net_address = net_address.to_ascii_string().to_string();
        self.metadata.next_epoch_net_address = option::some(net_address);
        validate_metadata(&self.metadata);
    }

    /// Update network address of this candidate validator
    public(package) fun update_candidate_network_address(self: &mut ValidatorV1, net_address: vector<u8>) {
        assert!(is_preactive(self), ENotValidatorCandidate);
        assert!(
            net_address.length() <= MAX_VALIDATOR_METADATA_LENGTH,
            EValidatorMetadataExceedingLengthLimit
        );
        let net_address = net_address.to_ascii_string().to_string();
        self.metadata.net_address = net_address;
        validate_metadata(&self.metadata);
    }

    /// Update p2p address of this validator, taking effects from next epoch
    public(package) fun update_next_epoch_p2p_address(self: &mut ValidatorV1, p2p_address: vector<u8>) {
        assert!(
            p2p_address.length() <= MAX_VALIDATOR_METADATA_LENGTH,
            EValidatorMetadataExceedingLengthLimit
        );
        let p2p_address = p2p_address.to_ascii_string().to_string();
        self.metadata.next_epoch_p2p_address = option::some(p2p_address);
        validate_metadata(&self.metadata);
    }

    /// Update p2p address of this candidate validator
    public(package) fun update_candidate_p2p_address(self: &mut ValidatorV1, p2p_address: vector<u8>) {
        assert!(is_preactive(self), ENotValidatorCandidate);
        assert!(
            p2p_address.length() <= MAX_VALIDATOR_METADATA_LENGTH,
            EValidatorMetadataExceedingLengthLimit
        );
        let p2p_address = p2p_address.to_ascii_string().to_string();
        self.metadata.p2p_address = p2p_address;
        validate_metadata(&self.metadata);
    }

    /// Update primary address of this validator, taking effects from next epoch
    public(package) fun update_next_epoch_primary_address(self: &mut ValidatorV1, primary_address: vector<u8>) {
        assert!(
            primary_address.length() <= MAX_VALIDATOR_METADATA_LENGTH,
            EValidatorMetadataExceedingLengthLimit
        );
        let primary_address = primary_address.to_ascii_string().to_string();
        self.metadata.next_epoch_primary_address = option::some(primary_address);
        validate_metadata(&self.metadata);
    }

    /// Update primary address of this candidate validator
    public(package) fun update_candidate_primary_address(self: &mut ValidatorV1, primary_address: vector<u8>) {
        assert!(is_preactive(self), ENotValidatorCandidate);
        assert!(
            primary_address.length() <= MAX_VALIDATOR_METADATA_LENGTH,
            EValidatorMetadataExceedingLengthLimit
        );
        let primary_address = primary_address.to_ascii_string().to_string();
        self.metadata.primary_address = primary_address;
        validate_metadata(&self.metadata);
    }

    /// Update authority public key of this validator, taking effects from next epoch
    public(package) fun update_next_epoch_authority_pubkey(self: &mut ValidatorV1, authority_pubkey: vector<u8>, proof_of_possession: vector<u8>) {
        self.metadata.next_epoch_authority_pubkey_bytes = option::some(authority_pubkey);
        self.metadata.next_epoch_proof_of_possession = option::some(proof_of_possession);
        validate_metadata(&self.metadata);
    }

    /// Update authority public key of this candidate validator
    public(package) fun update_candidate_authority_pubkey(self: &mut ValidatorV1, authority_pubkey: vector<u8>, proof_of_possession: vector<u8>) {
        assert!(is_preactive(self), ENotValidatorCandidate);
        self.metadata.authority_pubkey_bytes = authority_pubkey;
        self.metadata.proof_of_possession = proof_of_possession;
        validate_metadata(&self.metadata);
    }

    /// Update network public key of this validator, taking effects from next epoch
    public(package) fun update_next_epoch_network_pubkey(self: &mut ValidatorV1, network_pubkey: vector<u8>) {
        self.metadata.next_epoch_network_pubkey_bytes = option::some(network_pubkey);
        validate_metadata(&self.metadata);
    }

    /// Update network public key of this candidate validator
    public(package) fun update_candidate_network_pubkey(self: &mut ValidatorV1, network_pubkey: vector<u8>) {
        assert!(is_preactive(self), ENotValidatorCandidate);
        self.metadata.network_pubkey_bytes = network_pubkey;
        validate_metadata(&self.metadata);
    }

    /// Update protocol public key of this validator, taking effects from next epoch
    public(package) fun update_next_epoch_protocol_pubkey(self: &mut ValidatorV1, protocol_pubkey: vector<u8>) {
        self.metadata.next_epoch_protocol_pubkey_bytes = option::some(protocol_pubkey);
        validate_metadata(&self.metadata);
    }

    /// Update protocol public key of this candidate validator
    public(package) fun update_candidate_protocol_pubkey(self: &mut ValidatorV1, protocol_pubkey: vector<u8>) {
        assert!(is_preactive(self), ENotValidatorCandidate);
        self.metadata.protocol_pubkey_bytes = protocol_pubkey;
        validate_metadata(&self.metadata);
    }

    /// Effectutate all staged next epoch metadata for this validator.
    /// NOTE: this function SHOULD ONLY be called by validator_set when
    /// advancing an epoch.
    public(package) fun effectuate_staged_metadata(self: &mut ValidatorV1) {
        if (next_epoch_network_address(self).is_some()) {
            self.metadata.net_address = self.metadata.next_epoch_net_address.extract();
            self.metadata.next_epoch_net_address = option::none();
        };

        if (next_epoch_p2p_address(self).is_some()) {
            self.metadata.p2p_address = self.metadata.next_epoch_p2p_address.extract();
            self.metadata.next_epoch_p2p_address = option::none();
        };

        if (next_epoch_primary_address(self).is_some()) {
            self.metadata.primary_address = self.metadata.next_epoch_primary_address.extract();
            self.metadata.next_epoch_primary_address = option::none();
        };

        if (next_epoch_authority_pubkey_bytes(self).is_some()) {
            self.metadata.authority_pubkey_bytes = self.metadata.next_epoch_authority_pubkey_bytes.extract();
            self.metadata.next_epoch_authority_pubkey_bytes = option::none();
            self.metadata.proof_of_possession = self.metadata.next_epoch_proof_of_possession.extract();
            self.metadata.next_epoch_proof_of_possession = option::none();
        };

        if (next_epoch_network_pubkey_bytes(self).is_some()) {
            self.metadata.network_pubkey_bytes = self.metadata.next_epoch_network_pubkey_bytes.extract();
            self.metadata.next_epoch_network_pubkey_bytes = option::none();
        };

        if (next_epoch_protocol_pubkey_bytes(self).is_some()) {
            self.metadata.protocol_pubkey_bytes = self.metadata.next_epoch_protocol_pubkey_bytes.extract();
            self.metadata.next_epoch_protocol_pubkey_bytes = option::none();
        };
    }

    /// Aborts if validator metadata is valid
    public fun validate_metadata(metadata: &ValidatorMetadataV1) {
        validate_metadata_bcs(bcs::to_bytes(metadata));
    }

    public native fun validate_metadata_bcs(metadata: vector<u8>);

    public(package) fun get_staking_pool_ref(self: &ValidatorV1) : &StakingPoolV1 {
        &self.staking_pool
    }

    public(package) fun get_validator_by_committee_index(
        validators: &vector<ValidatorV1>,
        committee_member_index: u64,
    ): &ValidatorV1 {
            let validators_length = validators.length();
            assert!(
                committee_member_index < validators_length,
                ECommitteeMembersOutOfRange,
            );

            &validators[committee_member_index]
    }

    public(package) fun get_validator_by_committee_index_mut(
        validators: &mut vector<ValidatorV1>,
        committee_member_index: u64,
    ): &mut ValidatorV1 {
            let validators_length = validators.length();
            assert!(
                committee_member_index < validators_length,
                ECommitteeMembersOutOfRange,
            );

            return &mut validators[committee_member_index]
    }

    /// Create a new validator from the given `ValidatorMetadataV1`, called by both `new` and `new_for_testing`.
    fun new_from_metadata(
        metadata: ValidatorMetadataV1,
        gas_price: u64,
        commission_rate: u64,
        ctx: &mut TxContext
    ): ValidatorV1 {
        let iota_address = metadata.iota_address;

        let staking_pool = staking_pool::new(ctx);

        let operation_cap_id = validator_cap::new_unverified_validator_operation_cap_and_transfer(iota_address, ctx);
        ValidatorV1 {
            metadata,
            // Initialize the voting power to be 0.
            // At the epoch change where this validator is actually added to the
            // committee validator set, the voting power will be updated accordingly.
            voting_power: 0,
            operation_cap_id,
            gas_price,
            staking_pool,
            commission_rate,
            next_epoch_stake: 0,
            next_epoch_gas_price: gas_price,
            next_epoch_commission_rate: commission_rate,
            extra_fields: bag::new(ctx),
        }
    }

    // CAUTION: THIS CODE IS ONLY FOR TESTING AND THIS MACRO MUST NEVER EVER BE REMOVED.
    // Creates a validator - bypassing the proof of possession check and other metadata
    // validation in the process.
    // Note: `proof_of_possession` MUST be a valid signature using iota_address and
    // authority_pubkey_bytes. To produce a valid PoP, run [fn test_proof_of_possession].
    #[test_only]
    public(package) fun new_for_testing(
        iota_address: address,
        authority_pubkey_bytes: vector<u8>,
        network_pubkey_bytes: vector<u8>,
        protocol_pubkey_bytes: vector<u8>,
        proof_of_possession: vector<u8>,
        name: vector<u8>,
        description: vector<u8>,
        image_url: vector<u8>,
        project_url: vector<u8>,
        net_address: vector<u8>,
        p2p_address: vector<u8>,
        primary_address: vector<u8>,
        mut initial_stake_option: Option<Balance<IOTA>>,
        gas_price: u64,
        commission_rate: u64,
        is_active_at_genesis: bool,
        ctx: &mut TxContext
    ): ValidatorV1 {
        let mut validator = new_from_metadata(
            new_metadata(
                iota_address,
                authority_pubkey_bytes,
                network_pubkey_bytes,
                protocol_pubkey_bytes,
                proof_of_possession,
                name.to_ascii_string().to_string(),
                description.to_ascii_string().to_string(),
                url::new_unsafe_from_bytes(image_url),
                url::new_unsafe_from_bytes(project_url),
                net_address.to_ascii_string().to_string(),
                p2p_address.to_ascii_string().to_string(),
                primary_address.to_ascii_string().to_string(),
                bag::new(ctx),
            ),
            gas_price,
            commission_rate,
            ctx
        );

        // Add the validator's starting stake to the staking pool if there exists one.
        if (initial_stake_option.is_some()) {
            request_add_stake_at_genesis(
                &mut validator,
                initial_stake_option.extract(),
                iota_address, // give the stake to the validator
                ctx
            );
        };
        initial_stake_option.destroy_none();

        if (is_active_at_genesis) {
            activate(&mut validator, 0);
        };

        validator
    }
}
