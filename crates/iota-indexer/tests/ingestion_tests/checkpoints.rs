// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use iota_indexer::{
    errors::{Context, IndexerError},
    models::checkpoints::StoredCheckpoint,
    schema::checkpoints,
};

use crate::{
    common::indexer_wait_for_checkpoint,
    utils::{TestSim, read_only_blocking},
};

#[tokio::test]
pub async fn test_epoch_boundary() -> Result<(), IndexerError> {
    let sim = TestSim::new("ingestion_epoch_boundary");
    sim.execute_transfer();
    sim.inner.create_checkpoint(); // checkpoint 1
    sim.inner.advance_epoch(false); // checkpoint 2 and epoch 1
    sim.execute_transfer();
    sim.inner.create_checkpoint(); // checkpoint 3

    let pg_store = sim.start_indexer(None).await;
    indexer_wait_for_checkpoint(&pg_store, 3).await;

    let db_checkpoint = read_only_blocking!(&pg_store.blocking_cp(), |conn| {
        checkpoints::table
            .order(checkpoints::sequence_number.desc())
            .first::<StoredCheckpoint>(conn)
    })
    .context("failed to read checkpoint")?;
    assert_eq!(db_checkpoint.sequence_number, 3);
    assert_eq!(db_checkpoint.epoch, 1);
    Ok(())
}
