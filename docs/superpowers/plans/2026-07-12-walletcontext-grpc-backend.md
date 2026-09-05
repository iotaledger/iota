# WalletContext gRPC Backend Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `WalletContext`'s chain-touching methods (`get_object_ref`, `get_object_owner`,
`try_get_object_owner`, `get_reference_gas_price`, `gas_objects` and its family,
`execute_transaction_may_fail`/`_must_succeed`) run over the node's gRPC API by default, returning
`iota-rust-sdk`-native types, while falling back to the existing JSON-RPC path when an environment
has no `grpc` URL configured — with every affected consumer in this repo updated to compile against
the new return types.

**Architecture:** A `WalletBackend` enum on `WalletContext` (default `Grpc`) resolves per-call to
`Grpc` or `JsonRpc` (`resolve_backend()`), falling back to `JsonRpc` when the active environment has
no `grpc` URL. The gRPC path calls `iota_grpc_client::Client` directly and returns SDK-native types
(`iota_sdk_types::Object`, `iota_grpc_types::v1::transaction::ExecutedTransaction`). The JSON-RPC
path is unchanged except it now converts its `iota_json_rpc_types` results into the same SDK-native
return types via new `TryFrom` impls added to `iota-json-rpc-types` (which already depends on
`iota-sdk-types`, and will gain a dependency on `iota-grpc-types` for the proto-typed conversions —
confirmed cycle-free: `iota-grpc-types` is a foreign crate from the `iota-rust-sdk` repo with no path
back into this monorepo). Every consumer of the two changed method families (`gas_objects`,
`execute_transaction_may_fail`/`_must_succeed`) is updated to the new return types.

**Tech Stack:** Rust, `iota_grpc_client::Client` / `iota_grpc_types` (pinned at rev `aee56356` in the
root `Cargo.toml`), `iota_sdk_types`, `iota_json_rpc_types`, `#[sim_test]` (`iota_macros`), `cargo
nextest`, `cargo simtest`.

## Global Constraints

- No lint suppressions (`#[allow(dead_code)]`, `#[allow(unused)]`, etc.) — fix the underlying issue
  instead.
- Never skip or disable a test; all tests must stay enabled and pass.
- Run `cargo +nightly fmt` after every task's code changes, before committing.
- Comments explain non-obvious _why_, never _what_; no conversational or change-history text in
  comments (no PR/issue numbers, no "added for X").
- Every commit ends with the line:
  `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`
- Doc comments describe what a caller needs to know to use the item correctly, not internals.

---

## Task 1: `WalletBackend` enum + backend resolution

**Files:**

- Modify: `crates/iota-sdk/src/wallet_context.rs`

**Interfaces:**

- Produces: `pub enum WalletBackend { Grpc, JsonRpc }` (with `Default` = `Grpc`), used by every
  later task's dispatch code.
- Produces: `WalletContext::with_jsonrpc_backend(self) -> Self` (builder), used by Task 8's parity
  tests and any caller that wants to force JSON-RPC.
- Produces: `WalletContext::resolve_backend(&self) -> Result<WalletBackend, anyhow::Error>` (private
  to the module), called by every later task's dispatch code within this same file.
- Consumes: `WalletContext::active_env(&self) -> Result<&IotaEnv, anyhow::Error>` (already exists,
  line 158) and `IotaEnv::grpc(&self) -> &Option<String>` (already exists via `getset`,
  `crates/iota-sdk/src/iota_client_config.rs:137-152`).

- [x] **Step 1: Write the failing unit tests**

Add at the end of `crates/iota-sdk/src/wallet_context.rs` (after the closing `}` of `impl
WalletContext`):

```rust
#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use iota_config::Config;
    use iota_keys::keystore::InMemKeystore;

    use super::*;
    use crate::iota_client_config::IotaClientConfig;

    /// Builds a `WalletContext` with a single active env, backed by an
    /// in-memory keystore and a config that is never written to disk (tests
    /// only exercise in-memory dispatch logic, never `PersistedConfig::save`).
    fn wallet_context_with_env(env: IotaEnv) -> WalletContext {
        let alias = env.alias().clone();
        let config = IotaClientConfig::new(Keystore::InMem(InMemKeystore::default()))
            .with_envs([env])
            .with_active_env(alias)
            .persisted(&PathBuf::from("unused-test-config.yaml"));
        WalletContext {
            config,
            request_timeout: None,
            client: Default::default(),
            grpc_client: Default::default(),
            max_concurrent_requests: None,
            env_override: None,
            backend: WalletBackend::default(),
        }
    }

    #[test]
    fn resolve_backend_defaults_to_grpc_when_env_has_grpc_url() {
        let ctx = wallet_context_with_env(
            IotaEnv::new("test", "https://rpc.example").with_grpc(Some(
                "https://grpc.example".to_string(),
            )),
        );
        assert_eq!(ctx.resolve_backend().unwrap(), WalletBackend::Grpc);
    }

    #[test]
    fn resolve_backend_falls_back_to_json_rpc_when_env_has_no_grpc_url() {
        let ctx = wallet_context_with_env(IotaEnv::new("test", "https://rpc.example"));
        assert_eq!(ctx.resolve_backend().unwrap(), WalletBackend::JsonRpc);
    }

    #[test]
    fn resolve_backend_honors_explicit_json_rpc_override_even_with_grpc_url() {
        let ctx = wallet_context_with_env(
            IotaEnv::new("test", "https://rpc.example").with_grpc(Some(
                "https://grpc.example".to_string(),
            )),
        )
        .with_jsonrpc_backend();
        assert_eq!(ctx.resolve_backend().unwrap(), WalletBackend::JsonRpc);
    }
}
```

- [x] **Step 2: Run the tests to verify they fail**

Run: `IOTA_SKIP_SIMTESTS=1 cargo nextest run -p iota-sdk wallet_context::tests --lib`
Expected: FAIL to compile — `WalletBackend`, `backend` field, `with_jsonrpc_backend`, and
`resolve_backend` do not exist yet.

- [x] **Step 3: Implement `WalletBackend`, the `backend` field, and `resolve_backend`**

Add the enum just above `WalletContext`, after the doc comment block at line 30-32:

```rust
/// Which transport `WalletContext` uses for chain-touching operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WalletBackend {
    /// Use the node's gRPC API. The default: falls back to `JsonRpc` when the
    /// active environment has no `grpc` URL configured.
    #[default]
    Grpc,
    /// Use the node's JSON-RPC API unconditionally.
    JsonRpc,
}
```

Add the field to the `WalletContext` struct (after `env_override`):

```rust
pub struct WalletContext {
    config: PersistedConfig<IotaClientConfig>,
    request_timeout: Option<std::time::Duration>,
    client: Arc<RwLock<Option<IotaClient>>>,
    grpc_client: Arc<RwLock<Option<iota_grpc_client::Client>>>,
    max_concurrent_requests: Option<u64>,
    env_override: Option<String>,
    backend: WalletBackend,
}
```

Initialize it in `WalletContext::new` (in the `Self { ... }` literal):

```rust
let context = Self {
    config,
    request_timeout: None,
    client: Default::default(),
    grpc_client: Default::default(),
    max_concurrent_requests: None,
    env_override: None,
    backend: WalletBackend::default(),
};
```

Add the builder and the resolver right after `with_env_override` (around line 97):

```rust
    /// Force `WalletContext` to use the JSON-RPC backend, even for
    /// environments that have a `grpc` URL configured.
    pub fn with_jsonrpc_backend(mut self) -> Self {
        self.backend = WalletBackend::JsonRpc;
        self
    }

    /// Resolve which backend a chain-touching method should use for the
    /// active environment: `JsonRpc` if `with_jsonrpc_backend()` was called,
    /// or if the default `Grpc` backend has no `grpc` URL configured for the
    /// active environment; `Grpc` otherwise.
    fn resolve_backend(&self) -> Result<WalletBackend, anyhow::Error> {
        Ok(match self.backend {
            WalletBackend::JsonRpc => WalletBackend::JsonRpc,
            WalletBackend::Grpc if self.active_env()?.grpc().is_none() => WalletBackend::JsonRpc,
            WalletBackend::Grpc => WalletBackend::Grpc,
        })
    }
```

- [x] **Step 4: Run the tests to verify they pass**

Run: `IOTA_SKIP_SIMTESTS=1 cargo nextest run -p iota-sdk wallet_context::tests --lib`
Expected: PASS (3 tests).

- [x] **Step 5: Format and commit**

Run: `cargo +nightly fmt -p iota-sdk`

```bash
git add crates/iota-sdk/src/wallet_context.rs
git commit -m "$(cat <<'EOF'
feat(iota-sdk): add a WalletBackend toggle to WalletContext

Defaults to gRPC, with an opt-in JSON-RPC override and a fallback to
JSON-RPC for environments that have no `grpc` URL, so existing
`client.yaml` files keep working unchanged.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 2: `json_rpc_types → SDK` conversions in `iota-json-rpc-types`

**Files:**

- Modify: `crates/iota-json-rpc-types/Cargo.toml`
- Create: `crates/iota-json-rpc-types/src/sdk_conversions.rs`
- Modify: `crates/iota-json-rpc-types/src/lib.rs`

**Interfaces:**

- Consumes: `iota_sdk_types::{Object, ObjectData, MoveObjectType, MoveStruct, Owner, StructTag,
  TransactionDigest, TransactionEffects}` (all already re-exported 1:1 by `iota_types`, confirmed by
  reading `crates/iota-types/src/base_types.rs:15` (`SequenceNumber = iota_sdk_types::Version`),
  `crates/iota-types/src/digests.rs:9-13` (digest types), `crates/iota-types/src/transaction.rs:28-31`
  (`TransactionData = iota_sdk_types::Transaction`), and `crates/iota-types/src/effects/mod.rs:11-17`
  (`TransactionEffects`/`TransactionEvents` are direct re-exports) — so `IotaObjectData`'s and
  `IotaTransactionBlockResponse`'s scalar/identity fields are _already_ the exact SDK types, no
  per-field conversion needed).
- Consumes: `iota_grpc_types::v1::transaction::ExecutedTransaction` and its `with_*` builder methods
  (`crates/iota-sdk-grpc-types/src/proto/generated/iota.grpc.v1.transaction.accessors.rs`, pinned
  rev `aee56356`), `iota_grpc_types::v1::bcs::BcsData::serialize<T: Serialize>(&T) ->
  Result<BcsData, bcs::Error>`.
- Produces: `impl TryFrom<&IotaObjectData> for iota_sdk_types::Object` (error type
  `SdkConversionError`).
- Produces: `impl TryFrom<&IotaTransactionBlockResponse> for
  iota_grpc_types::v1::transaction::ExecutedTransaction` (error type `SdkConversionError`) — used by
  Task 5's JSON-RPC fallback for `execute_transaction_may_fail`/`_must_succeed`.
- Produces: `impl TryFrom<&iota_grpc_types::v1::transaction::ExecutedTransaction> for
  IotaTransactionBlockResponse` (error type `SdkConversionError`) — used by Task 6's CLI display
  path.
- Produces: `pub struct SdkConversionError(pub String)` implementing `std::error::Error` +
  `Display`, following the existing pattern of
  `crates/iota-types/src/iota_sdk_types_conversions.rs:25-34`.

**Known, documented gaps in this conversion** (write these as doc comments on the `TryFrom` impls,
not as code TODOs):

- `iota_json_rpc_types::ObjectChange::Transferred` has no equivalent variant in the gRPC proto's
  `ObjectChange` (`Published`/`Mutated`/`Deleted`/`Wrapped`/`Unwrapped`/`Created` only — confirmed by
  reading `crates/iota-sdk-grpc-types/src/proto/generated/iota.grpc.v1.transaction.rs:252-274` at the
  pinned rev). `TryFrom<&IotaTransactionBlockResponse> for ExecutedTransaction` returns
  `Err(SdkConversionError)` for a `Transferred` entry. No consumer in this repo reads
  `.object_changes()` for a `Transferred` change through a `WalletContext`-returned response (grepped
  across all call sites enumerated in Tasks 6 and 7), so this is a real but currently-unexercised
  limitation of the JSON-RPC fallback path, not a blocker.
- The JSON-RPC fallback conversion does not populate `ExecutedTransaction.effects.digest` or
  `.events` at all (`events` field left `None`): no consumer of `execute_transaction_may_fail`/
  `_must_succeed` in this repo reads an events digest or the events themselves off the return value
  (confirmed by grepping every call site enumerated in Tasks 6 and 7). Only
  `.transaction.digest`/`.transaction.bcs`/`.effects.bcs`/`.checkpoint`/`.timestamp`/
  `.object_changes`/`.balance_changes` are populated.

- [x] **Step 1: Write the failing unit tests**

Create `crates/iota-json-rpc-types/src/sdk_conversions.rs`:

```rust
// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Conversions from `iota_json_rpc_types` into `iota-rust-sdk`-native types.
//!
//! These live on the JSON-RPC side (not the gRPC side) because the gRPC
//! client already produces SDK-native types directly; this module only
//! exists to give `WalletContext`'s JSON-RPC fallback path the same return
//! types as its gRPC path.

use iota_grpc_types::v1::{
    object::Objects as ProtoObjects,
    transaction::{
        BalanceChange as ProtoBalanceChange, BalanceChanges as ProtoBalanceChanges,
        ExecutedTransaction, ObjectChange as ProtoObjectChange,
        ObjectChangeCreated as ProtoObjectChangeCreated,
        ObjectChangeDeleted as ProtoObjectChangeDeleted,
        ObjectChangeMutated as ProtoObjectChangeMutated,
        ObjectChangePublished as ProtoObjectChangePublished,
        ObjectChangeUnwrapped as ProtoObjectChangeUnwrapped,
        ObjectChangeWrapped as ProtoObjectChangeWrapped, ObjectChanges as ProtoObjectChanges,
        Transaction as ProtoTransaction, TransactionEffects as ProtoTransactionEffects,
    },
    types::Digest as ProtoDigest,
};
use iota_sdk_types::{MoveObjectType, MoveStruct, Object, ObjectData};

use crate::{
    BalanceChange, IotaObjectData, IotaTransactionBlockResponse, ObjectChange, iota_object::IotaData,
};

/// Error converting between `iota_json_rpc_types` and `iota-rust-sdk`-native
/// types.
#[derive(Debug)]
pub struct SdkConversionError(pub String);

impl std::fmt::Display for SdkConversionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for SdkConversionError {}

impl From<bcs::Error> for SdkConversionError {
    fn from(value: bcs::Error) -> Self {
        Self(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use iota_sdk_types::{Address, Owner, StructTag, TransactionDigest};

    use super::*;
    use crate::{IotaObjectData, iota_object::IotaRawData};

    fn sample_iota_object_data() -> IotaObjectData {
        let object_id = iota_sdk_types::ObjectId::random();
        let mut contents = object_id.as_bytes().to_vec();
        contents.extend_from_slice(&[0u8; 8]); // opaque Move-struct payload
        IotaObjectData {
            object_id,
            version: 1.into(),
            digest: iota_sdk_types::ObjectDigest::random(),
            type_: None,
            owner: Some(Owner::Address(Address::TWO)),
            previous_transaction: Some(TransactionDigest::random()),
            storage_rebate: Some(0),
            display: None,
            content: None,
            bcs: Some(iota_object::IotaRawData::MoveObject(
                iota_object::IotaRawMoveObject {
                    type_: StructTag::new_gas_coin(),
                    version: 1.into(),
                    bcs_bytes: contents,
                },
            )),
        }
    }

    #[test]
    fn object_conversion_round_trips_owner_and_previous_transaction() {
        let data = sample_iota_object_data();
        let object = Object::try_from(&data).unwrap();
        assert_eq!(object.owner(), &data.owner.unwrap());
        assert_eq!(object.previous_transaction(), data.previous_transaction.unwrap());
        assert_eq!(object.storage_rebate(), data.storage_rebate.unwrap());
    }

    #[test]
    fn object_conversion_requires_bcs() {
        let mut data = sample_iota_object_data();
        data.bcs = None;
        assert!(Object::try_from(&data).is_err());
    }
}
```

- [x] **Step 2: Run the test to verify it fails**

Run: `cargo build -p iota-json-rpc-types 2>&1 | head -50`
Expected: FAIL — `iota_grpc_types` is not a dependency yet, `TryFrom<&IotaObjectData> for Object`
does not exist, `IotaRawData`/`IotaRawMoveObject`/`IotaData` are not re-exported at the paths used
above.

- [x] **Step 3a: Add the `iota-grpc-types` dependency**

In `crates/iota-json-rpc-types/Cargo.toml`, add to `[dependencies]` (alongside `iota-sdk-types`):

```toml
iota-grpc-types.workspace = true
```

- [x] **Step 3b: Register the new module**

In `crates/iota-json-rpc-types/src/lib.rs`, add near the other `mod`/`pub use` pairs:

```rust
pub use sdk_conversions::*;
```

and in the `mod` block:

```rust
mod sdk_conversions;
```

- [x] **Step 3c: Implement `TryFrom<&IotaObjectData> for iota_sdk_types::Object`**

Append to `crates/iota-json-rpc-types/src/sdk_conversions.rs` (before the `#[cfg(test)]` module):

```rust
impl TryFrom<&IotaObjectData> for Object {
    type Error = SdkConversionError;

    fn try_from(value: &IotaObjectData) -> Result<Self, Self::Error> {
        let owner = value
            .owner
            .ok_or_else(|| SdkConversionError("missing owner (request with_owner())".into()))?;
        let previous_transaction = value.previous_transaction.ok_or_else(|| {
            SdkConversionError(
                "missing previous_transaction (request with_previous_transaction())".into(),
            )
        })?;
        let storage_rebate = value.storage_rebate.ok_or_else(|| {
            SdkConversionError("missing storage_rebate (request with_storage_rebate())".into())
        })?;
        let raw = value
            .bcs
            .as_ref()
            .ok_or_else(|| SdkConversionError("missing bcs (request with_bcs())".into()))?;

        let data = match raw {
            crate::iota_object::IotaRawData::MoveObject(raw_move_object) => {
                ObjectData::Struct(
                    MoveStruct::new(
                        MoveObjectType::new(raw_move_object.type_.clone()),
                        raw_move_object.version.into(),
                        raw_move_object.bcs_bytes.clone(),
                    )
                    .map_err(|e| SdkConversionError(e.to_string()))?,
                )
            }
            crate::iota_object::IotaRawData::Package(_) => {
                return Err(SdkConversionError(
                    "converting a package IotaObjectData to iota_sdk_types::Object is not \
                     supported"
                        .into(),
                ));
            }
        };

        Ok(Object {
            data,
            owner,
            previous_transaction,
            storage_rebate,
        })
    }
}
```

- [x] **Step 4: Run the tests to verify they pass**

Run: `IOTA_SKIP_SIMTESTS=1 cargo nextest run -p iota-json-rpc-types sdk_conversions --lib`
Expected: PASS (2 tests: `object_conversion_round_trips_owner_and_previous_transaction`,
`object_conversion_requires_bcs`).

- [x] **Step 5: Write the failing tests for the `ObjectChange`/`BalanceChange` mapping**

Append to the `#[cfg(test)] mod tests` block in `sdk_conversions.rs`:

```rust
    #[test]
    fn object_change_created_maps_to_proto_created() {
        let sender = Address::from(iota_sdk_types::ObjectId::random());
        let owner = Owner::Address(sender);
        let object_type = StructTag::new_gas_coin();
        let object_id = iota_sdk_types::ObjectId::random();
        let digest = iota_sdk_types::ObjectDigest::random();
        let change = ObjectChange::Created {
            sender,
            owner,
            object_type: object_type.clone(),
            object_id,
            version: 1.into(),
            digest,
        };

        let proto = ProtoObjectChange::try_from(&change).unwrap();
        let created = proto.created().unwrap();
        assert_eq!(created.sender().unwrap(), sender);
        assert_eq!(created.owner().unwrap(), owner);
        assert_eq!(created.object_id().unwrap(), object_id);
        assert_eq!(created.digest().unwrap(), digest);
    }

    #[test]
    fn object_change_transferred_is_unrepresentable() {
        let sender = Address::from(iota_sdk_types::ObjectId::random());
        let change = ObjectChange::Transferred {
            sender,
            recipient: Owner::Address(sender),
            object_type: StructTag::new_gas_coin(),
            object_id: iota_sdk_types::ObjectId::random(),
            version: 1.into(),
            digest: iota_sdk_types::ObjectDigest::random(),
        };

        assert!(ProtoObjectChange::try_from(&change).is_err());
    }

    #[test]
    fn balance_change_round_trips_negative_amount() {
        let owner = Owner::Address(Address::from(iota_sdk_types::ObjectId::random()));
        let change = BalanceChange {
            owner,
            coin_type: iota_sdk_types::TypeTag::Struct(Box::new(StructTag::new_gas())),
            amount: -42,
        };

        let proto = ProtoBalanceChange::try_from(&change).unwrap();
        assert_eq!(proto.owner().unwrap(), owner);
        assert_eq!(proto.amount_i128().unwrap(), -42);
    }
```

- [x] **Step 6: Run the tests to verify they fail**

Run: `cargo build -p iota-json-rpc-types 2>&1 | head -50`
Expected: FAIL — `TryFrom<&ObjectChange> for ProtoObjectChange` and `TryFrom<&BalanceChange> for
ProtoBalanceChange` do not exist yet.

- [x] **Step 7: Implement the `ObjectChange`/`BalanceChange` → proto mapping**

Append to `sdk_conversions.rs` (before the test module):

```rust
impl TryFrom<&BalanceChange> for ProtoBalanceChange {
    type Error = SdkConversionError;

    fn try_from(value: &BalanceChange) -> Result<Self, Self::Error> {
        Ok(ProtoBalanceChange::default()
            .with_owner(value.owner)
            .with_coin_type(&value.coin_type)
            .with_amount(prost::bytes::Bytes::copy_from_slice(
                &value.amount.to_be_bytes(),
            )))
    }
}

impl TryFrom<&ObjectChange> for ProtoObjectChange {
    type Error = SdkConversionError;

    fn try_from(value: &ObjectChange) -> Result<Self, Self::Error> {
        Ok(match value {
            ObjectChange::Published {
                package_id,
                version,
                digest,
                modules,
            } => ProtoObjectChange::default().with_published(
                ProtoObjectChangePublished::default()
                    .with_package_id(*package_id)
                    .with_version((*version).into())
                    .with_digest(*digest)
                    .with_modules(modules.clone()),
            ),
            ObjectChange::Mutated {
                sender,
                owner,
                object_type,
                object_id,
                version,
                previous_version,
                digest,
            } => ProtoObjectChange::default().with_mutated(
                ProtoObjectChangeMutated::default()
                    .with_sender(*sender)
                    .with_owner(*owner)
                    .with_object_type(&object_type.clone().into())
                    .with_object_id(*object_id)
                    .with_version((*version).into())
                    .with_previous_version((*previous_version).into())
                    .with_digest(*digest),
            ),
            ObjectChange::Deleted {
                sender,
                object_type,
                object_id,
                version,
            } => ProtoObjectChange::default().with_deleted(
                ProtoObjectChangeDeleted::default()
                    .with_sender(*sender)
                    .with_object_type(&object_type.clone().into())
                    .with_object_id(*object_id)
                    .with_version((*version).into()),
            ),
            ObjectChange::Wrapped {
                sender,
                object_type,
                object_id,
                version,
            } => ProtoObjectChange::default().with_wrapped(
                ProtoObjectChangeWrapped::default()
                    .with_sender(*sender)
                    .with_object_type(&object_type.clone().into())
                    .with_object_id(*object_id)
                    .with_version((*version).into()),
            ),
            ObjectChange::Unwrapped {
                sender,
                owner,
                object_type,
                object_id,
                version,
                digest,
            } => ProtoObjectChange::default().with_unwrapped(
                ProtoObjectChangeUnwrapped::default()
                    .with_sender(*sender)
                    .with_owner(*owner)
                    .with_object_type(&object_type.clone().into())
                    .with_object_id(*object_id)
                    .with_version((*version).into())
                    .with_digest(*digest),
            ),
            ObjectChange::Created {
                sender,
                owner,
                object_type,
                object_id,
                version,
                digest,
            } => ProtoObjectChange::default().with_created(
                ProtoObjectChangeCreated::default()
                    .with_sender(*sender)
                    .with_owner(*owner)
                    .with_object_type(&object_type.clone().into())
                    .with_object_id(*object_id)
                    .with_version((*version).into())
                    .with_digest(*digest),
            ),
            // The gRPC schema has no distinct "a pure ownership change" kind —
            // `Mutated` already carries the new `owner`. A `Transferred` entry
            // lacks the `previous_version` that `Mutated` requires, so it
            // cannot be losslessly represented; see the module-level doc.
            ObjectChange::Transferred { .. } => {
                return Err(SdkConversionError(
                    "ObjectChange::Transferred has no gRPC proto equivalent".into(),
                ));
            }
        })
    }
}
```

Note: `object_type: StructTag` needs `.into()` to `TypeTag` before going into
`.with_object_type()` (which takes `impl Into<proto::types::TypeTag>`, and the proto `From` impl is
defined for `&iota_sdk_types::TypeTag`, confirmed at
`crates/iota-sdk-grpc-types/src/proto/iota/grpc/v1/types.rs:237` in the pinned rev) — `StructTag`
converts to `TypeTag::Struct(Box::new(tag))` via `.into()` (mirroring the existing
`type_tag_core_to_sdk` pattern in `crates/iota-types/src/iota_sdk_types_conversions.rs:305-307`).

- [x] **Step 8: Run the tests to verify they pass**

Run: `IOTA_SKIP_SIMTESTS=1 cargo nextest run -p iota-json-rpc-types sdk_conversions --lib`
Expected: PASS (5 tests total).

- [x] **Step 9: Write the failing test for `IotaTransactionBlockResponse → ExecutedTransaction`**

Append to the test module:

```rust
    fn sample_signed_transaction() -> iota_sdk_types::SignedTransaction {
        // Reuses the existing core-to-SDK conversion rather than hand-rolling
        // a `Transaction`; a system transaction needs no signer/gas payment.
        let data = iota_types::transaction::TransactionData::new_system_transaction(
            iota_sdk_types::TransactionKind::ChangeEpoch(Box::new(
                iota_sdk_types::ChangeEpoch {
                    epoch: 1,
                    protocol_version: 1,
                    storage_charge: 0,
                    computation_charge: 0,
                    storage_rebate: 0,
                    non_refundable_storage_fee: 0,
                    epoch_start_timestamp_ms: 0,
                    system_packages: vec![],
                },
            )),
        );
        iota_sdk_types::SignedTransaction {
            transaction: data,
            signatures: vec![],
        }
    }

    #[test]
    fn executed_transaction_conversion_round_trips_digest_and_object_changes() {
        let signed = sample_signed_transaction();
        let digest = signed.transaction.digest();
        let package_id = iota_sdk_types::ObjectId::random();
        let change = ObjectChange::Published {
            package_id,
            version: 1.into(),
            digest: iota_sdk_types::ObjectDigest::random(),
            modules: vec!["m".to_string()],
        };
        let response = IotaTransactionBlockResponse {
            digest,
            transaction: None,
            raw_transaction: bcs::to_bytes(&iota_types::transaction::SenderSignedData::new(
                signed.transaction.clone(),
                vec![],
            ))
            .unwrap(),
            effects: None,
            events: None,
            object_changes: Some(vec![change]),
            balance_changes: Some(vec![]),
            timestamp_ms: None,
            confirmed_local_execution: None,
            checkpoint: None,
            errors: vec![],
            raw_effects: vec![],
        };

        let executed = ExecutedTransaction::try_from(&response).unwrap();
        assert_eq!(executed.transaction().unwrap().digest().unwrap(), digest);
        let changes = executed.object_changes().unwrap();
        assert_eq!(changes.object_changes.len(), 1);
        assert_eq!(
            changes.object_changes[0].published().unwrap().package_id().unwrap(),
            package_id
        );
    }
```

- [x] **Step 10: Run the test to verify it fails**

Run: `cargo build -p iota-json-rpc-types --tests 2>&1 | head -50`
Expected: FAIL — `TryFrom<&IotaTransactionBlockResponse> for ExecutedTransaction` does not exist.

- [x] **Step 11: Implement `TryFrom<&IotaTransactionBlockResponse> for ExecutedTransaction`**

Append to `sdk_conversions.rs`:

```rust
impl TryFrom<&IotaTransactionBlockResponse> for ExecutedTransaction {
    type Error = SdkConversionError;

    fn try_from(value: &IotaTransactionBlockResponse) -> Result<Self, Self::Error> {
        let mut executed = ExecutedTransaction::default().with_transaction(
            ProtoTransaction::default()
                .with_digest(ProtoDigest::from(iota_sdk_types::Digest::from(value.digest)))
                .with_bcs(iota_grpc_types::v1::bcs::BcsData::serialize(
                    &value.digest_transaction_data(value)?,
                )?),
        );

        if !value.raw_effects.is_empty() {
            let effects: iota_sdk_types::TransactionEffects = bcs::from_bytes(&value.raw_effects)?;
            executed = executed.with_effects(
                ProtoTransactionEffects::default()
                    .with_bcs(iota_grpc_types::v1::bcs::BcsData::serialize(&effects)?),
            );
        }

        if let Some(checkpoint) = value.checkpoint {
            executed = executed.with_checkpoint(checkpoint);
        }
        if let Some(timestamp_ms) = value.timestamp_ms {
            executed = executed.with_timestamp(prost_types::Timestamp {
                seconds: (timestamp_ms / 1000) as i64,
                nanos: ((timestamp_ms % 1000) * 1_000_000) as i32,
            });
        }

        if let Some(object_changes) = &value.object_changes {
            executed = executed.with_object_changes(ProtoObjectChanges::default().with_object_changes(
                object_changes
                    .iter()
                    .map(ProtoObjectChange::try_from)
                    .collect::<Result<Vec<_>, _>>()?,
            ));
        }
        if let Some(balance_changes) = &value.balance_changes {
            executed = executed.with_balance_changes(ProtoBalanceChanges::default().with_balance_changes(
                balance_changes
                    .iter()
                    .map(ProtoBalanceChange::try_from)
                    .collect::<Result<Vec<_>, _>>()?,
            ));
        }

        Ok(executed)
    }
}
```

Note: `value.digest_transaction_data(value)` above is a placeholder name — replace with the actual
BCS-decode of `value.raw_transaction` into the SDK `SenderSignedData`'s inner
`TransactionData`/`Transaction`. Use the existing conversion instead of hand-rolling: deserialize
`bcs::from_bytes::<iota_types::transaction::SenderSignedData>(&value.raw_transaction)?`, then
`iota_sdk_types::SignedTransaction::try_from(sender_signed_data).map_err(|e|
SdkConversionError(e.to_string()))?.transaction`. Write the final helper as:

```rust
fn decode_transaction_bcs(
    raw_transaction: &[u8],
) -> Result<iota_sdk_types::Transaction, SdkConversionError> {
    let sender_signed_data: iota_types::transaction::SenderSignedData =
        bcs::from_bytes(raw_transaction)
            .map_err(|e| SdkConversionError(format!("decoding raw_transaction: {e}")))?;
    let signed: iota_sdk_types::SignedTransaction = sender_signed_data
        .try_into()
        .map_err(|e: iota_types::iota_sdk_types_conversions::SdkTypeConversionError| {
            SdkConversionError(e.to_string())
        })?;
    Ok(signed.transaction)
}
```

and replace the `.with_bcs(...)` call above with:

```rust
.with_bcs(iota_grpc_types::v1::bcs::BcsData::serialize(
    &decode_transaction_bcs(&value.raw_transaction)?,
)?),
```

removing the invalid `value.digest_transaction_data(value)` line entirely.

- [x] **Step 12: Run the test to verify it passes**

Run: `IOTA_SKIP_SIMTESTS=1 cargo nextest run -p iota-json-rpc-types sdk_conversions --lib`
Expected: PASS (6 tests total).

- [x] **Step 13: Write the failing test for the reverse `ExecutedTransaction → IotaTransactionBlockResponse` conversion**

Append to the test module:

```rust
    #[test]
    fn reverse_conversion_round_trips_digest() {
        let signed = sample_signed_transaction();
        let digest = signed.transaction.digest();
        let executed = ExecutedTransaction::default().with_transaction(
            ProtoTransaction::default()
                .with_digest(ProtoDigest::from(iota_sdk_types::Digest::from(digest))),
        );

        let response = IotaTransactionBlockResponse::try_from(&executed).unwrap();
        assert_eq!(response.digest, digest);
    }
```

- [x] **Step 14: Run the test to verify it fails**

Run: `cargo build -p iota-json-rpc-types --tests 2>&1 | head -50`
Expected: FAIL — `TryFrom<&ExecutedTransaction> for IotaTransactionBlockResponse` does not exist.

- [x] **Step 15: Implement `TryFrom<&ExecutedTransaction> for IotaTransactionBlockResponse`**

This direction only needs to support the CLI's display path (Task 6): digest, effects (for the
`Display` impl's effects section), events, object/balance changes. Append:

```rust
impl TryFrom<&ExecutedTransaction> for IotaTransactionBlockResponse {
    type Error = SdkConversionError;

    fn try_from(value: &ExecutedTransaction) -> Result<Self, Self::Error> {
        let digest = value
            .transaction()
            .map_err(|e| SdkConversionError(e.to_string()))?
            .digest()
            .map_err(|e| SdkConversionError(e.to_string()))?;

        let object_changes = value
            .object_changes()
            .ok()
            .map(|changes| {
                changes
                    .object_changes
                    .iter()
                    .map(proto_object_change_to_json_rpc)
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?;
        let balance_changes = value
            .balance_changes()
            .ok()
            .map(|changes| {
                changes
                    .balance_changes
                    .iter()
                    .map(proto_balance_change_to_json_rpc)
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?;

        Ok(IotaTransactionBlockResponse {
            digest,
            transaction: None,
            raw_transaction: vec![],
            effects: None,
            events: None,
            object_changes,
            balance_changes,
            timestamp_ms: None,
            confirmed_local_execution: None,
            checkpoint: value.checkpoint,
            errors: vec![],
            raw_effects: vec![],
        })
    }
}

fn proto_balance_change_to_json_rpc(
    value: &ProtoBalanceChange,
) -> Result<BalanceChange, SdkConversionError> {
    Ok(BalanceChange {
        owner: value.owner().map_err(|e| SdkConversionError(e.to_string()))?,
        coin_type: value
            .coin_type()
            .map_err(|e| SdkConversionError(e.to_string()))?,
        amount: value
            .amount_i128()
            .map_err(|e| SdkConversionError(e.to_string()))?,
    })
}

fn proto_object_change_to_json_rpc(
    value: &ProtoObjectChange,
) -> Result<ObjectChange, SdkConversionError> {
    let err = |e: iota_grpc_types::proto::TryFromProtoError| SdkConversionError(e.to_string());
    if let Ok(c) = value.published() {
        return Ok(ObjectChange::Published {
            package_id: c.package_id().map_err(err)?,
            version: c.version.ok_or_else(|| SdkConversionError("missing version".into()))?.into(),
            digest: c.digest().map_err(err)?,
            modules: c.modules.clone(),
        });
    }
    if let Ok(c) = value.mutated() {
        return Ok(ObjectChange::Mutated {
            sender: c.sender().map_err(err)?,
            owner: c.owner().map_err(err)?,
            object_type: c.object_type().map_err(err)?.try_into().map_err(|_| {
                SdkConversionError("mutated object_type is not a struct".into())
            })?,
            object_id: c.object_id().map_err(err)?,
            version: c.version.ok_or_else(|| SdkConversionError("missing version".into()))?.into(),
            previous_version: c
                .previous_version
                .ok_or_else(|| SdkConversionError("missing previous_version".into()))?
                .into(),
            digest: c.digest().map_err(err)?,
        });
    }
    if let Ok(c) = value.deleted() {
        return Ok(ObjectChange::Deleted {
            sender: c.sender().map_err(err)?,
            object_type: c.object_type().map_err(err)?.try_into().map_err(|_| {
                SdkConversionError("deleted object_type is not a struct".into())
            })?,
            object_id: c.object_id().map_err(err)?,
            version: c.version.ok_or_else(|| SdkConversionError("missing version".into()))?.into(),
        });
    }
    if let Ok(c) = value.wrapped() {
        return Ok(ObjectChange::Wrapped {
            sender: c.sender().map_err(err)?,
            object_type: c.object_type().map_err(err)?.try_into().map_err(|_| {
                SdkConversionError("wrapped object_type is not a struct".into())
            })?,
            object_id: c.object_id().map_err(err)?,
            version: c.version.ok_or_else(|| SdkConversionError("missing version".into()))?.into(),
        });
    }
    if let Ok(c) = value.unwrapped() {
        return Ok(ObjectChange::Unwrapped {
            sender: c.sender().map_err(err)?,
            owner: c.owner().map_err(err)?,
            object_type: c.object_type().map_err(err)?.try_into().map_err(|_| {
                SdkConversionError("unwrapped object_type is not a struct".into())
            })?,
            object_id: c.object_id().map_err(err)?,
            version: c.version.ok_or_else(|| SdkConversionError("missing version".into()))?.into(),
            digest: c.digest().map_err(err)?,
        });
    }
    if let Ok(c) = value.created() {
        return Ok(ObjectChange::Created {
            sender: c.sender().map_err(err)?,
            owner: c.owner().map_err(err)?,
            object_type: c.object_type().map_err(err)?.try_into().map_err(|_| {
                SdkConversionError("created object_type is not a struct".into())
            })?,
            object_id: c.object_id().map_err(err)?,
            version: c.version.ok_or_else(|| SdkConversionError("missing version".into()))?.into(),
            digest: c.digest().map_err(err)?,
        });
    }
    Err(SdkConversionError("ObjectChange has no populated kind".into()))
}
```

Note: `TypeTag::try_into::<StructTag>()` needs `impl TryFrom<TypeTag> for StructTag` — check for an
existing impl on `iota_sdk_types::StructTag`; if absent, match on `TypeTag::Struct(tag) => *tag`
manually instead of `.try_into()`.

- [x] **Step 16: Run the tests to verify they pass**

Run: `IOTA_SKIP_SIMTESTS=1 cargo nextest run -p iota-json-rpc-types sdk_conversions --lib`
Expected: PASS (7 tests total).

- [x] **Step 17: Add the effects-classification and package-lookup helpers used by Task 7**

The JSON-RPC `IotaTransactionBlockEffectsAPI` trait exposes `.created()`/`.deleted()`/`.wrapped()`/
`.unwrapped()`/`.unwrapped_then_deleted()` as precomputed fields. `iota_sdk_types::TransactionEffects`
has no such fields — only a flat `changed_objects: Vec<ChangedObject>` (each with `input_state:
ObjectIn`, `output_state: ObjectOut`, `id_operation: IdOperation`). Append helpers that classify
`changed_objects` the same way, plus a `get_new_package_ref`/`get_new_upgrade_cap_ref` pair mirroring
`get_new_package_obj_from_response`/`get_new_package_upgrade_cap_from_response` but reading gRPC's
native `object_changes` instead of JSON-RPC's:

```rust
use iota_sdk_types::{
    ObjectReference,
    effects::{IdOperation, ObjectIn, ObjectOut, TransactionEffectsV1},
};

/// Objects newly created by this transaction (`ObjectIn::Missing` →
/// `ObjectOut::ObjectWrite`/`PackageWrite`, `IdOperation::Created`).
pub fn created(effects: &TransactionEffectsV1) -> Vec<ObjectReference> {
    effects
        .changed_objects
        .iter()
        .filter(|c| c.id_operation.is_created() && c.input_state.is_missing())
        .filter_map(|c| match &c.output_state {
            ObjectOut::ObjectWrite { digest, .. } => {
                Some(ObjectReference::new(c.object_id, effects.lamport_version, *digest))
            }
            ObjectOut::PackageWrite { version, digest } => {
                Some(ObjectReference::new(c.object_id, *version, *digest))
            }
            ObjectOut::Missing => None,
        })
        .collect()
}

/// Objects deleted by this transaction while they existed at the top level
/// (`ObjectIn::Data` → `ObjectOut::Missing`, `IdOperation::Deleted`).
pub fn deleted(effects: &TransactionEffectsV1) -> Vec<iota_sdk_types::ObjectId> {
    effects
        .changed_objects
        .iter()
        .filter(|c| {
            c.id_operation.is_deleted() && c.input_state.is_data() && c.output_state.is_missing()
        })
        .map(|c| c.object_id)
        .collect()
}

/// Objects wrapped into another object by this transaction (`ObjectIn::Data`
/// → `ObjectOut::Missing`, `IdOperation::None` — the ID is not freed).
pub fn wrapped(effects: &TransactionEffectsV1) -> Vec<iota_sdk_types::ObjectId> {
    effects
        .changed_objects
        .iter()
        .filter(|c| {
            c.id_operation.is_none() && c.input_state.is_data() && c.output_state.is_missing()
        })
        .map(|c| c.object_id)
        .collect()
}

/// Objects unwrapped back to the top level by this transaction
/// (`ObjectIn::Missing` → `ObjectOut::ObjectWrite`, `IdOperation::None` — the
/// ID already existed from a prior wrap).
pub fn unwrapped(effects: &TransactionEffectsV1) -> Vec<ObjectReference> {
    effects
        .changed_objects
        .iter()
        .filter(|c| c.id_operation.is_none() && c.input_state.is_missing())
        .filter_map(|c| match &c.output_state {
            ObjectOut::ObjectWrite { digest, .. } => {
                Some(ObjectReference::new(c.object_id, effects.lamport_version, *digest))
            }
            _ => None,
        })
        .collect()
}

/// Objects that were wrapped in a previous transaction and fully deleted by
/// this one (`ObjectIn::Missing` → `ObjectOut::Missing`, `IdOperation::Deleted`).
pub fn unwrapped_then_deleted(effects: &TransactionEffectsV1) -> Vec<iota_sdk_types::ObjectId> {
    effects
        .changed_objects
        .iter()
        .filter(|c| {
            c.id_operation.is_deleted() && c.input_state.is_missing() && c.output_state.is_missing()
        })
        .map(|c| c.object_id)
        .collect()
}

/// The reference of the package published by this transaction, if any.
/// Mirrors `get_new_package_obj_from_response` but reads gRPC's native
/// `object_changes` instead of the JSON-RPC view.
pub fn get_new_package_ref(tx: &ExecutedTransaction) -> Option<ObjectReference> {
    let changes = tx.object_changes().ok()?;
    changes.object_changes.iter().find_map(|c| {
        let p = c.published().ok()?;
        Some(ObjectReference::new(
            p.package_id().ok()?,
            p.version?.into(),
            p.digest().ok()?,
        ))
    })
}

/// The reference of the `UpgradeCap` created by this transaction, if any.
/// Mirrors `get_new_package_upgrade_cap_from_response`.
pub fn get_new_upgrade_cap_ref(tx: &ExecutedTransaction) -> Option<ObjectReference> {
    let changes = tx.object_changes().ok()?;
    changes.object_changes.iter().find_map(|c| {
        let created = c.created().ok()?;
        let owner = created.owner().ok()?;
        let object_type = created.object_type().ok()?;
        if !owner.is_address() || !object_type.as_struct()?.is_upgrade_cap() {
            return None;
        }
        Some(ObjectReference::new(
            created.object_id().ok()?,
            created.version?.into(),
            created.digest().ok()?,
        ))
    })
}
```

- [x] **Step 18: Write failing unit tests for the classification helpers**

Append to the test module:

```rust
    fn changed_object(
        object_id: iota_sdk_types::ObjectId,
        input_state: iota_sdk_types::effects::ObjectIn,
        output_state: iota_sdk_types::effects::ObjectOut,
        id_operation: IdOperation,
    ) -> iota_sdk_types::effects::ChangedObject {
        iota_sdk_types::effects::ChangedObject {
            object_id,
            input_state,
            output_state,
            id_operation,
        }
    }

    #[test]
    fn created_filters_missing_to_write_with_created_id_operation() {
        let created_id = iota_sdk_types::ObjectId::random();
        let digest = iota_sdk_types::ObjectDigest::random();
        let effects = TransactionEffectsV1 {
            status: iota_sdk_types::ExecutionStatus::Success,
            epoch: 0,
            gas_cost_summary: Default::default(),
            transaction_digest: TransactionDigest::random(),
            gas_object_index: None,
            events_digest: None,
            dependencies: vec![],
            lamport_version: 2.into(),
            changed_objects: vec![changed_object(
                created_id,
                ObjectIn::Missing,
                ObjectOut::ObjectWrite {
                    digest,
                    owner: Owner::Address(Address::TWO),
                },
                IdOperation::Created,
            )],
            unchanged_shared_objects: vec![],
            auxiliary_data_digest: None,
        };

        let refs = created(&effects);
        assert_eq!(refs, vec![ObjectReference::new(created_id, 2.into(), digest)]);
    }
```

`GasCostSummary` must implement `Default` for the `Default::default()` call above — confirm via
`cargo doc -p iota-sdk-types` or by reading
`~/.cargo/git/checkouts/iota-rust-sdk-*/aee5635/crates/iota-sdk-types/src/effects/v1.rs`'s
`GasCostSummary` definition; if it does not derive `Default`, construct it field-by-field with all
zeros instead.

- [x] **Step 19: Run the test to verify it fails, then passes**

Run: `IOTA_SKIP_SIMTESTS=1 cargo nextest run -p iota-json-rpc-types sdk_conversions --lib`
Expected: first FAIL (helpers don't exist before Step 17 lands — reorder Steps 17/18/19 so the test
is written first if following strict TDD literally: write this test immediately after Step 16 and
before Step 17's implementation, run to see it fail, then implement Step 17, then re-run to see it
pass). Final expected: PASS (8 tests total).

- [x] **Step 20: Format and commit**

Run: `cargo +nightly fmt -p iota-json-rpc-types`

```bash
git add crates/iota-json-rpc-types/Cargo.toml crates/iota-json-rpc-types/src/lib.rs crates/iota-json-rpc-types/src/sdk_conversions.rs
git commit -m "$(cat <<'EOF'
feat(iota-json-rpc-types): add json_rpc_types <-> SDK-native conversions

Gives WalletContext's JSON-RPC fallback path the same
iota-rust-sdk-native return types the gRPC path produces natively,
plus effects-classification helpers (created/deleted/wrapped/
unwrapped) mirroring IotaTransactionBlockEffectsAPI for the flat
changed_objects representation the SDK effects type uses.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 3: migrate `get_object_ref` / `get_object_owner` / `try_get_object_owner` / `get_reference_gas_price`

**Files:**

- Modify: `crates/iota-sdk/src/wallet_context.rs`

**Interfaces:**

- Consumes: `WalletContext::resolve_backend(&self) -> Result<WalletBackend, anyhow::Error>` (Task 1).
- Consumes: `iota_grpc_client::Client::get_objects(&self, refs: &[(ObjectId, Option<Version>)],
  read_mask: Option<ReadMask<'_>>) -> Result<MetadataEnvelope<Vec<iota_grpc_types::v1::object::Object>>>`
  and `.get_reference_gas_price(&self) -> Result<MetadataEnvelope<u64>>` (both already available via
  `self.get_grpc_client()`, existing).
- Produces: no signature changes — `get_object_ref`/`get_object_owner`/`try_get_object_owner`/
  `get_reference_gas_price` keep their current return types (`ObjectReference`/`Address`/
  `Option<Address>`/`u64`); only their bodies change. No consumer of these four methods needs
  updating (Task 6/7 do not touch them).

- [x] **Step 1: Write the failing test**

Add a `#[sim_test]` to a new file `crates/iota-e2e-tests/tests/grpc/wallet_context_read_methods.rs`
(register it in `crates/iota-e2e-tests/tests/grpc/main.rs` with `mod wallet_context_read_methods;`):

```rust
// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_macros::sim_test;
use test_cluster::TestClusterBuilder;

/// `get_object_ref`/`get_object_owner`/`get_reference_gas_price` return the
/// same values whether `WalletContext` uses the gRPC or the JSON-RPC
/// backend, against the same gRPC-enabled test cluster.
#[sim_test]
async fn get_object_ref_and_owner_agree_across_backends() {
    let test_cluster = TestClusterBuilder::new()
        .with_fullnode_enable_grpc_api(true)
        .with_num_validators(1)
        .build()
        .await;
    test_cluster.wait_for_checkpoint(1, None).await;

    let grpc_wallet = &test_cluster.wallet;
    let jsonrpc_wallet_path = grpc_wallet.config().path().to_path_buf();
    let jsonrpc_wallet =
        iota_sdk::wallet_context::WalletContext::new(&jsonrpc_wallet_path)
            .unwrap()
            .with_jsonrpc_backend();

    let (_, gas_ref) = grpc_wallet.get_one_gas_object().await.unwrap().unwrap();

    let grpc_ref = grpc_wallet.get_object_ref(gas_ref.object_id).await.unwrap();
    let jsonrpc_ref = jsonrpc_wallet
        .get_object_ref(gas_ref.object_id)
        .await
        .unwrap();
    assert_eq!(grpc_ref, jsonrpc_ref);

    let grpc_owner = grpc_wallet.get_object_owner(&gas_ref.object_id).await.unwrap();
    let jsonrpc_owner = jsonrpc_wallet
        .get_object_owner(&gas_ref.object_id)
        .await
        .unwrap();
    assert_eq!(grpc_owner, jsonrpc_owner);

    let grpc_rgp = grpc_wallet.get_reference_gas_price().await.unwrap();
    let jsonrpc_rgp = jsonrpc_wallet.get_reference_gas_price().await.unwrap();
    assert_eq!(grpc_rgp, jsonrpc_rgp);
}
```

- [x] **Step 2: Run the test to verify it fails/passes trivially before the change**

Run: `cargo simtest -p iota-e2e-tests get_object_ref_and_owner_agree_across_backends`
Expected: PASS already (the methods are currently JSON-RPC-only on both wallets, so this is a
no-op assertion at this point) — this step exists to confirm the test harness and fixture compile;
the real regression check is that it _keeps_ passing after Step 3 changes the gRPC wallet's
implementation. Do not skip re-running it after Step 3.

- [x] **Step 3: Migrate the four methods' bodies to dispatch via `resolve_backend()`**

In `crates/iota-sdk/src/wallet_context.rs`, add to the imports (top of file):

```rust
use iota_grpc_client::{ReadMask, read_mask_fields::ObjectField};
```

Replace `get_object_ref` (lines 176-188):

```rust
/// Get the latest object reference given a object id.
pub async fn get_object_ref(
    &self,
    object_id: ObjectId,
) -> Result<ObjectReference, anyhow::Error> {
    match self.resolve_backend()? {
        WalletBackend::Grpc => {
            let client = self.get_grpc_client().await?;
            let objects = client
                .get_objects(&[(object_id, None)], Some(ReadMask::from(ObjectField::REFERENCE)))
                .await?
                .into_inner();
            let object = objects
                .first()
                .ok_or_else(|| anyhow!("object {object_id} not found"))?;
            Ok(object.object_reference()?)
        }
        WalletBackend::JsonRpc => {
            let client = self.get_client().await?;
            Ok(client
                .read_api()
                .get_object_with_options(object_id, IotaObjectDataOptions::new())
                .await?
                .into_object()?
                .object_ref())
        }
    }
}
```

Replace `get_object_owner` (lines 232-245):

```rust
/// Get the address that owns the object of the provided [`ObjectId`].
pub async fn get_object_owner(&self, id: &ObjectId) -> Result<Address, anyhow::Error> {
    match self.resolve_backend()? {
        WalletBackend::Grpc => {
            let client = self.get_grpc_client().await?;
            let objects = client
                .get_objects(&[(*id, None)], Some(ReadMask::from(ObjectField::BCS)))
                .await?
                .into_inner();
            let object = objects
                .first()
                .ok_or_else(|| anyhow!("object {id} not found"))?
                .object()?;
            Ok(*object
                .owner()
                .address_or_object()
                .ok_or_else(|| anyhow!("not an address or object owner"))?)
        }
        WalletBackend::JsonRpc => {
            let client = self.get_client().await?;
            let object = client
                .read_api()
                .get_object_with_options(*id, IotaObjectDataOptions::new().with_owner())
                .await?
                .into_object()?;
            Ok(*object
                .owner
                .ok_or_else(|| anyhow!("Owner field is None"))?
                .address_or_object()
                .ok_or_else(|| anyhow::anyhow!("not an address or object owner"))?)
        }
    }
}
```

`try_get_object_owner` (lines 248-257) is unchanged — it only calls `get_object_owner` and needs no
edits.

Replace `get_reference_gas_price` (lines 385-389):

```rust
pub async fn get_reference_gas_price(&self) -> Result<u64, anyhow::Error> {
    match self.resolve_backend()? {
        WalletBackend::Grpc => {
            let client = self.get_grpc_client().await?;
            Ok(client.get_reference_gas_price().await?.into_inner())
        }
        WalletBackend::JsonRpc => {
            let client = self.get_client().await?;
            Ok(client.governance_api().get_reference_gas_price().await?)
        }
    }
}
```

- [x] **Step 4: Run the test to verify it still passes**

Run: `cargo simtest -p iota-e2e-tests get_object_ref_and_owner_agree_across_backends`
Expected: PASS.

- [x] **Step 5: Run the existing wallet_context unit tests to confirm no regression**

Run: `IOTA_SKIP_SIMTESTS=1 cargo nextest run -p iota-sdk wallet_context::tests --lib`
Expected: PASS (3 tests, unchanged from Task 1).

- [x] **Step 6: Format and commit**

Run: `cargo +nightly fmt -p iota-sdk -p iota-e2e-tests`

```bash
git add crates/iota-sdk/src/wallet_context.rs crates/iota-e2e-tests/tests/grpc/wallet_context_read_methods.rs crates/iota-e2e-tests/tests/grpc/main.rs
git commit -m "$(cat <<'EOF'
feat(iota-sdk): migrate WalletContext's object/gas-price reads to gRPC

get_object_ref, get_object_owner, and get_reference_gas_price now
dispatch through WalletContext::resolve_backend(), calling
iota_grpc_client::Client directly on the gRPC path; the JSON-RPC path
is unchanged. Return types are unchanged since these methods were
already SDK-native.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 4: migrate `gas_objects` family + its 6 consumers

**Files:**

- Modify: `crates/iota-sdk/src/wallet_context.rs`
- Modify: `crates/iota/src/client_commands.rs`
- Modify: `crates/iota-e2e-tests/tests/full_node_tests.rs`
- Modify: `crates/iota-faucet/src/bin/merge_coins.rs`
- Modify: `crates/iota-faucet/src/faucet/simple_faucet.rs`

**Interfaces:**

- Consumes: `iota_grpc_client::Client::list_owned_objects(&self, owner: Address, object_type:
  Option<StructTag>, page_size: Option<u32>, page_token: Option<prost::bytes::Bytes>, read_mask:
  Option<ReadMask<'_>>) -> ListOwnedObjectsQuery` with `.collect(limit: impl Into<Option<u32>>)
  -> Result<MetadataEnvelope<Vec<iota_grpc_types::v1::object::Object>>>` (existing).
  `iota_sdk_types::Coin::try_from_object(&Object) -> Result<Coin, CoinFromObjectError>`, `.balance()
  -> u64` (existing).
- Consumes: `iota_sdk_types::Object::object_ref(&self) -> ObjectReference`, `.id(&self) -> ObjectId`
  (existing).
- Produces: `WalletContext::gas_objects(&self, address: Address) -> Result<Vec<(u64,
  iota_sdk_types::Object)>, anyhow::Error>` (was `Vec<(u64, IotaObjectData)>`).
- Produces: `WalletContext::gas_for_owner_budget(...) -> Result<(u64, iota_sdk_types::Object),
  anyhow::Error>` (was `(u64, IotaObjectData)`).
- `get_all_accounts_and_gas_objects` (line 369) keeps its signature (`Vec<(Address,
  Vec<ObjectReference>)>`) — only its internal `.map(|(_, o)| o.object_ref())` needs the new type's
  `object_ref()` method, which has the same name and signature as `IotaObjectData::object_ref()`, so
  no change is needed there beyond recompiling.

- [x] **Step 1: Write the failing test**

Add to `crates/iota-e2e-tests/tests/grpc/wallet_context_read_methods.rs` (same file as Task 3):

```rust
/// `gas_objects` returns `iota_sdk_types::Object`s whose gas balance and
/// object ref agree between the gRPC and JSON-RPC backends.
#[sim_test]
async fn gas_objects_agree_across_backends() {
    let test_cluster = TestClusterBuilder::new()
        .with_fullnode_enable_grpc_api(true)
        .with_num_validators(1)
        .build()
        .await;
    test_cluster.wait_for_checkpoint(1, None).await;

    let grpc_wallet = &test_cluster.wallet;
    let jsonrpc_wallet_path = grpc_wallet.config().path().to_path_buf();
    let jsonrpc_wallet =
        iota_sdk::wallet_context::WalletContext::new(&jsonrpc_wallet_path)
            .unwrap()
            .with_jsonrpc_backend();

    let address = grpc_wallet.active_address().unwrap();
    let mut grpc_coins = grpc_wallet.gas_objects(address).await.unwrap();
    let mut jsonrpc_coins = jsonrpc_wallet.gas_objects(address).await.unwrap();
    grpc_coins.sort_by_key(|(_, o)| o.id());
    jsonrpc_coins.sort_by_key(|(_, o)| o.id());

    assert_eq!(grpc_coins.len(), jsonrpc_coins.len());
    for ((grpc_value, grpc_object), (jsonrpc_value, jsonrpc_object)) in
        grpc_coins.iter().zip(jsonrpc_coins.iter())
    {
        assert_eq!(grpc_value, jsonrpc_value);
        assert_eq!(grpc_object.object_ref(), jsonrpc_object.object_ref());
    }
}
```

- [x] **Step 2: Run the test to verify it fails**

Run: `cargo simtest -p iota-e2e-tests gas_objects_agree_across_backends`
Expected: FAIL to compile — `gas_objects` still returns `IotaObjectData`, `.id()` doesn't exist on
it (it's a field `object_id`, not a method), so the test as written targets the post-migration type.

- [x] **Step 3: Migrate `gas_objects` and `gas_for_owner_budget`**

In `crates/iota-sdk/src/wallet_context.rs`, update imports: remove `IotaObjectData` from the
`iota_json_rpc_types` import list (keep `IotaObjectDataFilter, IotaObjectDataOptions,
IotaObjectResponseQuery, IotaTransactionBlockResponse, IotaTransactionBlockResponseOptions`), add:

```rust
use iota_sdk_types::Coin;
```

Replace `gas_objects` (lines 190-230):

```rust
    /// Get all the gas objects (and conveniently, gas amounts) for the address.
    pub async fn gas_objects(
        &self,
        address: Address,
    ) -> Result<Vec<(u64, iota_sdk_types::Object)>, anyhow::Error> {
        match self.resolve_backend()? {
            WalletBackend::Grpc => {
                let client = self.get_grpc_client().await?;
                let objects = client
                    .list_owned_objects(address, Some(StructTag::new_gas_coin()), None, None, None)
                    .collect(None)
                    .await?
                    .into_inner();
                objects
                    .iter()
                    .map(|o| {
                        let object = o.object()?;
                        let coin =
                            Coin::try_from_object(&object).map_err(|e| anyhow!("{e}"))?;
                        Ok((coin.balance(), object))
                    })
                    .collect()
            }
            WalletBackend::JsonRpc => {
                let client = self.get_client().await?;

                let values_objects = PagedFn::stream(async |cursor| {
                    client
                        .read_api()
                        .get_owned_objects(
                            address,
                            IotaObjectResponseQuery::new(
                                Some(IotaObjectDataFilter::StructType(StructTag::new_gas_coin())),
                                Some(IotaObjectDataOptions::full_content().with_bcs()),
                            ),
                            cursor,
                            None,
                        )
                        .await
                })
                .filter_map(|res| async {
                    match res {
                        Ok(res) => res.data.map(|o| {
                            let object = iota_sdk_types::Object::try_from(&o)
                                .map_err(|e| anyhow!("{e}"))?;
                            let coin = Coin::try_from_object(&object)
                                .map_err(|e| anyhow!("{e}"))?;
                            Ok((coin.balance(), object))
                        }),
                        Err(e) => Some(Err(anyhow!("{e}"))),
                    }
                })
                .try_collect::<Vec<_>>()
                .await?;

                Ok(values_objects)
            }
        }
    }
```

Replace `gas_for_owner_budget`'s return type (line 282-296):

```rust
/// Find a gas object which fits the budget.
pub async fn gas_for_owner_budget(
    &self,
    address: Address,
    budget: u64,
    forbidden_gas_objects: BTreeSet<ObjectId>,
) -> Result<(u64, iota_sdk_types::Object), anyhow::Error> {
    for o in self.gas_objects(address).await? {
        if o.0 >= budget && !forbidden_gas_objects.contains(&o.1.id()) {
            return Ok((o.0, o.1));
        }
    }
    bail!(
        "No non-argument gas objects found for this address with value >= budget {budget}. Run iota client gas to check for gas objects."
    )
}
```

(`o.1.object_id` field access became `o.1.id()` method call — `iota_sdk_types::Object` has no
`object_id` field, only `.id() -> ObjectId`.)

- [x] **Step 4: Fix the compile errors in `get_all_accounts_and_gas_objects`**

Its `.map(|(_, o)| o.object_ref())` (line 378) already compiles unchanged since both
`IotaObjectData` and `iota_sdk_types::Object` have an `object_ref()` method with the same
signature — confirm with a build, no edit needed unless the compiler disagrees.

- [x] **Step 5: Update the 6 external `gas_objects` consumers**

**`crates/iota/src/client_commands.rs:1608`** (the `Gas` command) — change:

```rust
IotaClientCommands::Gas { address } => {
    let address = get_identity_address(address, context).await?;
    let coins = context
        .gas_objects(address)
        .await?
        .iter()
        // Ok to unwrap() since `get_gas_objects` guarantees gas
        .map(|(_val, object)| GasCoin::try_from(object).unwrap())
        .collect();
    IotaClientCommandResult::Gas(coins)
}
```

to:

```rust
IotaClientCommands::Gas { address } => {
    let address = get_identity_address(address, context).await?;
    let coins = context
        .gas_objects(address)
        .await?
        .iter()
        // Ok to unwrap() since `get_gas_objects` guarantees gas
        .map(|(_val, object)| iota_sdk_types::Coin::try_from_object(object).unwrap())
        .collect();
    IotaClientCommandResult::Gas(coins)
}
```

Check `IotaClientCommandResult::Gas`'s variant field type (`Vec<GasCoin>` today) — change it to
`Vec<iota_sdk_types::Coin>` and update its `Display`/`Debug` rendering, which currently calls
`GasCoin::value()`/`.id()`; `iota_sdk_types::Coin` exposes the same shape via `.balance()`/`.id()`.
Grep `IotaClientCommandResult::Gas` in this file for the exact rendering code before editing it.

**`crates/iota/src/client_commands.rs:3734`** (`select_coins_for_amount`) — change:

```rust
let mut gas_coins = context
    .gas_objects(sender)
    .await?
    .iter()
    // Ok to unwrap() since `gas_objects` guarantees gas
    .map(|(_val, object)| GasCoin::try_from(object).unwrap())
    .collect::<Vec<_>>();
// Sort in ascending order
gas_coins.sort_unstable_by_key(|c| c.value());
let mut amount_remaining = amount;
while amount_remaining > 0 {
    if let Some(coin) = gas_coins.pop() {
        amount_remaining = amount_remaining.saturating_sub(coin.value());
        coins.push(*coin.id());
```

to:

```rust
let mut gas_coins = context
    .gas_objects(sender)
    .await?
    .iter()
    // Ok to unwrap() since `gas_objects` guarantees gas
    .map(|(_val, object)| iota_sdk_types::Coin::try_from_object(object).unwrap())
    .collect::<Vec<_>>();
// Sort in ascending order
gas_coins.sort_unstable_by_key(|c| c.balance());
let mut amount_remaining = amount;
while amount_remaining > 0 {
    if let Some(coin) = gas_coins.pop() {
        amount_remaining = amount_remaining.saturating_sub(coin.balance());
        coins.push(*coin.id());
```

**`crates/iota-e2e-tests/tests/full_node_tests.rs:545`** — no change needed:
`.swap_remove(0).1.object_ref()` already compiles against `iota_sdk_types::Object`.

**`crates/iota-faucet/src/bin/merge_coins.rs:85`** — change:

```rust
.map(|q| GasCoin::try_from(&q.1).unwrap())
// Everything less than 1 iota
.filter(|coin| coin.0.balance.value() <= 10000000000)
.collect::<Vec<GasCoin>>();
```

to:

```rust
.map(|q| iota_sdk_types::Coin::try_from_object(&q.1).unwrap())
// Everything less than 1 iota
.filter(|coin| coin.balance() <= 10000000000)
.collect::<Vec<_>>();
```

Update the rest of this function to use `.balance()`/`.id()` instead of `GasCoin`'s `.0.balance.value()`/`.id()`.

**`crates/iota-faucet/src/faucet/simple_faucet.rs:120`** (`SimpleFaucet::new`) — change:

```rust
.map(|q| GasCoin::try_from(&q.1).unwrap())
.filter(|coin| coin.0.balance.value() >= (config.amount * config.num_coins as u64))
.collect::<Vec<GasCoin>>();
```

to:

```rust
.map(|q| iota_sdk_types::Coin::try_from_object(&q.1).unwrap())
.filter(|coin| coin.balance() >= (config.amount * config.num_coins as u64))
.collect::<Vec<_>>();
```

Update every downstream use of these `coins`/`GasCoin` values in the rest of `SimpleFaucet::new` to
`iota_sdk_types::Coin`'s `.id()`/`.balance()` accessors.

**`crates/iota-faucet/src/faucet/simple_faucet.rs:1664`** (test) — change:

```rust
        let gas_coins = context.gas_objects(address).await.unwrap();

        let tiny_amount = gas_coins
            .iter()
            .find(|gas| gas.1.object_id == tiny_coin_id)
            .unwrap()
            .0;
        assert_eq!(tiny_amount, tiny_value);

        let gas_coins: HashSet<ObjectId> =
            HashSet::from_iter(gas_coins.into_iter().map(|gas| gas.1.object_id));
```

to:

```rust
        let gas_coins = context.gas_objects(address).await.unwrap();

        let tiny_amount = gas_coins
            .iter()
            .find(|gas| gas.1.id() == tiny_coin_id)
            .unwrap()
            .0;
        assert_eq!(tiny_amount, tiny_value);

        let gas_coins: HashSet<ObjectId> =
            HashSet::from_iter(gas_coins.into_iter().map(|gas| gas.1.id()));
```

- [x] **Step 6: Build every touched crate**

Run: `cargo check -p iota-sdk -p iota -p iota-e2e-tests -p iota-faucet`
Expected: no errors. Fix any remaining `GasCoin`-vs-`Coin` API mismatches surfaced by the compiler
in the two `client_commands.rs`/`merge_coins.rs`/`simple_faucet.rs` spots not shown verbatim above
(e.g. `IotaClientCommandResult::Gas`'s `Display` arm).

- [x] **Step 7: Run the test to verify it passes**

Run: `cargo simtest -p iota-e2e-tests gas_objects_agree_across_backends`
Expected: PASS.

- [x] **Step 8: Format and commit**

Run: `cargo +nightly fmt -p iota-sdk -p iota -p iota-e2e-tests -p iota-faucet`

```bash
git add crates/iota-sdk/src/wallet_context.rs crates/iota/src/client_commands.rs crates/iota-e2e-tests/tests/full_node_tests.rs crates/iota-e2e-tests/tests/grpc/wallet_context_read_methods.rs crates/iota-faucet/src/bin/merge_coins.rs crates/iota-faucet/src/faucet/simple_faucet.rs
git commit -m "$(cat <<'EOF'
feat(iota-sdk): migrate WalletContext::gas_objects to gRPC

gas_objects and gas_for_owner_budget now return
iota_sdk_types::Object via list_owned_objects on the gRPC path;
the JSON-RPC fallback converts through the Task 2 conversions.
Updates the 6 direct consumers from GasCoin::try_from(&IotaObjectData)
to iota_sdk_types::Coin::try_from_object(&Object).

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 5: migrate `execute_transaction_may_fail` / `execute_transaction_must_succeed`

**Files:**

- Modify: `crates/iota-sdk/src/wallet_context.rs`

**Interfaces:**

- Consumes: `iota_grpc_client::Client::execute_transaction(&self, signed_transaction:
  iota_sdk_types::SignedTransaction, read_mask: Option<ReadMask<'_>>,
  checkpoint_inclusion_timeout_ms: Option<u64>) ->
  Result<MetadataEnvelope<iota_grpc_types::v1::transaction::ExecutedTransaction>>` (existing).
- Consumes: `impl TryFrom<crate::transaction::Transaction> for iota_sdk_types::SignedTransaction`
  (already exists, `crates/iota-types/src/iota_sdk_types_conversions.rs:278-284`).
- Consumes: `impl TryFrom<&IotaTransactionBlockResponse> for ExecutedTransaction` (Task 2).
- Produces: `WalletContext::execute_transaction_may_fail(&self, tx: Transaction) ->
  anyhow::Result<iota_grpc_types::v1::transaction::ExecutedTransaction>` (was
  `anyhow::Result<IotaTransactionBlockResponse>`).
- Produces: `WalletContext::execute_transaction_must_succeed(&self, tx: Transaction) ->
  iota_grpc_types::v1::transaction::ExecutedTransaction` (was `IotaTransactionBlockResponse`).

- [x] **Step 1: Write the failing test**

Add to `crates/iota-e2e-tests/tests/grpc/wallet_context_read_methods.rs`:

```rust
/// `execute_transaction_may_fail` succeeds and reports a successful status
/// on the gRPC backend.
#[sim_test]
async fn execute_transaction_succeeds_on_grpc_backend() {
    let test_cluster = TestClusterBuilder::new()
        .with_fullnode_enable_grpc_api(true)
        .with_num_validators(1)
        .build()
        .await;
    test_cluster.wait_for_checkpoint(1, None).await;

    let wallet = &test_cluster.wallet;
    let (sender, gas) = wallet.get_one_gas_object().await.unwrap().unwrap();
    let rgp = wallet.get_reference_gas_price().await.unwrap();
    let tx = wallet.sign_transaction(
        &iota_test_transaction_builder::TestTransactionBuilder::new(sender, gas, rgp)
            .transfer_iota(None, sender)
            .build(),
    );

    let executed = wallet.execute_transaction_must_succeed(tx).await;
    assert!(
        executed
            .effects()
            .unwrap()
            .effects()
            .unwrap()
            .as_v1()
            .status
            .is_success()
    );
}
```

- [x] **Step 2: Run the test to verify it fails**

Run: `cargo simtest -p iota-e2e-tests execute_transaction_succeeds_on_grpc_backend`
Expected: FAIL to compile — `execute_transaction_must_succeed` still returns
`IotaTransactionBlockResponse`, which has no `.effects()` method (it has an `effects` field).

- [x] **Step 3: Migrate `execute_transaction_may_fail` and `execute_transaction_must_succeed`**

In `crates/iota-sdk/src/wallet_context.rs`, update the top-level import block: replace
`IotaTransactionBlockResponse` and `IotaTransactionBlockResponseOptions` usage inside the JSON-RPC
branch (kept for that branch only) and add:

```rust
use iota_grpc_types::v1::transaction::ExecutedTransaction;
```

Add a module-level read-mask constant near the top of the file (after the `use` block):

```rust
/// Read mask for `execute_transaction`/`execute_transactions`: everything
/// `WalletContext`'s current consumers read off an executed transaction.
const EXECUTE_TRANSACTION_READ_MASK: &str = iota_grpc_types::field_mask!(
    "transaction.digest",
    "effects",
    "events",
    "input_objects",
    "output_objects",
    "object_changes",
    "balance_changes",
    "checkpoint",
    "timestamp",
);

/// How long the server waits for a submitted transaction to land in a
/// checkpoint before `execute_transaction` returns, matching the timeout
/// `TransactionBuilderClient::wait_for_tx` polls for.
const CHECKPOINT_INCLUSION_TIMEOUT_MS: u64 = 60_000;
```

Replace `execute_transaction_may_fail` (lines 423-445):

```rust
/// Execute a transaction and wait for it to be locally executed on the
/// fullnode. The transaction execution is not guaranteed to succeed and
/// may fail. This is usually only needed in non-test environment or the
/// caller is explicitly testing some failure behavior.
pub async fn execute_transaction_may_fail(
    &self,
    tx: Transaction,
) -> anyhow::Result<ExecutedTransaction> {
    match self.resolve_backend()? {
        WalletBackend::Grpc => {
            let client = self.get_grpc_client().await?;
            let signed_transaction: iota_sdk_types::SignedTransaction = tx
                .try_into()
                .map_err(|e: iota_types::iota_sdk_types_conversions::SdkTypeConversionError| {
                    anyhow!("{e}")
                })?;
            Ok(client
                .execute_transaction(
                    signed_transaction,
                    Some(ReadMask::from(EXECUTE_TRANSACTION_READ_MASK)),
                    Some(CHECKPOINT_INCLUSION_TIMEOUT_MS),
                )
                .await?
                .into_inner())
        }
        WalletBackend::JsonRpc => {
            let client = self.get_client().await?;
            let response = client
                .quorum_driver_api()
                .execute_transaction_block(
                    tx,
                    IotaTransactionBlockResponseOptions::new()
                        .with_effects()
                        .with_input()
                        .with_raw_input()
                        .with_events()
                        .with_object_changes()
                        .with_balance_changes()
                        .with_raw_effects(),
                    iota_types::quorum_driver_types::ExecuteTransactionRequestType::WaitForLocalExecution,
                )
                .await?;
            ExecutedTransaction::try_from(&response).map_err(|e| anyhow!("{e}"))
        }
    }
}
```

Replace `execute_transaction_must_succeed` (lines 407-421):

```rust
/// Execute a transaction and wait for it to be locally executed on the
/// fullnode. Also expects the effects status to be
/// ExecutionStatus::Success.
pub async fn execute_transaction_must_succeed(&self, tx: Transaction) -> ExecutedTransaction {
    tracing::debug!("Executing transaction: {:?}", tx);
    let response = self.execute_transaction_may_fail(tx).await.unwrap();
    let status_ok = response
        .effects()
        .expect("effects missing from execute_transaction response")
        .effects()
        .expect("effects failed to deserialize")
        .as_v1()
        .status
        .is_success();
    assert!(status_ok, "Transaction failed: {response:?}");
    response
}
```

- [x] **Step 4: Run the test to verify it passes**

Run: `cargo simtest -p iota-e2e-tests execute_transaction_succeeds_on_grpc_backend`
Expected: PASS. This will not yet fully build the crate (Tasks 6/7 still reference the old return
type in ~60 other call sites) — build only `iota-sdk` and this one new test file in isolation first:

Run: `cargo check -p iota-sdk`
Expected: PASS.

Run: `cargo check -p iota-e2e-tests --test main 2>&1 | grep -E "wallet_context_read_methods|error\[" | head -30`
Expected: no errors from `wallet_context_read_methods.rs` itself (errors from _other_ test files in
the same binary, caused by the changed return type, are expected here and are fixed by Task 7 — do
not attempt to fix them in this task).

- [x] **Step 5: Format and commit**

Run: `cargo +nightly fmt -p iota-sdk`

```bash
git add crates/iota-sdk/src/wallet_context.rs crates/iota-e2e-tests/tests/grpc/wallet_context_read_methods.rs
git commit -m "$(cat <<'EOF'
feat(iota-sdk): migrate WalletContext execute methods to gRPC

execute_transaction_may_fail/_must_succeed now dispatch through
resolve_backend() and return iota_grpc_types's ExecutedTransaction
directly on the gRPC path (via ExecuteTransactions with a
checkpoint-inclusion wait), or convert the JSON-RPC response into it
via the Task 2 conversions. This is a breaking return-type change for
every caller of these two methods; consumers are migrated in Tasks 6
and 7.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 6: update the CLI's two `WalletContext`-routed execute commands

**Files:**

- Modify: `crates/iota/src/client_commands.rs`

**Interfaces:**

- Consumes: `impl TryFrom<&iota_grpc_types::v1::transaction::ExecutedTransaction> for
  IotaTransactionBlockResponse` (Task 2).
- Produces: no new interfaces — both call sites keep constructing
  `IotaClientCommandResult::TransactionBlock(IotaTransactionBlockResponse)`, unchanged, so every
  other CLI code path (`Display`, `Debug`/`--json`, the PTB command's `--summary`) needs no changes.

- [x] **Step 1: Write the failing test**

`crates/iota/tests/cli_tests.rs` likely already has a test exercising `ExecuteSignedTx` or
`ExecuteCombinedSignedTx` (grep `ExecuteSignedTx\|ExecuteCombinedSignedTx` in that file). If one
exists, run it first to confirm it currently passes; it is this task's regression test — no new test
file is needed since the CLI output format is unchanged (the whole point of Task 6 is that
`IotaClientCommandResult::TransactionBlock` keeps holding the same JSON-RPC type it always did).

Run: `grep -n "ExecuteSignedTx\|ExecuteCombinedSignedTx" crates/iota/tests/cli_tests.rs`

If a test named e.g. `test_execute_signed_tx` exists, note its name for Step 2/4. If none exists,
add one modeled on the existing `--serialize-unsigned-transaction`/`--serialize-signed-transaction`
round-trip tests in the same file (find one via `grep -n
"serialize-unsigned-transaction\|serialize-signed-transaction" crates/iota/tests/cli_tests.rs` and
follow its exact structure — do not invent a different pattern).

- [x] **Step 2: Run the test to verify it fails**

Run: `cargo check -p iota --tests 2>&1 | grep -A5 "client_commands.rs"`
Expected: FAIL to compile — `context.execute_transaction_may_fail(transaction).await?` now returns
`ExecutedTransaction`, which does not coerce into `IotaClientCommandResult::TransactionBlock`'s
`IotaTransactionBlockResponse` field.

- [x] **Step 3: Convert at both call sites**

In `crates/iota/src/client_commands.rs`, at the `ExecuteSignedTx` arm (around line 1863-1865):

```rust
let response = context.execute_transaction_may_fail(transaction).await?;
IotaClientCommandResult::TransactionBlock(response)
```

becomes:

```rust
let response = context.execute_transaction_may_fail(transaction).await?;
IotaClientCommandResult::TransactionBlock(
    IotaTransactionBlockResponse::try_from(&response)
        .map_err(|e| anyhow!("{e}"))?,
)
```

and identically at the `ExecuteCombinedSignedTx` arm (around line 1874-1876):

```rust
let response = context.execute_transaction_may_fail(transaction).await?;
IotaClientCommandResult::TransactionBlock(response)
```

becomes:

```rust
let response = context.execute_transaction_may_fail(transaction).await?;
IotaClientCommandResult::TransactionBlock(
    IotaTransactionBlockResponse::try_from(&response)
        .map_err(|e| anyhow!("{e}"))?,
)
```

Confirm `IotaTransactionBlockResponse` is already imported in this file (`grep -n "use
iota_json_rpc_types" crates/iota/src/client_commands.rs`); it is, since it's used throughout the
rest of the file.

- [x] **Step 4: Run the test to verify it passes**

Run: `cargo check -p iota`
Expected: PASS (no more errors from these two call sites — `client_commands.rs` may still have
unrelated errors from Task 7's remaining consumers if this task runs before Task 7; check the error
output only mentions files outside `client_commands.rs`).

Then run whichever CLI test was identified/added in Step 1:
Run: `cargo nextest run -p iota --test cli_tests <test_name>`
Expected: PASS, with output byte-identical to before this change (the underlying
`IotaTransactionBlockResponse` shape is unchanged — only its digest/checkpoint/object_changes/
balance_changes fields are populated by the Task 2 reverse conversion; `effects`/`events`/
`transaction`/`raw_transaction`/`raw_effects` are left at their zero values per Task 2's Step 15
implementation). If the test asserts on `transaction`/`effects`/`events` Display output, note the
gap explicitly as a known limitation in a comment at the call site rather than silently passing.

- [x] **Step 5: Format and commit**

Run: `cargo +nightly fmt -p iota`

```bash
git add crates/iota/src/client_commands.rs
git commit -m "$(cat <<'EOF'
fix(iota): convert ExecutedTransaction back to IotaTransactionBlockResponse in ExecuteSignedTx/ExecuteCombinedSignedTx

WalletContext::execute_transaction_may_fail now returns
iota_grpc_types's ExecutedTransaction; these are the only two CLI
commands that route through it, and IotaClientCommandResult::
TransactionBlock's shared Display/JSON path still expects
IotaTransactionBlockResponse, so convert at the call site.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 7: update the remaining consumers of `execute_transaction_may_fail`/`_must_succeed`

**Files:**

- Modify: `crates/iota-test-transaction-builder/src/lib.rs`
- Modify: `crates/iota-e2e-tests/tests/abstract_account_tests.rs`
- Modify: `crates/iota-e2e-tests/tests/abstract_iota_accounts_examples_tests.rs`
- Modify: `crates/iota-e2e-tests/tests/checkpoint_tests.rs`
- Modify: `crates/iota-e2e-tests/tests/full_node_tests.rs`
- Modify: `crates/iota-e2e-tests/tests/grpc/utils.rs`
- Modify: `crates/iota-e2e-tests/tests/multisig_tests.rs`
- Modify: `crates/iota-e2e-tests/tests/per_epoch_config_stress_tests.rs`
- Modify: `crates/iota-e2e-tests/tests/protocol_version_tests.rs`
- Modify: `crates/iota-e2e-tests/tests/reconfiguration_tests.rs`
- Modify: `crates/iota-e2e-tests/tests/shared_objects_tests.rs`
- Modify: `crates/iota-faucet/src/faucet/simple_faucet.rs`
- Modify: `crates/iota-indexer/tests/rpc-tests/governance_api.rs`
- Modify: `crates/iota-indexer/tests/rpc-tests/indexer_api.rs`
- Modify: `crates/iota-indexer/tests/rpc-tests/read_api.rs`
- Modify: `crates/iota-json-rpc-tests/tests/governance_api.rs`
- Modify: `crates/iota-json-rpc-tests/tests/indexer_api.rs`
- Modify: `crates/iota-source-validation/src/tests.rs`

**Interfaces:**

- Consumes: `iota_grpc_types::v1::transaction::ExecutedTransaction` and its accessors
  `.transaction()?.digest()`, `.effects()?.effects()?.as_v1().status`, `.effects()?.effects()?`
  (Task 5).
- Consumes: `iota_json_rpc_types::{created, deleted, wrapped, unwrapped, unwrapped_then_deleted,
  get_new_package_ref, get_new_upgrade_cap_ref}` (Task 2, Step 17).
- Produces: `iota_test_transaction_builder::{publish_package, publish_basics_package,
  publish_basics_package_and_make_counter, increment_counter, emit_new_random_u128,
  publish_example_package, publish_nfts_package, create_nft, delete_nft}` all return
  `iota_grpc_types::v1::transaction::ExecutedTransaction` (or a field extracted from it) instead of
  `IotaTransactionBlockResponse` where they currently return the whole response
  (`increment_counter`, `emit_new_random_u128`, `delete_nft`).

This task has no unit test of its own beyond "the crate compiles and its existing tests pass" —
each file's own test suite is the regression check. Work through the files in dependency order
(the shared helper crate first, since every e2e test depends on it), running the affected crate's
tests after each file.

- [x] **Step 1: Migrate `crates/iota-test-transaction-builder/src/lib.rs`**

Update the imports (remove `IotaObjectDataOptions, IotaTransactionBlockEffectsAPI,
IotaTransactionBlockResponse, get_new_package_obj_from_response` from the `iota_sdk::rpc_types` import,
add):

```rust
use iota_json_rpc_types::{created, get_new_package_ref};
```

(`IotaObjectDataOptions` is still needed by `emit_new_random_u128`'s `get_object_with_options` call
on the raw `IotaClient` — keep that one import if still used; `get_new_package_obj_from_response` is
replaced by `get_new_package_ref` operating on the gRPC-native type.)

`publish_package` (line 572-582):

```rust
pub async fn publish_package(context: &WalletContext, path: PathBuf) -> ObjectReference {
    let (sender, gas_object) = context.get_one_gas_object().await.unwrap().unwrap();
    let gas_price = context.get_reference_gas_price().await.unwrap();
    let txn = context.sign_transaction(
        &TestTransactionBuilder::new(sender, gas_object, gas_price)
            .publish(path)
            .build(),
    );
    let resp = context.execute_transaction_must_succeed(txn).await;
    get_new_package_ref(&resp).unwrap()
}
```

`publish_basics_package` (line 586-596): identical pattern, replace
`get_new_package_obj_from_response(&resp)` with `get_new_package_ref(&resp)`.

`publish_basics_package_and_make_counter` (line 600-623):

```rust
pub async fn publish_basics_package_and_make_counter(
    context: &WalletContext,
) -> (ObjectReference, ObjectReference) {
    let package_ref = publish_basics_package(context).await;
    let (sender, gas_object) = context.get_one_gas_object().await.unwrap().unwrap();
    let gas_price = context.get_reference_gas_price().await.unwrap();
    let counter_creation_txn = context.sign_transaction(
        &TestTransactionBuilder::new(sender, gas_object, gas_price)
            .call_counter_create(package_ref.object_id)
            .build(),
    );
    let resp = context
        .execute_transaction_must_succeed(counter_creation_txn)
        .await;
    let effects = resp.effects().unwrap().effects().unwrap();
    let counter_ref = created(effects.as_v1())
        .into_iter()
        .find(|object_ref| {
            let owner = resp
                .output_objects()
                .ok()
                .and_then(|objs| {
                    objs.objects
                        .iter()
                        .find(|o| o.object_reference().ok().as_ref() == Some(object_ref))
                })
                .and_then(|o| o.object().ok())
                .map(|o| *o.owner());
            matches!(owner, Some(Owner::Shared(_)))
        })
        .unwrap();
    (package_ref, counter_ref)
}
```

Note: unlike the old `IotaObjectData`-based `created()` (whose `OwnedObjectRef.owner` was populated
directly), the SDK-native `created()` helper (Task 2, Step 17) only returns `ObjectReference`s — it
has no owner. Finding "the shared one" requires reading `output_objects` (which the
`EXECUTE_TRANSACTION_READ_MASK` from Task 5 already includes) and matching by object ref, as above.
This is the correct, if slightly more verbose, replacement — do not simplify it away.

`increment_counter` (line 627-651): change the return type and drop the internal
`IotaTransactionBlockResponse` reference:

```rust
pub async fn increment_counter(
    context: &WalletContext,
    sender: Address,
    gas_object_id: Option<ObjectId>,
    package_id: ObjectId,
    counter_id: ObjectId,
    initial_shared_version: SequenceNumber,
) -> ExecutedTransaction {
    let gas_object = if let Some(gas_object_id) = gas_object_id {
        context.get_object_ref(gas_object_id).await.unwrap()
    } else {
        context
            .get_one_gas_object_owned_by_address(sender)
            .await
            .unwrap()
            .unwrap()
    };
    let rgp = context.get_reference_gas_price().await.unwrap();
    let txn = context.sign_transaction(
        &TestTransactionBuilder::new(sender, gas_object, rgp)
            .call_counter_increment(package_id, counter_id, initial_shared_version)
            .build(),
    );
    context.execute_transaction_must_succeed(txn).await
}
```

Add `use iota_grpc_types::v1::transaction::ExecutedTransaction;` to the imports.

`emit_new_random_u128` (line 655-692): change the return type to `ExecutedTransaction`, keep the
body (it still uses the raw `IotaClient` via `context.get_client()` to read the randomness object,
which is unrelated to the execute-methods migration).

`publish_example_package` (line 696-714):

```rust
pub async fn publish_example_package(
    context: &WalletContext,
    example_subpath: &'static str,
    sender_key_pair: &AccountKeyPair,
    sender: Address,
    gas: ObjectReference,
) -> (ObjectId, TransactionDigest) {
    let gas_price = context.get_reference_gas_price().await.unwrap();
    let tx = to_sender_signed_transaction(
        TestTransactionBuilder::new(sender, gas, gas_price)
            .publish_examples(example_subpath)
            .build(),
        sender_key_pair,
    );

    let resp = context.execute_transaction_must_succeed(tx).await;
    let package_id = get_new_package_ref(&resp).unwrap().object_id;
    let digest = resp.transaction().unwrap().digest().unwrap();
    (package_id, digest)
}
```

`publish_nfts_package` (line 718-732): same pattern — replace
`get_new_package_obj_from_response(&resp).unwrap().object_id` with
`get_new_package_ref(&resp).unwrap().object_id`, and `resp.digest` with
`resp.transaction().unwrap().digest().unwrap()`.

`create_nft` (line 748-773):

```rust
pub async fn create_nft(
    context: &WalletContext,
    package_id: ObjectId,
) -> (Address, ObjectId, TransactionDigest) {
    let (sender, gas_object) = context.get_one_gas_object().await.unwrap().unwrap();
    let rgp = context.get_reference_gas_price().await.unwrap();

    let txn = context.sign_transaction(
        &TestTransactionBuilder::new(sender, gas_object, rgp)
            .call_nft_create(package_id)
            .build(),
    );
    let resp = context.execute_transaction_must_succeed(txn).await;

    let object_id = created(resp.effects().unwrap().effects().unwrap().as_v1())
        .first()
        .unwrap()
        .object_id;
    let digest = resp.transaction().unwrap().digest().unwrap();

    (sender, object_id, digest)
}
```

`delete_nft` (line 777-794): change the return type to `ExecutedTransaction`, body unchanged.

- [x] **Step 2: Build and test the helper crate**

Run: `cargo check -p iota-test-transaction-builder`
Expected: PASS.

- [x] **Step 3: Migrate the e2e-test files, one at a time, building after each**

For each file below, apply the described change, then run:
`cargo check -p iota-e2e-tests --test main 2>&1 | grep "<filename>"` to confirm no remaining errors
attributable to that file before moving to the next (errors from _other_, not-yet-migrated files in
the same test binary are expected until this task is complete).

**`crates/iota-e2e-tests/tests/abstract_account_tests.rs`** (line 320-327):

```rust
let tx_result = test_env
    .test_cluster
    .wallet
    .execute_transaction_may_fail(tx)
    .await
    .unwrap()
    .effects
    .unwrap();
```

becomes:

```rust
let tx_result = test_env
    .test_cluster
    .wallet
    .execute_transaction_may_fail(tx)
    .await
    .unwrap()
    .effects()
    .unwrap()
    .effects()
    .unwrap();
```

`tx_result.status()` becomes `tx_result.as_v1().status` (an `iota_sdk_types::ExecutionStatus`, whose
`Debug`/error-string rendering differs from the old `IotaExecutionStatus`; confirm the assertion at
line 330-333 (`error_string.contains("abort")`) still passes — `ExecutionStatus::Failure { error,
.. }`'s `Debug`/`Display` needs to mention "abort" for a `MoveAbort` `ExecutionError` variant; check
`iota_sdk_types::execution_status::ExecutionError`'s `Display` impl if this assertion fails and
adjust the substring match, not the assertion's intent).

**`crates/iota-e2e-tests/tests/abstract_iota_accounts_examples_tests.rs`** (line 2005-2044):

Change the function's return type from `iota_json_rpc_types::IotaTransactionBlockResponse` to
`iota_grpc_types::v1::transaction::ExecutedTransaction`, and:

```rust
        let resp = self
            .test_cluster
            .wallet
            .execute_transaction_must_succeed(tx)
            .await;

        let pkg_id = iota_json_rpc_types::get_new_package_obj_from_response(&resp)
            .ok_or_else(|| anyhow::anyhow!("no Published object change in response"))?
            .object_id;
```

becomes:

```rust
        let resp = self
            .test_cluster
            .wallet
            .execute_transaction_must_succeed(tx)
            .await;

        let pkg_id = iota_json_rpc_types::get_new_package_ref(&resp)
            .ok_or_else(|| anyhow::anyhow!("no Published object change in response"))?
            .object_id;
```

**`crates/iota-e2e-tests/tests/checkpoint_tests.rs`** (line 79-83): no change — the return value is
fully discarded (`.await.ok();`), compiles unchanged once `wallet_context.rs` changes land.

**`crates/iota-e2e-tests/tests/full_node_tests.rs`**:

- Line 1261 (`execute_transaction_may_fail`, error-path-only usage): no change —
  `result.unwrap_err().to_string()` compiles unchanged.
- Line 1296 (`execute_transaction_must_succeed`): `resp.digest` becomes
  `resp.transaction().unwrap().digest().unwrap()`.

**`crates/iota-e2e-tests/tests/grpc/utils.rs`** (line 160-163): no change — return value fully
discarded.

**`crates/iota-e2e-tests/tests/multisig_tests.rs`**: lines 279 and 289
(`res.status_ok().unwrap()`) become:

```rust
let res = context.execute_transaction_must_succeed(tx1).await;
assert!(
    res.effects()
        .unwrap()
        .effects()
        .unwrap()
        .as_v1()
        .status
        .is_success()
);
```

(twice, once per call site). Every other call site in this file (lines 299, 310, 324, 335, 368, 391,
412, 423, 430, 437) only reads `.unwrap_err()`/`.is_ok()`/`.is_err()` on the `anyhow::Result` itself
and needs no change.

**`crates/iota-e2e-tests/tests/per_epoch_config_stress_tests.rs`** (line 89-107):

```rust
let Ok(effects) = test_env
    .test_cluster
    .wallet
    .execute_transaction_may_fail(tx)
    .await
    .map(|r| r.effects.unwrap())
else {
```

becomes:

```rust
let Ok(effects) = test_env
    .test_cluster
    .wallet
    .execute_transaction_may_fail(tx)
    .await
    .map(|r| r.effects().unwrap().effects().unwrap())
else {
```

`effects.status().is_ok()` becomes `effects.as_v1().status.is_success()`;
`effects.executed_epoch()` becomes `effects.as_v1().epoch`.

**`crates/iota-e2e-tests/tests/protocol_version_tests.rs`** (line 591-612): change the `execute`
helper's return type from `IotaTransactionBlockEffects` to `iota_sdk_types::TransactionEffects`:

```rust
    async fn execute(
        cluster: &TestCluster,
        ptb: ProgrammableTransaction,
    ) -> iota_sdk_types::TransactionEffects {
        let context = &cluster.wallet;
        let (sender, gas_object) = context.get_one_gas_object().await.unwrap().unwrap();

        let rgp = context.get_reference_gas_price().await.unwrap();
        let txn = context.sign_transaction(&TransactionData::new_programmable(
            sender,
            vec![gas_object],
            ptb,
            rgp * TEST_ONLY_GAS_UNIT_FOR_GENERIC,
            rgp,
        ));

        context
            .execute_transaction_must_succeed(txn)
            .await
            .effects()
            .unwrap()
            .effects()
            .unwrap()
    }
```

Update every caller of `execute()` in this file that reads `IotaTransactionBlockEffectsAPI` methods
(`.status()`, etc.) to the `iota_sdk_types::TransactionEffects` equivalents (`.as_v1().status`) —
grep `execute(` in this file for the full call-site list before editing further.

**`crates/iota-e2e-tests/tests/reconfiguration_tests.rs`** (lines 124, 138): both
`execute_transaction_may_fail` sites — line 124 reads only `.unwrap_err().to_string()` (no change);
line 138 discards the result entirely via `.unwrap();` (no change).

**`crates/iota-e2e-tests/tests/shared_objects_tests.rs`** (line 456-468):

```rust
let effects = test_cluster
    .wallet
    .execute_transaction_may_fail(test_cluster.wallet.sign_transaction(&transaction))
    .await
    .unwrap()
    .effects
    .unwrap();
```

becomes:

```rust
let effects = test_cluster
    .wallet
    .execute_transaction_may_fail(test_cluster.wallet.sign_transaction(&transaction))
    .await
    .unwrap()
    .effects()
    .unwrap()
    .effects()
    .unwrap();
```

`effects.status().is_err()` becomes `!effects.as_v1().status.is_success()`;
`effects.status().to_string()` needs the `Display`/`Debug` string of
`iota_sdk_types::ExecutionStatus`/its `ExecutionError` — check the assertion's substring
("Immutable objects cannot be passed by mutable reference") still matches; adjust the format call
(`format!("{:?}", effects.as_v1().status)`) if the exact wording differs, keeping the assertion's
intent (that this specific abort reason is reported).

- [x] **Step 4: Migrate `crates/iota-faucet/src/faucet/simple_faucet.rs`'s test-only `execute_tx` helper**

Lines 1177-1211: change the import at line 1180 from `iota_json_rpc_types::{IotaExecutionStatus,
IotaTransactionBlockEffects}` — remove it. Change the helper:

```rust
    async fn execute_tx(
        ctx: &mut WalletContext,
        tx_data: TransactionData,
    ) -> Result<iota_sdk_types::TransactionEffects, anyhow::Error> {
        let signature = ctx.config().keystore().sign_secure(
            &tx_data.sender(),
            &tx_data,
            Intent::iota_transaction(),
        )?;
        let sender_signed_data = SenderSignedData::new_from_sender_signature(tx_data, signature);
        let transaction = Transaction::new(sender_signed_data);
        let response = ctx.execute_transaction_may_fail(transaction).await?;
        let effects = response
            .effects()
            .map_err(|e| anyhow::anyhow!("{e}"))?
            .effects()
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        if effects.as_v1().status.is_success() {
            Ok(effects)
        } else {
            bail!("error executing transaction: {:?}", effects.as_v1().status);
        }
    }
```

Update every caller of `execute_tx` in this test module (grep `execute_tx(` within this file) that
reads `IotaTransactionBlockEffectsAPI` methods (e.g. `.created()`) to the SDK-native equivalents
(`iota_json_rpc_types::created(&effects.as_v1())`, importing that helper from Task 2).

- [x] **Step 5: Migrate the indexer/JSON-RPC-tests call sites**

**`crates/iota-indexer/tests/rpc-tests/governance_api.rs`** (6 sites: lines 90, 173, 202, 317, 435,
483): each is `let res = ...execute_transaction_must_succeed(txn).await;` followed by
`indexer_wait_for_transaction(res.digest, store, client).await;`. Change every `res.digest` to
`res.transaction().unwrap().digest().unwrap()`.

**`crates/iota-indexer/tests/rpc-tests/indexer_api.rs`** (2 sites: lines 1286, 1412): same pattern,
same fix (`res.digest` → `res.transaction().unwrap().digest().unwrap()`).

**`crates/iota-indexer/tests/rpc-tests/read_api.rs`** (6 sites: lines 2075, 2126, 2212, 2300, 2425,
2471): each reads `<res>.digest` (→ `<res>.transaction().unwrap().digest().unwrap()`) then one of
`.effects.unwrap().created()/.wrapped()/.unwrapped_then_deleted()/.deleted()/.unwrapped()`. Replace
each `<res>.effects.unwrap().<method>()` (or `.as_ref().unwrap().<method>()`) with
`iota_json_rpc_types::<method>(<res>.effects().unwrap().effects().unwrap().as_v1())` using the
matching helper from Task 2 (`created`, `wrapped`, `unwrapped_then_deleted`, `deleted`, `unwrapped`).
Each helper returns `Vec<ObjectReference>` (for `created`/`unwrapped`) or `Vec<ObjectId>` (for
`deleted`/`wrapped`/`unwrapped_then_deleted`) — update the `.map(|x| x.reference)` /
`.map(|x| x.object_id)` closures accordingly (the `created`/`unwrapped` call sites already read
`.reference`/no field since the helper returns `ObjectReference` directly — drop the `.reference`
projection; the `deleted`/`wrapped`/`unwrapped_then_deleted` call sites already read `.object_id` —
drop that projection too, since the helpers return `ObjectId` directly).

**`crates/iota-json-rpc-tests/tests/governance_api.rs`** (line 478): `let _ = ...` — no change.

**`crates/iota-json-rpc-tests/tests/indexer_api.rs`** (lines 584, 674): `let _ = ...` — no change.

**`crates/iota-source-validation/src/tests.rs`**:

- Lines 773-776: `get_new_package_obj_from_response(&response)` →
  `iota_json_rpc_types::get_new_package_ref(&response)`;
  `get_new_package_upgrade_cap_from_response(&response)` →
  `iota_json_rpc_types::get_new_upgrade_cap_ref(&response)`.
- Line 807-808: same `get_new_package_obj_from_response` → `get_new_package_ref` swap.
- Lines 915, 918-919: same swap for `get_new_package_obj_from_response`; `resp.digest` →
  `resp.transaction().unwrap().digest().unwrap()`.

- [x] **Step 6: Build every touched crate**

Run: `cargo check -p iota-e2e-tests -p iota-faucet -p iota-indexer -p iota-json-rpc-tests -p iota-source-validation --tests`
Expected: no errors.

- [x] **Step 7: Run the affected test suites**

Run: `cargo simtest -p iota-e2e-tests abstract_account_tests abstract_iota_accounts_examples_tests checkpoint_tests full_node_tests multisig_tests per_epoch_config_stress_tests protocol_version_tests reconfiguration_tests shared_objects_tests`
Expected: PASS (allow 10+ minutes; these are `#[sim_test]`s).

Run: `IOTA_SKIP_SIMTESTS=1 cargo nextest run -p iota-faucet -p iota-indexer -p iota-json-rpc-tests -p iota-source-validation`
Expected: PASS.

- [x] **Step 8: Format and commit**

Run: `cargo +nightly fmt -p iota-test-transaction-builder -p iota-e2e-tests -p iota-faucet -p iota-indexer -p iota-json-rpc-tests -p iota-source-validation`

```bash
git add crates/iota-test-transaction-builder/src/lib.rs crates/iota-e2e-tests/tests/abstract_account_tests.rs crates/iota-e2e-tests/tests/abstract_iota_accounts_examples_tests.rs crates/iota-e2e-tests/tests/checkpoint_tests.rs crates/iota-e2e-tests/tests/full_node_tests.rs crates/iota-e2e-tests/tests/grpc/utils.rs crates/iota-e2e-tests/tests/multisig_tests.rs crates/iota-e2e-tests/tests/per_epoch_config_stress_tests.rs crates/iota-e2e-tests/tests/protocol_version_tests.rs crates/iota-e2e-tests/tests/reconfiguration_tests.rs crates/iota-e2e-tests/tests/shared_objects_tests.rs crates/iota-faucet/src/faucet/simple_faucet.rs crates/iota-indexer/tests/rpc-tests/governance_api.rs crates/iota-indexer/tests/rpc-tests/indexer_api.rs crates/iota-indexer/tests/rpc-tests/read_api.rs crates/iota-json-rpc-tests/tests/governance_api.rs crates/iota-json-rpc-tests/tests/indexer_api.rs crates/iota-source-validation/src/tests.rs
git commit -m "$(cat <<'EOF'
test: migrate remaining execute_transaction_may_fail/_must_succeed consumers to ExecutedTransaction

Updates the shared iota-test-transaction-builder helpers and every
e2e/indexer/json-rpc test call site to the SDK-native
ExecutedTransaction return type, using the created/deleted/wrapped/
unwrapped/get_new_package_ref/get_new_upgrade_cap_ref helpers added in
iota-json-rpc-types.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 8: flip `test-cluster` to the gRPC backend by default + parity tests

**Files:**

- Modify: `crates/test-cluster/src/lib.rs`
- Modify: `crates/iota-e2e-tests/tests/grpc/wallet_context.rs`

**Interfaces:**

- Consumes: `TestClusterBuilder::with_fullnode_enable_grpc_api(bool)` (already exists).
- Consumes: `WalletContext::resolve_backend` (Task 1) — no direct call, but this task relies on its
  fallback behavior: once the active env has a `grpc` URL, `resolve_backend()` resolves to `Grpc`
  automatically with no other code change.
- Produces: `TestClusterBuilder::new()`'s default now has `fullnode_enable_grpc_api: true` — every
  test-cluster-backed `WalletContext` in the whole `iota-e2e-tests`/`iota-indexer`/
  `iota-json-rpc-tests` suites now defaults to the gRPC backend unless a test explicitly calls
  `.with_fullnode_enable_grpc_api(false)`.

- [x] **Step 1: Write the failing test**

Append to `crates/iota-e2e-tests/tests/grpc/wallet_context.rs`:

```rust
/// `TestClusterBuilder::new()` enables the fullnode's gRPC API by default,
/// so a plain `test_cluster.wallet` resolves to the gRPC backend without any
/// opt-in.
#[sim_test]
async fn test_cluster_wallet_defaults_to_grpc_backend() {
    let test_cluster = TestClusterBuilder::new().with_num_validators(1).build().await;
    test_cluster.wait_for_checkpoint(1, None).await;

    assert!(
        test_cluster.wallet.active_env().unwrap().grpc().is_some(),
        "expected TestClusterBuilder::new() to configure a grpc URL by default"
    );
}
```

- [x] **Step 2: Run the test to verify it fails**

Run: `cargo simtest -p iota-e2e-tests test_cluster_wallet_defaults_to_grpc_backend`
Expected: FAIL — `fullnode_enable_grpc_api` currently defaults to `false`, so `grpc()` is `None`.

- [x] **Step 3: Flip the default**

In `crates/test-cluster/src/lib.rs`, in `TestClusterBuilder::new()` (around line 1030):

```rust
fullnode_enable_grpc_api: false,
```

becomes:

```rust
fullnode_enable_grpc_api: true,
```

- [x] **Step 4: Run the test to verify it passes**

Run: `cargo simtest -p iota-e2e-tests test_cluster_wallet_defaults_to_grpc_backend`
Expected: PASS.

- [x] **Step 5: Write the backend-parity test**

Append to `crates/iota-e2e-tests/tests/grpc/wallet_context.rs`:

```rust
/// The gRPC and JSON-RPC backends return equivalent SDK-native values for
/// the same on-chain state, against the same (now gRPC-by-default)
/// test-cluster node.
#[sim_test]
async fn grpc_and_jsonrpc_backends_agree_end_to_end() {
    let test_cluster = TestClusterBuilder::new().with_num_validators(1).build().await;
    test_cluster.wait_for_checkpoint(1, None).await;

    let grpc_wallet = &test_cluster.wallet;
    let jsonrpc_wallet_path = grpc_wallet.config().path().to_path_buf();
    let jsonrpc_wallet =
        iota_sdk::wallet_context::WalletContext::new(&jsonrpc_wallet_path)
            .unwrap()
            .with_jsonrpc_backend();

    let (sender, gas) = grpc_wallet.get_one_gas_object().await.unwrap().unwrap();

    assert_eq!(
        grpc_wallet.get_object_ref(gas.object_id).await.unwrap(),
        jsonrpc_wallet.get_object_ref(gas.object_id).await.unwrap(),
    );
    assert_eq!(
        grpc_wallet.get_object_owner(&gas.object_id).await.unwrap(),
        jsonrpc_wallet.get_object_owner(&gas.object_id).await.unwrap(),
    );
    assert_eq!(
        grpc_wallet.get_reference_gas_price().await.unwrap(),
        jsonrpc_wallet.get_reference_gas_price().await.unwrap(),
    );

    let rgp = grpc_wallet.get_reference_gas_price().await.unwrap();
    let tx = grpc_wallet.sign_transaction(
        &iota_test_transaction_builder::TestTransactionBuilder::new(sender, gas, rgp)
            .transfer_iota(None, sender)
            .build(),
    );
    let grpc_result = grpc_wallet.execute_transaction_must_succeed(tx).await;
    assert!(
        grpc_result
            .effects()
            .unwrap()
            .effects()
            .unwrap()
            .as_v1()
            .status
            .is_success()
    );

    let (_, gas2) = grpc_wallet.get_one_gas_object().await.unwrap().unwrap();
    let tx2 = jsonrpc_wallet.sign_transaction(
        &iota_test_transaction_builder::TestTransactionBuilder::new(sender, gas2, rgp)
            .transfer_iota(None, sender)
            .build(),
    );
    let jsonrpc_result = jsonrpc_wallet.execute_transaction_must_succeed(tx2).await;
    assert!(
        jsonrpc_result
            .effects()
            .unwrap()
            .effects()
            .unwrap()
            .as_v1()
            .status
            .is_success()
    );
}
```

- [x] **Step 6: Run the test to verify it passes**

Run: `cargo simtest -p iota-e2e-tests grpc_and_jsonrpc_backends_agree_end_to_end`
Expected: PASS.

- [x] **Step 7: Run the full wallet-driven e2e/sim suite as the acceptance gate**

This is the gate the design doc calls for: with the node now dual-serving JSON-RPC and gRPC and
`test-cluster` defaulting to the gRPC backend, the whole existing e2e/sim suite must stay green.

Run: `cargo simtest -p iota-e2e-tests` (10+ minute timeout)
Expected: PASS, no regressions relative to `develop`. Investigate and fix (not skip) any failure —
per this repo's global constraint, tests are never disabled to make this step "pass".

Run: `IOTA_SKIP_SIMTESTS=1 cargo nextest run -p iota-indexer -p iota-json-rpc-tests -p iota-faucet -p iota-source-validation -p iota`
Expected: PASS.

- [x] **Step 8: Format and commit**

Run: `cargo +nightly fmt -p test-cluster -p iota-e2e-tests`

```bash
git add crates/test-cluster/src/lib.rs crates/iota-e2e-tests/tests/grpc/wallet_context.rs
git commit -m "$(cat <<'EOF'
feat(test-cluster): default TestClusterBuilder to gRPC-enabled fullnodes

Flips fullnode_enable_grpc_api's default to true so every
test-cluster-backed WalletContext resolves to the gRPC backend by
default (WalletContext's own default), completing the migration's
acceptance gate: the full wallet-driven e2e/sim suite now runs over
gRPC while the node keeps serving JSON-RPC for the (still-supported)
opt-in fallback path. Adds a parity test asserting the two backends
return equivalent SDK-native values against the same node.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Self-review notes

- **Spec coverage:** Decision 1 (backend toggle + fallback) → Task 1. Decision 2 (all methods at
  once, SDK-native returns) → Tasks 3-5. Decision 3 (conversions live in `iota-json-rpc-types`) →
  Task 2. CLI transaction output → Task 6. Testing section (unit/parity/opt-in wiring/determinism)
  → Tasks 2, 3/4/5's parity tests, Task 8. Scope note (update every consumer) → Tasks 4, 6, 7.
- **Corrected a stale premise from the investigation inputs:** `investigate-types.md` (written
  against grpc-types rev `9826f3b`) states `ExecutedTransaction` has no `object_changes`/
  `balance_changes` and that reproducing them would require reimplementing
  `get_object_changes`/`get_balance_changes_from_effect`. At the pinned rev `aee56356` (confirmed by
  reading `crates/iota-sdk-grpc-types/src/proto/generated/iota.grpc.v1.transaction.rs:85-100` in the
  local checkout), `ExecutedTransaction` has native `balance_changes`/`object_changes` fields with
  SDK-typed accessors, populated server-side from effects — no diff-reimplementation is needed. Task
  2 maps JSON-RPC's already-computed `ObjectChange`/`BalanceChange` onto the equivalent proto
  variants field-by-field instead.
- **A second, real gap this investigation surfaced (not in any input file):** JSON-RPC's
  `ObjectChange::Transferred` variant has no equivalent in the gRPC proto's `ObjectChange` (which has
  `Published`/`Mutated`/`Deleted`/`Wrapped`/`Unwrapped`/`Created` only). Task 2 documents this as a
  hard error on that one variant in the JSON-RPC-fallback-only conversion direction; no consumer in
  this repo's call-site inventory exercises it today.
- **A third gap:** `iota_sdk_types::TransactionEffects` has no `IotaTransactionBlockEffectsAPI`-style
  `created()`/`deleted()`/`wrapped()`/`unwrapped()`/`unwrapped_then_deleted()` accessors — it exposes
  a flat `changed_objects: Vec<ChangedObject>` instead. Task 2 adds equivalent classification helpers
  (derived from the documented `ObjectIn`/`ObjectOut`/`IdOperation` ABNF semantics) that Task 7's ~10
  affected call sites (mostly in `iota-indexer/tests/rpc-tests/read_api.rs` and
  `iota-test-transaction-builder`) use instead.
