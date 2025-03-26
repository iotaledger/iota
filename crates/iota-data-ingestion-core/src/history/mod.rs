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
//! Ingestion Source History Directory Layout
//! ```text
//!  - ingestion/
//!     - historical/
//!          - MANIFEST
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
//! MANIFEST File Disk Format
//! ┌──────────────────────────────┐
//! │        magic<4 byte>         │
//! ├──────────────────────────────┤
//! │   serialized manifest        │
//! ├──────────────────────────────┤
//! │      sha3 <32 bytes>         │
//! └──────────────────────────────┘
//! ```

pub mod manifest;
pub mod reader;

pub const CHECKPOINT_FILE_MAGIC: u32 = 0x0000BEEF;
pub const CHECKPOINT_FILE_SUFFIX: &str = "chk";
const HISTORICAL_DIR_NAME: &str = "historical";
const INGESTION_DIR_NAME: &str = "ingestion";
pub const MAGIC_BYTES: usize = 4;
pub const MANIFEST_FILE_MAGIC: u32 = 0x0000FACE;
pub const MANIFEST_FILENAME: &str = "MANIFEST";
