// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This module includes types and useful conversions.

use core::str;
use std::{fmt::Display, sync::Arc};

use iota_storage::http_key_value_store::Key;
use serde::Deserialize;

use crate::kv_store_client::KvStoreClient;

/// Represents a shared instance of the [`KvStoreClient`], primerely used by the
/// REST API server global [`State`](axum::extract::State).
pub type SharedKvStoreClient = Arc<KvStoreClient>;

/// Represent the supported items the REST API accepts when fetching the data
/// based on Digest or Sequence number.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum ItemType {
    #[serde(rename = "tx")]
    Tx,
    #[serde(rename = "fx")]
    Fx,
    #[serde(rename = "cc")]
    CheckpointContents,
    #[serde(rename = "cs")]
    CheckpointSummary,
    #[serde(rename = "tx2c")]
    TxToCheckpoint,
    #[serde(rename = "ob")]
    ObjectKey,
    #[serde(rename = "evtx")]
    EventsByTxDigest,
}

impl Display for ItemType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ItemType::Tx => "tx",
            ItemType::Fx => "fx",
            ItemType::CheckpointContents => "cc",
            ItemType::CheckpointSummary => "cs",
            ItemType::TxToCheckpoint => "tx2c",
            ItemType::ObjectKey => "ob",
            ItemType::EventsByTxDigest => "evtx",
        }
        .fmt(f)
    }
}

impl From<Key> for ItemType {
    fn from(value: Key) -> Self {
        match value {
            Key::Tx(_) => Self::Tx,
            Key::Fx(_) => Self::Fx,
            Key::CheckpointContents(_) => Self::CheckpointContents,
            Key::CheckpointSummary(_) | Key::CheckpointSummaryByDigest(_) => {
                Self::CheckpointSummary
            }
            Key::TxToCheckpoint(_) => Self::TxToCheckpoint,
            Key::ObjectKey(_, _) => Self::ObjectKey,
            Key::EventsByTxDigest(_) => Self::EventsByTxDigest,
        }
    }
}
