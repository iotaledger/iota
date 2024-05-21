// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// The purpose of a CoinManager is to allow access to all
/// properties of a Coin on-chain from within a single shared object
/// This includes access to the total supply and metadata
/// In addition a optional maximum supply can be set and a custom
/// additional Metadata field can be added. 
module sui::coin_manager {

    use sui::event;
    use std::type_name;
    use std::string;
    use std::ascii;
    use sui::url::Url;
    use sui::coin::{Self, CoinMetadata, TreasuryCap, Coin};
    use sui::balance::{Balance, Supply};
    use sui::dynamic_field as df;

    /// The error returned when the maximum supply reached.
    const EMaximumSupplyReached: u64 = 0;

    // The error returned if a attempt is made to change the maximum supply after setting it
    const EMaximumSupplyAlreadySet: u64 = 1;

    const EAdditionalMetadataAlreadyExists: u64 = 2;

    const EAdditionalMetadataDoesNotExist: u64 = 3;


    /// Holds all related objects to a Coin in a convenient shared function
    public struct CoinManager<phantom T> has key, store {
        id: UID,
        treasury_cap: TreasuryCap<T>,
        metadata: CoinMetadata<T>,
        maximum_supply: Option<u64>,
        supply_immutable: bool,
        metadata_immutable: bool
    }

    /// Like `TreasuryCap`, but for dealing with `TreasuryCap` inside `CoinManager` objects
    public struct CoinManagerTreasuryCap<phantom T> has key, store {
        id: UID
    }
    
    /// Metadata has it's own Cap, independent of the supply. 
    public struct CoinManagerMetadataCap<phantom T> has key, store {
        id: UID
    }

    public struct CoinManaged has copy, drop {
        coin_name: std::ascii::String
    }
    
    public struct TreasuryOwnershipRenounced has copy, drop {
        coin_name: std::ascii::String
    }
    
    public struct MetadataOwnershipRenounced has copy, drop {
        coin_name: std::ascii::String
    }

    /// Wraps all important objects related to a `Coin` inside a shared object
    public fun new<T> (
        treasury_cap: TreasuryCap<T>,
        metadata: CoinMetadata<T>,
        ctx: &mut TxContext,
    ): (CoinManagerTreasuryCap<T>, CoinManagerMetadataCap<T>, CoinManager<T>) {

        let manager = CoinManager {
            id: object::new(ctx),
            treasury_cap,
            metadata,
            maximum_supply: option::none(),
            supply_immutable: false,
            metadata_immutable: false
        };

        event::emit(CoinManaged {
            coin_name: type_name::into_string(type_name::get<T>())
        });

        (   
            CoinManagerTreasuryCap<T> { 
                id: object::new(ctx) 
            },
            CoinManagerMetadataCap<T> { 
                id: object::new(ctx) 
            },
            manager
        )
    }

    /// Convenience wrapper to create a new `Coin` and instantly wrap the cap inside a `CoinManager`
    public fun create<T: drop> (
        witness: T,
        decimals: u8,
        symbol: vector<u8>,
        name: vector<u8>,
        description: vector<u8>,
        icon_url: Option<Url>,
        ctx: &mut TxContext
    ): (CoinManagerTreasuryCap<T>, CoinManagerMetadataCap<T>, CoinManager<T>) {

        let (cap, meta) = coin::create_currency(
            witness,
            decimals, 
            symbol,
            name,
            description,
            icon_url,
            ctx
        );

        new(cap, meta, ctx)
    }

    // Option to add a additional metadata object to the manager
    // Can contain whatever you need in terms of additional metadata as a object
    public fun add_additional_metadata<T, Value: store>(
        _: &CoinManagerMetadataCap<T>,
        manager: &mut CoinManager<T>,
        value: Value
    ) {
        assert!(!df::exists_(&manager.id, b"additional_metadata"), EAdditionalMetadataAlreadyExists);
        df::add(&mut manager.id, b"additional_metadata", value);
    }
    
    // Option to replace a additional metadata object to the manager
    // Can contain whatever you need in terms of additional metadata as a object
    public fun replace_additional_metadata<T, Value: store, OldValue: store>(
        _: &CoinManagerMetadataCap<T>,
        manager: &mut CoinManager<T>,
        value: Value
    ): OldValue {
        assert!(df::exists_(&manager.id, b"additional_metadata"), EAdditionalMetadataDoesNotExist);
        let old_value = df::remove<vector<u8>, OldValue>(&mut manager.id, b"additional_metadata");
        df::add(&mut manager.id, b"additional_metadata", value);
        old_value
    }

    // Retrieve the additional metadata
    public fun additional_metadata<T, Value: store>(
        manager: &mut CoinManager<T>
    ): &Value {
        assert!(df::exists_(&manager.id, b"additional_metadata"), EAdditionalMetadataDoesNotExist);
        let meta: &Value = df::borrow(&manager.id, b"additional_metadata");
        meta
    }

    /// A one-time callable function to set a maximum mintable supply on a coin. 
    /// This can only be set once and is irrevertable. 
    public fun enforce_maximum_supply<T>(
        _: &CoinManagerTreasuryCap<T>, 
        manager: &mut CoinManager<T>, 
        maximum_supply: u64
    ) {
        assert!(option::is_none(&manager.maximum_supply), EMaximumSupplyAlreadySet);
        option::fill(&mut manager.maximum_supply, maximum_supply);
    }

    /// A irreversible action renouncing supply ownership which can be called if you hold the `CoinManagerTreasuryCap`.
    /// This action provides `Coin` holders with some assurances if called, namely that there will
    /// not be any new minting or changes to the supply from this point onward. The maximum supply
    /// will be set to the current supply and will not be changed any more afterwards.
    public fun renounce_treasury_ownership<T>(
        cap: CoinManagerTreasuryCap<T>,
        manager: &mut CoinManager<T>
    ) {
        // Deleting the Cap
        let CoinManagerTreasuryCap { id } = cap;
        object::delete(id);

        // Updating the maximum supply to the total supply
        let total_supply = total_supply(manager);
        if(manager.has_maximum_supply()) {
            option::swap(&mut manager.maximum_supply, total_supply);
        } else {
            option::fill(&mut manager.maximum_supply, total_supply);
        };

        // Setting ownership renounced to true
        manager.supply_immutable = true;

        event::emit(TreasuryOwnershipRenounced {
            coin_name: type_name::into_string(type_name::get<T>())
        });
    }
    
    /// A irreversible action renouncing manager ownership which can be called if you hold the `CoinManagerTreasuryCap`.
    /// This action provides `Coin` holders with some assurances if called, namely that there will
    /// not be any new minting or changes to the metadata from this point onward. The maximum supply
    /// will be set to the current supply and will not be changed any more afterwards.
    public fun renounce_metadata_ownership<T>(
        cap: CoinManagerMetadataCap<T>,
        manager: &mut CoinManager<T>
    ) {
        // Deleting the Cap
        let CoinManagerMetadataCap { id } = cap;
        object::delete(id);

        // Setting ownership renounced to true
        manager.metadata_immutable = true;

        event::emit(MetadataOwnershipRenounced {
            coin_name: type_name::into_string(type_name::get<T>())
        });
    }

    /// Convenience function allowing users to query if the ownership of the supply of this `Coin` 
    /// and thus the ability to mint new `Coin` has been renounced.
    public fun supply_is_immutable<T>(manager: &CoinManager<T>): bool {
        manager.supply_immutable
    }
    
    /// Convenience function allowing users to query if the ownership of the metadata management
    /// and thus the ability to change any of the metadata has been renounced.
    public fun metadata_is_immutable<T>(manager: &CoinManager<T>): bool {
        manager.metadata_immutable
    }

    // Get a read-only version of the metadata, available for everyone
    public fun metadata<T>(manager: &CoinManager<T>): &CoinMetadata<T> {
        &manager.metadata
    }

    /// Get the total supply as a number
    public fun total_supply<T>(manager: &CoinManager<T>): u64 {
        coin::total_supply(&manager.treasury_cap)
    }
    
    /// Get the maximum supply possible as a number. 
    /// If no maximum set it's the maximum u64 possible
    public fun maximum_supply<T>(manager: &CoinManager<T>): u64 {
        option::get_with_default(&manager.maximum_supply, 18_446_744_073_709_551_615u64)
    }

    /// Convenience function returning the remaining supply that can be minted still
    public fun available_supply<T>(manager: &CoinManager<T>): u64 {
        maximum_supply(manager) - total_supply(manager)
    }

    /// Returns if a maximum supply has been set for this Coin or not
    public fun has_maximum_supply<T>(manager: &CoinManager<T>): bool {
        option::is_some(&manager.maximum_supply)
    }

    /// Get immutable reference to the treasury's `Supply`.
    public fun supply_immut<T>(manager: &CoinManager<T>): &Supply<T> {
        coin::supply_immut(&manager.treasury_cap)
    }
    
    /// Create a coin worth `value` and increase the total supply
    /// in `cap` accordingly.
    public fun mint<T>(
        _: &CoinManagerTreasuryCap<T>, 
        manager: &mut CoinManager<T>, 
        value: u64, 
        ctx: &mut TxContext
    ): Coin<T> {
        assert!(total_supply(manager) + value <= maximum_supply(manager), EMaximumSupplyReached);
        coin::mint(&mut manager.treasury_cap, value, ctx)
    }

    /// Mint some amount of T as a `Balance` and increase the total
    /// supply in `cap` accordingly.
    /// Aborts if `value` + `cap.total_supply` >= U64_MAX
    public fun mint_balance<T>(
        _: &CoinManagerTreasuryCap<T>, 
        manager: &mut CoinManager<T>, 
        value: u64
    ): Balance<T> {
        assert!(total_supply(manager) + value <= maximum_supply(manager), EMaximumSupplyReached);
        coin::mint_balance(&mut manager.treasury_cap, value)
    }

    /// Destroy the coin `c` and decrease the total supply in `cap`
    /// accordingly.
    public entry fun burn<T>(
        _: &CoinManagerTreasuryCap<T>, 
        manager: &mut CoinManager<T>, 
        c: Coin<T>
    ): u64 {
        coin::burn(&mut manager.treasury_cap, c)
    }

    /// Mint `amount` of `Coin` and send it to `recipient`. Invokes `mint()`.
    public fun mint_and_transfer<T>(
       _: &CoinManagerTreasuryCap<T>,
       manager: &mut CoinManager<T>, 
       amount: u64, 
       recipient: address, 
       ctx: &mut TxContext
    ) {
        assert!(total_supply(manager) + amount <= maximum_supply(manager), EMaximumSupplyReached);
        coin::mint_and_transfer(&mut manager.treasury_cap, amount, recipient, ctx)
    }

    // === Update coin metadata ===

    /// Update the `name` of the coin in the `CoinMetadata`.
    public fun update_name<T>(
        _: &CoinManagerMetadataCap<T>,
        manager: &mut CoinManager<T>,
        name: string::String
    ) {
        coin::update_name(&manager.treasury_cap, &mut manager.metadata, name)
    }

    /// Update the `symbol` of the coin in the `CoinMetadata`.
    public fun update_symbol<T>(
        _: &CoinManagerMetadataCap<T>,
        manager: &mut CoinManager<T>,
        symbol: ascii::String
    ) {
        coin::update_symbol(&manager.treasury_cap, &mut manager.metadata, symbol)
    }

    /// Update the `description` of the coin in the `CoinMetadata`.
    public fun update_description<T>(
        _: &CoinManagerMetadataCap<T>,
        manager: &mut CoinManager<T>,
        description: string::String
    ) {
        coin::update_description(&manager.treasury_cap, &mut manager.metadata, description)
    }

    /// Update the `url` of the coin in the `CoinMetadata`
    public fun update_icon_url<T>(
        _: &CoinManagerMetadataCap<T>,
        manager: &mut CoinManager<T>,
        url: ascii::String
    ) {
        coin::update_icon_url(&manager.treasury_cap, &mut manager.metadata, url)
    }
    
    // === Convenience functions ===

    public fun decimals<T>(manager: &CoinManager<T>): u8 {
        coin::get_decimals(&manager.metadata)
    }

    public fun name<T>(manager: &CoinManager<T>): string::String {
        coin::get_name(&manager.metadata)
    }

    public fun symbol<T>(manager: &CoinManager<T>): ascii::String {
        coin::get_symbol(&manager.metadata)
    }

    public fun description<T>(manager: &CoinManager<T>): string::String {
        coin::get_description(&manager.metadata)
    }

    public fun icon_url<T>(manager: &CoinManager<T>): Option<Url> {
        coin::get_icon_url(&manager.metadata)
    }
}