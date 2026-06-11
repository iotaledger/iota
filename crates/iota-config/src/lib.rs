// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[cfg(not(target_arch = "wasm32"))]
pub mod certificate_deny_config;
#[cfg(not(target_arch = "wasm32"))]
pub mod genesis;
#[cfg(not(target_arch = "wasm32"))]
pub mod local_ip_utils;
#[cfg(not(target_arch = "wasm32"))]
pub mod migration_tx_data;
#[cfg(not(target_arch = "wasm32"))]
pub mod node;
#[cfg(not(target_arch = "wasm32"))]
pub mod node_config_metrics;
#[cfg(not(target_arch = "wasm32"))]
pub mod object_storage_config;
#[cfg(not(target_arch = "wasm32"))]
pub mod p2p;
// Filesystem-backed config helpers (the `Config` / `PersistedConfig` traits and
// path utilities). Node-only, so the whole module is gated out on wasm.
#[cfg(not(target_arch = "wasm32"))]
mod persisted_config;
// `transaction_deny_config` + `verifier_signing_config` are the two modules
// the execution path needs at sign-time. Everything else pulls in network /
// filesystem / config-file machinery and is gated.
pub mod transaction_deny_config;
pub mod validator_client_monitor_config;
pub mod verifier_signing_config;

#[cfg(not(target_arch = "wasm32"))]
pub use node::{ConsensusConfig, ExecutionCacheConfig, NodeConfig, WritebackCacheConfig};
#[cfg(not(target_arch = "wasm32"))]
pub use persisted_config::{
    Config, PersistedConfig, genesis_blob_exists, iota_config_dir, ssfn_config_file,
    validator_config_file,
};

pub const IOTA_CONFIG_DIR: &str = "iota_config";
pub const IOTA_NETWORK_CONFIG: &str = "network.yaml";
pub const IOTA_FULLNODE_CONFIG: &str = "fullnode.yaml";
pub const IOTA_CLIENT_CONFIG: &str = "client.yaml";
pub const IOTA_KEYSTORE_FILENAME: &str = "iota.keystore";
pub const IOTA_BENCHMARK_GENESIS_GAS_KEYSTORE_FILENAME: &str = "benchmark.keystore";
pub const IOTA_GENESIS_FILENAME: &str = "genesis.blob";
pub const IOTA_GENESIS_MIGRATION_TX_DATA_FILENAME: &str = "migration.blob";
pub const IOTA_DEV_NET_URL: &str = "https://api.devnet.iota.cafe:443";

pub const AUTHORITIES_DB_NAME: &str = "authorities_db";
pub const CONSENSUS_DB_NAME: &str = "consensus_db";
pub const FULL_NODE_DB_PATH: &str = "full_node_db";
