// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Coin<IOTA> is the token used to pay for gas in IOTA.
/// It has 9 decimals, and the smallest unit (10^-9) is called "nano".
module iota::iota;

use iota::balance::Balance;
use iota::coin::{Self, Coin, TreasuryCap};
use iota::url;

const EAlreadyMinted: u64 = 0;
/// Sender is not @0x0 the system address.
const ENotSystemAddress: u64 = 1;

/// Name of the coin
public struct IOTA has drop {}

/// The IOTA token treasury capability.
/// Protects the token from unauthorized changes.
public struct IotaTreasuryCap has store {
    inner: TreasuryCap<IOTA>,
}

#[allow(unused_function)]
/// Register the `IOTA` Coin to acquire `IotaTreasuryCap`.
/// This should be called only once during genesis creation.
fun new(ctx: &mut TxContext): IotaTreasuryCap {
    assert!(ctx.sender() == @0x0, ENotSystemAddress);
    assert!(ctx.epoch() == 0, EAlreadyMinted);

    let (treasury, metadata) = coin::create_currency(
        IOTA {},
        9,
        b"IOTA",
        b"IOTA",
        b"The main (gas)token of the IOTA Network.",
        option::some(url::new_unsafe_from_bytes(b"https://iota.org/logo.png")),
        ctx,
    );

    transfer::public_freeze_object(metadata);

    IotaTreasuryCap {
        inner: treasury,
    }
}

public entry fun transfer(c: coin::Coin<IOTA>, recipient: address) {
    transfer::public_transfer(c, recipient)
}

/// Create an IOTA coin worth `value` and increase the total supply in `cap` accordingly.
public fun mint(cap: &mut IotaTreasuryCap, value: u64, ctx: &mut TxContext): Coin<IOTA> {
    assert!(ctx.sender() == @0x0, ENotSystemAddress);

    cap.inner.mint(value, ctx)
}

/// Mint some amount of IOTA as a `Balance` and increase the total supply in `cap` accordingly.
/// Aborts if `value` + `cap.inner.total_supply` >= U64_MAX
public fun mint_balance(cap: &mut IotaTreasuryCap, value: u64, ctx: &TxContext): Balance<IOTA> {
    assert!(ctx.sender() == @0x0, ENotSystemAddress);

    cap.inner.mint_balance(value)
}

/// Destroy the IOTA coin `c` and decrease the total supply in `cap` accordingly.
public fun burn(cap: &mut IotaTreasuryCap, c: Coin<IOTA>, ctx: &TxContext): u64 {
    assert!(ctx.sender() == @0x0, ENotSystemAddress);

    cap.inner.burn(c)
}

/// Destroy the IOTA balance `b` and decrease the total supply in `cap` accordingly.
public fun burn_balance(cap: &mut IotaTreasuryCap, b: Balance<IOTA>, ctx: &TxContext): u64 {
    assert!(ctx.sender() == @0x0, ENotSystemAddress);

    cap.inner.supply_mut().decrease_supply(b)
}

/// Return the total number of IOTA's in circulation.
public fun total_supply(cap: &IotaTreasuryCap): u64 {
    cap.inner.total_supply()
}

#[test_only]
public fun create_for_testing(ctx: &mut TxContext): IotaTreasuryCap {
    // The `new` function must be called here to be sure that the test function
    // contains all the important checks.
    new(ctx)
}
