// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Data source abstractions for checkpoint ingestion.

use std::path::PathBuf;

/// Represents different sources of checkpoint data for ingestion.
#[derive(Debug, Clone)]
pub enum DataSource {
    /// Local directory containing checkpoint files.
    Local(PathBuf),
    /// Remote source via REST API or object store.
    Remote {
        store_url: String,
        store_options: Vec<(String, String)>,
    },
    /// gRPC endpoint for streaming checkpoints.
    Grpc { url: String },
}

impl DataSource {
    /// Creates a local data source.
    pub fn local<P: Into<PathBuf>>(path: P) -> Self {
        Self::Local(path.into())
    }

    /// Creates a remote data source with store URL.
    pub fn remote<S: Into<String>>(store_url: S) -> Self {
        Self::Remote {
            store_url: store_url.into(),
            store_options: vec![],
        }
    }

    /// Creates a remote data source with store URL and options.
    pub fn remote_with_options<S: Into<String>>(
        store_url: S,
        store_options: Vec<(String, String)>,
    ) -> Self {
        Self::Remote {
            store_url: store_url.into(),
            store_options,
        }
    }

    /// Creates a gRPC data source.
    pub fn grpc<S: Into<String>>(url: S) -> Self {
        Self::Grpc { url: url.into() }
    }

    /// Returns true if this is a gRPC data source.
    pub fn is_grpc(&self) -> bool {
        matches!(self, Self::Grpc { .. })
    }

    /// Returns true if this is a local data source.
    pub fn is_local(&self) -> bool {
        matches!(self, Self::Local(_))
    }

    /// Returns true if this is a remote data source.
    pub fn is_remote(&self) -> bool {
        matches!(self, Self::Remote { .. })
    }
}
