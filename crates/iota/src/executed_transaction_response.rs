// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Converts a gRPC `ExecutedTransaction` into the JSON-RPC
//! `IotaTransactionBlockResponse` so the CLI's `--json` output keeps the
//! historical shape (`effects.status`, `objectChanges`, `balanceChanges`).
//!
//! This lives in the CLI crate because it bridges `iota-grpc-types` and
//! `iota-json-rpc-types`; keeping it here avoids giving the shared
//! `iota-json-rpc-types` crate a dependency on `iota-grpc-types`.

use anyhow::{Context, Result};
use iota_grpc_types::v1::transaction::{
    BalanceChange as ProtoBalanceChange, ExecutedTransaction, ObjectChange as ProtoObjectChange,
    object_change::Kind as ProtoObjectChangeKind,
};
use iota_json_rpc_types::{
    BalanceChange, IotaTransactionBlockEffects, IotaTransactionBlockResponse, ObjectChange,
};
use iota_sdk_types::{StructTag, TypeTag, Version};

/// Converts a gRPC `ExecutedTransaction` into the JSON-RPC
/// `IotaTransactionBlockResponse` the CLI serializes for `--json`.
///
/// Every proto accessor is guarded, since the server's read mask may omit
/// fields. `events` is always left unset: the JSON-RPC events conversion needs
/// a Move type resolver that is not available here.
pub(crate) fn executed_transaction_to_response(
    executed: &ExecutedTransaction,
) -> Result<IotaTransactionBlockResponse> {
    let digest = executed
        .transaction()
        .context("missing transaction")?
        .digest()
        .context("missing transaction digest")?;

    let (effects, raw_effects) = match executed.effects().ok().and_then(|e| e.effects().ok()) {
        Some(sdk_effects) => {
            let raw = bcs::to_bytes(&sdk_effects).context("serializing effects")?;
            let effects =
                IotaTransactionBlockEffects::try_from(sdk_effects).context("converting effects")?;
            (Some(effects), raw)
        }
        None => (None, vec![]),
    };

    let object_changes = executed
        .object_changes()
        .ok()
        .map(|changes| {
            changes
                .object_changes
                .iter()
                .map(proto_object_change_to_json_rpc)
                .collect::<Result<Vec<_>>>()
        })
        .transpose()?;

    let balance_changes = executed
        .balance_changes()
        .ok()
        .map(|changes| {
            changes
                .balance_changes
                .iter()
                .map(proto_balance_change_to_json_rpc)
                .collect::<Result<Vec<_>>>()
        })
        .transpose()?;

    Ok(IotaTransactionBlockResponse {
        digest,
        transaction: None,
        raw_transaction: vec![],
        effects,
        events: None,
        object_changes,
        balance_changes,
        timestamp_ms: executed.timestamp_ms().ok(),
        confirmed_local_execution: None,
        checkpoint: executed.checkpoint,
        errors: vec![],
        raw_effects,
    })
}

/// Serialize an `ExecutedTransaction` field as an `IotaTransactionBlockResponse`
/// so `--json` emits the historical JSON-RPC shape. On conversion failure the
/// error message is serialized instead.
pub(crate) fn serialize_executed_transaction<S>(
    executed: &ExecutedTransaction,
    serializer: S,
) -> std::result::Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    match executed_transaction_to_response(executed) {
        Ok(response) => serde::Serialize::serialize(&response, serializer),
        Err(error) => {
            use serde::ser::SerializeStruct;
            let mut state = serializer.serialize_struct("ExecutedTransaction", 1)?;
            state.serialize_field("error", &error.to_string())?;
            state.end()
        }
    }
}

fn proto_balance_change_to_json_rpc(value: &ProtoBalanceChange) -> Result<BalanceChange> {
    Ok(BalanceChange {
        owner: value.owner()?,
        coin_type: value.coin_type()?,
        amount: value.amount_i128()?,
    })
}

fn struct_tag_from_type_tag(type_tag: TypeTag, context: &str) -> Result<StructTag> {
    type_tag
        .into_opt_struct_tag()
        .ok_or_else(|| anyhow::anyhow!("{context} object_type is not a struct"))
}

fn missing_version() -> anyhow::Error {
    anyhow::anyhow!("missing version")
}

fn proto_object_change_to_json_rpc(value: &ProtoObjectChange) -> Result<ObjectChange> {
    match value
        .kind
        .as_ref()
        .context("ObjectChange has no populated kind")?
    {
        ProtoObjectChangeKind::Published(c) => Ok(ObjectChange::Published {
            package_id: c.package_id()?,
            version: Version::from_u64(c.version.ok_or_else(missing_version)?),
            digest: c.digest()?,
            modules: c.modules.clone(),
        }),
        ProtoObjectChangeKind::Mutated(c) => Ok(ObjectChange::Mutated {
            sender: c.sender()?,
            owner: c.owner()?,
            object_type: struct_tag_from_type_tag(c.object_type()?, "mutated")?,
            object_id: c.object_id()?,
            version: Version::from_u64(c.version.ok_or_else(missing_version)?),
            previous_version: Version::from_u64(
                c.previous_version
                    .ok_or_else(|| anyhow::anyhow!("missing previous_version"))?,
            ),
            digest: c.digest()?,
        }),
        ProtoObjectChangeKind::Deleted(c) => Ok(ObjectChange::Deleted {
            sender: c.sender()?,
            object_type: struct_tag_from_type_tag(c.object_type()?, "deleted")?,
            object_id: c.object_id()?,
            version: Version::from_u64(c.version.ok_or_else(missing_version)?),
        }),
        ProtoObjectChangeKind::Wrapped(c) => Ok(ObjectChange::Wrapped {
            sender: c.sender()?,
            object_type: struct_tag_from_type_tag(c.object_type()?, "wrapped")?,
            object_id: c.object_id()?,
            version: Version::from_u64(c.version.ok_or_else(missing_version)?),
        }),
        ProtoObjectChangeKind::Unwrapped(c) => Ok(ObjectChange::Unwrapped {
            sender: c.sender()?,
            owner: c.owner()?,
            object_type: struct_tag_from_type_tag(c.object_type()?, "unwrapped")?,
            object_id: c.object_id()?,
            version: Version::from_u64(c.version.ok_or_else(missing_version)?),
            digest: c.digest()?,
        }),
        ProtoObjectChangeKind::Created(c) => Ok(ObjectChange::Created {
            sender: c.sender()?,
            owner: c.owner()?,
            object_type: struct_tag_from_type_tag(c.object_type()?, "created")?,
            object_id: c.object_id()?,
            version: Version::from_u64(c.version.ok_or_else(missing_version)?),
            digest: c.digest()?,
        }),
        _ => Err(anyhow::anyhow!("unknown ObjectChange kind")),
    }
}
