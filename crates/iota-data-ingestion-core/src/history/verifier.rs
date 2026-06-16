// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Verify the last checkpoint of an epoch against the committee of that epoch.
//!
//! The committee of an epoch is recorded only in the last checkpoint of the
//! previous epoch, as `EndOfEpochData::next_epoch_committee`. Verifying the
//! last checkpoint of epoch `N` therefore requires the committee taken from the
//! last checkpoint of epoch `N - 1`, which must itself be verified the same
//! way going all the way back to the genesis committee.
//!
//! [`EpochBoundaryVerifier`] runs this verification given a starting committee
//! (the genesis committee) after resolving the epoch boundaries from the remote
//! store. Each checkpoint is fetched into memory from the remote store and
//! dropped once verified. The verified checkpoints are exposed as a [`Stream`],
//! so callers can consume each epoch's checkpoint as soon as it is verified.
//!
//! The most prominent use of this logic is the verification of formal
//! snapshots. This is done by comparing the elliptic-curve multiset hash (ECMH)
//! of the live objects included in the snapshot against the
//! [`CheckpointCommitment`](iota_types::messages_checkpoint::CheckpointCommitment)
//! stored in the last checkpoint of the respective epoch.

use futures::{Stream, stream::TryStreamExt};
use iota_config::genesis::Genesis;
use iota_types::{
    committee::{Committee, CommitteeChainVerifier, EpochId},
    messages_checkpoint::{
        CertifiedCheckpointSummary, CheckpointSequenceNumber, VerifiedCheckpoint,
    },
};

use crate::{
    IngestionError,
    errors::IngestionResult as Result,
    history::{epoch_boundaries::EpochBoundaries, reader::HistoricalReader},
};

/// Verifies the last checkpoint of each listed epoch against the committee of
/// that epoch.
///
/// A verifier is defined by a starting committee and the sequence numbers of
/// the checkpoints to verify; the committee advances as each checkpoint is
/// verified.
///
/// # Examples
///
/// ```ignore
/// use iota_config::{genesis::Genesis, node::ArchiveReaderConfig};
/// use iota_data_ingestion_core::history::{reader::HistoricalReader, verifier::EpochBoundaryVerifier};
///
/// let reader = HistoricalReader::new(config)?;
/// let genesis = Genesis::load(genesis_path)?;
///
/// let target_epoch = 1000;
/// let verifier = EpochBoundaryVerifier::from_genesis(reader, &genesis, target_epoch).await?;
/// let target = verifier.verify_target_epoch_boundary().await?;
/// println!("verified last checkpoint of epoch {}", target.epoch());
/// ```
pub struct EpochBoundaryVerifier {
    reader: HistoricalReader,
    /// The committee-chain walk; its committee is the one expected to have
    /// signed the next checkpoint to verify.
    chain_verifier: CommitteeChainVerifier,
    /// The epoch boundaries stored in the remote store.
    epoch_boundaries: EpochBoundaries,
    /// The final epoch to verify.
    target_epoch: EpochId,
}

impl EpochBoundaryVerifier {
    /// Creates a verifier from a starting committee for the target epoch.
    ///
    /// # Errors
    ///
    /// Fails if the target epoch precedes the starting committee's epoch, or if
    /// the epoch boundaries cannot be read from the remote store.
    pub async fn new(
        reader: HistoricalReader,
        starting_committee: Committee,
        target_epoch: EpochId,
    ) -> Result<Self> {
        if target_epoch < starting_committee.epoch {
            return Err(IngestionError::Verification(format!(
                "target epoch {target_epoch} precedes the starting committee's epoch {}",
                starting_committee.epoch
            )));
        }
        let epoch_boundaries = reader.epoch_boundaries().await?;
        Ok(Self {
            reader,
            chain_verifier: CommitteeChainVerifier::new(starting_committee),
            epoch_boundaries,
            target_epoch,
        })
    }

    /// Creates a verifier for the target epoch, whose starting committee is the
    /// genesis committee.
    ///
    /// # Errors
    ///
    /// Fails if the epoch boundaries cannot be read from the remote store.
    pub async fn from_genesis(
        reader: HistoricalReader,
        genesis: &Genesis,
        target_epoch: EpochId,
    ) -> Result<Self> {
        let committee = genesis.committee().map_err(|e| {
            IngestionError::Verification(format!("failed to load genesis committee: {e}"))
        })?;
        Self::new(reader, committee, target_epoch).await
    }

    /// Verifies the last checkpoint of the given epoch.
    ///
    /// This consumes the verifier, draining the stream returned by
    /// [`Self::stream_verified_checkpoints`].
    ///
    /// # Errors
    ///
    /// Fails if [`Self::stream_verified_checkpoints`] fails.
    pub async fn verify_target_epoch_boundary(self) -> Result<VerifiedCheckpoint> {
        let last = self
            .stream_verified_checkpoints()
            .await?
            .try_fold(None, |_, verified| async move { Ok(Some(verified)) })
            .await?
            .expect("stream guarantees to yield at least one checkpoint if successful");

        Ok(last)
    }

    /// Streams the verified last checkpoints from the starting committee's
    /// epoch up to the target epoch of the verifier.
    ///
    /// This consumes the verifier. Each checkpoint is fetched into memory and
    /// verified only when the stream is polled. Upon successful verification
    /// the committee is advanced to verify the last checkpoint of the
    /// next epoch.
    ///
    /// # Errors
    ///
    /// Generating the stream only fails if the MANIFEST in the remote store
    /// cannot be synced.
    ///
    /// The stream returns an error in the following occasions:
    ///
    /// * If the last checkpoint of the next epoch is not recorded in the epoch
    ///   boundaries.
    /// * If the summary cannot be fetched from the remote store
    /// * If signature verification fails
    /// * If the checkpoint is not the last checkpoint of the epoch
    pub async fn stream_verified_checkpoints(
        self,
    ) -> Result<impl Stream<Item = Result<VerifiedCheckpoint>> + Send> {
        // Refresh the manifest once before fetching; the per-checkpoint fetches
        // read from the cached manifest.
        self.reader.sync_manifest_once().await?;

        Ok(futures::stream::try_unfold(
            self,
            |mut verifier| async move {
                let Some(verified) = verifier.verify_next().await? else {
                    return Ok(None);
                };
                Ok(Some((verified, verifier)))
            },
        ))
    }

    /// Verifies the checkpoint of the next epoch in queue.
    ///
    /// In order to do so, the checkpoint summary is fetched from the remote
    /// store and it is verified against the active committee of the
    /// corresponding epoch.
    ///
    /// The method returns [`None`] if the target epoch has been already
    /// verified.
    ///
    /// Otherwise it returns the verified checkpoint, advancing the committee
    /// chain to the next epoch.
    ///
    /// # Errors
    ///
    /// Fails in the following occasions:
    ///
    /// * If the last checkpoint of the next epoch is not recorded in the epoch
    ///   boundaries.
    /// * If the summary cannot be fetched from the remote store
    /// * If signature verification fails
    /// * If the checkpoint is not the last checkpoint of the epoch
    async fn verify_next(&mut self) -> Result<Option<VerifiedCheckpoint>> {
        let epoch_to_verify = self.chain_verifier.epoch();
        if epoch_to_verify > self.target_epoch {
            return Ok(None);
        }
        let Some(sequence_number) = self.epoch_boundaries.get(epoch_to_verify) else {
            return Err(IngestionError::EpochBoundary(format!(
                "did not find epoch boundary for epoch {epoch_to_verify}"
            )));
        };

        let summary = self.fetch_summary(sequence_number).await?;

        let verified = self
            .chain_verifier
            .verify_epoch_close(summary)
            .map_err(|e| {
                IngestionError::Verification(format!(
                    "failed to verify checkpoint {sequence_number} as the close of epoch \
                 {epoch_to_verify}: {e}"
                ))
            })?;

        Ok(Some(verified))
    }

    /// Fetches a single checkpoint summary into memory.
    ///
    /// The method downloads the file with the respective batch of checkpoints,
    /// and then gets the summary from the full-checkpoint data that match the
    /// requested `sequence_number`.
    ///
    /// # Errors
    ///
    /// Fails if the checkpoint cannot be read from the remote store, or if
    /// it is not found.
    async fn fetch_summary(
        &self,
        sequence_number: CheckpointSequenceNumber,
    ) -> Result<CertifiedCheckpointSummary> {
        self.reader
            .iter_for_range(sequence_number..sequence_number + 1)
            .await?
            .next()
            .map(|data| data.checkpoint_summary)
            .ok_or_else(|| {
                IngestionError::HistoryRead(format!(
                    "checkpoint {sequence_number} not found in remote store"
                ))
            })
    }
}
