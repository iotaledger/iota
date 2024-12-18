// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use iota_types::messages_checkpoint::CheckpointSequenceNumber;

use crate::backfill::{
    BackfillTaskKind, IngestionBackfillKind,
    backfill_instances::ingestion_backfills::{
        digest_task::DigestBackfill, ingestion_backfill_task::IngestionBackfillTask,
        raw_checkpoints::RawCheckpointsBackFill, tx_affected_objects::TxAffectedObjectsBackfill,
    },
    backfill_task::BackfillTask,
};

mod ingestion_backfills;
mod sql_backfill;
mod system_state_summary_json;

pub async fn get_backfill_task(
    kind: BackfillTaskKind,
    range_start: usize,
) -> Arc<dyn BackfillTask> {
    match kind {
        BackfillTaskKind::SystemStateSummaryJson => {
            Arc::new(system_state_summary_json::SystemStateSummaryJsonBackfill)
        }
        BackfillTaskKind::Sql { sql, key_column } => {
            Arc::new(sql_backfill::SqlBackFill::new(sql, key_column))
        }
        BackfillTaskKind::Ingestion {
            kind,
            remote_store_url,
        } => match kind {
            IngestionBackfillKind::Digest => Arc::new(
                IngestionBackfillTask::<DigestBackfill>::new(
                    remote_store_url,
                    range_start as CheckpointSequenceNumber,
                )
                .await,
            ),
            IngestionBackfillKind::RawCheckpoints => Arc::new(
                IngestionBackfillTask::<RawCheckpointsBackFill>::new(
                    remote_store_url,
                    range_start as CheckpointSequenceNumber,
                )
                .await,
            ),
            IngestionBackfillKind::TxAffectedObjects => Arc::new(
                IngestionBackfillTask::<TxAffectedObjectsBackfill>::new(
                    remote_store_url,
                    range_start as CheckpointSequenceNumber,
                )
                .await,
            ),
        },
    }
}
