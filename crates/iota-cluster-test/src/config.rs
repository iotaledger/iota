// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{fmt, path::PathBuf};

use clap::*;
use iota_genesis_builder::SnapshotUrl;
use regex::Regex;

#[derive(Parser, Clone, ValueEnum, Debug)]
pub enum Env {
    Devnet,
    Testnet,
    CustomRemote,
    NewLocal,
}

#[derive(derive_more::Debug, Parser)]
#[clap(name = "", rename_all = "kebab-case")]
pub struct ClusterTestOpt {
    #[clap(value_enum)]
    pub env: Env,
    #[clap(long)]
    pub faucet_address: Option<String>,
    #[clap(long)]
    pub fullnode_address: Option<String>,
    #[clap(long)]
    pub epoch_duration_ms: Option<u64>,
    /// URL for the indexer RPC server
    #[clap(long)]
    pub indexer_address: Option<String>,
    /// URL for the Indexer Postgres DB
    #[clap(long)]
    #[debug("{}", ObfuscatedPgAddress(pg_address))]
    pub pg_address: Option<String>,
    #[clap(long)]
    pub config_dir: Option<PathBuf>,
    /// URL for the indexer RPC server
    #[clap(long)]
    pub graphql_address: Option<String>,
    /// Locations for local migration snapshots.
    #[clap(long, name = "path")]
    #[arg(num_args(0..))]
    pub local_migration_snapshots: Vec<PathBuf>,
    /// Remote migration snapshots.
    #[clap(long, name = "iota|<full-url>")]
    #[arg(num_args(0..))]
    pub remote_migration_snapshots: Vec<SnapshotUrl>,
}

// This is not actually dead, but rust thinks it is because it is only used in
// the derive macro above.
#[allow(dead_code)]
struct ObfuscatedPgAddress<'a>(&'a Option<String>);

impl std::fmt::Display for ObfuscatedPgAddress<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            None => write!(f, "None"),
            Some(val) => {
                write!(
                    f,
                    "{}",
                    Regex::new(r":.*@")
                        .unwrap()
                        .replace_all(val.as_str(), ":*****@")
                )
            }
        }
    }
}

impl ClusterTestOpt {
    pub fn new_local() -> Self {
        Self {
            env: Env::NewLocal,
            faucet_address: None,
            fullnode_address: None,
            epoch_duration_ms: None,
            indexer_address: None,
            pg_address: None,
            config_dir: None,
            graphql_address: None,
            local_migration_snapshots: Default::default(),
            remote_migration_snapshots: Default::default(),
        }
    }
}
