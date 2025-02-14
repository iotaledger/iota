// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This module includes types and useful conversions.

use std::sync::Arc;

use crate::kv_store_client::KvStoreClient;

/// Represents a shared instance of the [`KvStoreClient`], primerely used by the
/// REST API server global [`State`](axum::extract::State).
pub type SharedKvStoreClient = Arc<KvStoreClient>;
