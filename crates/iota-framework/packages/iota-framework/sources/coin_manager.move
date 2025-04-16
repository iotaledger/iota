// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// The purpose of a CoinManager is to allow access to all
/// properties of a Coin on-chain from within a single shared object
/// This includes access to the total supply and metadata
/// In addition a optional maximum supply can be set and a custom
/// additional Metadata field can be added.
module iota::coin_manager;

use iota::balance::{Balance, Supply};
use iota::coin::{Self, CoinMetadata, TreasuryCap, Coin};
use iota::dynamic_field as df;
use iota::event;
use iota::url::Url;
use std::ascii;
use std::string;
use std::type_name;

/// The error returned when the maximum supply reached.
const EMaximumSupplyReached: u64 = 0;

/// The error returned if an attempt is made to change the maximum supply after setting it.
const EMaximumSupplyAlreadySet: u64 = 1;

/// The error returned if an attempt is made to change the maximum supply that is lower than the total supply.
const EMaximumSupplyLowerThanTotalSupply: u64 = 2;

/// The error returned if an attempt is made to change the maximum supply that is higher than the maximum possible supply.
const EMaximumSupplyHigherThanPossible: u64 = 3;

/// The error returned if you try to edit nonexisting additional metadata.
const EAdditionalMetadataDoesNotExist: u64 = 4;

/// The maximum supply supported by `CoinManager`.
const MAX_SUPPLY: u64 = 18_446_744_073_709_551_614u64;

/// The name of the related additional metadata dynamic field.
const ADDITIONAL_METADATA_NAME: vector<u8> = b"additional_metadata";

/// Holds all the related objects to the coin of type `T` in a convenient shared function.
public struct CoinManager<phantom T> has key, store {
    id: UID,
    /// The original `TreasuryCap` object as returned by `create_currency`.
    treasury_cap: TreasuryCap<T>,
    /// Metadata object, original one from the `coin` module, if available.
    metadata: Option<CoinMetadata<T>>,
    /// Immutable Metadata object, only to be used as a last resort if the original metadata is frozen.
    immutable_metadata: Option<ImmutableCoinMetadata<T>>,
    /// Optional maximum supply, if set you can't mint more as this number - can only be set once.
    maximum_supply: Option<u64>,
    /// Flag indicating if the supply is considered immutable (TreasuryCap is exchanged for this).
    supply_immutable: bool,
    /// Flag indicating if the metadata is considered immutable (MetadataCap is exchanged for this).
    metadata_immutable: bool,
}

/// Like `TreasuryCap`, but for dealing with `TreasuryCap` inside `CoinManager` objects.
public struct CoinManagerTreasuryCap<phantom T> has key, store {
    id: UID,
}

/// Metadata has it's own Cap, independent of the `TreasuryCap`.
public struct CoinManagerMetadataCap<phantom T> has key, store {
    id: UID,
}

/// The immutable version of `CoinMetadata`, used in case of migrating from frozen objects
/// to a `CoinManager` holding the metadata.
public struct ImmutableCoinMetadata<phantom T> has store {
    /// Number of decimal places the coin uses.
    /// A coin with `value` N and `decimals` D should be shown as N / 10^D
    /// E.g., a coin with `value` 7002 and decimals 3 should be displayed as 7.002
    /// This is metadata for display usage only.
    decimals: u8,
    /// Name for the token.
    name: string::String,
    /// Symbol for the token.
    symbol: ascii::String,
    /// Description of the token.
    description: string::String,
    /// URL for the token logo.
    icon_url: Option<Url>,
}

/// Event triggered once `Coin` ownership is transferred to a new `CoinManager`.
public struct CoinManaged has copy, drop {
    coin_name: std::ascii::String,
}

/// Event triggered if the ownership of the treasury part of a `CoinManager` is renounced.
public struct TreasuryOwnershipRenounced has copy, drop {
    coin_name: std::ascii::String,
}

/// Event triggered if the ownership of the metadata part of a `CoinManager` is renounced.
public struct MetadataOwnershipRenounced has copy, drop {
    coin_name: std::ascii::String,
}

/// Wraps all important objects related to a `Coin` inside a shared object.
public fun new<T>(
    treasury_cap: TreasuryCap<T>,
    metadata: CoinMetadata<T>,
    ctx: &mut TxContext,
): (CoinManagerTreasuryCap<T>, CoinManagerMetadataCap<T>, CoinManager<T>) {
    let manager = CoinManager {
        id: object::new(ctx),
        treasury_cap,
        metadata: option::some(metadata),
        immutable_metadata: option::none(),
        maximum_supply: option::none(),
        supply_immutable: false,
        metadata_immutable: false,
    };

    event::emit(CoinManaged {
        coin_name: type_name::into_string(type_name::get<T>()),
    });

    (
        CoinManagerTreasuryCap<T> {
            id: object::new(ctx),
        },
        CoinManagerMetadataCap<T> {
            id: object::new(ctx),
        },
        manager,
    )
}

/// This function allows the same as `new` but under the assumption the Metadata can not be transferred.
/// This would typically be the case with `Coin` instances where the metadata is already frozen.
public fun new_with_immutable_metadata<T>(
    treasury_cap: TreasuryCap<T>,
    metadata: &CoinMetadata<T>,
    ctx: &mut TxContext,
): (CoinManagerTreasuryCap<T>, CoinManager<T>) {
    let metacopy = ImmutableCoinMetadata<T> {
        decimals: metadata.get_decimals(),
        name: metadata.get_name(),
        symbol: metadata.get_symbol(),
        description: metadata.get_description(),
        icon_url: metadata.get_icon_url(),
    };

    let manager = CoinManager {
        id: object::new(ctx),
        treasury_cap,
        metadata: option::none(),
        immutable_metadata: option::some(metacopy),
        maximum_supply: option::none(),
        supply_immutable: false,
        metadata_immutable: true,
    };

    event::emit(CoinManaged {
        coin_name: type_name::into_string(type_name::get<T>()),
    });

    (
        CoinManagerTreasuryCap<T> {
            id: object::new(ctx),
        },
        manager,
    )
}

/// Convenience wrapper to create a new `Coin` and instantly wrap the cap inside a `CoinManager`.
public fun create<T: drop>(
    witness: T,
    decimals: u8,
    symbol: vector<u8>,
    name: vector<u8>,
    description: vector<u8>,
    icon_url: Option<Url>,
    ctx: &mut TxContext,
): (CoinManagerTreasuryCap<T>, CoinManagerMetadataCap<T>, CoinManager<T>) {
    let (cap, meta) = coin::create_currency(
        witness,
        decimals,
        symbol,
        name,
        description,
        icon_url,
        ctx,
    );

    new(cap, meta, ctx)
}

/// Option to add an additional metadata object to the manager.
/// Can contain whatever you need in terms of additional metadata as a object.
public fun add_additional_metadata<T, Value: store>(
    _: &CoinManagerMetadataCap<T>,
    manager: &mut CoinManager<T>,
    value: Value,
) {
    df::add(&mut manager.id, ADDITIONAL_METADATA_NAME, value);
}

/// Option to replace an additional metadata object to the manager.
/// Can contain whatever you need in terms of additional metadata as a object.
public fun replace_additional_metadata<T, Value: store, OldValue: store>(
    _: &CoinManagerMetadataCap<T>,
    manager: &mut CoinManager<T>,
    value: Value,
): OldValue {
    assert!(df::exists_(&manager.id, ADDITIONAL_METADATA_NAME), EAdditionalMetadataDoesNotExist);
    let old_value = df::remove<vector<u8>, OldValue>(&mut manager.id, ADDITIONAL_METADATA_NAME);
    df::add(&mut manager.id, ADDITIONAL_METADATA_NAME, value);
    old_value
}

#[deprecated(note = b"Use `iota::coin_manager::get_additional_metadata` instead.")]
public fun additional_metadata<T, Value: store>(manager: &mut CoinManager<T>): &Value {
    assert!(df::exists_(&manager.id, ADDITIONAL_METADATA_NAME), EAdditionalMetadataDoesNotExist);
    let meta: &Value = df::borrow(&manager.id, ADDITIONAL_METADATA_NAME);
    meta
}

/// Immutably borrows the additional metadata.
public fun get_additional_metadata<T, Value: store>(manager: &CoinManager<T>): &Value {
    assert!(df::exists_(&manager.id, ADDITIONAL_METADATA_NAME), EAdditionalMetadataDoesNotExist);
    let meta: &Value = df::borrow(&manager.id, ADDITIONAL_METADATA_NAME);
    meta
}

/// A one-time callable function to set a maximum mintable supply on a coin.
/// This can only be set once and is irrevertable.
public fun enforce_maximum_supply<T>(
    _: &CoinManagerTreasuryCap<T>,
    manager: &mut CoinManager<T>,
    maximum_supply: u64,
) {
    assert!(option::is_none(&manager.maximum_supply), EMaximumSupplyAlreadySet);
    assert!(maximum_supply <= MAX_SUPPLY, EMaximumSupplyHigherThanPossible);
    assert!(total_supply(manager) <= maximum_supply, EMaximumSupplyLowerThanTotalSupply);
    option::fill(&mut manager.maximum_supply, maximum_supply);
}

/// An irreversible action renouncing supply ownership which can be called if you hold the `CoinManagerTreasuryCap`.
/// This action provides `Coin` holders with some assurances if called, namely that there will
/// not be any new minting or changes to the supply from this point onward. The maximum supply
/// will be set to the current supply and will not be changed any more afterwards.
public fun renounce_treasury_ownership<T>(
    cap: CoinManagerTreasuryCap<T>,
    manager: &mut CoinManager<T>,
) {
    // Deleting the Cap
    let CoinManagerTreasuryCap { id } = cap;
    object::delete(id);

    // Updating the maximum supply to the total supply
    let total_supply = total_supply(manager);
    if (manager.has_maximum_supply()) {
        option::swap(&mut manager.maximum_supply, total_supply);
    } else {
        option::fill(&mut manager.maximum_supply, total_supply);
    };

    // Setting ownership renounced to true
    manager.supply_immutable = true;

    event::emit(TreasuryOwnershipRenounced {
        coin_name: type_name::into_string(type_name::get<T>()),
    });
}

/// An irreversible action renouncing manager ownership which can be called if you hold the `CoinManagerMetadataCap`.
/// This action provides `Coin` holders with some assurances if called, namely that there will
/// not be any changes to the metadata from this point onward.
public fun renounce_metadata_ownership<T>(
    cap: CoinManagerMetadataCap<T>,
    manager: &mut CoinManager<T>,
) {
    // Deleting the Cap
    let CoinManagerMetadataCap { id } = cap;
    object::delete(id);

    // Setting ownership renounced to true
    manager.metadata_immutable = true;

    event::emit(MetadataOwnershipRenounced {
        coin_name: type_name::into_string(type_name::get<T>()),
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
    manager.metadata_immutable || option::is_some(&manager.immutable_metadata)
}

/// Get a read-only version of the metadata, available for everyone.
public fun metadata<T>(manager: &CoinManager<T>): &CoinMetadata<T> {
    option::borrow(&manager.metadata)
}

/// Get a read-only version of the read-only metadata, available for everyone.
public fun immutable_metadata<T>(manager: &CoinManager<T>): &ImmutableCoinMetadata<T> {
    option::borrow(&manager.immutable_metadata)
}

/// Get the total supply as a number.
public fun total_supply<T>(manager: &CoinManager<T>): u64 {
    coin::total_supply(&manager.treasury_cap)
}

/// Get the maximum supply possible as a number.
/// If no maximum set it's the maximum u64 possible.
public fun maximum_supply<T>(manager: &CoinManager<T>): u64 {
    option::get_with_default(&manager.maximum_supply, MAX_SUPPLY)
}

/// Convenience function returning the remaining supply that can be minted still.
public fun available_supply<T>(manager: &CoinManager<T>): u64 {
    maximum_supply(manager) - total_supply(manager)
}

/// Returns if a maximum supply has been set for this Coin or not.
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
    ctx: &mut TxContext,
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
    value: u64,
): Balance<T> {
    assert!(total_supply(manager) + value <= maximum_supply(manager), EMaximumSupplyReached);
    coin::mint_balance(&mut manager.treasury_cap, value)
}

/// Destroy the coin `c` and decrease the total supply in `cap`
/// accordingly.
public entry fun burn<T>(
    _: &CoinManagerTreasuryCap<T>,
    manager: &mut CoinManager<T>,
    c: Coin<T>,
): u64 {
    coin::burn(&mut manager.treasury_cap, c)
}

/// Mint `amount` of `Coin` and send it to `recipient`. Invokes `mint()`.
public fun mint_and_transfer<T>(
    _: &CoinManagerTreasuryCap<T>,
    manager: &mut CoinManager<T>,
    amount: u64,
    recipient: address,
    ctx: &mut TxContext,
) {
    assert!(total_supply(manager) + amount <= maximum_supply(manager), EMaximumSupplyReached);
    coin::mint_and_transfer(&mut manager.treasury_cap, amount, recipient, ctx)
}

// === Update coin metadata ===

/// Update the `name` of the coin in the `CoinMetadata`.
public fun update_name<T>(
    _: &CoinManagerMetadataCap<T>,
    manager: &mut CoinManager<T>,
    name: string::String,
) {
    coin::update_name(&manager.treasury_cap, option::borrow_mut(&mut manager.metadata), name)
}

/// Update the `symbol` of the coin in the `CoinMetadata`.
public fun update_symbol<T>(
    _: &CoinManagerMetadataCap<T>,
    manager: &mut CoinManager<T>,
    symbol: ascii::String,
) {
    coin::update_symbol(&manager.treasury_cap, option::borrow_mut(&mut manager.metadata), symbol)
}

/// Update the `description` of the coin in the `CoinMetadata`.
public fun update_description<T>(
    _: &CoinManagerMetadataCap<T>,
    manager: &mut CoinManager<T>,
    description: string::String,
) {
    coin::update_description(
        &manager.treasury_cap,
        option::borrow_mut(&mut manager.metadata),
        description,
    )
}

/// Update the `url` of the coin in the `CoinMetadata`.
public fun update_icon_url<T>(
    _: &CoinManagerMetadataCap<T>,
    manager: &mut CoinManager<T>,
    url: ascii::String,
) {
    coin::update_icon_url(&manager.treasury_cap, option::borrow_mut(&mut manager.metadata), url)
}

// === Convenience functions ===

public fun decimals<T>(manager: &CoinManager<T>): u8 {
    if (option::is_some(&manager.metadata)) {
        coin::get_decimals(option::borrow(&manager.metadata))
    } else {
        option::borrow(&manager.immutable_metadata).decimals
    }
}

public fun name<T>(manager: &CoinManager<T>): string::String {
    if (option::is_some(&manager.metadata)) {
        coin::get_name(option::borrow(&manager.metadata))
    } else {
        option::borrow(&manager.immutable_metadata).name
    }
}

public fun symbol<T>(manager: &CoinManager<T>): ascii::String {
    if (option::is_some(&manager.metadata)) {
        coin::get_symbol(option::borrow(&manager.metadata))
    } else {
        option::borrow(&manager.immutable_metadata).symbol
    }
}

public fun description<T>(manager: &CoinManager<T>): string::String {
    if (option::is_some(&manager.metadata)) {
        coin::get_description(option::borrow(&manager.metadata))
    } else {
        option::borrow(&manager.immutable_metadata).description
    }
}

public fun icon_url<T>(manager: &CoinManager<T>): Option<Url> {
    if (option::is_some(&manager.metadata)) {
        coin::get_icon_url(option::borrow(&manager.metadata))
    } else {
        option::borrow(&manager.immutable_metadata).icon_url
    }
}
