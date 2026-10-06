# Account discoverability: design

A wallet that holds only a seed must be able to find the `SmartAccount`s its keys control, and learn whether the IOTA
wallet can authenticate each one. This document describes how PR #13071 does that: a Move event, a Rust key identity,
and an indexer that folds public on-chain events into a lookup table served over JSON-RPC.

The first part (§1–§4) is the contract: what a wallet or SDK sees and can rely on. The second part (§5–§10) is the
implementation, for reviewers. §11 answers the questions most likely to come up.

## Status

* PR [#13071](https://github.com/iotaledger/iota/pull/13071), fixes issue
  [#12815](https://github.com/iotaledger/iota/issues/12815).
* Base branch:
  [`vm-lang/10722-default-iota-account-alpha`](https://github.com/iotaledger/iota/tree/vm-lang/10722-default-iota-account-alpha).
* The features the index depends on, `enable_builtin_move_authenticators` and `enable_claim_account_transaction`, are
  enabled at protocol version 38, on every chain except testnet and mainnet.

## How the data flows

> **Diagram:** insert `account-discoverability-data-flow.drawio` here with the draw.io macro.

1. The Move framework emits events whenever a `SmartAccount` is created, gains or loses a key, or changes its
   authenticator (§5).
2. The indexer reads the checkpointed transactions from a fullnode and turns those events into rows in three
   PostgreSQL tables (§7).
3. A wallet sends each of its public keys to `iotax_getAccountsByPublicKey` on the indexer, which returns the accounts
   that key is attached to, with their authenticator (§3).
4. Optionally, the wallet follows new events live with `iota_subscribeEvent` on a fullnode.

---

## 1. What already exists (base branch)

This PR builds on account-abstraction work already on the base branch:

* **`iota::smart_account`** — the framework `SmartAccount` object and its builders: `builder_v1` (any
  `AuthenticatorFunctionRefV1`), `builtin_auth_builder_v1` (a `PublicKey` plus the built-in authenticator of its
  scheme), finished by `build_v1` (shared, mutable) or `build_immutable_v1` (frozen). After creation, the key and the
  authenticator change only through functions that require the account itself to be the transaction sender.
* **`iota::builtin_authenticator_functions`** — the five built-in authenticators (Ed25519, Secp256k1, Secp256r1,
  MultiSig, Passkey), and `attach_public_key` / `detach_public_key` / `rotate_public_key`, which emit
  `PublicKeyAttached` / `PublicKeyDetached` / `PublicKeyRotated` (#11856).
* **`iota::account`** — `create_account_v1` / `create_immutable_account_v1` / `rotate_auth_function_ref_v1`, which emit
  `MutableAccountCreated` / `ImmutableAccountCreated` / `AuthenticatorFunctionRefV1Rotated`.
* **Claiming** — a `ClaimAccount` transaction creates a `SmartAccount` at the address the sender's key derives, through
  the private `claim_builder` (#12082).

## 2. Account states

A `SmartAccount` either has a built-in public key attached or not, and its authenticator is either built-in or custom.
That gives four states:

* **Green** — key + built-in authenticator. Found by its key, and the IOTA wallet can authenticate it.
* **Yellow** — key + custom authenticator. Found by its key, but the IOTA wallet cannot authenticate it.
* **Grey** — no key, with a built-in or a custom authenticator. Not found by any key, so never returned.

> **Diagram:** insert `account-discoverability-states.drawio` here with the draw.io macro. Arrows use the short labels
> below.

| Label | Function |
| --- | --- |
| `claim` | a `ClaimAccount` transaction (`smart_account::claim_builder`, then `build_v1` or `build_immutable_v1`) |
| `builder_v1(builtin)` / `builder_v1(custom)` | `smart_account::builder_v1` with a built-in or a custom `AuthenticatorFunctionRefV1` |
| `build_v1` | `smart_account::build_v1`; `build_immutable_v1` reaches the same state, frozen, with no further transitions |
| `attach_pk` | `smart_account::attach_builtin_auth_public_key` |
| `detach_pk` | `smart_account::detach_builtin_auth_public_key` |
| `rotate_pk` | `smart_account::rotate_builtin_auth_public_key` |
| `rotate_auth(builtin \| custom)` | `smart_account::rotate_auth_function_ref_v1` with a built-in or a custom ref |

**What each step emits, and what the indexer does with it:**

| Step | Events emitted, in order | Indexer rows written |
| --- | --- | --- |
| `builtin_auth_builder_v1` / `claim`, then `build_v1` | `PublicKeyAttached`, `SmartAccountCreated` (key: some), `MutableAccountCreated` | link (active, `attach`); `smart_accounts`; `account_authenticators` (built-in kind) |
| `builder_v1(…)`, then `build_v1` | `SmartAccountCreated` (key: none), `MutableAccountCreated` | `smart_accounts`; `account_authenticators` (built-in or `custom`); no link |
| `attach_pk` | `PublicKeyAttached` | link (active, `attach`) |
| `detach_pk` | `PublicKeyDetached` | link (unlinked, `detach`) |
| `rotate_pk` | `PublicKeyRotated` | old link (unlinked, `rotate`), then new link (active, `rotate`) |
| `rotate_auth(…)` | `AuthenticatorFunctionRefV1Rotated` | `account_authenticators` (kind of the new ref) |

With `build_immutable_v1` the lifecycle event is `ImmutableAccountCreated` and `smart_accounts.immutable` is true.

When the key is a MultiSig key, every link written above is also written once per committee member, under the
member's own `key_id` (§7).

> **Note on grey (built-in auth, no PK) → green.** `attach_pk` requires the account itself as the sender, and an
> account with a built-in authenticator and no key cannot authenticate a transaction. So this arrow can only be
> taken inside the same transaction that detached the key. Across transactions, that grey state cannot be left by
> the account itself (see §9).

## 3. RPC contract: `iotax_getAccountsByPublicKey`

Served only by the indexer (`iota-node` does not register the `iotax` namespace).

**Parameters**

1. `public_key` — Base64 of the scheme-flag-prefixed public key: `flag || raw key bytes`, the format of
   `iota::public_key::from_prefixed_bytes`. Flags: `0x00` Ed25519, `0x01` Secp256k1, `0x02` Secp256r1, `0x03` MultiSig,
   `0x06` Passkey. The bytes are hashed, never parsed (§6), so any non-empty value is accepted; an empty one is an error.
2. `include_unlinked` (optional, default `false`) — also return links the key was rotated away from or detached from.

**Result** — an array of `AccountKeyLink`, newest change first:

| Field | Meaning |
| --- | --- |
| `address` | The account's address. |
| `status` | `active`: the key is attached now. `unlinked`: it used to be (only with `include_unlinked`). |
| `source` | What last changed the link: `attach`, `rotate` or `detach`. |
| `smartAccount` | Whether the address is a framework `SmartAccount` (has a `smart_accounts` row). |
| `authenticator` | The account's current authenticator: `ed25519`, `secp256k1`, `secp256r1`, `multisig`, `passkey` (built-in: green) or `custom` (yellow). `null` when the address is not a framework `SmartAccount`. |
| `scheme` | The key's scheme flag as recorded on chain, as a number. Not resolved to a name, so a flag this build does not know still indexes. |
| `multisigKeyId` | Set when the queried key is one member of the account's MultiSig key: the Base64 `key_id` of that whole MultiSig key. The key alone may not be enough to sign (§4). `null` when the queried key is the account's key itself. |
| `lastChangeEpoch` | The epoch of the last change to this link, as a decimal string. |

**Example** — the Ed25519 key `cc62332e…12bd88` after a claim:

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

An empty key returns the error ``Invalid argument with error: `public key must not be empty` ``.

**What a result does not prove.** Nothing in this call is authenticated, and links are true facts but not
endorsements:

* anyone can create a `SmartAccount` whose key is someone else's public key; only the key holder can use it, but it
  appears in that key's results;
* `builtin_authenticator_functions::attach_public_key` (and `detach_public_key`, `rotate_public_key`) is already
  `public` over any object's `UID`. Any package can attach a key to its own object, and the index records that link
  like any other. `PublicKeyAttached` says only "this key was attached to this UID"; only `SmartAccountCreated` says
  the UID is a framework `SmartAccount`. That is what the `smart_accounts` table is for: a link with no
  `smart_accounts` row is returned with `smartAccount = false` and `authenticator = null`.

## 4. Wallet flow

1. Derive the wallet's keys from the seed, for every supported scheme and derivation path.
2. Query `iotax_getAccountsByPublicKey` once per key, with the prefixed key bytes.
3. Keep the results with `status = active`, `smartAccount = true` and a built-in `authenticator`: these are the
   accounts the IOTA wallet can use with that key. `custom` accounts can be shown as found but not usable here. A
   result with `multisigKeyId` set means the key is one signer of the account's MultiSig key: to sign, the wallet
   reads the committee from the account on chain, and needs members whose weights reach the threshold.
4. Optionally, verify each kept account on chain: the object exists, its attached key is the queried one, and its
   authenticator is the reported one.

For live updates, subscribe with `iota_subscribeEvent` to the events of §5. They live in three modules
(`builtin_authenticator_functions`, `smart_account`, `account`), so either three `MoveEventModule` filters or one
broader filter.

The RPC filters only by key and by link status. Whether an account can be unlocked with the key (step 3) is decided
by the wallet.

### Example: which accounts can key K unlock?

> **Diagram:** insert `account-discoverability-query.drawio` here with the draw.io macro.

Eight transactions use the Ed25519 key `K`. In the last one, `M` is a 1-of-2 MultiSig key whose members are `K` and
another Ed25519 key `L`:

| Tx | What happens | Account |
| --- | --- | --- |
| 101 | `builtin_auth_builder_v1(K)`, then `build_v1` | `A1` |
| 102 | `ClaimAccount` with `K` | `A2` |
| 103 | `builtin_auth_builder_v1(K)`, then `build_v1` | `A3` |
| 104 | `A3` calls `rotate_auth_function_ref_v1` to a custom authenticator | `A3` |
| 105 | `builtin_auth_builder_v1(K)`, then `build_v1` | `A4` |
| 106 | `A4` calls `rotate_builtin_auth_public_key`, from `K` to `K2` | `A4` |
| 107 | A third-party package calls `attach_public_key` on its own object | `O5` |
| 108 | `builtin_auth_builder_v1(M)`, then `build_v1` | `A6` |

The wallet sends `base64(0x00 || K)`; the indexer computes `h(K) = key_id` and runs the equivalent of:

```sql
SELECT l.*, s.*, a.*
FROM account_key_links l
LEFT JOIN smart_accounts s ON s.account_id = l.account_id
LEFT JOIN account_authenticators a ON a.account_id = l.account_id
WHERE l.key_id = h(K)
  AND l.status = 0            -- active only, unless include_unlinked
ORDER BY l.last_change_tx_sequence_number DESC
```

| Account | Returned by the RPC | `smartAccount` | `authenticator` | `multisigKeyId` | Wallet keeps it |
| --- | --- | --- | --- | --- | --- |
| `A6` | yes: `K` is a member of `M` | `true` | `multisig` | `h(M)` | **yes**: 1-of-2, so `K` alone reaches the threshold |
| `O5` | yes | `false` | `null` | `null` | no: not a `SmartAccount` |
| `A3` | yes | `true` | `custom` | `null` | no: the IOTA wallet cannot sign for it |
| `A2` | yes | `true` | `ed25519` | `null` | **yes** |
| `A1` | yes | `true` | `ed25519` | `null` | **yes** |
| `A4` | no: its link to `h(K)` is unlinked | | | | |

The accounts `K` can unlock are `A1`, `A2` and `A6`. Tx 108 also links `A6` under `h(M)` and `h(L)`, which this query
does not select.

---

## 5. Move framework (this PR)

**New event** in `iota::smart_account`:

```move
public struct SmartAccountCreated has copy, drop {
    account_id: ID,
    public_key: Option<PublicKey>,
    immutable: bool,
}
```

`build_v1` and `build_immutable_v1` call `emit_smart_account_event` before handing the account to
`account::create_account_v1` / `create_immutable_account_v1`, so every framework `SmartAccount` announces itself,
whether it was claimed, built with `builtin_auth_builder_v1`, or built with `builder_v1` and no key
(`public_key = none`). A claim and any other build look the same on the wire.

**Removed in this PR:** `SmartAccountClaimed` (the claim entry points no longer emit anything of their own), and
`public_key::key_id`, which nothing on chain used (§8).

**Events the indexer consumes:**

| Event | Module | Fields |
| --- | --- | --- |
| `PublicKeyAttached` | `builtin_authenticator_functions` | `account_id: ID, public_key: PublicKey` |
| `PublicKeyDetached` | `builtin_authenticator_functions` | `account_id: ID, public_key: PublicKey` |
| `PublicKeyRotated` | `builtin_authenticator_functions` | `account_id: ID, from: PublicKey, to: PublicKey` |
| `SmartAccountCreated` | `smart_account` | `account_id: ID, public_key: Option<PublicKey>, immutable: bool` |
| `MutableAccountCreated<SmartAccount>` / `ImmutableAccountCreated<SmartAccount>` | `account` | `account_id: ID, authenticator: AuthenticatorFunctionRefV1` |
| `AuthenticatorFunctionRefV1Rotated<SmartAccount>` | `account` | `account_id: ID, from: AuthenticatorFunctionRefV1, to: AuthenticatorFunctionRefV1` |

`PublicKey` is `{ scheme: SignatureScheme { flag: u8 }, raw_bytes: vector<u8> }`; `AuthenticatorFunctionRefV1` is
`{ package: ID, module_name: ascii::String, function_name: ascii::String }`.

**Why the three `PublicKey*` events are enough for the links.** Every change to a `SmartAccount`'s built-in key goes
through one of them: `attach_public_key` is called by `builtin_auth_builder_v1`, `claim_builder` and
`attach_builtin_auth_public_key`; `detach_public_key` by `detach_builtin_auth_public_key`; `rotate_public_key` by
`rotate_builtin_auth_public_key`. A `SmartAccount` can never be deleted, so no link goes stale through deletion.

## 6. Rust part of the framework (`iota-types`)

All in `crates/iota-types/src/account_abstraction/public_key.rs`.

**`key_id`** — the identity the index is keyed by:

```
key_id = blake2b256( scheme_flag || raw_key_bytes )    // 32 bytes
```

* `key_id(scheme_flag: u8, raw_key_bytes: &[u8]) -> [u8; 32]`
* `MovePublicKey::key_id(&self) -> [u8; 32]`
* `key_id_from_prefixed_bytes(prefixed: &[u8]) -> Option<[u8; 32]>` — for the RPC input; `None` only for empty input.

It is not the account address, on purpose:

* **It cannot fail.** It hashes bytes without parsing them. Address derivation (`MovePublicKey::address`) returns a
  `Result`, because MultiSig must decode its committee, and the indexer reads chain bytes it does not re-validate.
* **It is uniform across schemes.** Address derivation omits the flag for Ed25519 and hashes a structured preimage
  for MultiSig. `key_id` includes the flag for every scheme. It happens to equal the address for Secp256k1, Secp256r1
  and Passkey, so comparing a `key_id` with an address is never a meaningful test.

It has no protocol role (authentication verifies against the derived address) and nothing on chain computes it. An
independent indexer must reproduce it exactly; `key_id_fixed_vectors` pins these values:

| Prefixed key bytes (hex) | `key_id` (hex) |
| --- | --- |
| Ed25519: `00cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88` | `43541042c153e0e498a08a8db868f1614c9366694fa730bd8a07fc5d7c931f0d` |
| 1-of-1 MultiSig (that Ed25519 key): `030100cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88010100` | `37330e88388d526046696b5b5113cd64e81eb1b1bcd403372666cc54970ddbf4` |
| 1-of-2 MultiSig (Ed25519 + Secp256k1): `030200cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88010102337cca2171fdbfcfd657fa59881f46269f1e590b5ffab6023686c7ad2ecc2c1c010100` | `dd22eb5c98cdc27de98174a69b68ca1603bdda8aeb226c5232273cfdc9655811` |

A MultiSig member's `key_id` is the same function over the member's own flag and key bytes, so it equals what the
member's wallet computes from its key. `MovePublicKey::raw_bytes()` gives the indexer the committee bytes to decode.

**`MovePublicKey::scheme_flag()`** returns the raw flag byte. `scheme()` panics on a flag this build does not know,
which a chain-read value may carry once the framework adds a scheme; the indexer uses `scheme_flag()` so the fold
never fails on one.

## 7. Indexer

### Fold rules

The index is a left fold of the §5 events in chain order `(checkpoint, transaction, event)`:

| Event | Effect |
| --- | --- |
| `PublicKeyAttached(account, pk)` | link `(key_id(pk), account)`: active, source `attach` |
| `PublicKeyRotated(account, from, to)` | link `(key_id(from), account)` unlinked, **then** `(key_id(to), account)` active; source `rotate` |
| `PublicKeyDetached(account, pk)` | link `(key_id(pk), account)`: unlinked, source `detach` |
| `SmartAccountCreated(account, _, immutable)` | `smart_accounts` row; no link |
| `{Mutable,Immutable}AccountCreated<SmartAccount>(account, authenticator)` | `account_authenticators` row, kind of `authenticator` |
| `AuthenticatorFunctionRefV1Rotated<SmartAccount>(account, _, to)` | `account_authenticators` row, kind of `to` |

* **MultiSig members.** For a MultiSig `pk`, each rule above also applies to every committee member: link
  `(key_id(member), account)` with `multisig_key_id = key_id(pk)` and the member's own scheme flag. The row for the
  whole key has `multisig_key_id` NULL.
* In a rotation every unlink of `from` comes before every link of `to`, so a key on both sides stays active: the same
  key, or a MultiSig member kept across the rotation.
* The key in `SmartAccountCreated` is not used: the `PublicKeyAttached` of the same transaction gives the link.
* The `iota::account` events are generic; only those whose single type parameter is
  `0x2::smart_account::SmartAccount` are read.
* Events are matched on their `StructTag` at `0x2`. An event whose type matches but whose payload does not decode
  contributes nothing: that is an indexer older than the framework it reads, not an error.
* A MultiSig committee is the only key material decoded. If it does not decode, only the whole key is linked.

**Authenticator kind.** Built-in when the ref's package is `0x2` and its module is `builtin_authenticator_functions`,
with the function name giving the kind; anything else is `custom`. Nothing is parsed or resolved.

| `function_name` | Kind (stored value) |
| --- | --- |
| `ed25519_authenticator_function_ref_v1` | `ed25519` (1) |
| `secp256k1_authenticator_function_ref_v1` | `secp256k1` (2) |
| `secp256r1_authenticator_function_ref_v1` | `secp256r1` (3) |
| `multisig_authenticator_function_ref_v1` | `multisig` (4) |
| `passkey_authenticator_function_ref_v1` | `passkey` (5) |
| anything else, or another package or module | `custom` (6) |

### Tables

Migration `crates/iota-indexer/migrations/pg/2026-09-15-000000_account_discoverability/up.sql`.

| Table | Key | Columns | Written from |
| --- | --- | --- | --- |
| `account_key_links` | `(key_id, account_id)` | `scheme`, `multisig_key_id` (NULL unless the row is a MultiSig member), `source` (0 attach, 1 rotate, 2 detach), `status` (0 active, 1 unlinked), `last_change_tx_sequence_number`, `last_change_epoch` | `PublicKey*` |
| `smart_accounts` | `account_id` | `immutable`, `created_tx_sequence_number`, `created_epoch` | `SmartAccountCreated` |
| `account_authenticators` | `account_id` | `kind` (1–6), `last_change_tx_sequence_number`, `last_change_epoch` | `iota::account` lifecycle events |

Indexes on `account_key_links`: `(account_id)`, and `(key_id) WHERE status = 0`. The stored numbers of `source` and
`kind` must never be renumbered (`LinkSource::from_stored`, `AuthenticatorKind::from_stored` pin them).

Keyless accounts get `smart_accounts` and `account_authenticators` rows too. `PublicKeyAttached` does not say which
authenticator an account has, so the kind must already be known when a key is attached later.

`account_key_links` is keyed by the pair `(key_id, account_id)`: there is at most one row per key and account, holding
the current state of that link. One key can have rows for many accounts, and one account can have rows for several
keys over time (a rotation leaves the old key's row `unlinked` and adds an `active` row for the new key). Every event
on a pair updates the same row; the history stays in the events table.

### Example: two transactions with the same key

> **Diagram:** insert `account-discoverability-table-fill.drawio` here with the draw.io macro.

Both transactions use the Ed25519 key `K`, in epoch 3:

* **tx 101**, sent by anyone: `builtin_auth_builder_v1(K)`, then `build_v1`. Creates `A1`, a fresh object ID.
* **tx 102**, a `ClaimAccount` sent by `A2`: `claim_account_v1(K)`, which runs `claim_builder`, then `build_v1`.
  Creates `A2`, the address derived from `K`.

Each emits the same three events, `PublicKeyAttached`, `SmartAccountCreated` and
`MutableAccountCreated<SmartAccount>`, and each event writes one row:

| Table | Row from tx 101 | Row from tx 102 |
| --- | --- | --- |
| `account_key_links` | `h(K)`, `A1`, scheme 0, multisig_key_id NULL, attach, active, tx 101, epoch 3 | `h(K)`, `A2`, scheme 0, multisig_key_id NULL, attach, active, tx 102, epoch 3 |
| `smart_accounts` | `A1`, immutable false, tx 101, epoch 3 | `A2`, immutable false, tx 102, epoch 3 |
| `account_authenticators` | `A1`, kind 1 (ed25519), tx 101, epoch 3 | `A2`, kind 1 (ed25519), tx 102, epoch 3 |

A claim and a build look the same in the index. `SmartAccountCreated` carries `some(K)`, but that key is not used for
links: the `PublicKeyAttached` of the same transaction already gives the link.

### Guarantees

* **Deterministic.** Replaying the same checkpoints yields the same rows; a second indexer can be checked against
  another row for row.
* **Idempotent and monotonic.** Every upsert is guarded by `excluded.<tx_sequence_number> >= existing`, so a replayed
  or out-of-order write never moves a row backwards.
* **Independent of batching.** Each batch is collapsed to the last write per key before writing.
* **Not prunable.** The three tables are current state, not history: a pruned row could only be rebuilt from events
  the node may itself have pruned. They are absent from `PrunableTable` on purpose.

### Internals, briefly

* `ingestion/primary/prepare.rs` `index_transactions` runs `account_key_link_ops`, `smart_account_row` and
  `account_authenticator_row` (`src/account_key_events.rs`) over each checkpointed transaction's events. Doing it
  there keeps not-yet-checkpointed transactions out. `AccountKeyLinkOp::for_key` adds the member ops for a MultiSig
  key.
* `ingestion/primary/persist.rs` collapses each batch with `collapse_last_write_wins`, then calls the three guarded
  upserts in `store/pg_indexer_store.rs` (`persist_account_key_links`, `persist_smart_accounts`,
  `persist_account_authenticators`) alongside the other checkpoint writes.
* `read.rs` `get_accounts_by_key_id` selects the links for a `key_id`, left-joined with `smart_accounts` and
  `account_authenticators`, active only unless `include_unlinked`, ordered by `last_change_tx_sequence_number` desc.
* `apis/extended_api.rs` hashes the input with `key_id_from_prefixed_bytes` and shapes each row into an
  `AccountKeyLink`. An unknown stored `source` or `kind`, or a `multisig_key_id` that is not 32 bytes, is reported as
  data corruption.
* Each table has a commit-latency histogram: `checkpoint_db_commit_latency_{account_key_links,smart_accounts,account_authenticators}`.

## 8. Decisions

* **No claimed/unclaimed distinction.** A claimed account and one someone else built with your key look the same.
  Telling them apart needed a dedicated claim event, and it is not something the wallet needs.
* **`key_id` only in Rust.** Nothing on chain used it, every event carries the full `PublicKey`, and a public function
  in a system package cannot be removed once a network activates it. Adding it to Move later is additive.
* **Links come only from the `PublicKey*` events.** They already cover every key change on a `SmartAccount`;
  `SmartAccountCreated`'s key would duplicate the `PublicKeyAttached` of the same transaction.
* **`SmartAccountCreated` is kept, with an optional key.** Without it the index could not tell a framework
  `SmartAccount` from any other object a key was attached to, and keyless accounts would not be recorded at all.
* **No `smart_account`-specific attach/detach/rotate events.** They would duplicate the `PublicKey*` events on every
  key change for no consumer.
* **The authenticator kind comes from the existing `iota::account` events**, for `SmartAccount` only. No framework
  change was needed for it.
* **The kind has its own table.** Different events write it than `smart_accounts`, and one event family per table
  keeps the same collapse-then-upsert pattern for all three.
* **MultiSig members are indexed, with the whole key's `key_id`.** Without member rows a wallet holding one member key
  cannot find the account. `multisig_key_id` tells the wallet the key is one signer of several, not the account's
  only key. The committee itself is not stored: the wallet reads it from the account when it signs.
* **A key and an authenticator of different schemes is out of scope.** Whoever rotates one is expected to rotate the
  other; the index reports the authenticator as it is.

## 9. Open problems and limitations

* **Restoring from a formal snapshot leaves the index incomplete, silently.** `iota-indexer restore` loads objects,
  not events, so the three tables miss everything created before the snapshot. The snapshot's objects could rebuild
  the active links, the `smart_accounts` rows and the authenticator kinds; tombstones and the per-change transaction
  and epoch would still be lost.
* **The RPC has no limit or paging.** Creating accounts with someone else's key costs only gas, so one key's result
  can grow without bound. The other list methods of `ExtendedApi` take a `cursor` and a `limit`.
* **`schema.patch` no longer matches.** Its only hunk locates the license header by the first table's context lines;
  the first table is now `account_authenticators`, so `generate.sh` will likely fail until the patch names it.
  Nothing in CI runs the script.
* **MultiSig members depend on the indexer build.** A committee with a member scheme this build does not know does not
  decode, so only the whole key is linked. Upgrading the indexer does not revisit past events; only re-indexing adds
  the missing member rows.
* **Links to objects that are not a `SmartAccount`.** `attach_public_key` is public over any `UID`; such links are
  indexed, with `smartAccount = false` and `authenticator = null`.
* **Anyone can use your key for an account.** It appears in your key's results; only you can operate it.
* **An account can lock itself permanently.** Detaching or rotating away the only key it can sign with, or rotating to
  an authenticator nobody can satisfy, leaves no way to authenticate again. The grey built-in state is one case: it
  cannot be left across transactions (§2).
* **Stale doc comments.** The doc comments of `build_v1`, `build_immutable_v1`, `claim_account_v1` and
  `claim_immutable_account_v1` in `smart_account.move` list the events they emit but not `SmartAccountCreated`.

## 10. Testing

| What | Tests |
| --- | --- |
| `SmartAccountCreated` from every build path | Move `smart_account_tests`: `claim_account_v1_emits_smart_account_created`, `claim_immutable_account_v1_emits_smart_account_created`, `builtin_auth_builder_v1_emits_smart_account_created_with_the_key`, `builder_v1_emits_smart_account_created_without_a_key` |
| `key_id` formula and fixed values | Rust `iota-types` `public_key_tests`: `key_id_fixed_vectors`, `key_id_is_total_on_bytes_that_are_not_a_valid_key`, `key_id_differs_from_address_for_ed25519` / `_for_multisig`, `key_id_coincides_with_address_for_flag_prefixed_schemes` |
| Fold and classification | Rust `iota-indexer` unit tests in `account_key_events.rs` (event decoding, unknown flags and payloads, the five built-in kinds, `custom`, other account types ignored) and `ingestion/primary/persist.rs` (batch collapse) |
| MultiSig members | `account_key_events.rs`: `attaching_a_multisig_key_links_the_whole_key_and_each_member`, `detaching_a_multisig_key_unlinks_the_whole_key_and_each_member`, `rotating_from_a_multisig_key_to_a_member_key_ends_linked_directly`, `rotating_from_a_member_key_to_its_multisig_key_ends_linked_as_a_member`, `a_multisig_committee_this_build_cannot_decode_links_only_the_whole_key`; `persist.rs`: `a_key_that_stops_being_a_multisig_member_is_stored_as_direct`; integration: `a_multisig_account_is_found_from_each_member_key` |
| Green: claim, build, lookup | `a_claim_links_the_key_and_records_the_smart_account`, `the_rpc_returns_the_account_for_its_key`, `every_account_holding_the_key_is_returned`, `an_immutable_account_is_recorded_as_immutable` |
| Grey | `a_keyless_smart_account_is_recorded_without_a_link`, `detaching_and_reattaching_in_one_transaction_keeps_the_account` |
| `rotate_pk`, `detach_pk`, `include_unlinked` | `a_rotation_tombstones_the_old_key_and_links_the_new_one`, `a_detach_tombstones_the_key` |
| `rotate_auth` (built-in, custom → yellow) | `rotating_the_authenticator_with_the_key_updates_the_kind`, `rotating_to_a_custom_authenticator_reports_custom` (uses `tests/data/custom_authenticator`) |
| Repeated claim, replay, RPC edges | `a_second_claim_of_the_same_address_replaces_the_first`, `a_second_indexer_replaying_the_chain_builds_the_same_tables`, `the_rpc_rejects_an_empty_public_key`, `an_unseen_key_returns_no_accounts` |

The integration tests are in `crates/iota-indexer/tests/account_key_links_tests.rs` and send real transactions on a
`TestCluster`. Transactions from an account are authenticated with a `MoveAuthenticator` over its built-in key.

Not covered by an integration test: a result with `smartAccount = false`, which needs a package that attaches a key
to its own object.

Two transactional tests in `crates/iota-adapter-transactional-tests/tests/abstract_account/smart_account/` cover
state changes that the account itself must send:

* `custom_builder_with_builtin_auth_without_key.move`: a `SmartAccount` built with `builder_v1` and the built-in
  Ed25519 authenticator but no key can never gain one (the grey built-in state of §2).
* `custom_to_builtin.move`: a `SmartAccount` with a custom authenticator and no key attaches an Ed25519 key, then
  rotates to the built-in Ed25519 authenticator (grey → yellow → green), each step sent by the account and
  authenticated by its custom authenticator.

**Running them:**

```sh
IOTA_SKIP_SIMTESTS=1 cargo nextest run -p iota-types -p iota-indexer --lib
cargo test -p iota-framework-tests --test move_tests -- iota-framework

pushd dev-tools/pg-services-local && docker compose up -d postgres && popd
cargo nextest run -p iota-indexer --features pg_integration --test account_key_links_tests
pushd dev-tools/pg-services-local && docker compose down -v && popd
```

The compose file fixes `container_name: postgres`, so check `docker ps -a --filter name=postgres` first: a container
from another checkout is reused silently.

## 11. FAQ

| Question | Answer |
| --- | --- |
| Why index by `key_id` and not by the derived address? | Address derivation can fail (MultiSig must decode its committee) and works differently per scheme. `key_id` hashes the bytes, so it never fails and is the same for every scheme. Authentication still checks the address. |
| Can someone flood the results for my key? | Yes. `builtin_auth_builder_v1` is public and takes any `PublicKey`, so anyone can build accounts with your key for the price of gas. Nobody else can use them, but the RPC has no limit or paging yet (§9). |
| Can a MultiSig member find its accounts? | Yes. Each member key is linked too, and the result's `multisigKeyId` names the whole MultiSig key, so the wallet knows it is one signer of several (§3, §7). |
| What happens after `iota-indexer restore`? | The tables miss everything before the snapshot, and the RPC returns partial results with no error (§9). |
| Why can a result have `smartAccount = false`? | `attach_public_key` is public over any `UID`, so a key can be attached to an object that is not a framework `SmartAccount`. The link is indexed, with `authenticator = null` (§3). |
| How does the wallet know it can sign for an account? | `authenticator` is one of the five built-in kinds. `custom` means the account was found but the IOTA wallet cannot sign for it. |
| Can I trust the result? | Nothing in it is authenticated. If needed, check on chain that the object exists, that its attached key is the one queried, and that its authenticator matches. |
| Can a claimed account be told apart from one built with my key? | No, on purpose (§8). Both emit the same events. |
| Can an account lock itself? | Yes: by detaching or rotating away the only key it can sign with, or by rotating to an authenticator nobody can satisfy. A keyless account with a built-in authenticator cannot recover (§2, §10). |
| Why does the authenticator kind have its own table? | Different events write it, and `PublicKeyAttached` does not say which authenticator the account has, so the kind must already be recorded when a key is attached later (§8). |
| Does this change consensus? | Only through the framework: the new `SmartAccountCreated` event. Everything else is indexer and RPC. The features it depends on are enabled at protocol version 38, not on testnet or mainnet. |
| How does a wallet get live updates? | `iota_subscribeEvent` on a fullnode, filtered on the three modules of §5. |
