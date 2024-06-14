# Stardust on Move

This document describes how Stardust UTXOs are migrated to the Move-based ledger. The document is split into sections of
each stardust output type. It doesn't describe every possible edge case. For that, please check out the code which is
part of a sub directory in the [`iota-genesis-builder`](../../../iota-genesis-builder/src/stardust/) crate. This
document is meant to give a high- to mid-level overview of the migration process.

## Foundry Outputs & Native Tokens

Foundry Outputs are migrated collectively before any other output. They do not have a corresponding object
representation on the Move side, but rather are represented by a variety of Move-native types.

First, the foundry output's IRC-30 Metadata is extracted. In the majority of cases this is possible, but if, for
example, the symbol of the token is not a valid move identifier (e.g. containing UTF-8 characters) or the metadata does
not follow the IRC-30 standard, a random identifier for the token package is generated and the metadata uses generic
defaults. As much as possible though, the migration attemps to do as little modifications to the input UTXO as possible.

A
[move package template](../../../iota-genesis-builder/src/stardust/native_token/package_template/sources/native_token_template.move)
is filled with the extracted data, compiled and published.

The result of the foundry migration is then:

- A package representing the native token, in particular containing a one-time witness representing the unique type of
  the native token (to be used as `Coin<package_id>::<token_name>::NativeTokenOneTimeWitness>`; abbreviated in the rest
  of this document).
- A minted coin (`Coin<NativeTokenOneTimeWitness>`), containing the entire circulating supply of the native tokens,
  owned by the `0x0` address.
- A gas coin (`Coin<IOTA>`), containing the migrated IOTA tokens of the foundry, owned by the address of the alias, that
  owned the original foundry.

After this process, the minted coin sits on the `0x0` address with the entire minted supply. When other output types are
migrated that contain a balance of _this_ native token, then that balance is split off of the minted coin into a new
`Coin` object, which is then owned by the migrated output. If by the end of this process a non-zero balance remains on
the minted coin they are left on the zero address. This means they were burned in stardust and therefore are effectively
also burned on the Move ledger, as no one controls the `0x0` address.

## Output Migration Design

Outputs that are not foundries are migrated using a common pattern. Their Move smart contract has a function called
`extract_assets` which returns all of the migrated assets. Generally, these outputs are migrated to an object which
contains the other associated assets in static or dynamic fields. Those assets can be the `Coin<IOTA>`,
`Coin<NativeTokenOneTimeWitness>` or another object. In particular, Native Tokens are stored in a `Bag` where the token
is behind a key of its own name (`<package_id>::<token_name>::NativeTokenOneTimeWitness`) and the stored value is of
type `Balance<NativeTokenOneTimeWitness>`. If an outupt owned multiple native tokens, the bag contains multiple keys.

The extraction function enforces that any potentially present unlock conditions, like `Timelock`, `Expiration` or
`Storage Deposit Return` are enforced. For instance, if a timelocked basic ouput's assets are attempted to be extracted,
the transaction would fail, just like it would in stardust.

Address Ownership is migrated directly by converting `Ed25519 Address`, `Alias Address` or `Nft Address` to an
`IotaAddress` without any modification. For `Ed25519 Address`es this means that their original backing keypair can
simply continue to be used to unlock objects in Move. The `Alias` and `Nft` address types represent object ownership.
Those are effectively migrated as a transfer. For example, an `Alias Output A` owning a `Basic Output B` in Stardust, is
migrated as setting the `owner` of `B` to the address of `A` (the Alias ID), which is equivalent to the Alias Address in
Stardust. This is the same as if `B` would have been transferred (using either of
`iota::transfer::{transfer,public_transfer}`) to the address of `A` (the Alias ID). The migrated alias `A` can then
_receive_ `B` using the `stardust::address_unlock_condition::unlock_alias_address_owned_basic` function, which is
essentially a wrapper around `iota::transfer::receive`. There are equivalent unlock functions for the other possible
variants of object ownership.

## Basic Outputs

Every Basic Output has an `Address Unlock` and some `coin` balance (`u64`). Depending on what other fields it has,
different objects are created. The most common case is that any output without special unlock conditions (or an expired
`Timelock`) are migrated to a `Coin<IOTA>` object which can be directly used as a gas object.

The migrated objects are owned by the address in the stardust output's `Address Unlock Condition`, except when an
`Expiration Unlock Condition` is present in which case the object is a _shared_ one.

A special case are vesting reward outputs, that is, those Basic Outputs whose `OutputId` begins with
`0xb191c4bc825ac6983789e50545d5ef07a1d293a98ad974fc9498cb18` and whose `Timelock` is still locked at the time of
migration. They are migrated to `Timelock<IOTA>` and contain the label
`00000000000000000000000000000000000000000000000000000000000010cf::stardust_upgrade_label::STARDUST_UPGRADE_LABEL` by
which they can be identified as vesting reward objects.

The full decision graph (without the vesting reward output case) is depicted here (with `coin` being `IOTA`):

[Decision graph on what to do with a basic output during migration](./basic_migration_graph.svg)

![](./basic_migration_graph.svg)

## Alias Outputs

Alias Outputs are migrated to two Move objects:

- `AliasOutput` object, containing the `Balance<IOTA>` and a `Bag` of native tokens in a static field.
- `Alias` object that is owned by `AliasOutput` in a dynamic object field.

Other noteworthy points:

- The `AliasOutput` is owned by the address in the stardust output's `Governor Address Unlock Condition`. There is no
  concept of a state controller on the Move side, and so the `State Controller` address from Stardust is functionally
  discarded, although it can be accessed in the `Alias` object.
- The `AliasOutput` object has a freshly generated `UID` while the `Alias` has its `UID` set to the `Alias ID` of the
  stardust output. If the `Alias ID` was zeroed in the stardust output, it is computed according to the protocol rules
  from [TIP-18](https://wiki.iota.org/tips/tips/TIP-0018/) and then set. Hence, no `Alias` object in Move has a zeroed
  `UID`.
- The Foundry Counter in Stardust is used to give foundries a unique ID. The Foundry ID is the concatenation of
  `Address || Serial Number || Token Scheme Type`. In Move the foundries are represented by unique packages that define
  the corresponding Coin Type (a one time witness) of the Native Token. Because the foundry counter can no longer be
  enforced to be incremented when a new package is deployed, which defines a native token and is owned by that Alias,
  the Foundry Counter becomes meaningless. Hence, it is not migrated and does not have an equivalent field in Move. The
  same count can be determined (off-chain) by counting the number of `TreasuryCap`s the Alias owns. In fact, the
  stardust constraint that foundries can only be owned by aliases is no longer enforced in the Move version.

## Nft Outputs

Nft Outputs are migrated - very simliar to Alias Outputs - to two Move objects:

- `NftOutput` object, containing the `Balance<IOTA>` and a `Bag` of native tokens in a static field.
- `Nft` object that is owned by `NftOutput` in a dynamic object field.

Other noteworthy points:

- The `NftOutput` is owned by the address in the stardust output's `Address Unlock Condition`.
- The `NftOutput` object has a freshly generated `UID` while the `Nft` has its `UID` set to the `Nft ID` of the stardust
  output. If the `Nft ID` was zeroed in the stardust output, it is computed according to the protocol rules from
  [TIP-18](https://wiki.iota.org/tips/tips/TIP-0018/) and then set. Hence, no `Nft` object in Move has a zeroed `UID`.
- The `Nft` move object contains an `Irc27Metadata` object. It is extracted from the immutable metadata of the stardust
  Nft, if possible. If the stardust Nft does not have a valid IRC-27 metadata, it is migrated on a best-effort basis.
  The interested reader is invited to examine the `convert_immutable_metadata` function in the migration code for more
  details.
