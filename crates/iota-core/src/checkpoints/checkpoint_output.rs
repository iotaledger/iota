// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use async_trait::async_trait;
use iota_types::{
    base_types::AuthorityName,
    error::IotaResult,
    message_envelope::Message,
    messages_checkpoint::{
        CertifiedCheckpointSummary, CheckpointContents, CheckpointSignatureMessage,
        CheckpointSummary, SignedCheckpointSummary, VerifiedCheckpoint,
    },
    messages_consensus::ConsensusTransaction,
};
use tracing::{debug, info, instrument, trace};

use super::{CheckpointMetrics, CheckpointStore};
use crate::{
    authority::{StableSyncAuthoritySigner, authority_per_epoch_store::AuthorityPerEpochStore},
    consensus_adapter::SubmitToConsensus,
    epoch::reconfiguration::ReconfigurationInitiator,
};

const REPORT_END_OF_EPOCH_MARGIN_MS: u64 = 2000;
const MIN_CHECKPOINTS_BETWEEN_REPORTS: u64 = 1000;
const MAX_CHECKPOINT_LAG_FOR_REPORT: u64 = 100;
#[async_trait]
pub trait CheckpointOutput: Sync + Send + 'static {
    async fn checkpoint_created(
        &self,
        summary: &CheckpointSummary,
        contents: &CheckpointContents,
        epoch_store: &Arc<AuthorityPerEpochStore>,
        checkpoint_store: &Arc<CheckpointStore>,
    ) -> IotaResult;
}

#[async_trait]
pub trait CertifiedCheckpointOutput: Sync + Send + 'static {
    async fn certified_checkpoint_created(
        &self,
        summary: &CertifiedCheckpointSummary,
    ) -> IotaResult;
}

pub struct SubmitCheckpointToConsensus<T> {
    pub sender: T,
    pub signer: StableSyncAuthoritySigner,
    pub authority: AuthorityName,
    pub next_reconfiguration_timestamp_ms: u64,
    pub metrics: Arc<CheckpointMetrics>,
}

pub struct LogCheckpointOutput;

impl LogCheckpointOutput {
    pub fn boxed() -> Box<dyn CheckpointOutput> {
        Box::new(Self)
    }

    pub fn boxed_certified() -> Box<dyn CertifiedCheckpointOutput> {
        Box::new(Self)
    }
}

#[async_trait]
impl<T: SubmitToConsensus + ReconfigurationInitiator> CheckpointOutput
    for SubmitCheckpointToConsensus<T>
{
    #[instrument(level = "debug", skip_all)]
    async fn checkpoint_created(
        &self,
        summary: &CheckpointSummary,
        contents: &CheckpointContents,
        epoch_store: &Arc<AuthorityPerEpochStore>,
        checkpoint_store: &Arc<CheckpointStore>,
    ) -> IotaResult {
        LogCheckpointOutput
            .checkpoint_created(summary, contents, epoch_store, checkpoint_store)
            .await?;

        let checkpoint_timestamp = summary.timestamp_ms;
        let checkpoint_seq = summary.sequence_number;
        self.metrics.checkpoint_creation_latency.observe(
            summary
                .timestamp()
                .elapsed()
                .unwrap_or_default()
                .as_secs_f64(),
        );

        let highest_verified_checkpoint = checkpoint_store
            .get_highest_verified_checkpoint()?
            .map(|x| *x.sequence_number());

        if Some(checkpoint_seq) > highest_verified_checkpoint {
            debug!(
                "Sending checkpoint signature at sequence {checkpoint_seq} to consensus, timestamp {checkpoint_timestamp}.
                {}ms left till end of epoch at timestamp {}",
                self.next_reconfiguration_timestamp_ms.saturating_sub(checkpoint_timestamp), self.next_reconfiguration_timestamp_ms
            );

            let summary = SignedCheckpointSummary::new(
                epoch_store.epoch(),
                summary.clone(),
                &*self.signer,
                self.authority,
            );

            let message = CheckpointSignatureMessage { summary };
            let transaction = ConsensusTransaction::new_checkpoint_signature_message(message);
            self.sender
                .submit_to_consensus(&[transaction], epoch_store)?;
            self.metrics
                .last_sent_checkpoint_signature
                .set(checkpoint_seq as i64);
        } else {
            debug!(
                "Checkpoint at sequence {checkpoint_seq} is already certified, skipping signature submission to consensus",
            );
            self.metrics
                .last_skipped_checkpoint_signature_submission
                .set(checkpoint_seq as i64);
        }

        // If `calculate_validator_scores` is enabled in protocol config, we also send
        // misbehavior reports to consensus at this point. Misbehavior reports
        // containing proofs of misbehaviour can be sent whenever the misbehavior is
        // detected, but we choose to send the ones that include only unprovable counts
        // at this point, due to periodicity reasons and to ensure a (approximate)
        // synchronization with the score updates.
        //
        // Reports are rate-limited: only sent when metrics have changed (different
        // summaries) and at least 1000 checkpoints have passed since the last report.
        // We also require that the checkpoint for which we want to send the report is
        // at most 100 checkpoints behind the highest verified checkpoint, to avoid
        // sending reports during resync.
        //
        // Additionally to these periodic reports, we also send a report when the epoch
        // is coming to an end. Since `close_epoch` is called according to local clocks,
        // we use an analogous rule for the last reports, requiring that the checkpoint
        // is close to the next reconfiguration timestamp.
        let should_send_last_report = checkpoint_timestamp
            >= self
                .next_reconfiguration_timestamp_ms
                .saturating_sub(REPORT_END_OF_EPOCH_MARGIN_MS)
            && !epoch_store.scorer.has_sent_end_of_epoch_report();
        if epoch_store.protocol_config().calculate_validator_scores()
            && ((checkpoint_seq.saturating_sub(epoch_store.scorer.last_report_checkpoint_seq())
                >= MIN_CHECKPOINTS_BETWEEN_REPORTS
                && Some(checkpoint_seq + MAX_CHECKPOINT_LAG_FOR_REPORT)
                    >= highest_verified_checkpoint)
                || should_send_last_report)
        {
            let misbehavior_report = epoch_store
                .scorer
                .generate_report_with_current_local_metrics();
            let new_report_summary = misbehavior_report.summary();
            if new_report_summary != epoch_store.scorer.last_report_summary()
                || should_send_last_report
            {
                let transaction = ConsensusTransaction::new_misbehavior_report(
                    epoch_store.name,
                    &misbehavior_report,
                    checkpoint_seq,
                );
                info!(?transaction, "submitting misbehavior report to consensus");
                self.sender
                    .submit_to_consensus(&[transaction], epoch_store)?;
                epoch_store
                    .scorer
                    .store_last_report_summary(new_report_summary);
                epoch_store
                    .scorer
                    .store_last_report_checkpoint_seq(checkpoint_seq);
                if should_send_last_report {
                    epoch_store.scorer.mark_end_of_epoch_report_sent();
                }
            }
        }

        if checkpoint_timestamp >= self.next_reconfiguration_timestamp_ms {
            // close_epoch is ok if called multiple times
            self.sender.close_epoch(epoch_store);
        }
        Ok(())
    }
}

#[async_trait]
impl CheckpointOutput for LogCheckpointOutput {
    async fn checkpoint_created(
        &self,
        summary: &CheckpointSummary,
        contents: &CheckpointContents,
        _epoch_store: &Arc<AuthorityPerEpochStore>,
        _checkpoint_store: &Arc<CheckpointStore>,
    ) -> IotaResult {
        trace!(
            "Including following transactions in checkpoint {}: {:?}",
            summary.sequence_number, contents
        );
        info!(
            "Creating checkpoint {:?} at epoch {}, sequence {}, previous digest {:?}, transactions count {}, content digest {:?}, end_of_epoch_data {:?}",
            summary.digest(),
            summary.epoch,
            summary.sequence_number,
            summary.previous_digest,
            contents.size(),
            summary.content_digest,
            summary.end_of_epoch_data,
        );

        Ok(())
    }
}

#[async_trait]
impl CertifiedCheckpointOutput for LogCheckpointOutput {
    async fn certified_checkpoint_created(
        &self,
        summary: &CertifiedCheckpointSummary,
    ) -> IotaResult {
        debug!(
            "Certified checkpoint with sequence {} and digest {}",
            summary.sequence_number,
            summary.digest()
        );
        Ok(())
    }
}

pub struct SendCheckpointToStateSync {
    handle: iota_network::state_sync::Handle,
}

impl SendCheckpointToStateSync {
    pub fn new(handle: iota_network::state_sync::Handle) -> Self {
        Self { handle }
    }
}

#[async_trait]
impl CertifiedCheckpointOutput for SendCheckpointToStateSync {
    #[instrument(level = "trace", name = "checkpoint_created_from_consensus", skip_all)]
    async fn certified_checkpoint_created(
        &self,
        summary: &CertifiedCheckpointSummary,
    ) -> IotaResult {
        debug!(
            "Certified checkpoint with sequence {} and digest {}",
            summary.sequence_number,
            summary.digest()
        );
        self.handle
            .send_checkpoint(VerifiedCheckpoint::new_unchecked(summary.to_owned()))
            .await;

        Ok(())
    }
}
