// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This module includes types and useful conversions

use core::str;
use std::{fmt::Display, str::FromStr, sync::Arc};

use iota_storage::http_key_value_store::Key;
use serde::Deserialize;

use crate::{errors::ApiError, services::KvStoreService};

/// Represents a shared instance of the KVStore service, primerely used by the
/// REST API server
pub type SharedKvStoreService = Arc<KvStoreService>;

/// Represent the supported items the REST API accepts when fetching the data
/// based on Digest or Sequence number
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum ItemType {
    Tx,
    Fx,
    Events,
    CheckpointContents,
    CheckpointSummary,
    CheckpointContentsByDigest,
    CheckpointSummaryByDigest,
    TxToCheckpoint,
    ObjectKey,
}

impl Display for ItemType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ItemType::Tx => "tx",
            ItemType::Fx => "fx",
            ItemType::Events => "ev",
            ItemType::CheckpointContents | ItemType::CheckpointContentsByDigest => "cc",
            ItemType::CheckpointSummary | ItemType::CheckpointSummaryByDigest => "cs",
            ItemType::TxToCheckpoint => "tx2c",
            ItemType::ObjectKey => "ob",
        }
        .fmt(f)
    }
}

impl FromStr for ItemType {
    type Err = ApiError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "tx" => Ok(Self::Tx),
            "fx" => Ok(Self::Fx),
            "ev" => Ok(Self::Events),
            "cc" => Ok(Self::CheckpointContents),
            "cs" => Ok(Self::CheckpointSummary),
            "tx2c" => Ok(Self::TxToCheckpoint),
            "ob" => Ok(Self::ObjectKey),
            _ => Err(ApiError::BadRequest(format!("invalid item type: {s}"))),
        }
    }
}

impl From<Key> for ItemType {
    fn from(value: Key) -> Self {
        match value {
            Key::Tx(_) => Self::Tx,
            Key::Fx(_) => Self::Fx,
            Key::Events(_) => Self::Events,
            Key::CheckpointContents(_) => Self::CheckpointContents,
            Key::CheckpointSummary(_) => Self::CheckpointSummary,
            Key::CheckpointContentsByDigest(_) => Self::CheckpointContentsByDigest,
            Key::CheckpointSummaryByDigest(_) => Self::CheckpointSummaryByDigest,
            Key::TxToCheckpoint(_) => Self::TxToCheckpoint,
            Key::ObjectKey(_, _) => Self::ObjectKey,
        }
    }
}
