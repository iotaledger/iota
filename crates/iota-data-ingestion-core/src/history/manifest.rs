// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Handle the manifest for historical checkpoint data.
//!
//! MANIFEST File Disk Format
//! ┌──────────────────────────────┐
//! │        magic<4 byte>         │
//! ├──────────────────────────────┤
//! │   serialized manifest        │
//! ├──────────────────────────────┤
//! │      sha3 <32 bytes>         │
//! └──────────────────────────────┘

use std::{
    io::{BufWriter, Cursor, Read, Seek, SeekFrom, Write},
    num::NonZeroUsize,
    ops::Range,
};

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use bytes::Bytes;
use fastcrypto::hash::{HashFunction, Sha3_256};
use iota_config::{
    node::ArchiveReaderConfig as HistoricalReaderConfig, object_storage_config::ObjectStoreConfig,
};
use iota_storage::{
    SHA3_BYTES,
    blob::{Blob, BlobEncoding},
    compute_sha3_checksum, compute_sha3_checksum_for_bytes,
    object_store::{
        ObjectStoreGetExt, ObjectStorePutExt,
        util::{get, put},
    },
};
use object_store::path::Path;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::{
    IngestionError,
    errors::IngestionResult as Result,
    history::{
        CHECKPOINT_FILE_SUFFIX, HISTORICAL_DIR_NAME, INGESTION_DIR_NAME, MAGIC_BYTES,
        MANIFEST_FILE_MAGIC, MANIFEST_FILENAME, reader::HistoricalReader,
    },
};

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct FileMetadata {
    pub checkpoint_seq_range: Range<u64>,
    pub sha3_digest: [u8; 32],
}

impl FileMetadata {
    pub fn file_path(&self) -> Path {
        Path::from(INGESTION_DIR_NAME)
            .child(HISTORICAL_DIR_NAME)
            .child(format!(
                "{}.{CHECKPOINT_FILE_SUFFIX}",
                self.checkpoint_seq_range.start
            ))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct ManifestV1 {
    pub archive_version: u8,
    pub next_checkpoint_seq_num: u64,
    pub file_metadata: Vec<FileMetadata>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
#[non_exhaustive]
pub enum Manifest {
    V1(ManifestV1),
}

impl Manifest {
    pub fn new(next_checkpoint_seq_num: u64) -> Self {
        Manifest::V1(ManifestV1 {
            archive_version: 1,
            next_checkpoint_seq_num,
            file_metadata: vec![],
        })
    }

    pub fn to_files(&self) -> Vec<FileMetadata> {
        match self {
            Manifest::V1(manifest) => manifest.file_metadata.clone(),
        }
    }

    pub fn next_checkpoint_seq_num(&self) -> u64 {
        match self {
            Manifest::V1(manifest) => manifest.next_checkpoint_seq_num,
        }
    }

    pub fn update(&mut self, checkpoint_sequence_number: u64, file_metadata: FileMetadata) {
        match self {
            Manifest::V1(manifest) => {
                manifest.file_metadata.push(file_metadata);
                manifest.next_checkpoint_seq_num = checkpoint_sequence_number;
            }
        }
    }

    pub fn file_path() -> Path {
        Path::from(INGESTION_DIR_NAME)
            .child(HISTORICAL_DIR_NAME)
            .child(MANIFEST_FILENAME)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct CheckpointUpdates {
    file_metadata: FileMetadata,
    manifest: Manifest,
}

impl CheckpointUpdates {
    pub fn new(
        checkpoint_sequence_number: u64,
        file_metadata: FileMetadata,
        manifest: &mut Manifest,
    ) -> Self {
        manifest.update(checkpoint_sequence_number, file_metadata.clone());
        CheckpointUpdates {
            file_metadata,
            manifest: manifest.clone(),
        }
    }

    pub fn file_path(&self) -> Path {
        self.file_metadata.file_path()
    }

    pub fn manifest_file_path(&self) -> Path {
        Path::from(MANIFEST_FILENAME)
    }
}

pub fn create_file_metadata(
    file_path: &std::path::Path,
    checkpoint_seq_range: Range<u64>,
) -> Result<FileMetadata> {
    let sha3_digest = compute_sha3_checksum(file_path)?;
    let file_metadata = FileMetadata {
        checkpoint_seq_range,
        sha3_digest,
    };
    Ok(file_metadata)
}

pub fn create_file_metadata_from_bytes(
    contents: Bytes,
    checkpoint_seq_range: Range<u64>,
) -> Result<FileMetadata> {
    let sha3_digest = compute_sha3_checksum_for_bytes(contents)?;
    let file_metadata = FileMetadata {
        checkpoint_seq_range,
        sha3_digest,
    };
    Ok(file_metadata)
}

/// Reads the manifest file from the store.
pub async fn read_manifest<S: ObjectStoreGetExt>(remote_store: S) -> Result<Manifest> {
    let vec = get(&remote_store, &Manifest::file_path()).await?.to_vec();
    read_manifest_from_bytes(vec)
}

/// Reads the manifest file from the given byte vector and verifies the
/// integrity of the file.
pub fn read_manifest_from_bytes(vec: Vec<u8>) -> Result<Manifest> {
    let manifest_file_size = vec.len();
    let mut manifest_reader = Cursor::new(vec);

    // Reads from the beginning of the file and verifies the magic byte
    // is MANIFEST_FILE_MAGIC
    manifest_reader.rewind()?;
    let magic = manifest_reader.read_u32::<BigEndian>()?;
    if magic != MANIFEST_FILE_MAGIC {
        return Err(IngestionError::HistoryRead(format!(
            "unexpected magic byte in manifest: {magic}",
        )));
    }

    // Reads from the end of the file and gets the SHA3 checksum
    // of the content.
    manifest_reader.seek(SeekFrom::End(-(SHA3_BYTES as i64)))?;
    let mut sha3_digest = [0u8; SHA3_BYTES];
    manifest_reader.read_exact(&mut sha3_digest)?;

    // Reads the content of the file and verifies the SHA3 checksum
    // of the content matches the stored checksum.
    manifest_reader.rewind()?;
    let mut content_buf = vec![0u8; manifest_file_size - SHA3_BYTES];
    manifest_reader.read_exact(&mut content_buf)?;
    let mut hasher = Sha3_256::default();
    hasher.update(&content_buf);
    let computed_digest = hasher.finalize().digest;
    if computed_digest != sha3_digest {
        return Err(IngestionError::HistoryRead(format!(
            "manifest corrupted, computed checksum: {computed_digest:?}, stored checksum: {sha3_digest:?}"
        )));
    }
    manifest_reader.rewind()?;
    manifest_reader.seek(SeekFrom::Start(MAGIC_BYTES as u64))?;
    Ok(Blob::read(&mut manifest_reader)?.decode()?)
}

/// Computes the SHA3 checksum of the Manifest and writes it to a byte vector.
pub fn finalize_manifest(manifest: Manifest) -> Result<Bytes> {
    let mut buf = BufWriter::new(vec![]);
    buf.write_u32::<BigEndian>(MANIFEST_FILE_MAGIC)?;
    let blob = Blob::encode(&manifest, BlobEncoding::Bcs)?;
    blob.write(&mut buf)?;
    buf.flush()?;
    let mut hasher = Sha3_256::default();
    hasher.update(buf.get_ref());
    let computed_digest = hasher.finalize().digest;
    buf.write_all(&computed_digest)?;
    Ok(Bytes::from(buf.into_inner().map_err(|e| e.into_error())?))
}

/// Writes the Manifest to the remote store.
pub async fn write_manifest<S: ObjectStorePutExt>(
    manifest: Manifest,
    remote_store: S,
) -> Result<()> {
    let bytes = finalize_manifest(manifest)?;
    put(&remote_store, &Manifest::file_path(), bytes).await?;
    Ok(())
}

pub async fn verify_historical_checkpoints_with_checksums(
    remote_store_config: ObjectStoreConfig,
    concurrency: usize,
) -> Result<()> {
    let config = HistoricalReaderConfig {
        remote_store_config,
        download_concurrency: NonZeroUsize::new(concurrency).unwrap(),
        use_for_pruning_watermark: false,
    };
    // Gets the Manifest from the remote store.
    let reader = HistoricalReader::new(config)?;
    reader.sync_manifest_once().await?;
    let manifest = reader.get_manifest().await;
    info!(
        "next checkpoint in archive store: {}",
        manifest.next_checkpoint_seq_num()
    );

    let file_metadata = reader.verify_and_get_manifest_files(manifest)?;

    // Account for both summary and content files
    let num_files = file_metadata.len() * 2;
    reader.verify_file_consistency(file_metadata).await?;
    info!("all {num_files} files are valid");
    Ok(())
}
