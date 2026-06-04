// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
//! Handle historical checkpoint data.
//!
//! Full checkpoint data for epochs starting from genesis are persisted in
//! batches as blob files in a remote store.
//!
//! Files are optionally compressed with the zstd
//! compression format. Filenames follow the format <checkpoint_seq_num>.chk
//! where `checkpoint_seq_num` is the first checkpoint present in that
//! file. MANIFEST is the index and source of truth for all files present in the
//! ingestion source history.
//!
//! EPOCH_BOUNDARIES holds the map between the epochs and the sequence number of
//! the respective last checkpoint. This allows reading directly the last
//! checkpoints from the store, which is useful for verification purposes.
//!
//! Ingestion Source History Directory Layout
//! ```text
//!  - ingestion/
//!     - historical/
//!          - MANIFEST
//!          - EPOCH_BOUNDARIES
//!          - 0.chk
//!          - 1000.chk
//!          - 3000.chk
//!          - ...
//!          - 100000.chk
//!
//! Blob File Disk Format
//! ┌──────────────────────────────┐
//! │       magic <4 byte>         │
//! ├──────────────────────────────┤
//! │  storage format <1 byte>     │
//! ├──────────────────────────────┤
//! │    file compression <1 byte> │
//! ├──────────────────────────────┤
//! │ ┌──────────────────────────┐ │
//! │ │         Blob 1           │ │
//! │ ├──────────────────────────┤ │
//! │ │          ...             │ │
//! │ ├──────────────────────────┤ │
//! │ │        Blob N            │ │
//! │ └──────────────────────────┘ │
//! └──────────────────────────────┘
//! Blob
//! ┌───────────────┬───────────────────┬──────────────┐
//! │ len <uvarint> │ encoding <1 byte> │ data <bytes> │
//! └───────────────┴───────────────────┴──────────────┘
//!
//! MANIFEST and EPOCH_BOUNDARIES File Disk Format
//! ┌──────────────────────────────┐
//! │        magic<4 byte>         │
//! ├──────────────────────────────┤
//! │   serialized contents        │
//! ├──────────────────────────────┤
//! │      sha3 <32 bytes>         │
//! └──────────────────────────────┘
//! ```

use std::io::{BufWriter, Cursor, Read, Seek, SeekFrom, Write};

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use bytes::Bytes;
use fastcrypto::hash::{HashFunction, Sha3_256};
use iota_storage::{
    SHA3_BYTES,
    blob::{Blob, BlobEncoding},
};
use serde::{Serialize, de::DeserializeOwned};

use crate::{IngestionError, errors::IngestionResult as Result};

pub mod epoch_boundaries;
pub mod manifest;
pub mod reader;

pub const CHECKPOINT_FILE_MAGIC: u32 = 0x0000BEEF;
pub const CHECKPOINT_FILE_SUFFIX: &str = "chk";
pub const MAGIC_BYTES: usize = 4;
pub const MANIFEST_FILE_MAGIC: u32 = 0x0000FACE;
pub const MANIFEST_FILENAME: &str = "MANIFEST";
pub const EPOCH_BOUNDARIES_FILE_MAGIC: u32 = 0x0000E90C;
pub const EPOCH_BOUNDARIES_FILENAME: &str = "EPOCH_BOUNDARIES";

/// Encodes `value` as a BCS [`Blob`] framed with a 4-byte `magic` prefix and a
/// trailing SHA3-256 checksum computed over the magic and the blob.
///
/// This is the on-disk format shared by the MANIFEST and EPOCH_BOUNDARIES
/// files.
pub(crate) fn finalize_magic_blob<T: Serialize>(value: &T, magic: u32) -> Result<Bytes> {
    let mut buf = BufWriter::new(vec![]);
    buf.write_u32::<BigEndian>(magic)?;
    Blob::encode(value, BlobEncoding::Bcs)?.write(&mut buf)?;
    buf.flush()?;
    let mut hasher = Sha3_256::default();
    hasher.update(buf.get_ref());
    let computed_digest = hasher.finalize().digest;
    buf.write_all(&computed_digest)?;
    Ok(Bytes::from(buf.into_inner().map_err(|e| e.into_error())?))
}

/// Decodes a value written by [`finalize_magic_blob`] and verifies its
/// integrity.
///
/// `filename` names the file in error messages (e.g. `"manifest"`).
///
/// # Errors
///
/// Fails if the `magic` prefix or the trailing SHA3-256 checksum does not
/// match.
pub(crate) fn read_magic_blob<T: DeserializeOwned>(
    vec: Vec<u8>,
    magic: u32,
    filename: &str,
) -> Result<T> {
    let file_size = vec.len();
    let mut reader = Cursor::new(vec);

    // Reads from the beginning of the file and verifies the magic byte.
    let found = reader.read_u32::<BigEndian>()?;
    if found != magic {
        return Err(IngestionError::HistoryRead(format!(
            "unexpected magic byte in {filename}: {found}",
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
            "{filename} corrupted, computed checksum: {computed_digest:?}, stored checksum: {sha3_digest:?}"
        )));
    }

    reader.seek(SeekFrom::Start(MAGIC_BYTES as u64))?;
    Ok(Blob::read(&mut reader)?.decode()?)
}
