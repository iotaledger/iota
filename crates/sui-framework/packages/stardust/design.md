# Stardust on Move

## Migrating Basic Outputs

Every Basic Output has an `Address Unlock` and an `IOTA` balance (u64). Depending on what other fields we have, we will create different objects.

[Decision graph on what to do with a basic output during migration](./basic_migration_graph.svg)

![](./basic_migration_graph.svg)

## User Flow

Majority of user funds are sitting in Basic Outputs without unlock conditions. Such tokens will be migrated to `0x2::coin::Coin<IOTA>` which one can directly use as a gas payment object.
If a user does not end up with such coin objects at migration, we will have to sponsor their transaction to extract assets.

- We can directly ask back the gas fee from the migrated object
- Take a look at the `test` function inside [`stardust::basic`](./sources/basic.move) on how to construct a PTB for a user to claim all assets and fuflill unlock conditions.
