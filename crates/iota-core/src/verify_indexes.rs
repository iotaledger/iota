// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeMap, sync::Arc};

use anyhow::{Result, anyhow, bail};
use iota_sdk_types::Owner;
use tracing::info;
use typed_store::traits::Map;

use crate::{
    global_state_hasher::GlobalStateHashStore,
    rpc_indexes::{
        RpcIndexesStore,
        schema::{OwnerIndexInfo, OwnerIndexKey},
    },
};

/// This is a very expensive function that verifies some of the secondary
/// indexes. This is done by iterating through the live object set and
/// recalculating these secondary indexes.
pub fn verify_indexes(
    store: &dyn GlobalStateHashStore,
    indexes: Arc<RpcIndexesStore>,
) -> Result<()> {
    info!("Begin running index verification checks");

    let mut owner_index = BTreeMap::new();

    tracing::info!("Reading live objects set");
    for live_object in store.iter_live_object_set() {
        let live_object = live_object?;
        let object = &live_object.object;
        let Owner::Address(owner) = object.owner else {
            continue;
        };

        // A coin's balance is part of its owner-index key, so the owner index
        // is the only live-state table the coin reads are served from.
        if let Some((key, info)) = OwnerIndexKey::for_object(owner, object) {
            owner_index.insert(key, info);
        }
    }

    tracing::info!("Live objects set is prepared, about to verify indexes");

    for item in indexes.tables().owner.safe_iter() {
        let (key, info): (OwnerIndexKey, OwnerIndexInfo) = item?;
        let calculated_info = owner_index.remove(&key).ok_or_else(|| {
            anyhow!(
                "owner_index: found extra, unexpected entry {:?}",
                (&key, &info)
            )
        })?;

        if calculated_info != info {
            bail!(
                "owner_index: entry {key:?} is different: expected {calculated_info:?} found {info:?}"
            );
        }
    }

    if !owner_index.is_empty() {
        bail!("owner_index: is missing entries: {owner_index:?}");
    }
    tracing::info!("Owner index is good");

    info!("Finished running index verification checks");

    Ok(())
}
