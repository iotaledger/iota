# [Alt 3 — current] Event Stream + Replayable Off-Chain Indexer: Design & Implementation Plan

**Goal:** Make every discoverability-relevant operation (claim, builtin-key attach/detach/rotate) emit a frozen, structured Move event, and extend the canonical `iota-indexer` to fold that event stream into a `key_id → accounts` reverse index served over JSON-RPC — so that any wallet, holding only a seed, recovers its accounts with one indexer query, and any third party can run an identical, independently verifiable indexer by replaying the same events.

**Architecture:** Zero new chain state. The Move framework gains one new event (`smart_account::SmartAccountClaimed`, already on the branch); the three `PublicKey*` events shipped in PR #11856 cover attach/detach/rotate. The indexer ingests these four event types in checkpoint order into two Postgres tables — `account_key_links` (the key→account link graph) and `claimed_accounts` (which accounts came from a `ClaimAccount` transaction) — and serves `iotax_getAccountsByPublicKey`. The event schema is the public, replayable contract; the IF-run indexer is one deployment of open-source code, not a trust point.

**Tech Stack:** Move (iota-framework system package), Rust (`iota-types`, `iota-indexer`, `iota-json-rpc-api`), Diesel/Postgres, `simulacrum` + `pg_integration` tests, insta snapshots.

**Written against** `vm-lang/10722-fix-after-rebase` at `9b9746e4f4` — _key_id_, 15 Sep 2026.

> **What changed since the original Alt 3 page.** The claim registry is gone: `ClaimRegistry`, `claim_registry.move`, the `0x10` singleton and `iota_types::claim_registry` no longer exist, in the framework, at genesis or in the node. Claiming is now split between `iota::claim` (a 36-line address helper) and `iota::smart_account` (the actual entry points). The claim event is `SmartAccountClaimed` in `iota::smart_account`, not `ClaimedAddress` in `claim_registry`, and it carries the whole `PublicKey` rather than a `key_id` field. `key_id` shipped in Move, which the original plan did not anticipate and which is now an open question (see **Decisions needed**). The single `account_key_links` table becomes two tables.

---

## Implementation status (as of 15 Sep 2026)

All nine tasks are implemented on `vm-lang/10722-fix-after-rebase`, commits `a772a4433c`..`7a19a54b3a`.
`cargo clippy --all-targets --all-features -- -D warnings` is clean on every touched crate; 335 `iota-types`
and 44 `iota-indexer` unit tests and 35 `smart_account` / 3 `key_id` Move tests pass.

### Still missing

1. **The `pg_integration` tests have never been run.** No PostgreSQL was reachable on `localhost:5432`.
   `crates/iota-indexer/tests/account_key_links_tests.rs` compiles under `--features pg_integration` but every
   assertion in it is unproven. Run:
   `cargo nextest run -p iota-indexer --features pg_integration --test account_key_links_tests`.
2. **`schema.rs` was hand-edited, not generated.** `scripts/indexer-schema/generate.sh` needs Docker (daemon not
   running) and the `diesel` CLI (not installed). The two `diesel::table!` blocks and the `for_all_tables!`
   entries were written to match what generation produces — alphabetical placement, matching column types — but
   that has not been confirmed. Re-run the script and check the diff is empty.
3. **`dprint fmt` was not run** (dprint not installed), so TOML/Markdown/YAML formatting is unverified for
   `.config/nextest.toml`, `iota-indexer.mdx` and this document.
4. **`cargo simtest -p iota-e2e-tests --test claim_account_tests` was not run** — the rollout's regression gate.
5. **Rotation and detach are not covered end to end.** Both need the account itself as transaction sender, hence
   a `MoveAuthenticator`; they are driven from event payloads in the unit tests only.

### Decisions still open, and what was assumed

* **#1 (`key_id` in Move)** — assumed **keep**, on the conservative reading that shipped code should not be
  deleted while the decision is open. Tests and the snapshot entry were added on that basis. If the decision goes
  the other way, `public_key::key_id`, its three Move tests and its `published_api.txt` entry all come back out;
  the Rust definition is unaffected.
* **#2 (what the events emit)** — unresolved; the implementation assumes the events keep carrying the full
  `PublicKey`, which is what is on the branch today.
* **#3 (`smart_account` attach/detach/rotate events)** — unresolved; **not implemented**, matching the leaning
  recorded below.

### Pre-existing failures on the branch (not caused by this work, still red)

Both were verified to fail at `HEAD` with these changes stashed:

* `iota-cost` `test_good_snapshot` — genesis aborts with
  `PackageTooBig: Move package with size 104133 is larger than the maximum object size 102400`. The framework
  package is over the limit; this work adds ~250 bytes to a package that was already ~1.5 KB over.
* `iota-framework` Move `bls12381_tests::test_uncompressed_g1_sum_too_long` — runs out of gas instead of aborting
  with code 2.

Both block the "suites green" gate in the rollout checklist and need fixing independently.

### Deviations from this document, for review

* `AccountKeyLink.immutable` is `Option<bool>`, not `bool`: an account with no claim record has an unknown
  immutability, not a false one.
* `MovePublicKey::scheme_flag()` was added to `iota-types` (not in the plan). `scheme()` panics on a flag byte the
  build does not know, which a chain-read value may carry once the framework gains a scheme — that would break
  the fold's totality.
* `LinkSource::from_stored` was added so the persisted discriminants are interpreted in one place; a test pins
  them against renumbering.
* `crates/iota-indexer/tests/account_key_links_tests.rs` carries `#[expect(dead_code)]` on the shared `mod
  common`, matching `ingestion_tests.rs`. It is a lint suppression, which the repo conventions forbid; flagged
  rather than removed because every test file in that directory does it.
* The regenerated `openrpc.json` also absorbs pre-existing branch drift (protocol version 35 → 36, a
  transaction-kind description), as does the refreshed `iota-swarm-config` genesis snapshot.

---

## 1. Design

### 1.1. Current state

Already merged on the feature branch `vm-lang/10722-fix-after-rebase`:

| **Piece** | **Where** | **Commit** |
| --- | --- | --- |
| Built-in Move authenticators + feature gating | `.../sources/account_abstraction/builtin_authenticator_functions.move` | `5015b8be0f` (PR #11184), `6dcad75cd3` (PR #11483) |
| `SmartAccount` + `SmartAccountBuilder` + `rotate_builtin_auth_public_key` | `.../sources/account_abstraction/smart_account.move` | `8165377a57` (PR #11728) |
| **Events** `PublicKeyAttached { account_id, public_key }`, `PublicKeyDetached { account_id, public_key }`, `PublicKeyRotated { account_id, from, to }` | `.../sources/account_abstraction/builtin_authenticator_functions.move:72-88` | `3e679e5ee6` (PR #11856) |
| `PublicKey { scheme, raw_bytes }` + full construction-time validation | `.../sources/account_abstraction/public_key.move` | `61f6b1fe17` (PR #12005), `e60dab7e82` (PR #12126) |
| `TransactionKind::ClaimAccount`, `iota::claim::claim_address`, the private `claim_account_v1` / `claim_immutable_account_v1` | `.../sources/account_abstraction/claim.move`, `.../smart_account.move`, `iota-execution/latest/iota-adapter/src/execution_engine.rs:2085` | `3f72b2497b` (PR #12082) |
| **Event** `SmartAccountClaimed { account_id, public_key, immutable }` | `.../sources/account_abstraction/smart_account.move:50-55` | `ea2da5d222` |
| `public_key::key_id` | `.../sources/account_abstraction/public_key.move:115-119` | `9b9746e4f4` |

**What the claim path looks like now.** There is no registry and no marker. `iota::claim` is the whole of what survived:

```move
public(package) fun claim_address(public_key: PublicKey, ctx: &TxContext): UID {
    let derived_addr = public_key.to_iota_address();
    assert!(derived_addr == ctx.sender(), EAddressMismatch);
    object::new_uid_from_hash(derived_addr)
}
```

The claim entry points live in `smart_account` and are **private on purpose**: the object they create has an id equal to a signature-derivable address, so `ClaimAccount` must stay the only way such an object can come into existence. A private function is reachable from the node's own PTB — which runs in an execution mode that bypasses visibility — and from nowhere else: not from a user PTB, and not from another package that could wrap a public entry point. `iota::clock::consensus_commit_prologue` uses the same idiom.

Because nothing shared is touched, a claim carries no shared inputs and stays off the consensus path.

**Gating.** `enable_claim_account_transaction`, checked in `TransactionKind::validity_check`, is turned on at **protocol version 36** for every chain except testnet and mainnet (`crates/iota-protocol-config/src/lib.rs:3554-3561`). A smart-account claim additionally requires `enable_builtin_move_authenticators`, since without it the claim would create an account that can never authenticate anything.

**Double-claiming is not prevented yet.** `claim_address` leaves it to the caller, and `claim_builder` does not implement it. `test_claim_account_twice_is_not_yet_prevented` (`crates/iota-e2e-tests/tests/claim_account_tests.rs:200`) pins the current behaviour for the fix to flip.

Infrastructure facts that shape the plan:

| **Piece** | **Description** | **Need** |
| --- | --- | --- |
| **Event pipeline** | Move `event::emit` → `TransactionEvents` (defined in the external `iota-sdk-types` crate) → checkpoint data → `iota-indexer`'s `index_transactions` already walks every transaction's events (`prepare.rs:292`, `IndexedEvent::from_event` at `prepare.rs:392`) and elsewhere parses one framework event by `StructTag` match + BCS decode (`SystemEpochInfoEvent`, `prepare.rs:160-176`) | That is the exact pattern this design extends |
| **Indexer serving** | The indexer runs the fullnode's JSON-RPC framework over HTTP and implements the `iota-json-rpc-api` traits. `ExtendedApi` (`iotax` namespace) is implemented **only** by the indexer — `iota-node` never registers it | This design adds one `ExtendedApi` method |
| **Pruning** | The indexer prunes `events` and `event_*` tables by tx-sequence range (`crates/iota-indexer/src/pruning/pruner.rs:62`) | The two new tables are _materialized state_, not history: **neither may be prunable** |
| **Upsert idiom** | `on_conflict_do_update_with_condition!` (`crates/iota-indexer/src/store/mod.rs:230`) already expresses guarded upserts | Reuse it for the monotonic guard, do not hand-roll |

### 1.2. The `key_id` → canonical key identity

```
key_id = blake2b256( scheme_flag_byte || raw_key_bytes )   →  32 bytes
```

The deciding property is **totality**: `key_id` is a hash of bytes and cannot fail, for any scheme, on any input — no curve check, no committee parse, no scheme-specific branch. The alternative identity, the derived address, is fallible: `MovePublicKey::address()` returns a `Result` because MultiSig has to BCS-decode a committee first (`crates/iota-types/src/account_abstraction/public_key.rs:97`). Since the indexer deserializes chain bytes without re-validating them, using the address would put a failure mode on the **primary key of the index**.

Two further properties, both minor on their own:

* **Uniform across schemes.** Address derivation is not: Ed25519 omits its flag, and MultiSig hashes a structured preimage rather than the raw committee bytes.
* **A separate namespace.** A `key_id` is never confusable with an account address.

`key_id` coincides with the derived address for Secp256k1, Secp256r1 and Passkey — for those schemes address derivation is exactly `blake2b256(flag ‖ pk)` — and differs for Ed25519 and MultiSig. Harmless, but it means `key_id == account_id` is **not** a usable test for anything; claim provenance comes from the event, never from a column comparison.

**`key_id` has no protocol role.** Authentication verifies a signature against the address derived from the stored `PublicKey` (`iota_types::account_abstraction::builtin_authenticator_functions::verify_builtin_signature`). `key_id` is an index identity and nothing more.

**Where it is defined today.** Only in Move, at `public_key.move:115-119`:

```move
public fun key_id(self: &PublicKey): address {
    let mut flag = vector[self.scheme.flag()];
    flag.append(self.raw_bytes);
    address::from_bytes(hash::blake2b256(&flag))
}
```

It has **no consumer** — nothing on chain calls it, no event carries it, and there is no Rust twin. The Rust side that the indexer actually needs is still outstanding (Task 3). Whether the Move definition should stay at all is an open question; see **Decisions needed** #1.

Because the four consumed events all carry the whole `PublicKey`, the indexer computes `key_id` from the payload and **address derivation disappears from the indexer entirely**. Nothing an independent implementer has to re-derive, and no fallible operation anywhere in the fold.

### 1.3. The event schema

| Event | Module | Fields | Carries key? | Status |
| --- | --- | --- | --- | --- |
| `SmartAccountClaimed` | `iota::smart_account` | `account_id: ID, public_key: PublicKey, immutable: bool` | **yes** | shipped (`ea2da5d222`) |
| `PublicKeyAttached` | `iota::builtin_authenticator_functions` | `account_id: ID, public_key: PublicKey` | **yes** | shipped (PR #11856) |
| `PublicKeyDetached` | `iota::builtin_authenticator_functions` | `account_id: ID, public_key: PublicKey` | **yes** | shipped (PR #11856) |
| `PublicKeyRotated` | `iota::builtin_authenticator_functions` | `account_id: ID, from: PublicKey, to: PublicKey` | **yes** | shipped (PR #11856) |
| `MutableAccountCreated<Account>` / `ImmutableAccountCreated<Account>` / `AuthenticatorFunctionRefV1Rotated<Account>` | `iota::account` | account lifecycle with an `AuthenticatorFunctionRefV1` | no | shipped; **not consumed** |

The three `iota::account` events are generic, so their `StructTag`s carry a type parameter that any matcher would have to allow for. They are not consumed: `SmartAccountClaimed.immutable` supplies the only property of theirs the index wants.

**Why a claim needs its own event.** At the event level a claim is otherwise indistinguishable from an ordinary key attachment — `claim_builder` calls `attach_public_key`, so both arrive as `PublicKeyAttached { account_id, public_key }`. An indexer could in principle classify a claim by computing `address(public_key)` and comparing it to `account_id`, but that makes replayability conditional on reimplementing IOTA address derivation for all five schemes, including the MultiSig structured preimage and the Ed25519 legacy rule that omits its own flag. An implementation that gets the Ed25519 exemption wrong produces a plausible but different index. It also puts a fallible operation in the fold, and it makes re-claims invisible — a second claim re-emits an identical `PublicKeyAttached`, which the upsert absorbs. One event turns a reimplementation into a read.

**The event carries the whole `PublicKey`**, not just a flag or a `key_id`, so it is self-sufficient: one event, one complete fact, no correlation with the neighbouring `PublicKeyAttached` required.

**Emission order.** Both entry points emit after the finalizer, so within a claim transaction the order is `PublicKeyAttached` → `{Mutable,Immutable}AccountCreated` → `SmartAccountClaimed`. The fold relies on this.

**Fold semantics** — the index is a pure left-fold of these four events in total order `(checkpoint_sequence, tx_sequence_in_checkpoint, event_sequence_in_tx)`:

```
PublicKeyAttached(account, pk)                → link      (key_id(pk), account)   source=attach, ACTIVE
PublicKeyRotated(account, from, to)           → tombstone (key_id(from), account)
                                                then link (key_id(to), account)   source=rotate, ACTIVE
PublicKeyDetached(account, pk)                → tombstone (key_id(pk), account)    source=detach
SmartAccountClaimed(account, pk, immutable)   → link      (key_id(pk), account)   source=claim,  ACTIVE
                                                and upsert claimed_accounts(account, key_id, immutable)
```

Two orderings carry weight:

* Within a rotation, the unlink must precede the link, so that rotating a key onto itself leaves the link active.
* Within a claim transaction, `PublicKeyAttached` precedes `SmartAccountClaimed`, and the batch collapse is last-write-wins per `(key_id, account_id)`. So the claim's `source = claim` overwrites the attach's `source = attach` deterministically, and a claim produces exactly one link row, not two.

Properties to preserve: **total** (every matched event yields its rows, no decode-or-drop path on the index key, no address derivation anywhere), **deterministic** (replaying the same checkpoints in the same order yields the same rows — this is what lets an independently run indexer be checked against ours), **idempotent and monotonic** (re-ingesting a checkpoint rewrites identical values; both upserts guarded on `excluded.<tx_sequence_number> >= existing`, so a replayed or out-of-order write can never move state backwards), and **batch-order independent** (collapse to one row per key before writing).

**Authentication.** Events are only ever emitted by authenticated chain operations: a claim requires `sender == derived address` (`claim_address`), and attach/detach/rotate require `sender == account` (`ensure_tx_sender_is_smart_account`). An adversary cannot fabricate a link to a victim's key on an account they control *and have it count as a claim*. The honest-but-unwanted case is dust-account gifting — creating an account through `builtin_auth_builder_v1` whose authenticator is the victim's key, a link that is _true_ — and the two-table split is what makes it filterable: such an account never appears in `claimed_accounts`, and every link on a claimed account is self-authorized because post-creation mutation requires the account itself to be the sender.

### 1.4. Trust model, retention, bootstrap

* **Default UX** queries the indexer; **trust-minimal mode**: spot-verify a returned account on chain (the account exists ∧ its attached built-in key is the one queried).
* **Retention:** fullnodes and indexers may prune old events. Both tables survive pruning, but a late-starting indexer cannot rebuild from a pruned source — it must bootstrap from an unpruned archive fullnode or import a table snapshot from a trusted/verified peer.
* **Privacy:** the shipped regime is R0 — anyone replaying public events can build the full reverse map, since key material is in the event payloads and `key_id` is derivable from it. A salted variant would require user-supplied salt in the claim payload. That is a protocol change, explicitly out of scope; recorded as future work.
* **Schema evolution:** after first public-network activation, the four event structs are additive-only — new event types may be added, existing fields never change. This is the reason to settle `SmartAccountClaimed`'s field set before testnet rather than after (**Decisions needed** #2).
* **MultiSig discovery does not work from a seed.** A MultiSig `PublicKey` holds the BCS-encoded committee as `raw_bytes`, so its `key_id` is a hash of the committee. A wallet restoring from a seed knows its own member key, not the committee, and cannot compute the lookup key. The fix is to additionally index each **member** key against the account; it carries a real privacy cost, so it is a decision, not an oversight.

### 1.5. Wallet discovery protocol (consumer contract)

```
on_seed_restore(seed):
    keys     = derive_all_known_keys(seed)                              # per scheme, per path
    p1       = [a for k in keys if exists_on_chain(derive_address(k))]  # indexer-free fallback
    linked   = flatten(rpc.iotax_getAccountsByPublicKey(prefixed(k)) for k in keys)
    verified = [l for l in linked if verify_on_chain(l.address, keys)]  # optional trust-minimal step
    return rank(dedupe(p1 + verified), claimed_first)
```

Live updates: wallets can subscribe to the four event types on a fullnode WebSocket today (`iota_subscribeEvent`). Note that the events now live in **two** modules, so a subscriber needs two `MoveEventModule` filters — `builtin_authenticator_functions` and `smart_account` — or one broader filter.

---

## 2. Implementation

### Global Constraints

* Branch off and PR **into** `vm-lang/10722-fix-after-rebase`, not `develop`.
* After any `.move` change: `UPDATE=1 cargo test -p iota-framework --test build-system-packages`, then `./scripts/update_all_snapshots.sh` before the final PR. Inspect every snapshot diff — an unintended change usually means a real regression.
* All new/changed files carry the license header: `// Copyright (c) 2026 IOTA Stiftung` / `// SPDX-License-Identifier: Apache-2.0`.
* Never disable or skip tests; no lint-suppression attributes.
* Indexer integration tests need a local Postgres and run under `--features pg_integration`.
* `LinkSource` discriminants are persisted in Postgres — they must never be renumbered.

---

### 2.1. Task 1: `smart_account::SmartAccountClaimed` (Move) — ✅ **DONE**

Landed on the branch in `ea2da5d222`. No TDD steps are reconstructed here; the code exists.

**Where it is:**

* Event struct: `crates/iota-framework/packages/iota-framework/sources/account_abstraction/smart_account.move:50-55`
* Emitted by `claim_account_v1` (`smart_account.move:352-360`, `immutable: false`) and `claim_immutable_account_v1` (`smart_account.move:373-381`, `immutable: true`), in both cases after the finalizer returns the account address.

```move
public struct SmartAccountClaimed has copy, drop {
    account_id: ID,
    public_key: PublicKey,
    immutable: bool,
}
```

**Produces:** the BCS layout consumed by Task 4.

**Outstanding on this task** (both real gaps on the branch, not bookkeeping):

* \[ \] **No test asserts the event.** `tests/account_abstraction/smart_account_tests.move:64-120` has four claim tests, and none of them inspects emitted events. Add: `claim_account_v1_for_testing` emits exactly one `SmartAccountClaimed` with `immutable = false`, the account id equal to the sender, and the claiming key; the immutable entry point emits `immutable = true`; the event follows `PublicKeyAttached` in the same transaction; `builtin_auth_builder_v1` + `build_v1` emits none.
* \[ \] **Framework snapshots not refreshed.** `crates/iota-framework/published_api.txt` does not list `SmartAccountClaimed`, and the working tree is clean — `UPDATE=1 cargo test -p iota-framework --test build-system-packages` and `./scripts/update_all_snapshots.sh` have not been run since `ea2da5d222`.

---

### 2.2. Task 2: `public_key::key_id` (Move) — ✅ **DONE**

Landed on the branch in `9b9746e4f4`. Code exists; see §1.2 for the body.

**Where it is:** `crates/iota-framework/packages/iota-framework/sources/account_abstraction/public_key.move:115-119`.

**Outstanding on this task:**

* \[ \] **No test.** `tests/account_abstraction/public_key_tests.move` does not mention `key_id`. If the function stays, it needs per-scheme fixed vectors and an explicit assertion that `key_id != to_iota_address()` for Ed25519 and MultiSig while coinciding for Secp256k1 / Secp256r1 / Passkey.
* \[ \] **Framework snapshots not refreshed.** `published_api.txt` lists `to_iota_address` for `0x2::public_key` but not `key_id`.
* \[ \] **No consumer.** Nothing on chain calls it, and no event carries a `key_id` field. See **Decisions needed** #1 — the alternative is to remove it and define `key_id` in Rust only.

---

### 2.3. Task 3: Rust `key_id` in `iota-types`

The Rust half of `key_id` does **not** exist on the branch — `crates/iota-types/src/claim_registry.rs` is gone along with the rest of the registry, and nothing replaced it. Tasks 4, 6 and 7 all consume this.

**Files:**

* Modify: `crates/iota-types/src/account_abstraction/public_key.rs`
* Test: same file, `#[cfg(test)] mod tests`

**Interfaces:**

* Produces (consumed by Tasks 4, 7):
    * `MovePublicKey::key_id(&self) -> [u8; 32]` — a method, because `MovePublicKey::raw_bytes` is a private field
    * `pub fn key_id(scheme_flag: u8, raw_key_bytes: &[u8]) -> [u8; 32]`
    * `pub fn key_id_from_prefixed_bytes(prefixed: &[u8]) -> Option<[u8; 32]>` — `None` on empty input, for callers holding loose prefixed bytes such as the RPC handler

* \[ \] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_id_is_blake2b256_of_flag_and_raw_bytes() {
        let pk = MovePublicKey::new(SignatureScheme::ED25519, ED25519_RAW.to_vec()).unwrap();
        assert_eq!(pk.key_id(), key_id(0x00, ED25519_RAW));
        assert_eq!(
            key_id_from_prefixed_bytes(&[&[0x00u8][..], ED25519_RAW].concat()).unwrap(),
            pk.key_id()
        );
    }

    #[test]
    fn key_id_differs_from_address_for_ed25519_and_multisig() {
        // Ed25519 address omits the flag; MultiSig hashes a structured
        // preimage. Both must therefore differ from key_id. Pin this: a later
        // "simplification" toward the address would pass a test that only
        // covered the coinciding schemes.
        let ed = MovePublicKey::new(SignatureScheme::ED25519, ED25519_RAW.to_vec()).unwrap();
        assert_ne!(ed.key_id(), ed.address().unwrap().into_inner());

        let ms = MovePublicKey::new(SignatureScheme::Multisig, MULTISIG_RAW.to_vec()).unwrap();
        assert_ne!(ms.key_id(), ms.address().unwrap().into_inner());
    }

    #[test]
    fn key_id_coincides_with_address_for_prefix_hashed_schemes() {
        for (scheme, raw) in [
            (SignatureScheme::Secp256k1, SECP256K1_RAW),
            (SignatureScheme::Secp256r1, SECP256R1_RAW),
            (SignatureScheme::PasskeyAuthenticator, PASSKEY_RAW),
        ] {
            let pk = MovePublicKey::new(scheme, raw.to_vec()).unwrap();
            assert_eq!(pk.key_id(), pk.address().unwrap().into_inner());
        }
    }

    #[test]
    fn key_id_from_prefixed_bytes_rejects_empty() {
        assert!(key_id_from_prefixed_bytes(&[]).is_none());
    }
}
```

* \[ \] **Step 2: Run test to verify it fails**

```sh
IOTA_SKIP_SIMTESTS=1 cargo nextest run -p iota-types --lib account_abstraction::public_key
```

Expected: FAIL — `key_id` is not a function.

* \[ \] **Step 3: Write the implementation**

```rust
use fastcrypto::hash::{Blake2b256, HashFunction};

/// Canonical identity hash of a public key: `Blake2b256(scheme_flag || raw_key_bytes)`.
///
/// This is an **index identity with no protocol role**. Authentication verifies
/// a signature against the address derived from the key — see
/// [`MovePublicKey::address`] — and the two values are deliberately different
/// for Ed25519 and MultiSig. The flag byte is included for every scheme.
pub fn key_id(scheme_flag: u8, raw_key_bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Blake2b256::default();
    hasher.update([scheme_flag]);
    hasher.update(raw_key_bytes);
    hasher.finalize().digest
}

/// `key_id` for flag-prefixed key bytes (`flag || raw`), the wire format of
/// `iota::public_key::from_prefixed_bytes`. Returns `None` for empty input.
pub fn key_id_from_prefixed_bytes(prefixed: &[u8]) -> Option<[u8; 32]> {
    let (flag, raw) = prefixed.split_first()?;
    Some(key_id(*flag, raw))
}

impl MovePublicKey {
    /// See the free [`key_id`] function. Total: unlike [`Self::address`], this
    /// cannot fail for any scheme or any byte input.
    pub fn key_id(&self) -> [u8; 32] {
        key_id(self.scheme().flag(), &self.raw_bytes)
    }
}
```

* \[ \] **Step 4: Run test to verify it passes**

```sh
IOTA_SKIP_SIMTESTS=1 cargo nextest run -p iota-types --lib account_abstraction::public_key
```

Expected: PASS.

* \[ \] **Step 5: Commit**

```sh
git add crates/iota-types/src/account_abstraction/public_key.rs
git commit -m "feat(iota-types): key_id canonical key identity for the discoverability index"
```

---

### 2.4. Task 4: Event mirrors + fold in the indexer

**Files:**

* Create: `crates/iota-indexer/src/account_key_events.rs`
* Modify: `crates/iota-indexer/src/lib.rs` (add `pub mod account_key_events;`)
* Test: same file, `#[cfg(test)] mod tests`

**Interfaces:**

* Consumes: `iota_types::account_abstraction::public_key::MovePublicKey` and its `key_id` (Task 3); `iota_sdk_types::events::Event` (`type_: StructTag`, `contents: Vec<u8>`).
* Produces (consumed by Task 6):
    * Event mirrors `SmartAccountClaimedEvent`, `PublicKeyAttachedEvent`, `PublicKeyDetachedEvent`, `PublicKeyRotatedEvent`
    * `pub fn account_key_link_ops(event: &Event, tx_sequence_number: i64, epoch: u64) -> Vec<AccountKeyLinkOp>`
    * `pub fn claimed_account_row(event: &Event, tx_sequence_number: i64, epoch: u64) -> Option<StoredClaimedAccount>`
    * `pub struct AccountKeyLinkOp { key_id, account_id, scheme, source, kind, tx_sequence_number, epoch }` with `enum LinkOpKind { Link, Unlink }` and `enum LinkSource { Attach = 0, Rotate = 1, Detach = 2, Claim = 3 }`

**Reuse, don't mirror `PublicKey`.** `MovePublicKey` already exists in `iota-types` with the right BCS layout. Hand-rolling `MovePublicKey` / `MoveSignatureScheme` mirrors inside the indexer, as the original Alt 3 draft did, duplicates a type that must stay byte-compatible with the framework.

* \[ \] **Step 1: Write the failing test**

BCS round-trip against hand-built payloads (Move `PublicKey` serializes as `flag_byte, uleb_len, raw_bytes…`; `ID` as 32 bytes; `bool` as one byte; event structs as plain field concatenation):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smart_account_claimed_bcs_layout() {
        // Move: SmartAccountClaimed { account_id: [0x11; 32],
        //   public_key: PublicKey { scheme: { flag: 0x00 }, raw_bytes: [0xAA; 32] },
        //   immutable: true }
        let mut bytes = vec![0x11u8; 32];
        bytes.extend([0x00u8, 32]);
        bytes.extend([0xAA; 32]);
        bytes.push(1);
        let e: SmartAccountClaimedEvent = bcs::from_bytes(&bytes).unwrap();
        assert_eq!(e.public_key.scheme().flag(), 0x00);
        assert!(e.immutable);
    }

    #[test]
    fn rotated_event_produces_unlink_then_link() {
        let ev = rotated_event(ed25519_pk(), secp256k1_pk());
        let ops = account_key_link_ops(&ev, 7, 3);
        assert_eq!(ops.len(), 2);
        assert!(matches!(ops[0].kind, LinkOpKind::Unlink));
        assert!(matches!(ops[1].kind, LinkOpKind::Link));
    }

    #[test]
    fn rotation_onto_the_same_key_ends_active() {
        let pk = ed25519_pk();
        let ops = account_key_link_ops(&rotated_event(pk.clone(), pk), 7, 3);
        assert_eq!(ops[0].key_id, ops[1].key_id);
        assert!(matches!(ops[1].kind, LinkOpKind::Link)); // link is last, so it wins
    }

    #[test]
    fn claimed_event_yields_a_claim_link_and_a_claimed_row() {
        let ev = claimed_event(ed25519_pk(), false);
        let ops = account_key_link_ops(&ev, 9, 4);
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].source, LinkSource::Claim);
        let row = claimed_account_row(&ev, 9, 4).unwrap();
        assert_eq!(row.key_id, ops[0].key_id);
        assert!(!row.immutable);
    }

    #[test]
    fn foreign_and_unknown_events_are_ignored() {
        // Not the framework package.
        assert!(account_key_link_ops(&event_from_package(SOME_OTHER_PACKAGE), 1, 1).is_empty());
        // Right module, wrong struct.
        assert!(account_key_link_ops(&smart_account_event("SomethingElse"), 1, 1).is_empty());
        // Right struct, payload this build cannot decode.
        assert!(account_key_link_ops(&claimed_event_with_garbage_payload(), 1, 1).is_empty());
    }
}
```

* \[ \] **Step 2: Run test to verify it fails**

```sh
IOTA_SKIP_SIMTESTS=1 cargo nextest run -p iota-indexer --lib account_key_events
```

Expected: FAIL — module missing.

* \[ \] **Step 3: Write the implementation**

`crates/iota-indexer/src/account_key_events.rs`, matching each event on its `StructTag`: address `0x2`, module `builtin_authenticator_functions` or `smart_account`, then the struct name.

```rust
//! Rust mirrors of the account-discoverability Move events and the fold that
//! turns them into `account_key_links` and `claimed_accounts` rows.
//!
//! These types mirror BCS layouts frozen in the iota-framework
//! (`builtin_authenticator_functions.move`, `smart_account.move`); evolution is
//! additive-only once the feature activates on a public network.

use iota_sdk_types::{Address, ObjectId, events::Event};
use iota_types::account_abstraction::public_key::MovePublicKey;
use serde::Deserialize;

/// Mirror of `iota::smart_account::SmartAccountClaimed`.
#[derive(Debug, Clone, Deserialize)]
pub struct SmartAccountClaimedEvent {
    pub account_id: ObjectId,
    pub public_key: MovePublicKey,
    pub immutable: bool,
}

/// Mirror of `iota::builtin_authenticator_functions::PublicKeyAttached`.
#[derive(Debug, Clone, Deserialize)]
pub struct PublicKeyAttachedEvent {
    pub account_id: ObjectId,
    pub public_key: MovePublicKey,
}

// PublicKeyDetachedEvent is the same shape; PublicKeyRotatedEvent carries
// `from` and `to` instead of `public_key`.

/// Provenance of a link row. Persisted as `SMALLINT`: never renumber.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkSource {
    Attach = 0,
    Rotate = 1,
    Detach = 2,
    Claim = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkOpKind {
    Link,
    Unlink,
}

/// One fold step of the discoverability event stream.
#[derive(Debug, Clone)]
pub struct AccountKeyLinkOp {
    pub key_id: Vec<u8>,
    pub account_id: Vec<u8>,
    pub scheme: i16,
    pub source: LinkSource,
    pub kind: LinkOpKind,
    pub tx_sequence_number: i64,
    pub epoch: i64,
}

fn is_framework_event(event: &Event, module: &str, name: &str) -> bool {
    event.type_.address() == &Address::TWO
        && event.type_.module().as_str() == module
        && event.type_.name().as_str() == name
}

/// Decodes `event` into zero, one, or two link operations (see the fold
/// semantics in the plan). Non-discoverability events yield `vec![]`.
///
/// An event whose type matches but whose payload will not decode also yields
/// nothing: this build predates a framework change and cannot interpret it.
/// There is no other failure path — `key_id` is total, and no address is
/// derived anywhere in this module.
pub fn account_key_link_ops(
    event: &Event,
    tx_sequence_number: i64,
    epoch: u64,
) -> Vec<AccountKeyLinkOp> {
    // … one branch per event type; rotate yields Unlink(from) then Link(to).
}

/// The `claimed_accounts` row a `SmartAccountClaimed` event produces, or `None`
/// for every other event.
pub fn claimed_account_row(
    event: &Event,
    tx_sequence_number: i64,
    epoch: u64,
) -> Option<StoredClaimedAccount> {
    // …
}
```

* \[ \] **Step 4: Run test to verify it passes**

```sh
IOTA_SKIP_SIMTESTS=1 cargo nextest run -p iota-indexer --lib account_key_events
```

Expected: PASS.

* \[ \] **Step 5: Commit**

```sh
git add crates/iota-indexer/src/account_key_events.rs crates/iota-indexer/src/lib.rs
git commit -m "feat(indexer): mirror account discoverability events and fold them into link ops"
```

---

### 2.5. Task 5: Postgres tables, Diesel schema, models

Two tables, split along the line between a link and a per-account fact.

**Why claimed status is its own table rather than a column on the link.** Being claimed is a property of the _account_, and it outlives any particular link: if an account claimed at `A` with key `K` rotates to `K2`, the original link is tombstoned and a new one appears, but `A` is still a claimed account and a wallet holding `K2` should see that. Denormalizing the flag onto link rows would mean the rotate handler has to look up whether the account was ever claimed — a read-modify-write inside ingestion, which breaks the pure-fold and batch-collapse properties in §1.3. Two tables keep both writes blind.

**Files:**

* Create: `crates/iota-indexer/migrations/pg/2026-09-15-000000_account_discoverability/up.sql` and `down.sql`
* Modify: `crates/iota-indexer/src/schema.rs` (two `diesel::table!` blocks + both names in `for_all_tables!`, `schema.rs:455-501`)
* Create: `crates/iota-indexer/src/models/account_key_links.rs`, `crates/iota-indexer/src/models/claimed_accounts.rs`
* Modify: `crates/iota-indexer/src/models/mod.rs`
* DO NOT modify: `crates/iota-indexer/src/pruning/pruner.rs` — neither table may appear in `PrunableTable` (`pruner.rs:62`)

**Interfaces:**

* Produces: both tables plus the `StoredAccountKeyLink` and `StoredClaimedAccount` models (consumed by Tasks 4, 6, 7).

* \[ \] **Step 1: Migration**

`up.sql`:

```sql
-- Materialized fold of the account-discoverability event stream: one row per
-- (key_id, account_id) pair with its latest link state.
-- NOT history (the events table holds that) and NOT prunable.
CREATE TABLE account_key_links (
    key_id                          BYTEA    NOT NULL,  -- blake2b256(flag || raw_bytes)
    account_id                      BYTEA    NOT NULL,
    scheme                          SMALLINT NOT NULL,  -- signature scheme flag
    source                          SMALLINT NOT NULL,  -- 0 attach, 1 rotate, 2 detach, 3 claim
    status                          SMALLINT NOT NULL,  -- 0 active, 1 unlinked (tombstone)
    last_change_tx_sequence_number  BIGINT   NOT NULL,
    last_change_epoch               BIGINT   NOT NULL,
    PRIMARY KEY (key_id, account_id)
);
CREATE INDEX account_key_links_account ON account_key_links (account_id);
CREATE INDEX account_key_links_active  ON account_key_links (key_id) WHERE status = 0;

-- Accounts created by a ClaimAccount transaction. Written only from
-- SmartAccountClaimed. Absence means the account was not claimed.
CREATE TABLE claimed_accounts (
    account_id                BYTEA    NOT NULL PRIMARY KEY,
    key_id                    BYTEA    NOT NULL,  -- the key that claimed the address
    immutable                 BOOLEAN  NOT NULL,
    claim_tx_sequence_number  BIGINT   NOT NULL,
    claim_epoch               BIGINT   NOT NULL
);
CREATE INDEX claimed_accounts_key ON claimed_accounts (key_id);
```

`down.sql`:

```sql
DROP TABLE IF EXISTS claimed_accounts;
DROP TABLE IF EXISTS account_key_links;
```

* \[ \] **Step 2: Diesel schema blocks**

Add to `crates/iota-indexer/src/schema.rs` in alphabetical position, matching the generated style, and add both names to `for_all_tables!`:

```rust
diesel::table! {
    account_key_links (key_id, account_id) {
        key_id -> Bytea,
        account_id -> Bytea,
        scheme -> Int2,
        source -> Int2,
        status -> Int2,
        last_change_tx_sequence_number -> Int8,
        last_change_epoch -> Int8,
    }
}

diesel::table! {
    claimed_accounts (account_id) {
        account_id -> Bytea,
        key_id -> Bytea,
        immutable -> Bool,
        claim_tx_sequence_number -> Int8,
        claim_epoch -> Int8,
    }
}
```

* \[ \] **Step 3: Models**

`StoredAccountKeyLink` with `LINK_STATUS_ACTIVE: i16 = 0` / `LINK_STATUS_UNLINKED: i16 = 1` and a `From<&AccountKeyLinkOp>` impl mapping `LinkOpKind::Link → ACTIVE`, `LinkOpKind::Unlink → UNLINKED`; `StoredClaimedAccount` mirroring the second table. Register both in `models/mod.rs`.

* \[ \] **Step 4: Note the pruning exemption**

Next to `PrunableTable` in `pruning/pruner.rs`, leave a comment saying neither discoverability table is prunable, and why: a pruned row could only be rebuilt by replaying events this node may itself have pruned. It is the kind of omission a later contributor will otherwise "fix".

* \[ \] **Step 5: Check + commit**

```sh
cargo check -p iota-indexer
git add crates/iota-indexer/migrations crates/iota-indexer/src/schema.rs \
        crates/iota-indexer/src/models crates/iota-indexer/src/pruning/pruner.rs
git commit -m "feat(indexer): account_key_links and claimed_accounts tables and models"
```

---

### 2.6. Task 6: Ingestion — parse, collapse, persist

**Files:**

* Modify: `crates/iota-indexer/src/ingestion/primary/prepare.rs` (`index_transactions` at `:292`; the per-transaction event walk around `IndexedEvent::from_event`, `:388-400`)
* Modify: `crates/iota-indexer/src/ingestion/primary/persist.rs` (batching, next to the `persist_events` call at `:168`)
* Modify: `crates/iota-indexer/src/store/indexer_store.rs` (trait) and `crates/iota-indexer/src/store/pg_indexer_store.rs` (impl)
* Modify: `crates/iota-indexer/src/metrics.rs` (one commit-latency histogram per table, following `checkpoint_db_commit_latency_*`)

**Interfaces:**

* Consumes: `account_key_link_ops` / `claimed_account_row` (Task 4), `StoredAccountKeyLink` / `StoredClaimedAccount` (Task 5).
* Produces: `IndexerStore::persist_account_key_links(..)` and `IndexerStore::persist_claimed_accounts(..)`.

Collect in `index_transactions` rather than further out: that is the level that has the checkpoint epoch and the transaction sequence number, and it keeps speculative pre-checkpoint transactions out of the index.

* \[ \] **Step 1: Write the failing test**

Unit-test the collapse in isolation, before wiring anything:

```rust
#[test]
fn collapse_keeps_the_last_op_per_pair() { /* two ops, same pair, later tx wins */ }

#[test]
fn attach_then_claim_in_one_batch_yields_one_row_sourced_claim() {
    // A claim transaction emits PublicKeyAttached then SmartAccountClaimed for
    // the same (key_id, account_id). Last write wins, so the row is source=claim.
}

#[test]
fn rotation_back_onto_the_same_key_stays_active() { /* unlink then link, same pair */ }

#[test]
fn an_empty_batch_writes_nothing() { /* no rows, no query */ }
```

* \[ \] **Step 2: Run test to verify it fails**

```sh
IOTA_SKIP_SIMTESTS=1 cargo nextest run -p iota-indexer --lib account_key
```

Expected: FAIL — the collapse helper and store methods do not exist.

* \[ \] **Step 3: Write the implementation**

Collect ops alongside the existing event walk and thread them through the same per-checkpoint container that carries `IndexedEvent`s to the writer:

```rust
let account_key_link_ops: Vec<AccountKeyLinkOp> = events
    .iter()
    .flat_map(|e| account_key_link_ops(e, tx_sequence_number, checkpoint_epoch))
    .collect();
let claimed_accounts: Vec<StoredClaimedAccount> = events
    .iter()
    .filter_map(|e| claimed_account_row(e, tx_sequence_number, checkpoint_epoch))
    .collect();
```

Before writing, collapse to final row state per key, so the result does not depend on how a batch is chunked:

```rust
// Last op per (key_id, account_id) wins: the table stores latest state, and
// batch upserts must not depend on intra-batch write order.
let mut final_state: HashMap<(Vec<u8>, Vec<u8>), StoredAccountKeyLink> = HashMap::new();
for op in &ops {
    final_state.insert(
        (op.key_id.clone(), op.account_id.clone()),
        StoredAccountKeyLink::from(op),
    );
}
let rows: Vec<StoredAccountKeyLink> = final_state.into_values().collect();
```

Then upsert both tables through the existing `on_conflict_do_update_with_condition!` macro (`store/mod.rs:230`), guarded monotonically on `excluded.last_change_tx_sequence_number >= account_key_links.last_change_tx_sequence_number` (and the `claim_tx_sequence_number` equivalent), so a replayed or out-of-order write can never move state backwards. Replays are then idempotent: re-ingesting a checkpoint upserts identical values.

* \[ \] **Step 4: Run test to verify it passes**

```sh
IOTA_SKIP_SIMTESTS=1 cargo nextest run -p iota-indexer --lib account_key
cargo check -p iota-indexer
```

Expected: PASS, and a clean check — the trait addition surfaces every store impl that needs the new methods; implement all of them.

* \[ \] **Step 5: Commit**

```sh
git add crates/iota-indexer/src
git commit -m "feat(indexer): ingest discoverability events into the account link tables"
```

---

### 2.7. Task 7: Serving — `iotax_getAccountsByPublicKey`

**Files:**

* Modify: `crates/iota-json-rpc-api/src/extended.rs` (trait, `iotax` namespace)
* Modify: `crates/iota-indexer/src/apis/extended_api.rs` (impl, alongside the ten existing methods)
* Modify: `crates/iota-indexer/src/read.rs` (reader query, following the `run_query!` idiom used 26 times in that file)
* Modify (generated): `crates/iota-open-rpc/spec/openrpc.json`

**Interfaces:**

* Consumes: `key_id_from_prefixed_bytes` (Task 3), `StoredAccountKeyLink` / `StoredClaimedAccount` (Task 5).
* Produces: RPC `iotax_getAccountsByPublicKey(publicKey: Base64, includeUnlinked: Option<bool>) -> Vec<AccountKeyLink>`.

`iota-node` never registers `ExtendedApi`, so only the indexer serves this.

**Return rows, not bare addresses.** The row shape is `{ address, claimed: bool, immutable: bool, status, scheme, source, last_change_epoch }`, produced by left-joining `account_key_links` against `claimed_accounts`. `claimed` is what makes the result rankable and it is not computable client-side from an address list; `immutable` tells a wallet the link can never change.

The handler hashes the supplied bytes into a `key_id` directly — no validation, no derivation.

* \[ \] **Step 1: Write the failing test**

Add the reader-level test to `read.rs`'s test module (or the nearest equivalent): given seeded rows, an active-only query excludes tombstones, `include_unlinked` includes them, and the join reports `claimed`/`immutable` correctly for a claimed account and for an attach-only account. Empty input to the handler is a parameter error, not a panic.

* \[ \] **Step 2: Run test to verify it fails**

```sh
IOTA_SKIP_SIMTESTS=1 cargo nextest run -p iota-indexer --lib get_accounts_by
```

Expected: FAIL — the reader method does not exist.

* \[ \] **Step 3: Write the implementation**

Trait method in `crates/iota-json-rpc-api/src/extended.rs`:

```rust
    /// Return the accounts controlled by the given public key, folded from the
    /// on-chain discoverability event stream (SmartAccountClaimed /
    /// PublicKeyAttached / PublicKeyRotated / PublicKeyDetached).
    #[method(name = "getAccountsByPublicKey")]
    async fn get_accounts_by_public_key(
        &self,
        /// Scheme-flag-prefixed public key bytes (`flag || raw key bytes`),
        /// Base64-encoded — the wire format of `iota::public_key::from_prefixed_bytes`.
        public_key: Base64,
        /// When true, also return tombstoned links (keys rotated away or
        /// detached), for "you used to control this" recovery UX.
        include_unlinked: Option<bool>,
    ) -> RpcResult<Vec<AccountKeyLink>>;
```

Reader query in `read.rs`, following the `run_query!` idiom, filtering on `status = LINK_STATUS_ACTIVE` unless `include_unlinked`, ordered by `last_change_tx_sequence_number` descending, left-joined against `claimed_accounts`; add the async `_in_blocking_task` wrapper next to it, mirroring the existing wrappers. API impl in `extended_api.rs` using that file's existing error helpers.

* \[ \] **Step 4: Run test to verify it passes, and regenerate the spec**

```sh
IOTA_SKIP_SIMTESTS=1 cargo nextest run -p iota-indexer --lib get_accounts_by
(cd crates/iota-open-rpc && cargo run --example generate-json-rpc-spec -- record)
cargo test -p iota-open-rpc
```

Expected: PASS; the spec diff shows exactly the one new `iotax_` method.

* \[ \] **Step 5: Commit**

```sh
git add crates/iota-json-rpc-api crates/iota-indexer crates/iota-open-rpc
git commit -m "feat(indexer): serve iotax_getAccountsByPublicKey from the account link tables"
```

---

### 2.8. Task 8: Integration tests — claim → rotate → query (`pg_integration`)

**Files:**

* Create: `crates/iota-indexer/tests/account_key_links_tests.rs`
* Modify: `.config/nextest.toml` — the new test binary shares a database with the other integration tests, so add it to the single-threaded override next to `package(iota-indexer) and (binary(ingestion_tests))` (`.config/nextest.toml:19`)

**Interfaces:** consumes everything above.

> **⚠️ Not yet verified — needs a local Postgres.** The tests in this task were written and compile
> (`cargo check -p iota-indexer --features pg_integration --tests` is clean), but they have **never been
> executed**: no PostgreSQL instance was reachable on `localhost:5432` in the environment they were written
> in, and `pg_integration` tests cannot run without one. Before this task can be ticked off, someone must
> start a local Postgres and run:
>
> ```sh
> cargo nextest run -p iota-indexer --features pg_integration --test account_key_links_tests
> ```
>
> Treat every assertion in this file as unproven until that passes. The same caveat applies to the whole
> `pg_integration` gate in the rollout checklist below.

* \[ \] **Step 1: Write the tests**

Use the existing harness (`Simulacrum` + `common::indexer_wait_for_checkpoint`). Mirror the genesis overrides in `crates/iota-e2e-tests/tests/claim_account_tests.rs` — `enable_claim_account_transaction` and `enable_builtin_move_authenticators` both on.

```rust
// 1. A real ClaimAccount transaction (SmartAccountBuildKind::Mutable)
//    → one ACTIVE account_key_links row, source = claim, key_id =
//      key_id_from_prefixed_bytes(sender pk), account_id = sender address;
//      plus one claimed_accounts row with immutable = false.
//    Expect the transaction to emit both PublicKeyAttached and
//    SmartAccountClaimed; the collapse must yield ONE link row, not two.
// 2. The same with SmartAccountBuildKind::Immutable → immutable = true.
// 3. builtin_auth_builder_v1 + build_v1 → ACTIVE link with source = attach and
//    NO claimed_accounts row. This is the dust-gifting shape: assert it is
//    distinguishable from the claims above.
// 4. A second claim of the same address (double-claiming is not prevented yet)
//    → still one row in each table, carrying the later transaction sequence.
// 5. Rotation and detach are NOT executable in Simulacrum: both need the
//    account itself as transaction sender, hence a MoveAuthenticator, which
//    Simulacrum does not support. Drive these from the exact event payloads
//    and assert the old key_id row is UNLINKED and the new one ACTIVE with
//    source = rotate.
// 6. Determinism: ingest the same checkpoints into a second database and
//    compare both row sets ordered by primary key.
```

* \[ \] **Step 2: Run to verify they fail** (needs a local Postgres)

```sh
cargo nextest run -p iota-indexer --features pg_integration account_key_links
```

Expected: FAIL before Tasks 4–7 are complete; this task is written last but the assertions are the acceptance criteria for all of them.

* \[ \] **Step 3: Run to verify they pass**

```sh
cargo nextest run -p iota-indexer --features pg_integration account_key_links
```

Expected: PASS.

* \[ \] **Step 4: Confirm nothing else regressed**

```sh
cargo simtest -p iota-e2e-tests --test claim_account_tests
```

Expected: PASS — the existing claim tests must stay green.

* \[ \] **Step 5: Commit**

```sh
git add crates/iota-indexer/tests .config/nextest.toml
git commit -m "test(indexer): account link tables across claim, attach and rotation"
```

---

### 2.9. Task 9: Operator & ecosystem documentation

**Files:**

* Modify: `docs/content/operator/extended-data-services/iota-indexer.mdx` — add an "Account discoverability index" section.

* \[ \] **Step 1: Document, in that section:**

    * The four-event schema and fold semantics (copy the tables from §1.3) — this is the public contract third-party indexers implement, and the reason both modules matter.
    * The `key_id` formula, and that it is an index identity with **no protocol role** — authentication verifies against the derived address.
    * Ordering rule `(checkpoint, tx, event)` and the determinism guarantee; the additive-only evolution policy after first public-network activation.
    * Claimed versus attached, and what the distinction means for dust filtering: a gifted account never appears in `claimed_accounts`, and every link on a claimed account is self-authorized.
    * Both tables are exempt from pruning; event-table retention only affects _bootstrapping_ new indexers, which needs an unpruned archive fullnode or an imported table snapshot.
    * Self-hosting: a standard `iota-indexer` deployment — the reverse index needs no extra configuration.
    * Wallet integration: the discovery protocol from §1.5, the optional per-result on-chain verification, and the `iota_subscribeEvent` live path with its two module filters.
    * The MultiSig gap: a MultiSig `key_id` hashes the committee, so a seed-only wallet cannot compute the lookup key.

* \[ \] **Step 2: Commit**

```sh
git add docs/content
git commit -m "docs(indexer): account discoverability index operator and integrator guide"
```

---

## Rollout checklist

1. **Finish Task 1 and Task 2** as the first PR into `vm-lang/10722-fix-after-rebase`: the missing Move tests plus the framework snapshot refresh for `SmartAccountClaimed` and (if it stays) `key_id`. The branch currently carries two framework commits whose snapshots were never regenerated, so this is a prerequisite for anything downstream, not bookkeeping. Resolve **Decisions needed** #1–#3 before this PR lands — the event schema freezes at testnet activation.
2. Land Tasks 3–6 (Rust identity, mirrors, tables, ingestion) as the second PR. Task 7 third. Tasks 8–9 with or after.
3. Before each PR: `cargo ci-clippy && cargo +nightly fmt && dprint fmt`; `IOTA_SKIP_SIMTESTS=1 cargo nextest run -p <touched crates>`; the `pg_integration` suite; and `cargo simtest -p iota-e2e-tests --test claim_account_tests` must stay green. **The `pg_integration` suite has not been run** — see the note in Task 8; it needs a local PostgreSQL instance.
4. Deploy the updated indexer for the alpha devnet. From-genesis sync builds both tables with no migration or backfill step: the fold starts at the first emitted event.
5. **Ordering constraint:** an indexer must not be pointed at a network whose framework predates `ea2da5d222`, or claims on that network are indexed as plain attachments with no `claimed_accounts` row.
6. Event schema freezes at first Testnet activation — from then on, additive-only.

Nothing here blocks on `iota-rust-sdk`.

## Decisions needed

1. **Keep `key_id` in Move, or remove it?** `public_key::key_id` shipped in `9b9746e4f4` and has no consumer: nothing on chain calls it, no event carries a `key_id` field, there is no Move test for it, and it is absent from `published_api.txt`. If it stays, it needs per-scheme test vectors, a snapshot refresh, and — once Task 3 lands — a cross-language parity test guarding a value with no on-chain consumer. If it goes, `key_id` is defined in Rust only, as `MovePublicKey::key_id()`, and re-adding it to Move later is purely additive. **Open.** The argument for keeping it is that it documents the canonical formula where the `PublicKey` type itself lives and makes an on-chain registry keyed by `key_id` cheap to reach for later; the argument against is dead code plus a parity test that guards nothing anyone reads.

2. **What should `SmartAccountClaimed` — and by extension the other key-carrying events — emit: the full `public_key`, `key_id` alone, or both?** The event currently carries the full `PublicKey`, and this reopens a call the V3 draft already made in favour of exactly that, at a time when `key_id` did not exist on chain. Now that it does, the question is live again. **Open.** The considerations, none of which is decisive on its own:

    * **Redundancy.** `key_id` is a pure hash of `public_key` — `blake2b256(flag ‖ raw_bytes)`. Emitting both means every event carries a value any consumer can compute from the other field in the same event. Emitting `key_id` alone makes the event no longer self-sufficient for anything but the index: a consumer that wants the key material has to go to the transaction or the account object for it.
    * **Payload and gas cost.** `raw_bytes` is fixed-length for four of the five schemes (32 or 33 bytes) but **variable-length for MultiSig**, where it is the BCS-encoded committee and grows with the number of members — up to ten, each contributing a key plus a weight. A `key_id` is always 32 bytes. For MultiSig claims the difference is real; for the others it is one byte of flag plus a length prefix.
    * **Consistency.** The three shipped `PublicKey*` events all carry a full `PublicKey`. Making `SmartAccountClaimed` carry `key_id` alone would make it the odd one out, and the fold would need two code paths for what is conceptually one fact. Changing all four to `key_id` is possible only before testnet activation, and would be a much larger change.
    * **Privacy.** Hashing does not meaningfully hide key material here: the claiming transaction is signed by that very key, so its public key is already on the wire in the transaction's own signature, and `PublicKeyAttached` carries it in the same transaction regardless. `key_id` only obscures the key against someone reading the claim event in isolation — which is not a realistic adversary model, since the events and the transactions are published together. The shipped privacy regime is R0 in either case (§1.4).

    A decision to carry `key_id` in addition to `public_key` would also make the Move `key_id` function load-bearing, which settles #1 in favour of keeping it. The three are entangled: decide #2 first.

3. **Should `smart_account` emit its own attach / detach / rotate events?** `SmartAccountClaimed` gave the claim path an event of its own; the same question applies to the other three key operations, which today emit only the shared `builtin_authenticator_functions` events. **Open.**

    The ambiguity is real. `attach_public_key`, `detach_public_key` and `rotate_public_key` are `public fun` over a bare `&mut UID` (`builtin_authenticator_functions.move:265`, `:283`, `:304`), and `attach_public_key` asserts only that no key is already attached — nothing requires the `UID` to carry an `AuthenticatorFunctionRefV1`, or to be an account at all. Any package holding a `&mut UID` can therefore emit a `PublicKeyAttached` for an arbitrary object. That the framework's only caller today is `smart_account` — at five sites, `smart_account.move:114`, `:235`, `:263`, `:312`, `:388` — is a property of the current framework, not of the event contract.

    **For adding them.** It would make "is this a framework `SmartAccount`?" a read rather than an inference, which is the same argument that justifies `SmartAccountClaimed` (§1.3), and it would bound the set of objects that can enter the index at all rather than admitting any `UID` whose owner chose to attach a key.

    **Against:**

    * **Pure duplication on the hot path.** Every attach, detach and rotate on a `SmartAccount` would emit two events carrying the same `account_id` and the same key material, doubling payload and gas — and for MultiSig the key is variable-length, the same cost point as #2. `SmartAccountClaimed` has no such duplicate: it adds facts (`immutable`, and that a claim happened at all) that nothing else on the wire carries.
    * **The fold gains branches with no consumer.** It would have to consume one of each pair and ignore the other, or dedupe them, to guard a distinction nothing currently asks for — the same trap as shipping `key_id` in Move ahead of a consumer (#1).
    * **It does not fix the case it resembles.** Dust gifting goes through `builtin_auth_builder_v1`, which produces a genuine `SmartAccount`, so a `smart_account`-scoped attach event would not filter it. `claimed_accounts` is what does (§1.3).
    * **Deferring is cheap.** The freeze is additive-only for *new event types*, so three more can be added after testnet activation.

    Leaning no for v1, with Task 9 documenting that `PublicKeyAttached.account_id` is not guaranteed to be an account. The reason to settle it now rather than later is that the cheaper alternative shape — a discriminating field on the three existing events — closes at the freeze, while adding whole event types does not.

Carried over from V3 and still open:

4. **Accounts with custom authenticators** (`builder_v1`, no key attached) never enter either table — no standard key binding, nothing to index. Confirm that is intended.
5. **MultiSig: index committee members?** Without it, MultiSig accounts are undiscoverable from a seed (§1.4). With it, committee membership becomes queryable rather than merely replayable. Recommend deferring to a follow-up with its own privacy review, and documenting the gap now.
6. **Double-claim prevention** is being fixed separately. Confirm the fix does not change the emitted event sequence; the `claimed_accounts` upsert is indifferent either way, but the assumption should be checked rather than assumed.

## Deferred (explicitly out of scope)

* Indexer WebSocket/SSE push feed — the fullnode `iota_subscribeEvent` path covers live wallets in v1.
* Salted key ids — requires a protocol change to carry user-supplied salt in the claim payload.
* Consuming the `iota::account` lifecycle events (`MutableAccountCreated`, `ImmutableAccountCreated`, `AuthenticatorFunctionRefV1Rotated`) — they carry no key material, and their generic `StructTag`s would each need a type-parameter-tolerant matcher.
* Indexing MultiSig committee members against the account (**Decisions needed** #5).
* GraphQL/gRPC surfaces for the two tables — JSON-RPC first; add on demand.
* Checkpoint-anchored table snapshots for fast third-party bootstrap.
