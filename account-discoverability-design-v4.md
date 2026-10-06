# Account discoverability index — design v4

> **Immutable `SmartAccount`s are deprecated.** Everything about them — `build_immutable_v1`, `create_immutable_account_v1`, `ImmutableAccountCreated`, the `immutable` field of `SmartAccountCreated` and the `immutable` column of `smart_accounts` — is kept here for reference only and is struck through where it appears. The design does not depend on them.

**Goal.** A wallet holding only a seed can find the `SmartAccount`s its public keys can unlock with one indexer query per public key, and anyone can rebuild the same index by replaying public events.

**Written against** branch [`vm-lang/12815-wallet-discoverability`](https://github.com/iotaledger/iota/tree/vm-lang/12815-wallet-discoverability), PR [#13071](https://github.com/iotaledger/iota/pull/13071).

The framework functions and events described here are those on the branch above. They are not final and may change. The implementation in the PR is given for reference only. What this document asks the reader to discuss is how the indexer should store the information a wallet needs to find its accounts, and how to make it available over RPC.

**Headline.**

1. The framework emits a Move event at every change to a `SmartAccount`: when the account is created, and when its public key or its authenticator changes.
2. The indexer reads those events from checkpoints and stores, in PostgreSQL, which accounts each public key is attached to, keyed by `key_id`, a hash of the public key.
3. The new `iotax_getAccountsByPublicKey` indexer RPC answers the following question: given a public key, which accounts is it attached to, and can the IOTA wallet authenticate each one?

---

## Basic concepts

**Address.** An IOTA address is normally derived from a public key: it is the hash of the public key, with a scheme flag in front for every scheme except Ed25519. A transaction sent from that address is valid when it carries a signature made with the matching private key. Whoever holds the private key controls the address, and the address cannot switch to another public key.

**Account.** With account abstraction, an object can also send transactions. Such an object is an account: its address is its object ID, and what proves that a transaction may act for it is a Move function, its authenticator function, instead of a fixed public key. The account stores a reference to this function (package, module and function name) in a dynamic field. When a transaction names the account as sender, the node calls the authenticator function with the account, the arguments the sender supplied (for example a signature) and the contents of the transaction. If the function returns, the transaction is accepted; if it aborts, the transaction is rejected. The account can replace its authenticator function later. The `iota::account` module creates accounts of any Move type and replaces their authenticator function. See the IOTA documentation on account abstraction (`docs/content/developer/account-abstraction.mdx`).

**`SmartAccount`.** `SmartAccount` (module `iota::smart_account`) is the account type the framework provides, so that users do not have to write their own. It keeps all its data in dynamic fields and can use any authenticator function. After creation, only transactions sent by the account itself can change it: attach, detach or replace its public key, or replace its authenticator function.

**Built-in authenticators.** The framework provides one authenticator function per standard signature scheme: Ed25519, Secp256k1, Secp256r1, MultiSig and Passkey (module `iota::builtin_authenticator_functions`). Each one checks the transaction's signature against the public key stored on the account. These are the authenticators a wallet that signs with private keys, such as the IOTA wallet, can sign for. Any other authenticator function is called a custom authenticator in this document.

**Claiming an address.** A `ClaimAccount` transaction turns an address derived from a public key into a `SmartAccount`. The owner of the address signs it with their private key. The framework then creates a `SmartAccount` whose ID is that same address, with the built-in authenticator for the public key's scheme and the public key attached. The owner keeps signing with the same private key.

**Dynamic field.** A value attached to an object under a name, stored as a separate object. Given the parent object's ID and the field name, the field can be read directly.

**Event.** Move code can emit events while a transaction runs. They are recorded in the transaction's effects and in checkpoints, and anyone can read them, but they are not chain state: Move code cannot read them back.

**Indexer.** `iota-indexer` is a separate service. It reads checkpoints from a fullnode, stores data in PostgreSQL, and serves extra RPC methods in the `iotax_` namespace.

---

## Problem statement

A `SmartAccount` address is not derived from a public key, and after creation the account can change both the public key used to authenticate it and its authenticator function. A wallet restored from a seed knows only its public keys, so it cannot tell from them alone which accounts they unlock.

The IOTA wallet therefore needs to know, for each of its public keys, which accounts use that public key to authenticate, and whether each account's authenticator function is one the wallet can sign for. On chain, each account stores its own public key, but nothing maps a public key to the accounts that use it.

For this reason the indexer maintains a mapping from public keys to accounts. It builds the mapping from the events the Move modules emit whenever an account is created or changes its public key or its authenticator function.

---

## 1. Background

### Framework modules

- `iota::smart_account` — the framework `SmartAccount` and its builders: `builder_v1` (any authenticator), `builtin_auth_builder_v1` (a `PublicKey` plus the built-in authenticator of its scheme), finished by `build_v1` (shared) or ~~`build_immutable_v1` (frozen)~~. After creation, the public key and the authenticator change only in transactions sent by the account itself.
- `iota::builtin_authenticator_functions` — the five built-in authenticators (Ed25519, Secp256k1, Secp256r1, MultiSig, Passkey) and `attach_public_key` / `detach_public_key` / `rotate_public_key`, which emit `PublicKeyAttached` / `PublicKeyDetached` / `PublicKeyRotated`.
- `iota::account` — creates accounts and rotates their authenticator, emitting `MutableAccountCreated` / ~~`ImmutableAccountCreated`~~ / `AuthenticatorFunctionRefV1Rotated`.
- **Claiming** — the `ClaimAccount` transaction (see Basic concepts).

### Framework events

The index is built from these events.

- `PublicKeyAttached { account_id, public_key }` (emitted by `builtin_authenticator_functions` module) — a public key was attached to an account that had none. Emitted by `attach_public_key`, which is called by `builtin_auth_builder_v1`, by the claim path, and by `attach_builtin_auth_public_key` on an existing account.
- `PublicKeyDetached { account_id, public_key }` (emitted by `builtin_authenticator_functions` module) — the public key was removed. Emitted by `detach_public_key`, called by `detach_builtin_auth_public_key`.
- `PublicKeyRotated { account_id, from, to }` (emitted by `builtin_authenticator_functions` module) — the public key was replaced by another. Emitted by `rotate_public_key`, called by `rotate_builtin_auth_public_key`.
- `SmartAccountCreated { account_id, public_key }`, plus ~~`immutable`~~ (emitted by `smart_account` module) — a framework `SmartAccount` was created. Emitted by `build_v1` and ~~`build_immutable_v1`~~, so once for every `SmartAccount`, with `public_key = none` when it has no public key.
- `MutableAccountCreated<SmartAccount>`, ~~`ImmutableAccountCreated<SmartAccount>`~~ (emitted by `account` module) — the account was created with this authenticator. Emitted by `account::create_account_v1` / ~~`create_immutable_account_v1`~~, which `build_v1` / ~~`build_immutable_v1`~~ call right after emitting `SmartAccountCreated`.
- `AuthenticatorFunctionRefV1Rotated<SmartAccount>` (emitted by `account` module) — the authenticator was replaced. Emitted by `account::rotate_auth_function_ref_v1`, called by `smart_account::rotate_auth_function_ref_v1`.

The three `iota::account` events are generic over the account type. The bytecode verifier lets only the module that defines the type call the functions that emit them, so an event with type parameter `0x2::smart_account::SmartAccount` always comes from `iota::smart_account`.

### Account states

A `SmartAccount` has a public key attached or not, and a built-in or a custom authenticator. That gives four states:

1. **Public key + built-in authenticator** (green). Found by its public key; the IOTA wallet can authenticate it.
2. **Built-in authenticator, no public key** (red). Not found by any public key, and cannot authenticate any transaction.
3. **Custom authenticator, no public key** (gray). Not found by any public key.
4. **Public key + custom authenticator** (yellow). Found by its public key, but the IOTA wallet cannot authenticate it.

> **Diagram:** `account-discoverability-states.drawio` (on Confluence as an image).

Every state change is a call to a Move function, and each of these functions emits the events the indexer reads. Each arrow in the diagram is one function:

| Arrow | Function | Events |
| --- | --- | --- |
| `builtin_auth_builder_v1` | `smart_account::builtin_auth_builder_v1` | `PublicKeyAttached` |
| `claim` | `ClaimAccount` transaction (`smart_account::claim_account_v1`) | `PublicKeyAttached` |
| `builder_v1(builtin)`, `builder_v1(custom)` | `smart_account::builder_v1` | none |
| `build_v1` | `smart_account::build_v1` / ~~`build_immutable_v1`~~ | `SmartAccountCreated`, then `MutableAccountCreated` / ~~`ImmutableAccountCreated`~~ |
| `attach_pk` | `smart_account::attach_builtin_auth_public_key` | `PublicKeyAttached` |
| `detach_pk` | `smart_account::detach_builtin_auth_public_key` | `PublicKeyDetached` |
| `rotate_pk` | `smart_account::rotate_builtin_auth_public_key` | `PublicKeyRotated` |
| `rotate_auth(builtin)`, `rotate_auth(custom)` | `smart_account::rotate_auth_function_ref_v1` | `AuthenticatorFunctionRefV1Rotated` |

The builder and `build_v1` steps run in the same transaction, so creating an account with a public key emits `PublicKeyAttached`, `SmartAccountCreated` and the `MutableAccountCreated` creation event, in that order.

An account that reaches state 2 (red) cannot authenticate a transaction, so it is basically frozen.

### Where the state lives on chain

All of an account's state is on its object. The object has type `0x2::smart_account::SmartAccount` and is shared (mutable) or ~~immutable~~. It has two dynamic fields:

- `Field<builtin_authenticator_functions::PublicKeyFieldName, PublicKey>` — the public key. Present only when a public key is attached.
- `Field<account::AuthenticatorFunctionRefV1Key, AuthenticatorFunctionRefV1<SmartAccount>>` — the authenticator. Always present. It is built-in when its package is `0x2` and its module is `builtin_authenticator_functions`.

A dynamic field's object ID is derived from the account ID and the field name (`derive_dynamic_field_id` in `iota-types`), so for a known account both can be read directly, for example with `iotax_getDynamicFieldObject`.

---

## 2. Design

> **Diagram:** `account-discoverability-data-flow.drawio` (on Confluence as an image).

### 2.1 Build the index from events

The link from an account to its public key is already on chain, as the dynamic fields previously mentioned. What is missing is the opposite direction, from a public key to its accounts. The index builds it from the events without storing anything new on chain:

- `PublicKeyAttached`, `PublicKeyDetached`, `PublicKeyRotated` give the links. Every change to a `SmartAccount`'s public key goes through one of them. A `SmartAccount` can never be deleted, so no link goes stale through deletion.
- `SmartAccountCreated` marks the object as a framework `SmartAccount`, with or without a public key. A `PublicKeyAttached` alone does not: `attach_public_key` is `public` over any `UID`, so it can come from any object.
- `MutableAccountCreated`, `AuthenticatorFunctionRefV1Rotated`, for `SmartAccount` only, give the current authenticator.

Framework changes: add `SmartAccountCreated`; remove `SmartAccountClaimed` and `public_key::key_id`, which nothing on chain used. The new event is the only change that affects consensus.

### 2.2 Public key identity: `key_id`

```
key_id = blake2b256( scheme_flag || raw_key_bytes )    // 32 bytes
```

The index is keyed by `key_id`, not by the address the public key derives:

- **It cannot fail.** It hashes bytes without parsing them. Address derivation returns an error for a malformed MultiSig committee, and the indexer reads chain bytes it does not re-validate.
- **It is the same for every scheme.** Address derivation omits the flag for Ed25519 and hashes a structured preimage for MultiSig.

It has no protocol role (authentication still checks the derived address) and is defined only in Rust (`iota-types`), since every event carries the full `PublicKey`. Adding it to Move later would be additive.

### 2.3 Store them in the indexer

The index models two relations:

1. **Public key ↔ account.** A public key can be attached to many accounts, and an account has had different public keys over time. Each pair has a status, active or unlinked, and the change that last touched it. Pairs can be about any object, since `attach_public_key` takes any `UID`.
2. **Account → its properties.** For each framework `SmartAccount`: that it is one, ~~whether it is immutable,~~ and its current authenticator.

The indexer folds the events in chain order `(checkpoint, transaction, event)` into three PostgreSQL tables of current state (columns and rules in Appendices B and C):

- `account_key_links` — relation 1: one row per public key and account.
- `smart_accounts` — relation 2: one row per framework `SmartAccount`.
- `account_authenticators` — relation 2: each `SmartAccount`'s current authenticator, one of the five built-in ones or `custom`.

Relation 2 is held in two tables, both keyed by `account_id` and joined one to one at query time, because they are written from different events under different rules: a `smart_accounts` row is written when the account is created, from `SmartAccountCreated`; an `account_authenticators` row is written at creation and overwritten at every authenticator rotation, from the `iota::account` events.

Guarantees:

- **Deterministic.** Replaying the same checkpoints gives the same rows, so a second indexer can be checked row for row.
- **Idempotent and monotonic.** Every write applies only if its transaction is not older than the stored one.
- **Not prunable.** The tables are state, not history; a pruned row could only be rebuilt from events the node may itself have pruned.

### 2.4 Query: `iotax_getAccountsByPublicKey`

Served only by the indexer (`iota-node` does not register the `iotax` namespace).

**Parameters:** `public_key`, Base64 of `scheme_flag || raw_key_bytes` (the format of `iota::public_key::from_prefixed_bytes`); `include_unlinked` (optional, default `false`), which also returns links the public key was detached or rotated away from.

**Result:** the accounts linked to the public key, newest change first:

| Field | Meaning |
| --- | --- |
| `address` | The account's address. |
| `status` | `active` (attached now) or `unlinked` (only with `include_unlinked`). |
| `source` | What last changed the link: `attach`, `rotate` or `detach`. |
| `smartAccount` | Whether the address is a framework `SmartAccount`. |
| `authenticator` | `ed25519`, `secp256k1`, `secp256r1`, `multisig`, `passkey` (built-in) or `custom`; `null` when not a `SmartAccount`. |
| `scheme` | The public key's scheme flag, as a number. |
| `lastChangeEpoch` | Epoch of the last change to the link. |

```json
{ "jsonrpc": "2.0", "id": 1, "method": "iotax_getAccountsByPublicKey",
  "params": ["AMxiMy40uy1c1p9g77sqNsuRbH60WDAeo2Y2xNuwEr2I", false] }
```

```json
{ "jsonrpc": "2.0", "id": 1, "result": [
  { "address": "0xcef6bafea1d59edb73ff5ec9e8aa58354796e1b572b695d64237ce9c15a34a03",
    "status": "active", "source": "attach", "smartAccount": true,
    "authenticator": "ed25519", "scheme": 0, "lastChangeEpoch": "3" } ] }
```

### 2.5 Wallet flow

1. Derive the wallet's public keys from the seed, for every supported scheme and derivation path.
2. Call `iotax_getAccountsByPublicKey` once per public key.
3. Keep the results with `status = active`, `smartAccount = true` and a built-in `authenticator`: these are the accounts the wallet can unlock. `custom` ones can be shown as found but not usable.
4. Optionally, check each kept account on chain: it exists, its public key is the queried one, its authenticator matches.

For live updates, subscribe with `iota_subscribeEvent` to the three modules of §2.1.

**Example: which accounts can public key `K` unlock?**

> **Diagram:** `account-discoverability-query.drawio` (on Confluence as an image).

| Tx | What happens | Returned | Wallet keeps it |
| --- | --- | --- | --- |
| 101 | `builtin_auth_builder_v1(K)` + `build_v1` creates `A1` | yes, `ed25519` | **yes** |
| 102 | `ClaimAccount` with `K` creates `A2` | yes, `ed25519` | **yes** |
| 103–104 | `A3` is built with `K`, then rotates to a custom authenticator | yes, `custom` | no: the wallet cannot sign for it |
| 105–106 | `A4` is built with `K`, then rotates its public key from `K` to `K2` | no: link is unlinked | — |
| 107 | A third-party package attaches `K` to its own object `O5` | yes, `smartAccount = false` | no: not a `SmartAccount` |

`K` can unlock `A1` and `A2`.

### 2.6 Trust model and limitations

- **Results are not authenticated.** A link is a true on-chain fact, not an endorsement. A wallet that needs certainty checks the account on chain.
- **Anyone can use your public key for an account.** `builtin_auth_builder_v1` takes any `PublicKey`, so anyone can create accounts with your public key for the price of gas. Only you can operate them, but they appear in your results.
- **No paging.** Because of the point above, one public key's result can grow without bound. Accepted for now.
- **A MultiSig account is found only by its whole MultiSig public key.** Its `key_id` hashes the whole committee, and the member public keys are not indexed, so a wallet that holds one member public key cannot find the account.
- **An account can lock itself**, by removing the only public key it can sign with or rotating to an authenticator nobody can satisfy.
- **A public key and an authenticator of different schemes** are reported as they are; keeping them consistent is up to whoever rotates them.

---

## 3. Alternatives considered

- **On-chain registry:** a `ClaimRegistry` shared object at `0x10` with dynamic-field markers. It adds chain state for data that is only read off chain, and puts a shared object, so consensus, on every claim.
- **Fullnode database:** index the same events in the fullnode's own store and serve the query from the fullnode's RPC. A wallet would not depend on an indexer deployment, but every fullnode that enables it carries the tables. _Why the indexer was chosen: to be completed._

---

## 4. Rollout

- The features the index depends on, `enable_builtin_move_authenticators` and `enable_claim_account_transaction`, are enabled at protocol version 38, on every chain except testnet and mainnet.
- The indexer builds the tables from genesis; no migration or backfill.
- An indexer must not read a network whose framework predates `SmartAccountCreated`: its accounts would get no `smart_accounts` row and be reported with `smartAccount = false`.

---

## 5. Decisions needed

1. **What the query returns.** (a) Every link, with `smartAccount` and `authenticator`, and the wallet decides what it can unlock (as implemented). (b) By default only active `SmartAccount`s with a built-in authenticator, the rest behind a flag.
2. **Restoring the indexer from a formal snapshot.** `iota-indexer restore` loads objects, not events, so the tables miss everything before the snapshot, with no error. (a) Accept and document it. (b) Rebuild the current state from the restored `objects` table: the `SmartAccount` objects and their two dynamic fields, found by exact object type and decoded one by one. This gives the active links, `smart_accounts` and authenticators; unlinked rows and the transaction and epoch of each change would still be lost.
3. **Links to objects that are not a `SmartAccount`.** (a) Keep them, with `smartAccount = false` (as implemented). (b) Leave them out of the RPC result. (c) Restrict `attach_public_key` in Move, a framework change.

---

## Appendix A. Events

| Event | Module | Fields |
| --- | --- | --- |
| `PublicKeyAttached` | `builtin_authenticator_functions` | `account_id: ID, public_key: PublicKey` |
| `PublicKeyDetached` | `builtin_authenticator_functions` | `account_id: ID, public_key: PublicKey` |
| `PublicKeyRotated` | `builtin_authenticator_functions` | `account_id: ID, from: PublicKey, to: PublicKey` |
| `SmartAccountCreated` (new) | `smart_account` | `account_id: ID, public_key: Option<PublicKey>`, ~~`immutable: bool`~~ |
| `MutableAccountCreated<SmartAccount>` / ~~`ImmutableAccountCreated<SmartAccount>`~~ | `account` | `account_id: ID, authenticator: AuthenticatorFunctionRefV1` |
| `AuthenticatorFunctionRefV1Rotated<SmartAccount>` | `account` | `account_id: ID, from: AuthenticatorFunctionRefV1, to: AuthenticatorFunctionRefV1` |

`PublicKey` is `{ scheme: SignatureScheme { flag: u8 }, raw_bytes: vector<u8> }`; `AuthenticatorFunctionRefV1` is `{ package: ID, module_name: ascii::String, function_name: ascii::String }`.

## Appendix B. Fold rules

| Event | Effect |
| --- | --- |
| `PublicKeyAttached(account, pk)` | link `(key_id(pk), account)` active, source `attach` |
| `PublicKeyRotated(account, from, to)` | `(key_id(from), account)` unlinked, **then** `(key_id(to), account)` active; source `rotate` |
| `PublicKeyDetached(account, pk)` | `(key_id(pk), account)` unlinked, source `detach` |
| `SmartAccountCreated(account, …)` | `smart_accounts` row |
| `MutableAccountCreated<SmartAccount>(account, auth)` / ~~`ImmutableAccountCreated<SmartAccount>`~~ | `account_authenticators` row, kind of `auth` |
| `AuthenticatorFunctionRefV1Rotated<SmartAccount>(account, _, to)` | `account_authenticators` row, kind of `to` |

- The unlink of `from` comes before the link of `to`, so rotating a public key onto itself leaves it active.
- The public key in `SmartAccountCreated` is not used for links: the `PublicKeyAttached` of the same transaction gives it.
- Only `iota::account` events whose type parameter is `0x2::smart_account::SmartAccount` are read.
- An event whose type matches but whose payload does not decode is skipped: the indexer is older than the framework.

**Authenticator kind.** Built-in when the ref's package is `0x2` and its module is `builtin_authenticator_functions`: `ed25519_authenticator_function_ref_v1` → `ed25519` (1), and likewise `secp256k1` (2), `secp256r1` (3), `multisig` (4), `passkey` (5). Anything else is `custom` (6).

## Appendix C. Tables

All three: primary key as shown, no foreign keys (joined on `account_id` at query time), not prunable. Stored numbers for `source` and `kind` must never be renumbered.

`account_key_links` — primary key `(key_id, account_id)`; indexes on `(account_id)` and on `(key_id) WHERE status = 0`.

| Column | Type | Meaning |
| --- | --- | --- |
| `key_id` | `BYTEA` | `key_id` of the public key (§2.2) |
| `account_id` | `BYTEA` | the object the public key is attached to |
| `scheme` | `SMALLINT` | scheme flag as recorded on chain, stored even if unknown to the build |
| `source` | `SMALLINT` | 0 attach, 1 rotate, 2 detach |
| `status` | `SMALLINT` | 0 active, 1 unlinked |
| `last_change_tx_sequence_number` | `BIGINT` | orders results and guards writes |
| `last_change_epoch` | `BIGINT` | returned as `lastChangeEpoch` |

`smart_accounts` — primary key `account_id`; columns ~~`immutable BOOLEAN`~~, `created_tx_sequence_number`, `created_epoch`. A second claim of the same address keeps the later transaction.

`account_authenticators` — primary key `account_id`; columns `kind SMALLINT` (Appendix B), `last_change_tx_sequence_number`, `last_change_epoch`.

**Example.** In epoch 3, tx 101 runs `builtin_auth_builder_v1(K)` + `build_v1` (creates `A1`), and tx 102 is a `ClaimAccount` with `K` (creates `A2`). Each emits `PublicKeyAttached`, `SmartAccountCreated` and `MutableAccountCreated<SmartAccount>`, and writes one row per table:

> **Diagram:** insert `account-discoverability-table-fill.drawio` here with the draw.io macro.

| Table | tx 101 | tx 102 |
| --- | --- | --- |
| `account_key_links` | `key_id(K)`, `A1`, scheme 0, attach, active, tx 101, epoch 3 | `key_id(K)`, `A2`, scheme 0, attach, active, tx 102, epoch 3 |
| `smart_accounts` | `A1`, ~~immutable false~~, tx 101, epoch 3 | `A2`, ~~immutable false~~, tx 102, epoch 3 |
| `account_authenticators` | `A1`, kind 1 (ed25519), tx 101, epoch 3 | `A2`, kind 1 (ed25519), tx 102, epoch 3 |

## Appendix D. `key_id` fixed values

| Prefixed public key bytes (hex) | `key_id` (hex) |
| --- | --- |
| Ed25519: `00cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88` | `43541042c153e0e498a08a8db868f1614c9366694fa730bd8a07fc5d7c931f0d` |
| 1-of-1 MultiSig (that Ed25519 public key): `030100cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88010100` | `37330e88388d526046696b5b5113cd64e81eb1b1bcd403372666cc54970ddbf4` |
| 1-of-2 MultiSig (Ed25519 + Secp256k1): `030200cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88010102337cca2171fdbfcfd657fa59881f46269f1e590b5ffab6023686c7ad2ecc2c1c010100` | `dd22eb5c98cdc27de98174a69b68ca1603bdda8aeb226c5232273cfdc9655811` |
