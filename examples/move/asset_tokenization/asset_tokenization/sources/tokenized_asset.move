// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// The `tokenized_asset` module will operate in a manner similar to the `coin` library. 
/// When it receives a new one-time witness type, it will create a unique representation of
/// a fractional asset.
/// This module employs similar implementations to some methods found in the Coin module.
/// It encompasses functionalities pertinent to asset tokenization,
/// including new asset creation, minting, splitting, joining, and burning.
module asset_tokenization::tokenized_asset {
    // std lib imports
    use std::string::{String};
    use std::ascii;
    use std::type_name::{Self};

    // IOTA imports
    use iota::url::{Url};
    use iota::vec_map::{Self, VecMap};
    use iota::balance::{Self, Supply, Balance};
    use iota::event::emit;
    
    const ENoSupply: u64 = 1;
    const EInsufficientTotalSupply: u64 = 2;
    const EUniqueAsset: u64 = 3;
    const ENonBurnable: u64 = 4;
    const EVecLengthMismatch: u64 = 5;
    const EInsufficientBalance: u64 = 6;
    const EZeroBalance: u64 = 7;
    const EBadWitness: u64 = 8;

    /// An AssetCap should be generated for each new Asset we wish to represent
    /// as a fractional NFT. In most scenarios, it is recommended to be created as
    /// an owned object, which can then be transferred to the platform's administrator
    /// for access restricted method invocation.
    public struct AssetCap<phantom T> has key, store {
        id: UID,
        /// The current circulating supply
        supply: Supply<T>,
        /// The total max supply allowed to exist at any time that was issued
        /// upon creation of Asset T
        total_supply: u64,
        /// TAs of type T can be burned by the admin
	    burnable: bool
    }

    /// The AssetMetadata struct defines the metadata representing the entire asset.
    /// that we intend to fractionalize. 
    /// It is recommended to be a shared object.
    public struct AssetMetadata<phantom T> has key, store {
        id: UID,
        /// Name of the asset
        name: String,
        /// The total max supply allowed to exist at any time that was issued
        /// upon creation of Asset T
        total_supply: u64,
        /// Symbol for the asset
        symbol: ascii::String,
        /// Description of the asset
        description: String,
        /// URL for the asset logo
        icon_url: Option<Url>
    }

    /// TokenizedAsset(TA) struct represents a tokenized asset of type T.
    public struct TokenizedAsset<phantom T> has key, store {
        id: UID,
        /// The balance of the tokenized asset.
        balance: Balance<T>,
        /// If the VecMap is populated, it is considered an NFT, else the asset is considered an FT.
        metadata: VecMap<String, String>,
        /// URL for the asset image (optional).
        image_url: Option<Url>,
    }

    /// Capability that is issued to the one deploying the contract.
    /// Allows access to the publisher.
    public struct PlatformCap has key, store { id: UID }

    /// Event emitted when a new asset is created.
    public struct AssetCreated has copy, drop {
        asset_metadata: ID,
        name: ascii::String
    }

    /// Creates a PlatformCap and sends it to the sender.
    fun init(ctx: &mut TxContext) {
        transfer::transfer(PlatformCap {
            id: object::new(ctx)
        }, tx_context::sender(ctx))
    }

    /// Creates a new Asset representation that can be fractionalized.
    public fun new_asset<T: drop>(
        witness: T,
        total_supply: u64,
        symbol: ascii::String,
        name: String,
        description: String,
        icon_url: Option<Url>,
        burnable: bool,
        ctx: &mut TxContext
    ): (AssetCap<T>, AssetMetadata<T>) {
        assert!(iota::types::is_one_time_witness(&witness), EBadWitness);
        assert!(total_supply > 0, EInsufficientTotalSupply);

        let asset_cap = AssetCap {
            id: object::new(ctx),
            supply: balance::create_supply(witness),
            total_supply,
            burnable
        };

        let asset_metadata = AssetMetadata {
            id: object::new(ctx),
            name,
            total_supply,
            symbol,
            description,
            icon_url
        };

        emit(AssetCreated {
            asset_metadata: object::id(&asset_metadata),
            name: type_name::into_string(type_name::get<T>()) 
        });

        (asset_cap, asset_metadata)
    }

    /// Mints a TA with the specified fields.
    /// If the VecMap of an asset is populated with values, indicating multiple unique entries,
    /// it is considered a non-fungible token (NFT). 
    /// Conversely, if the VecMap of an asset is not populated, 
    /// indicating an absence of individual entries, 
    /// it is considered a fungible token (FT).
    public fun mint<T>(
        cap: &mut AssetCap<T>,
        keys: vector<String>,
        values: vector<String>,
        value: u64,
        ctx: &mut TxContext
    ): TokenizedAsset<T> {
        let supply_value = supply(cap);
        assert!(supply_value + value <= cap.total_supply, ENoSupply);
        let metadata = create_vec_map_from_arrays(keys, values);
        assert!(!vec_map::is_empty(&metadata) && value == 1 || vec_map::is_empty(&metadata) && value > 0, EUniqueAsset);
        let balance = balance::increase_supply(&mut cap.supply, value);

        TokenizedAsset {
            id: object::new(ctx),
            balance,
            metadata,
            image_url: option::none<Url>(),
        }
    }

    /// Split a tokenized_asset.
    /// Creates a new tokenized asset of balance split_amount and updates tokenized_asset's balance accordingly.
    /// If the asset is unique (NFT) it can not be split into a new TA.
    public fun split<T>(
        self: &mut TokenizedAsset<T>,
        split_amount: u64,
        ctx: &mut TxContext
    ): TokenizedAsset<T> {
        assert!(vec_map::is_empty(&self.metadata), EUniqueAsset);
        let balance_value = value(self);
        assert!(balance_value > 1 && split_amount < balance_value, EInsufficientBalance);
        assert!(split_amount > 0, EZeroBalance);

        let new_balance = balance::split(&mut self.balance, split_amount);

        TokenizedAsset {
            id: object::new(ctx),
            balance: new_balance,
            metadata: self.metadata,
            image_url: option::none<Url>(),
        }
    }


    /// Merge other's balance into self's balance.
    /// other is burned.
    /// If the asset is unique (NFT) it can not be merged with other TAs
    /// of type T since they describe unique variations of the underlying asset T.
    public fun join<T>(
        self: &mut TokenizedAsset<T>,
        other: TokenizedAsset<T>
    ): ID {
        let ft1 = vec_map::is_empty(&self.metadata);
        let ft2 = vec_map::is_empty(&other.metadata);
        assert!(ft1 == true && ft2 == true, EUniqueAsset);

        let item = object::id(&other);
        let TokenizedAsset { id, balance, metadata: _, image_url: _ } = other;
        balance::join(&mut self.balance, balance);
        object::delete(id);

        item
    }

    /// Destroy the tokenized asset and decrease the supply in `cap` accordingly.
    public fun burn<T>(
        cap: &mut AssetCap<T>,
        tokenized_asset: TokenizedAsset<T>
    ) {
        assert!(cap.burnable == true, ENonBurnable);
        let TokenizedAsset { id, balance, metadata: _, image_url: _} = tokenized_asset;
        balance::decrease_supply(&mut cap.supply, balance);
        object::delete(id);
    }

    /// Returns the value of the total supply.
    public fun total_supply<T>(cap: &AssetCap<T>): u64 {
        cap.total_supply
    }

    /// Returns the value of the current circulating supply.
    public fun supply<T>(cap: &AssetCap<T>): u64 {
        balance::supply_value(&cap.supply)
    }

    /// Returns the balance value of a TokenizedAsset<T>.
    public fun value<T>(tokenized_asset: &TokenizedAsset<T>): u64 {
        balance::value(&tokenized_asset.balance)
    }

    /// Internal helper function used to populate a VecMap<String, String>.
    fun create_vec_map_from_arrays(
        keys: vector<String>,
        values: vector<String>
    ): VecMap<String, String> {
        assert!(vector::length(&keys) == vector::length(&values), EVecLengthMismatch);

        let mut vec_map = vec_map::empty();
        let len = vector::length(&keys);
        let mut i = 0;

        while (i < len) {
            vec_map::insert(
                &mut vec_map, 
                *vector::borrow(&keys, i),
                *vector::borrow(&values, i)
            );
            i = i + 1;
        };
        vec_map
    }

    #[test_only]
    public fun test_init(ctx: &mut TxContext) {
        init(ctx);
    }
}
