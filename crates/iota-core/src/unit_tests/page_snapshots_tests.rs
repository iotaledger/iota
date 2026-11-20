// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeSet, sync::Arc, time::Duration};

use iota_config::node::PageSnapshotConfig;
use iota_node_storage::GrpcIndexes;
use iota_sdk_types::ObjectId;
use iota_types::storage::{DynamicFieldKey, PageSnapshotRef};
use prometheus_filtered::Registry;

use super::{PageSnapshot, PageSnapshots};
use crate::rpc_indexes::{IndexGroup, RpcIndexesStore};

const IDLE_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_LIFETIME: Duration = Duration::from_secs(30);

fn open_store(path: &std::path::Path) -> RpcIndexesStore {
    RpcIndexesStore::new_without_init(path.to_path_buf(), None, BTreeSet::from([IndexGroup::Grpc]))
}

fn page_snapshots(store: &RpcIndexesStore, max_snapshots: usize) -> Arc<PageSnapshots> {
    PageSnapshots::new(
        store.tables.watermark.db.clone(),
        &PageSnapshotConfig {
            idle_timeout_ms: IDLE_TIMEOUT.as_millis() as u64,
            max_lifetime_ms: MAX_LIFETIME.as_millis() as u64,
            max_snapshots,
        },
        &Registry::new(),
    )
}

/// Writes `fields` of `parent` and moves the watermark to `checkpoint` in one
/// batch, as indexing a checkpoint does.
fn index_checkpoint(
    store: &RpcIndexesStore,
    checkpoint: u64,
    parent: ObjectId,
    fields: &[ObjectId],
) {
    let tables = &store.tables;
    let mut batch = tables.watermark.batch();
    batch
        .insert_batch(
            &tables.dynamic_field,
            fields
                .iter()
                .map(|id| (DynamicFieldKey::new(parent, *id), ())),
        )
        .unwrap();
    batch
        .insert_batch(&tables.watermark, [((), checkpoint)])
        .unwrap();
    batch.write().unwrap();
}

fn acquire(
    store: &RpcIndexesStore,
    snapshots: &PageSnapshots,
    requested: Option<PageSnapshotRef>,
) -> PageSnapshot {
    snapshots
        .acquire(requested, &store.tables.watermark)
        .unwrap()
}

/// The fields of `parent` that `page` reads.
fn fields(store: &RpcIndexesStore, page: &PageSnapshot, parent: ObjectId) -> BTreeSet<ObjectId> {
    store
        .tables
        .dynamic_field
        .safe_iter_with_prefix_after_at_shared_snapshot::<_, ObjectId>(
            page.snapshot.clone(),
            &parent,
            None,
        )
        .map(|row| row.unwrap().0.field_id)
        .collect()
}

fn checkpoint_of(page: &PageSnapshot) -> Option<u64> {
    page.reference.map(|reference| reference.checkpoint)
}

#[tokio::test(start_paused = true)]
async fn pages_read_from_the_snapshot_their_token_names() {
    let dir = iota_common::tempdir();
    let store = open_store(dir.path());
    let snapshots = page_snapshots(&store, 16);
    let parent = ObjectId::random();
    let (a, b, c) = (ObjectId::random(), ObjectId::random(), ObjectId::random());

    index_checkpoint(&store, 1, parent, &[a, b]);
    let first = acquire(&store, &snapshots, None);
    assert_eq!(checkpoint_of(&first), Some(1));

    index_checkpoint(&store, 2, parent, &[c]);
    let next = acquire(&store, &snapshots, first.reference);
    assert_eq!(next.reference, first.reference);
    assert_eq!(fields(&store, &next, parent), BTreeSet::from([a, b]));

    let new_query = acquire(&store, &snapshots, None);
    assert_eq!(checkpoint_of(&new_query), Some(2));
    assert_eq!(
        fields(&store, &new_query, parent),
        BTreeSet::from([a, b, c])
    );
}

#[tokio::test(start_paused = true)]
async fn queries_starting_at_the_same_checkpoint_share_a_snapshot() {
    let dir = iota_common::tempdir();
    let store = open_store(dir.path());
    let snapshots = page_snapshots(&store, 16);
    index_checkpoint(&store, 1, ObjectId::random(), &[ObjectId::random()]);

    let first = acquire(&store, &snapshots, None);
    let second = acquire(&store, &snapshots, None);
    assert!(Arc::ptr_eq(&first.snapshot, &second.snapshot));
    assert_eq!(snapshots.kept_checkpoints(), vec![1]);
}

#[tokio::test(start_paused = true)]
async fn nothing_indexed_keeps_no_snapshot() {
    let dir = iota_common::tempdir();
    let store = open_store(dir.path());
    let snapshots = page_snapshots(&store, 16);

    assert_eq!(acquire(&store, &snapshots, None).reference, None);
    assert!(snapshots.kept_checkpoints().is_empty());
}

#[tokio::test(start_paused = true)]
async fn an_unextended_snapshot_expires_after_the_idle_timeout() {
    let dir = iota_common::tempdir();
    let store = open_store(dir.path());
    let snapshots = page_snapshots(&store, 16);
    let parent = ObjectId::random();
    index_checkpoint(&store, 1, parent, &[ObjectId::random()]);
    let first = acquire(&store, &snapshots, None);
    index_checkpoint(&store, 2, parent, &[ObjectId::random()]);

    // Reading a page that is not followed by another one does not extend it.
    tokio::time::advance(IDLE_TIMEOUT / 2).await;
    assert_eq!(
        checkpoint_of(&acquire(&store, &snapshots, first.reference)),
        Some(1)
    );
    tokio::time::advance(IDLE_TIMEOUT / 2).await;
    assert_eq!(
        checkpoint_of(&acquire(&store, &snapshots, first.reference)),
        Some(2)
    );
}

#[tokio::test(start_paused = true)]
async fn extending_keeps_a_snapshot_up_to_its_maximum_lifetime() {
    let dir = iota_common::tempdir();
    let store = open_store(dir.path());
    let snapshots = page_snapshots(&store, 16);
    let parent = ObjectId::random();
    index_checkpoint(&store, 1, parent, &[ObjectId::random()]);
    let first = acquire(&store, &snapshots, None);
    let reference = first.reference.unwrap();
    index_checkpoint(&store, 2, parent, &[ObjectId::random()]);

    let step = IDLE_TIMEOUT - Duration::from_secs(1);
    let mut elapsed = Duration::ZERO;
    while elapsed + step < MAX_LIFETIME {
        tokio::time::advance(step).await;
        elapsed += step;
        let page = acquire(&store, &snapshots, Some(reference));
        assert_eq!(checkpoint_of(&page), Some(1), "after {elapsed:?}");
        snapshots.extend(reference);
    }
    tokio::time::advance(step).await;
    assert_eq!(
        checkpoint_of(&acquire(&store, &snapshots, Some(reference))),
        Some(2)
    );
}

#[tokio::test(start_paused = true)]
async fn a_token_from_another_node_reads_a_new_snapshot() {
    let dir = iota_common::tempdir();
    let store = open_store(dir.path());
    let snapshots = page_snapshots(&store, 16);
    let parent = ObjectId::random();
    index_checkpoint(&store, 1, parent, &[ObjectId::random()]);
    let reference = acquire(&store, &snapshots, None).reference.unwrap();
    index_checkpoint(&store, 2, parent, &[ObjectId::random()]);

    let foreign = PageSnapshotRef {
        node: reference.node.wrapping_add(1),
        ..reference
    };
    let page = acquire(&store, &snapshots, Some(foreign));
    assert_eq!(
        page.reference,
        Some(PageSnapshotRef {
            node: reference.node,
            checkpoint: 2,
        })
    );
}

#[tokio::test(start_paused = true)]
async fn at_the_cap_a_new_query_reads_the_latest_state_without_keeping_it() {
    let dir = iota_common::tempdir();
    let store = open_store(dir.path());
    let snapshots = page_snapshots(&store, 1);
    let parent = ObjectId::random();
    let (a, b) = (ObjectId::random(), ObjectId::random());
    index_checkpoint(&store, 1, parent, &[a]);
    acquire(&store, &snapshots, None);
    index_checkpoint(&store, 2, parent, &[b]);

    let page = acquire(&store, &snapshots, None);
    assert_eq!(page.reference, None);
    assert_eq!(fields(&store, &page, parent), BTreeSet::from([a, b]));
    assert_eq!(snapshots.kept_checkpoints(), vec![1]);
}

#[tokio::test(start_paused = true)]
async fn expired_snapshots_are_dropped_without_further_requests() {
    let dir = iota_common::tempdir();
    let store = open_store(dir.path());
    let snapshots = page_snapshots(&store, 16);
    index_checkpoint(&store, 1, ObjectId::random(), &[ObjectId::random()]);
    acquire(&store, &snapshots, None);
    assert_eq!(snapshots.kept_checkpoints(), vec![1]);

    tokio::time::sleep(IDLE_TIMEOUT * 2).await;
    assert!(snapshots.kept_checkpoints().is_empty());
}

#[tokio::test(start_paused = true)]
async fn the_store_reads_paginated_queries_from_one_snapshot() {
    let dir = iota_common::tempdir();
    let store = open_store(dir.path());
    let parent = ObjectId::random();
    let (a, b) = (ObjectId::random(), ObjectId::random());
    index_checkpoint(&store, 1, parent, &[a]);

    let first = store.dynamic_field_page_iter(parent, None, None).unwrap();
    let reference = first.snapshot;
    assert_eq!(reference.map(|r| r.checkpoint), Some(1));
    drop(first);
    index_checkpoint(&store, 2, parent, &[b]);

    let read = |requested| -> BTreeSet<ObjectId> {
        store
            .dynamic_field_page_iter(parent, None, requested)
            .unwrap()
            .iter
            .map(|key| key.unwrap().field_id)
            .collect()
    };
    assert_eq!(read(reference), BTreeSet::from([a]));
    assert_eq!(read(None), BTreeSet::from([a, b]));
    assert_eq!(
        store
            .account_owned_objects_info_page_iter(
                iota_sdk_types::Address::random(),
                None,
                None,
                reference
            )
            .unwrap()
            .snapshot,
        reference
    );
}
