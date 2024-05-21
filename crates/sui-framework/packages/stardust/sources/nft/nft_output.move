// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module stardust::nft_output {

    use sui::bag::Bag;
    use sui::balance::Balance;
    use sui::dynamic_object_field;
    use sui::sui::SUI;
    use sui::transfer::Receiving;

    use stardust::nft::Nft;

    use stardust::expiration_unlock_condition::ExpirationUnlockCondition;
    use stardust::storage_deposit_return_unlock_condition::StorageDepositReturnUnlockCondition;
    use stardust::timelock_unlock_condition::TimelockUnlockCondition;

    /// The NFT dynamic field name.
    const NFT_NAME: vector<u8> = b"nft";

    /// The Stardust NFT output representation.
    public struct NftOutput has key {
        /// This is a "random" UID, not the NFTID from Stardust.
        id: UID,

        /// The amount of IOTA tokens held by the output.
        iota: Balance<SUI>,

        /// The `Bag` holds native tokens, key-ed by the stringified type of the asset.
        /// Example: key: "0xabcded::soon::SOON", value: Balance<0xabcded::soon::SOON>.
        native_tokens: Bag,

        /// The storage deposit return unlock condition.
        storage_deposit_return_uc: Option<StorageDepositReturnUnlockCondition>,
        /// The timelock unlock condition.
        timelock_uc: Option<TimelockUnlockCondition>,
        /// The expiration unlock condition.
        expiration_uc: Option<ExpirationUnlockCondition>,
    }

    /// The function extracts assets from a legacy NFT output.
    public fun extract_assets(mut output: NftOutput, ctx: &mut TxContext): (Balance<SUI>, Bag, Nft) {
        // Load the related Nft object.
        let nft = load_nft(&mut output);

        // Unpuck the output.
        let NftOutput {
            id,
            iota: mut iota,
            native_tokens,
            storage_deposit_return_uc: mut storage_deposit_return_uc,
            timelock_uc: mut timelock_uc,
            expiration_uc: mut expiration_uc
        } = output;

        // If the output has a timelock unlock condition, then we need to check if the timelock_uc has expired.
        if (timelock_uc.is_some()) {
            timelock_uc.extract().unlock(ctx);
        };

        // If the output has an expiration unlock condition, then we need to check who can unlock the output.
        if (expiration_uc.is_some()) {
            expiration_uc.extract().unlock(ctx);
        };

        // If the output has a storage deposit return unlock condition, then we need to return the deposit.
        if (storage_deposit_return_uc.is_some()) {
            storage_deposit_return_uc.extract().unlock(&mut iota, ctx);
        };

        // Destroy the output.
        option::destroy_none(timelock_uc);
        option::destroy_none(expiration_uc);
        option::destroy_none(storage_deposit_return_uc);

        object::delete(id);

        return (iota, native_tokens, nft)
    }

    /// Loads the related `Nft` object.
    fun load_nft(output: &mut NftOutput): Nft {
        dynamic_object_field::remove(&mut output.id, NFT_NAME)
    }

    // === Public-Package Functions ===

    /// Utility function to attach an `Alias` to an `AliasOutput`.
    public fun attach_nft(output: &mut NftOutput, nft: Nft) {
        dynamic_object_field::add(&mut output.id, NFT_NAME, nft)
    }

    /// Utility function to receive an `NftOutput` in other Stardust modules.
    /// Other modules in the stardust package can call this function to receive an `NftOutput` (alias).
    public(package) fun receive(parent: &mut UID, nft: Receiving<NftOutput>) : NftOutput {
        transfer::receive(parent, nft)
    }

    // === Test Functions ===

    #[test_only]
    public fun create_for_testing(
        iota: Balance<SUI>,
        native_tokens: Bag,
        storage_deposit_return_uc: Option<StorageDepositReturnUnlockCondition>,
        timelock_uc: Option<TimelockUnlockCondition>,
        expiration_uc: Option<ExpirationUnlockCondition>,
        ctx: &mut TxContext,
    ): NftOutput {
        NftOutput {
            id: object::new(ctx),
            iota,
            native_tokens,
            storage_deposit_return_uc,
            timelock_uc,
            expiration_uc,
        }
    }
}
