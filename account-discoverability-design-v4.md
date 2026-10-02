# Account discoverability index — design v4

**Goal.** A wallet holding only a seed can find the `SmartAccount`s its keys can unlock with one indexer query per
key, and anyone can rebuild the same index by replaying public events.

**Written against** PR [#13071](https://github.com/iotaledger/iota/pull/13071) (fixes
[#12815](https://github.com/iotaledger/iota/issues/12815)), on
[`vm-lang/10722-default-iota-account-alpha`](https://github.com/iotaledger/iota/tree/vm-lang/10722-default-iota-account-alpha).

**Headline.**

1. The link between a public key and an account is persisted as Move events. The key events already exist; one new
   framework event, `SmartAccountCreated`, is added.
2. The indexer reads those events from checkpoints and stores the current links in PostgreSQL, keyed by `key_id`, a
   hash of the public key.
3. A new indexer RPC, `iotax_getAccountsByPublicKey`, answers: given a public key, which accounts is it attached to,
   and can the IOTA wallet authenticate each one?

No new chain state. Everything except the event is indexer and RPC.

---

## 0. Status of earlier versions

| | Written against | Status |
| --- | --- | --- |
| **v1** — _[Alt 3] Event Stream + Replayable Off-Chain Indexer_ | a `ClaimRegistry` at `0x10` with dynamic-field claim markers | Superseded |
| **v2** — _Account Discoverability Index Design V2 [DEPRECATED]_ | `vm-lang/12815-account-discoverability-events` | Superseded |
| **v3** — _Account Discoverability Index Design V3_ | `vm-lang/10722-default-iota-account-alpha` @ `3f72b2497b` | Superseded |
| **v4** — this document | PR #13071 on `vm-lang/10722-default-iota-account-alpha` | Current |

What v4 changes from v3:

* **`SmartAccountCreated` replaces `SmartAccountClaimed`.** It is emitted for every framework `SmartAccount`, not only
  claimed ones, with an optional key. The `claimed_accounts` table becomes `smart_accounts`. A claimed account and one
  built with `builtin_auth_builder_v1` now look the same: the wallet does not need to tell them apart.
* **The authenticator is indexed.** v3 ignored the `iota::account` events; v4 reads them, so a result says whether the
  IOTA wallet can authenticate the account (built-in authenticator) or not (custom).
* **Keyless accounts are recorded** (v3 left them out, its decision 4), so their authenticator is already known if a key is
  attached later.
* `key_id` stays as in v3: Rust only, not the address.

---

## 1. Background

On the base branch:

* **`iota::smart_account`** — the framework `SmartAccount` and its builders: `builder_v1` (any authenticator),
  `builtin_auth_builder_v1` (a `PublicKey` plus the built-in authenticator of its scheme), finished by `build_v1`
  (shared) or `build_immutable_v1` (frozen). After creation, the key and the authenticator change only in
  transactions sent by the account itself.
* **`iota::builtin_authenticator_functions`** — the five built-in authenticators (Ed25519, Secp256k1, Secp256r1,
  MultiSig, Passkey) and `attach_public_key` / `detach_public_key` / `rotate_public_key`, which emit
  `PublicKeyAttached` / `PublicKeyDetached` / `PublicKeyRotated`.
* **`iota::account`** — creates accounts and rotates their authenticator, emitting `MutableAccountCreated` /
  `ImmutableAccountCreated` / `AuthenticatorFunctionRefV1Rotated`.
* **Claiming** — a `ClaimAccount` transaction creates a `SmartAccount` at the address the sender's key derives.

Why a wallet cannot find its accounts today: only a claimed account sits at the address derived from the key. An
account built with `builtin_auth_builder_v1` gets a fresh object ID, so a wallet restored from a seed has no way to
find it without an index.

### Account states

A `SmartAccount` has a built-in key attached or not, and a built-in or a custom authenticator. The four combinations
fall into three groups:

* **Green** — key + built-in authenticator. Found by its key, and the IOTA wallet can authenticate it.
* **Yellow** — key + custom authenticator. Found by its key, but the IOTA wallet cannot authenticate it.
* **Grey** — no key. Not found by any key.

> **Diagram:** insert `account-discoverability-states.drawio` here with the draw.io macro.

A grey account with a built-in authenticator cannot authenticate a transaction, so it can never attach a key (that
needs the account as sender). It can only become green inside the same transaction that detached its key.

---

## 2. Design

> **Diagram:** insert `account-discoverability-data-flow.drawio` here with the draw.io macro.

### 2.1 Persist the links as Move events

The index is built only from events; no link is stored on chain. Events read (fields in Appendix A):

* **`PublicKeyAttached`, `PublicKeyDetached`, `PublicKeyRotated`** (existing) — give the links. Every change to a
  `SmartAccount`'s built-in key goes through one of them: `builtin_auth_builder_v1`, `claim_builder` and
  `attach_builtin_auth_public_key` call `attach_public_key`; the detach and rotate functions call the other two. A
  `SmartAccount` can never be deleted, so no link goes stale through deletion.
* **`SmartAccountCreated`** (new, in `iota::smart_account`) — emitted by `build_v1` and `build_immutable_v1`, so by
  every framework `SmartAccount`, keyless ones included. It is the only event that says an object is a framework
  `SmartAccount`: `attach_public_key` is `public` over any `UID`, so a `PublicKeyAttached` alone could come from any
  object.
* **`MutableAccountCreated`, `ImmutableAccountCreated`, `AuthenticatorFunctionRefV1Rotated`** (existing), for
  `SmartAccount` only — give the current authenticator.

Framework changes: add `SmartAccountCreated`; remove `SmartAccountClaimed` and `public_key::key_id`, which nothing on
chain used. The new event is the only change that affects consensus.

### 2.2 Key identity: `key_id`

```
key_id = blake2b256( scheme_flag || raw_key_bytes )    // 32 bytes
```

The index is keyed by `key_id`, not by the address the key derives:

* **It cannot fail.** It hashes bytes without parsing them. Address derivation returns an error for a malformed
  MultiSig committee, and the indexer reads chain bytes it does not re-validate.
* **It is the same for every scheme.** Address derivation omits the flag for Ed25519 and hashes a structured
  preimage for MultiSig.

It has no protocol role (authentication still checks the derived address) and is defined only in Rust
(`iota-types`), since every event carries the full `PublicKey`. Adding it to Move later would be additive. Fixed
values are in Appendix D.

**MultiSig members.** The `key_id` of a MultiSig key hashes its committee, which a wallet restoring from a seed does
not know. So every event about a MultiSig key also links each committee member, under the member's own `key_id`, and
that row records the `key_id` of the whole MultiSig key. The committee is the only payload the indexer decodes. If it
does not decode, only the whole key is linked.

### 2.3 Store them in the indexer

The indexer folds the events in chain order `(checkpoint, transaction, event)` into three PostgreSQL tables of current
state (columns and rules in Appendices B and C):

* `account_key_links` — one row per key and account: active or unlinked, and what last changed it.
* `smart_accounts` — one row per framework `SmartAccount`.
* `account_authenticators` — each `SmartAccount`'s current authenticator: one of the five built-in ones, or `custom`.

Guarantees:

* **Deterministic.** Replaying the same checkpoints gives the same rows, so a second indexer can be checked row for
  row.
* **Idempotent and monotonic.** Every write applies only if its transaction is not older than the stored one.
* **Not prunable.** The tables are state, not history; a pruned row could only be rebuilt from events the node may
  itself have pruned.

### 2.4 Query: `iotax_getAccountsByPublicKey`

Served only by the indexer (`iota-node` does not register the `iotax` namespace).

**Parameters:** `public_key`, Base64 of `scheme_flag || raw_key_bytes` (the format of
`iota::public_key::from_prefixed_bytes`); `include_unlinked` (optional, default `false`), which also returns links the
key was detached or rotated away from.

**Result:** the accounts linked to the key, newest change first:

| Field | Meaning |
| --- | --- |
| `address` | The account's address. |
| `status` | `active` (attached now) or `unlinked` (only with `include_unlinked`). |
| `source` | What last changed the link: `attach`, `rotate` or `detach`. |
| `smartAccount` | Whether the address is a framework `SmartAccount`. |
| `authenticator` | `ed25519`, `secp256k1`, `secp256r1`, `multisig`, `passkey` (built-in) or `custom`; `null` when not a `SmartAccount`. |
| `scheme` | The key's scheme flag, as a number. |
| `multisigKeyId` | Base64 `key_id` of the whole MultiSig key when the key is one of its members; `null` otherwise. |
| `lastChangeEpoch` | Epoch of the last change to the link. |

```json
{ "jsonrpc": "2.0", "id": 1, "method": "iotax_getAccountsByPublicKey",
  "params": ["AMxiMy40uy1c1p9g77sqNsuRbH60WDAeo2Y2xNuwEr2I", false] }
```

```json
{ "jsonrpc": "2.0", "id": 1, "result": [
  { "address": "0xcef6bafea1d59edb73ff5ec9e8aa58354796e1b572b695d64237ce9c15a34a03",
    "status": "active", "source": "attach", "smartAccount": true,
    "authenticator": "ed25519", "scheme": 0, "multisigKeyId": null, "lastChangeEpoch": "3" } ] }
```

### 2.5 Wallet flow

1. Derive the wallet's keys from the seed, for every supported scheme and derivation path.
2. Call `iotax_getAccountsByPublicKey` once per key.
3. Keep the results with `status = active`, `smartAccount = true` and a built-in `authenticator`: these are the
   accounts the wallet can unlock. `custom` ones can be shown as found but not usable.
4. Optionally, check each kept account on chain: it exists, its key is the queried one, its authenticator matches.

For live updates, subscribe with `iota_subscribeEvent` to the three modules of §2.1.

**Example: which accounts can key `K` unlock?**

> **Diagram:** insert `account-discoverability-query.drawio` here with the draw.io macro.

| Tx | What happens | Returned | Wallet keeps it |
| --- | --- | --- | --- |
| 101 | `builtin_auth_builder_v1(K)` + `build_v1` creates `A1` | yes, `ed25519` | **yes** |
| 102 | `ClaimAccount` with `K` creates `A2` | yes, `ed25519` | **yes** |
| 103–104 | `A3` is built with `K`, then rotates to a custom authenticator | yes, `custom` | no: the wallet cannot sign for it |
| 105–106 | `A4` is built with `K`, then rotates its key from `K` to `K2` | no: link is unlinked | — |
| 107 | A third-party package attaches `K` to its own object `O5` | yes, `smartAccount = false` | no: not a `SmartAccount` |

`K` can unlock `A1` and `A2`.

### 2.6 Trust model and limitations

* **Results are not authenticated.** A link is a true on-chain fact, not an endorsement. A wallet that needs certainty
  checks the account on chain (§2.5 step 4).
* **Anyone can use your key for an account.** `builtin_auth_builder_v1` takes any `PublicKey`, so anyone can create
  accounts with your key for the price of gas. Only you can operate them, but they appear in your results.
* **No paging.** Because of the point above, one key's result can grow without bound. Accepted for now.
* **An account can lock itself**, by removing the only key it can sign with or rotating to an authenticator nobody can
  satisfy.
* **A key and an authenticator of different schemes** are reported as they are; keeping them consistent is up to
  whoever rotates them.

---

## 3. Alternatives considered

* **On-chain registry** (v1): a `ClaimRegistry` shared object at `0x10` with dynamic-field markers. It adds chain
  state for data that is only read off chain, and puts a shared object, so consensus, on every claim. Dropped in v3.
* **Fullnode database:** index the same events in the fullnode's own store and serve the query from the fullnode's
  RPC. A wallet would not depend on an indexer deployment, but every fullnode that enables it carries the tables.
  _Why the indexer was chosen: to be completed._

---

## 4. Rollout

* The features the index depends on, `enable_builtin_move_authenticators` and `enable_claim_account_transaction`, are
  enabled at protocol version 38, on every chain except testnet and mainnet.
* The indexer builds the tables from genesis; no migration or backfill.
* An indexer must not read a network whose framework predates `SmartAccountCreated`: its accounts would get no
  `smart_accounts` row and be reported with `smartAccount = false`.

---

## 5. Decisions needed

1. **What the query returns.** (a) Every link, with `smartAccount` and `authenticator`, and the wallet decides what it
   can unlock (as implemented). (b) By default only active `SmartAccount`s with a built-in authenticator, the rest
   behind a flag.
2. **Restoring the indexer from a formal snapshot.** `iota-indexer restore` loads objects, not events, so the tables
   miss everything before the snapshot, with no error. (a) Accept and document it. (b) Rebuild the active links,
   `smart_accounts` and authenticators from the snapshot's objects; unlinked rows and per-change transaction and epoch
   would still be lost.
3. **Links to objects that are not a `SmartAccount`.** (a) Keep them, with `smartAccount = false` (as implemented).
   (b) Leave them out of the RPC result. (c) Restrict `attach_public_key` in Move, a framework change.

---

## Appendix A. Events

| Event | Module | Fields |
| --- | --- | --- |
| `PublicKeyAttached` | `builtin_authenticator_functions` | `account_id: ID, public_key: PublicKey` |
| `PublicKeyDetached` | `builtin_authenticator_functions` | `account_id: ID, public_key: PublicKey` |
| `PublicKeyRotated` | `builtin_authenticator_functions` | `account_id: ID, from: PublicKey, to: PublicKey` |
| `SmartAccountCreated` (new) | `smart_account` | `account_id: ID, public_key: Option<PublicKey>, immutable: bool` |
| `MutableAccountCreated<SmartAccount>` / `ImmutableAccountCreated<SmartAccount>` | `account` | `account_id: ID, authenticator: AuthenticatorFunctionRefV1` |
| `AuthenticatorFunctionRefV1Rotated<SmartAccount>` | `account` | `account_id: ID, from: AuthenticatorFunctionRefV1, to: AuthenticatorFunctionRefV1` |

`PublicKey` is `{ scheme: SignatureScheme { flag: u8 }, raw_bytes: vector<u8> }`; `AuthenticatorFunctionRefV1` is
`{ package: ID, module_name: ascii::String, function_name: ascii::String }`.

## Appendix B. Fold rules

| Event | Effect |
| --- | --- |
| `PublicKeyAttached(account, pk)` | link `(key_id(pk), account)` active, source `attach` |
| `PublicKeyRotated(account, from, to)` | `(key_id(from), account)` unlinked, **then** `(key_id(to), account)` active; source `rotate` |
| `PublicKeyDetached(account, pk)` | `(key_id(pk), account)` unlinked, source `detach` |
| `SmartAccountCreated(account, _, immutable)` | `smart_accounts` row |
| `{Mutable,Immutable}AccountCreated<SmartAccount>(account, auth)` | `account_authenticators` row, kind of `auth` |
| `AuthenticatorFunctionRefV1Rotated<SmartAccount>(account, _, to)` | `account_authenticators` row, kind of `to` |

* A MultiSig key also links or unlinks each committee member: `(key_id(member), account)`, with
  `multisig_key_id = key_id(pk)` and the same source.
* Every unlink of `from` comes before every link of `to`, so a key on both sides of a rotation stays active: the
  same key, or a MultiSig member kept across the rotation.
* The key in `SmartAccountCreated` is not used for links: the `PublicKeyAttached` of the same transaction gives it.
* Only `iota::account` events whose type parameter is `0x2::smart_account::SmartAccount` are read.
* An event whose type matches but whose payload does not decode is skipped: the indexer is older than the framework.

**Authenticator kind.** Built-in when the ref's package is `0x2` and its module is `builtin_authenticator_functions`:
`ed25519_authenticator_function_ref_v1` → `ed25519` (1), and likewise `secp256k1` (2), `secp256r1` (3), `multisig`
(4), `passkey` (5). Anything else is `custom` (6).

## Appendix C. Tables

All three: primary key as shown, no foreign keys (joined on `account_id` at query time), not prunable. Stored numbers
for `source` and `kind` must never be renumbered.

**`account_key_links`** — primary key `(key_id, account_id)`; indexes on `(account_id)` and on `(key_id) WHERE
status = 0`.

| Column | Type | Meaning |
| --- | --- | --- |
| `key_id` | `BYTEA` | `key_id` of the key (§2.2) |
| `account_id` | `BYTEA` | the object the key is attached to |
| `scheme` | `SMALLINT` | scheme flag as recorded on chain, stored even if unknown to the build |
| `multisig_key_id` | `BYTEA NULL` | `key_id` of the whole MultiSig key when `key_id` is one of its members |
| `source` | `SMALLINT` | 0 attach, 1 rotate, 2 detach |
| `status` | `SMALLINT` | 0 active, 1 unlinked |
| `last_change_tx_sequence_number` | `BIGINT` | orders results and guards writes |
| `last_change_epoch` | `BIGINT` | returned as `lastChangeEpoch` |

**`smart_accounts`** — primary key `account_id`; columns `immutable BOOLEAN`, `created_tx_sequence_number`,
`created_epoch`. A second claim of the same address keeps the later transaction.

**`account_authenticators`** — primary key `account_id`; columns `kind SMALLINT` (Appendix B),
`last_change_tx_sequence_number`, `last_change_epoch`.

**Example.** In epoch 3, tx 101 runs `builtin_auth_builder_v1(K)` + `build_v1` (creates `A1`), and tx 102 is a
`ClaimAccount` with `K` (creates `A2`). Each emits `PublicKeyAttached`, `SmartAccountCreated` and
`MutableAccountCreated<SmartAccount>`, and writes one row per table:

> **Diagram:** insert `account-discoverability-table-fill.drawio` here with the draw.io macro.

| Table | tx 101 | tx 102 |
| --- | --- | --- |
| `account_key_links` | `key_id(K)`, `A1`, scheme 0, attach, active, tx 101, epoch 3 | `key_id(K)`, `A2`, scheme 0, attach, active, tx 102, epoch 3 |
| `smart_accounts` | `A1`, immutable false, tx 101, epoch 3 | `A2`, immutable false, tx 102, epoch 3 |
| `account_authenticators` | `A1`, kind 1 (ed25519), tx 101, epoch 3 | `A2`, kind 1 (ed25519), tx 102, epoch 3 |

## Appendix D. `key_id` fixed values

| Prefixed key bytes (hex) | `key_id` (hex) |
| --- | --- |
| Ed25519: `00cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88` | `43541042c153e0e498a08a8db868f1614c9366694fa730bd8a07fc5d7c931f0d` |
| 1-of-1 MultiSig (that Ed25519 key): `030100cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88010100` | `37330e88388d526046696b5b5113cd64e81eb1b1bcd403372666cc54970ddbf4` |
| 1-of-2 MultiSig (Ed25519 + Secp256k1): `030200cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88010102337cca2171fdbfcfd657fa59881f46269f1e590b5ffab6023686c7ad2ecc2c1c010100` | `dd22eb5c98cdc27de98174a69b68ca1603bdda8aeb226c5232273cfdc9655811` |
