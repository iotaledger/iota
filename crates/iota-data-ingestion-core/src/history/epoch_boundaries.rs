// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Maintain the sequence number of the last checkpoint of each epoch.

use std::{collections::BTreeMap, ops::RangeBounds};

use bytes::Bytes;
use fastcrypto::hash::{HashFunction, Sha3_256};
use iota_storage::{
    SHA3_BYTES,
    blob::{Blob, BlobEncoding},
    object_store::{
        ObjectStoreGetExt, ObjectStorePutExt,
        util::{exists, get, put},
    },
};
use iota_types::{committee::EpochId, messages_checkpoint::CheckpointSequenceNumber};
use object_store::path::Path;
use serde::{Deserialize, Serialize};

use crate::{
    IngestionError,
    errors::IngestionResult as Result,
    history::{EPOCH_BOUNDARIES_FILE_MAGIC, EPOCH_BOUNDARIES_FILENAME, MAGIC_BYTES},
};

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
/// If the remote file is not found, this returns an empty collection.
///
/// # Errors
///
/// Fails if the file fails to decode.
pub async fn read_epoch_boundaries_or_default<S: ObjectStoreGetExt>(
    remote_store: S,
) -> Result<EpochBoundaries> {
    if !exists(&remote_store, &EpochBoundaries::file_path()).await {
        return Ok(Default::default());
    }
    let bytes = get(&remote_store, &EpochBoundaries::file_path()).await?;
    read_epoch_boundaries_from_bytes(bytes.to_vec())
}

/// Decodes epoch boundaries from the given byte vector and verifies their
/// integrity.
///
/// # Errors
///
/// Fails if the magic byte or the trailing SHA3-256 checksum does not match.
pub fn read_epoch_boundaries_from_bytes(vec: Vec<u8>) -> Result<EpochBoundaries> {
    let file_size = vec.len();
    let mut reader = Cursor::new(vec);

    // Reads from the beginning of the file and verifies the magic byte.
    reader.rewind()?;
    let magic = reader.read_u32::<BigEndian>()?;
    if magic != EPOCH_BOUNDARIES_FILE_MAGIC {
        return Err(IngestionError::HistoryRead(format!(
            "unexpected magic byte in epoch boundaries: {magic}",
        )));
    }

    // Reads the SHA3 checksum stored at the end of the file.
    reader.seek(SeekFrom::End(-(SHA3_BYTES as i64)))?;
    let mut sha3_digest = [0u8; SHA3_BYTES];
    reader.read_exact(&mut sha3_digest)?;

    // Reads the content and verifies it against the stored checksum.
    reader.rewind()?;
    let mut content_buf = vec![0u8; file_size - SHA3_BYTES];
    reader.read_exact(&mut content_buf)?;
    let mut hasher = Sha3_256::default();
    hasher.update(&content_buf);
    let computed_digest = hasher.finalize().digest;
    if computed_digest != sha3_digest {
        return Err(IngestionError::HistoryRead(format!(
            "epoch boundaries corrupted, computed checksum: {computed_digest:?}, stored checksum: {sha3_digest:?}"
        )));
    }
    reader.rewind()?;
    reader.seek(SeekFrom::Start(MAGIC_BYTES as u64))?;
    Ok(Blob::read(&mut reader)?.decode()?)
}

/// Encodes the epoch boundaries with its magic byte and a trailing SHA3-256
/// checksum.
pub fn finalize_epoch_boundaries(boundaries: &EpochBoundaries) -> Result<Bytes> {
    let mut buf = BufWriter::new(vec![]);
    buf.write_u32::<BigEndian>(EPOCH_BOUNDARIES_FILE_MAGIC)?;
    let blob = Blob::encode(boundaries, BlobEncoding::Bcs)?;
    blob.write(&mut buf)?;
    buf.flush()?;
    let mut hasher = Sha3_256::default();
    hasher.update(buf.get_ref());
    let computed_digest = hasher.finalize().digest;
    buf.write_all(&computed_digest)?;
    Ok(Bytes::from(buf.into_inner().map_err(|e| e.into_error())?))
}

/// Writes the epoch boundaries to the store.
///
/// # Errors
///
/// Fails if encoding or the upload fails.
pub async fn write_epoch_boundaries<S: ObjectStorePutExt>(
    boundaries: &EpochBoundaries,
    remote_store: S,
) -> Result<()> {
    let bytes = finalize_epoch_boundaries(boundaries)?;
    put(&remote_store, &EpochBoundaries::file_path(), bytes).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
