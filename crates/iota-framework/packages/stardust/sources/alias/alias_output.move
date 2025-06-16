// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module stardust::alias_output;

use iota::bag::Bag;
use iota::balance::Balance;
use iota::dynamic_object_field;
use iota::transfer::Receiving;
use stardust::alias::Alias;

/// The Alias dynamic object field name.
const ALIAS_NAME: vector<u8> = b"alias";

/// Owned Object controlled by the Governor Address.
public struct AliasOutput<phantom T> has key {
    /// This is a "random" UID, not the AliasID from Stardust.
    id: UID,
    /// The amount of coins held by the output.
    balance: Balance<T>,
    /// The `Bag` holds native tokens, key-ed by the stringified type of the asset.
    /// Example: key: "0xabcded::soon::SOON", value: Balance<0xabcded::soon::SOON>.
    native_tokens: Bag,
}

// === Public-Mutative Functions ===

/// The function extracts assets from a legacy `AliasOutput`.
///    - returns the coin Balance,
///    - the native tokens Bag,
///    - and the `Alias` object that persists the AliasID=ObjectID from Stardust.
public fun extract_assets<T>(mut output: AliasOutput<T>): (Balance<T>, Bag, Alias) {
    // Load the related alias object.
    let alias = load_alias(&mut output);

    // Unpack the output into its basic part.
    let AliasOutput {
        id,
        balance,
        native_tokens,
    } = output;

    // Delete the output.
    object::delete(id);

    (balance, native_tokens, alias)
}

// === Public-Package Functions ===

/// Utility function to receive an `AliasOutput` object in other Stardust modules.
/// Other modules in the Stardust package can call this function to receive an `AliasOutput` object (nft).
public(package) fun receive<T>(
    parent: &mut UID,
    output: Receiving<AliasOutput<T>>,
): AliasOutput<T> {
    transfer::receive(parent, output)
}

/// Utility function to attach an `Alias` to an `AliasOutput`.
public fun attach_alias<T>(output: &mut AliasOutput<T>, alias: Alias) {
    dynamic_object_field::add(&mut output.id, ALIAS_NAME, alias)
}

// === Private Functions ===

/// Loads the `Alias` object from the dynamic object field.
fun load_alias<T>(output: &mut AliasOutput<T>): Alias {
    dynamic_object_field::remove(&mut output.id, ALIAS_NAME)
}

// === Test Functions ===

#[test_only]
public fun create_for_testing<T>(
    balance: Balance<T>,
    native_tokens: Bag,
    ctx: &mut TxContext,
): AliasOutput<T> {
    AliasOutput<T> {
        id: object::new(ctx),
        balance,
        native_tokens,
    }
}
