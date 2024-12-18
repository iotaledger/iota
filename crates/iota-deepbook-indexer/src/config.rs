// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::env;

use serde::{Deserialize, Serialize};

/// config as loaded from `config.yaml`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IndexerConfig {
    pub remote_store_url: String,
    #[serde(default = "default_db_url")]
    pub db_url: String,
    /// Only provide this if you use a colocated FN
    pub checkpoints_path: Option<String>,
    pub iota_rpc_url: String,
    pub deepbook_package_id: String,
    pub deepbook_genesis_checkpoint: u64,
    pub concurrency: u64,
    pub metric_port: u16,
    pub service_port: u16,
}

impl iota_config::Config for IndexerConfig {}

pub fn default_db_url() -> String {
    env::var("DB_URL").expect("db_url must be set in config or via the $DB_URL env var")
}
