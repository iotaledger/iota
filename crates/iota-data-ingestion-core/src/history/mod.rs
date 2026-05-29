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
