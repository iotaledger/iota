// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Maintain the sequence number of the last checkpoint of each epoch.

use std::{collections::BTreeMap, ops::RangeBounds, time::Duration};

use bytes::Bytes;
use iota_storage::object_store::{ObjectStoreGetExt, util::get};
use iota_types::{committee::EpochId, messages_checkpoint::CheckpointSequenceNumber};
use object_store::{ObjectStore, PutMode, path::Path};
use serde::{Deserialize, Serialize};

use crate::{
    IngestionError,
    errors::IngestionResult as Result,
    history::{
        EPOCH_BOUNDARIES_FILE_MAGIC, EPOCH_BOUNDARIES_FILENAME, finalize_magic_blob,
        read_magic_blob,
    },
};

const GET_TIMEOUT_SECS: u64 = 5;

/// Stores the epoch boundaries.
///
/// The representation stored is a map between the epoch and the sequence number
/// of the respective last checkpoint.
///
/// # Examples
///
/// ```
/// # use iota_data_ingestion_core::history::epoch_boundaries::EpochBoundaries;
/// let boundaries: EpochBoundaries = [(0, 5), (1, 100), (2, 1000)].into_iter().collect();
/// assert_eq!(boundaries.get(1), Some(100));
/// // The last checkpoints of a range of epochs, in epoch order.
/// assert_eq!(
///     boundaries.range(..2).collect::<Vec<_>>(),
///     vec![(0, 5), (1, 100)]
/// );
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize, Eq, PartialEq)]
pub struct EpochBoundaries(BTreeMap<EpochId, CheckpointSequenceNumber>);

impl FromIterator<(EpochId, CheckpointSequenceNumber)> for EpochBoundaries {
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = (EpochId, CheckpointSequenceNumber)>,
    {
        Self(iter.into_iter().collect())
    }
}

impl EpochBoundaries {
    /// Returns the boundary of the given epoch.
    pub fn get(&self, epoch: EpochId) -> Option<CheckpointSequenceNumber> {
        self.0.get(&epoch).copied()
    }

    /// Returns the recorded `(epoch, last checkpoint)` pairs for the epochs in
    /// `range`, in epoch order.
    pub fn range(
        &self,
        range: impl RangeBounds<EpochId>,
    ) -> impl Iterator<Item = (EpochId, CheckpointSequenceNumber)> + '_ {
        self.0
            .range(range)
            .map(|(&epoch, &boundary)| (epoch, boundary))
    }

    /// Returns whether the given epoch has a recorded boundary.
    pub fn contains(&self, epoch: EpochId) -> bool {
        self.0.contains_key(&epoch)
    }

    /// Inserts a new epoch boundary, keeping the recorded epochs contiguous.
    /// Any existing boundary for the same epoch is overwritten.
    ///
    /// # Errors
    ///
    /// Fails if the previous epoch has not been already recorded.
    pub fn insert_next(
        &mut self,
        epoch: EpochId,
        boundary: CheckpointSequenceNumber,
    ) -> Result<()> {
        if epoch > 0 && !self.contains(epoch - 1) {
            return Err(IngestionError::EpochBoundary(format!(
                "epoch {epoch} just ended but its predecessor is not recorded"
            )));
        }
        self.0.insert(epoch, boundary);
        Ok(())
    }

    /// The relative path of the file with the serialized boundaries.
    pub fn file_path() -> Path {
        Path::from(EPOCH_BOUNDARIES_FILENAME)
    }
}

/// Reads the epoch boundaries from the store.
///
/// # Errors
///
/// Fails if the file cannot be fetched, of if it fails to decode.
pub async fn read_epoch_boundaries<S: ObjectStoreGetExt>(
    remote_store: S,
) -> Result<EpochBoundaries> {
    let bytes = tokio::time::timeout(
        Duration::from_secs(GET_TIMEOUT_SECS),
        get(&remote_store, &EpochBoundaries::file_path()),
    )
    .await
    .map_err(|e| IngestionError::EpochBoundary(e.to_string()))??;
    read_epoch_boundaries_from_bytes(bytes.to_vec())
}

/// Decodes epoch boundaries from the given byte vector and verifies their
/// integrity.
///
/// # Errors
///
/// Fails if the magic byte or the trailing SHA3-256 checksum does not match.
pub fn read_epoch_boundaries_from_bytes(vec: Vec<u8>) -> Result<EpochBoundaries> {
    read_magic_blob(vec, EPOCH_BOUNDARIES_FILE_MAGIC, EPOCH_BOUNDARIES_FILENAME)
}

/// Encodes the epoch boundaries with its magic byte and a trailing SHA3-256
/// checksum.
pub fn finalize_epoch_boundaries(boundaries: &EpochBoundaries) -> Result<Bytes> {
    finalize_magic_blob(boundaries, EPOCH_BOUNDARIES_FILE_MAGIC)
}

/// Writes the epoch boundaries to the store atomically.
///
///
///
/// # Errors
///
/// Fails if the encoding fails, if the [`PutMode`] invariants are not upheld,
/// or for any other reason the upload might fail.
pub async fn write_epoch_boundaries<S: ObjectStore>(
    boundaries: &EpochBoundaries,
    remote_store: S,
    put_mode: PutMode,
) -> Result<()> {
    let bytes = finalize_epoch_boundaries(boundaries)?;
    remote_store
        .put_opts(&EpochBoundaries::file_path(), bytes.into(), put_mode.into())
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{IngestionError, history::MAGIC_BYTES};

    fn sample() -> EpochBoundaries {
        [(0, 5), (1, 100), (2, 1000)].into_iter().collect()
    }

    #[test]
    fn insert_next_enforces_contiguity() {
        let mut boundaries = EpochBoundaries::default();
        // The first recorded epoch must be 0.
        assert!(matches!(
            boundaries.insert_next(1, 50),
            Err(IngestionError::EpochBoundary(_))
        ));
        boundaries.insert_next(0, 5).unwrap();
        boundaries.insert_next(1, 100).unwrap();
        // A gap is rejected.
        assert!(boundaries.insert_next(3, 200).is_err());
    }

    #[test]
    fn write_read() {
        for boundaries in [EpochBoundaries::default(), sample()] {
            let bytes = finalize_epoch_boundaries(&boundaries).unwrap();
            assert_eq!(
                read_epoch_boundaries_from_bytes(bytes.to_vec()).unwrap(),
                boundaries
            );
        }
    }

    #[test]
    fn rejects_wrong_magic() {
        let mut bytes = finalize_epoch_boundaries(&sample()).unwrap().to_vec();
        bytes[0] ^= 0xFF;
        assert!(matches!(
            read_epoch_boundaries_from_bytes(bytes),
            Err(IngestionError::HistoryRead(_))
        ));
    }

    #[test]
    fn rejects_corrupted_content() {
        let mut bytes = finalize_epoch_boundaries(&sample()).unwrap().to_vec();
        // Flip a byte in the encoded body, past the 4-byte magic.
        bytes[MAGIC_BYTES + 1] ^= 0xFF;
        assert!(matches!(
            read_epoch_boundaries_from_bytes(bytes),
            Err(IngestionError::HistoryRead(_))
        ));
    }
}
