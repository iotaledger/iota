// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module stardust::address_unlock_condition;

use iota::coin_manager::CoinManagerTreasuryCap;
use iota::transfer::Receiving;
use stardust::alias::Alias;
use stardust::alias_output::{Self, AliasOutput};
use stardust::basic_output::{Self, BasicOutput};
use stardust::nft::Nft;
use stardust::nft_output::{Self, NftOutput};

// === Receiving on Alias Address/AliasID as ObjectID ===

/// Unlock a `BasicOutput` locked to the alias address.
public fun unlock_alias_address_owned_basic<T>(
    self: &mut Alias,
    output_to_unlock: Receiving<BasicOutput<T>>,
): BasicOutput<T> {
    basic_output::receive(self.id(), output_to_unlock)
}

/// Unlock an `NftOutput` locked to the alias address.
public fun unlock_alias_address_owned_nft<T>(
    self: &mut Alias,
    output_to_unlock: Receiving<NftOutput<T>>,
): NftOutput<T> {
    nft_output::receive(self.id(), output_to_unlock)
}

/// Unlock an `AliasOutput` locked to the alias address.
public fun unlock_alias_address_owned_alias<T>(
    self: &mut Alias,
    output_to_unlock: Receiving<AliasOutput<T>>,
): AliasOutput<T> {
    alias_output::receive(self.id(), output_to_unlock)
}

/// Unlock a `CoinManagerTreasuryCap` locked to the alias address.
public fun unlock_alias_address_owned_coinmanager_treasury<T>(
    self: &mut Alias,
    treasury_to_unlock: Receiving<CoinManagerTreasuryCap<T>>,
): CoinManagerTreasuryCap<T> {
    transfer::public_receive(self.id(), treasury_to_unlock)
}

// === Receiving on NFT Address/NFTID as ObjectID ===

/// Unlock a `BasicOutput` locked to the `Nft` address.
public fun unlock_nft_address_owned_basic<T>(
    self: &mut Nft,
    output_to_unlock: Receiving<BasicOutput<T>>,
): BasicOutput<T> {
    basic_output::receive(self.id(), output_to_unlock)
}

/// Unlock an `NftOutput` locked to the `Nft` address.
public fun unlock_nft_address_owned_nft<T>(
    self: &mut Nft,
    output_to_unlock: Receiving<NftOutput<T>>,
): NftOutput<T> {
    nft_output::receive(self.id(), output_to_unlock)
}

/// Unlock an `AliasOutput` locked to the `Nft` address.
public fun unlock_nft_address_owned_alias<T>(
    self: &mut Nft,
    output_to_unlock: Receiving<AliasOutput<T>>,
): AliasOutput<T> {
    alias_output::receive(self.id(), output_to_unlock)
}
