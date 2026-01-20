// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This module includes types and useful conversions.

use std::{num::NonZeroUsize, sync::Arc};

use crate::bigtable::KvStoreClient;

/// Represents a shared instance of the application state, used by the
/// REST API server global [`State`](axum::extract::State).
pub type SharedRestServerAppState = Arc<RestServerAppState>;

pub struct RestServerAppState {
    pub kv_store_client: Arc<KvStoreClient>,
    pub multiget_max_items: NonZeroUsize,
}
