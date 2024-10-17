// Copyright (c) 2021, Facebook, Inc. and its affiliates
// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#![warn(
    future_incompatible,
    nonstandard_style,
    rust_2018_idioms,
    rust_2021_compatibility
)]
#![allow(clippy::mutable_key_type)]

use std::{
    collections::{BTreeMap, HashSet},
    fs::{self, OpenOptions},
    io::{BufWriter, Write as _},
    num::NonZeroU32,
};

use crypto::{NetworkPublicKey, PublicKey};
use fastcrypto::traits::EncodeDecodeBase64;
use iota_network_stack::Multiaddr;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use thiserror::Error;

pub mod committee;
pub use committee::*;
pub mod utils;

/// The epoch number.
pub type Epoch = u64;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Node {0} is not in the committee")]
    NotInCommittee(String),

    #[error("Node {0} is not in the worker cache")]
    NotInWorkerCache(String),

    #[error("Unknown worker id {0}")]
    UnknownWorker(WorkerId),

    #[error("Failed to read config file '{file}': {message}")]
    ImportError { file: String, message: String },

    #[error("Failed to write config file '{file}': {message}")]
    ExportError { file: String, message: String },
}

pub trait Import: DeserializeOwned {
    fn import(path: &str) -> Result<Self, ConfigError> {
        let reader = || -> Result<Self, std::io::Error> {
            let data = fs::read(path)?;
            Ok(serde_json::from_slice(data.as_slice())?)
        };
        reader().map_err(|e| ConfigError::ImportError {
            file: path.to_string(),
            message: e.to_string(),
        })
    }
}

impl<D: DeserializeOwned> Import for D {}

pub trait Export: Serialize {
    fn export(&self, path: &str) -> Result<(), ConfigError> {
        let writer = || -> Result<(), std::io::Error> {
            let file = OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(path)?;
            let mut writer = BufWriter::new(file);
            let data = serde_json::to_string_pretty(self).unwrap();
            writer.write_all(data.as_ref())?;
            writer.write_all(b"\n")?;
            Ok(())
        };
        writer().map_err(|e| ConfigError::ExportError {
            file: path.to_string(),
            message: e.to_string(),
        })
    }
}

impl<S: Serialize> Export for S {}

// TODO: This actually represents voting power (out of 10,000) and not amount
// staked. Consider renaming to `VotingPower`.
pub type Stake = u64;
pub type WorkerId = u32;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct AnemoParameters {
    /// Per-peer rate-limits (in requests/sec) for the PrimaryToPrimary service.
    pub send_certificate_rate_limit: Option<NonZeroU32>,

    /// Per-peer rate-limits (in requests/sec) for the WorkerToWorker service.
    pub report_batch_rate_limit: Option<NonZeroU32>,
    pub request_batches_rate_limit: Option<NonZeroU32>,

    /// Size in bytes above which network messages are considered excessively
    /// large. Excessively large messages will still be handled, but logged
    /// and reported in metrics for debugging.
    ///
    /// If unspecified, this will default to 8 MiB.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub excessive_message_size: Option<usize>,
}

impl AnemoParameters {
    // By default, at most 10 certificates can be sent concurrently to a peer.
    pub fn send_certificate_rate_limit(&self) -> u32 {
        self.send_certificate_rate_limit
            .unwrap_or(NonZeroU32::new(20).unwrap())
            .get()
    }

    // By default, at most 100 batches can be broadcasted concurrently.
    pub fn report_batch_rate_limit(&self) -> u32 {
        self.report_batch_rate_limit
            .unwrap_or(NonZeroU32::new(200).unwrap())
            .get()
    }

    // As of 11/02/2023, when one worker is actively fetching, each peer receives
    // 20~30 requests per second.
    pub fn request_batches_rate_limit(&self) -> u32 {
        self.request_batches_rate_limit
            .unwrap_or(NonZeroU32::new(100).unwrap())
            .get()
    }

    pub fn excessive_message_size(&self) -> usize {
        const EXCESSIVE_MESSAGE_SIZE: usize = 8 << 20;

        self.excessive_message_size
            .unwrap_or(EXCESSIVE_MESSAGE_SIZE)
    }
}

#[derive(Clone, Serialize, Deserialize, Eq, Hash, PartialEq, Debug)]
pub struct WorkerInfo {
    /// The public key of this worker.
    pub name: NetworkPublicKey,
    /// Address to receive client transactions (WAN).
    pub transactions: Multiaddr,
    /// Address to receive messages from other workers (WAN) and our primary.
    pub worker_address: Multiaddr,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct WorkerIndex(pub BTreeMap<WorkerId, WorkerInfo>);

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct WorkerCache {
    /// The authority to worker index.
    pub workers: BTreeMap<PublicKey, WorkerIndex>,
    /// The epoch number for workers
    pub epoch: Epoch,
}

impl std::fmt::Display for WorkerIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "WorkerIndex {:?}",
            self.0
                .iter()
                .map(|(key, value)| { format!("{}:{:?}", key, value) })
                .collect::<Vec<_>>()
        )
    }
}

impl std::fmt::Display for WorkerCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "WorkerCache E{}: {:?}",
            self.epoch(),
            self.workers
                .iter()
                .map(|(k, v)| {
                    if let Some(x) = k.encode_base64().get(0..16) {
                        format!("{}: {}", x, v)
                    } else {
                        format!("Invalid key: {}", k)
                    }
                })
                .collect::<Vec<_>>()
        )
    }
}

impl WorkerCache {
    pub const DEFAULT_FILENAME: &'static str = "workers.json";

    /// Returns the current epoch.
    pub fn epoch(&self) -> Epoch {
        self.epoch
    }

    /// Returns the addresses of a specific worker (`id`) of a specific
    /// authority (`to`).
    pub fn worker(&self, to: &PublicKey, id: &WorkerId) -> Result<WorkerInfo, ConfigError> {
        self.workers
            .iter()
            .find_map(|v| match_opt::match_opt!(v, (name, authority) if name == to => authority))
            .ok_or_else(|| {
                ConfigError::NotInWorkerCache(ToString::to_string(&(*to).encode_base64()))
            })?
            .0
            .iter()
            .find(|(worker_id, _)| worker_id == &id)
            .map(|(_, worker)| worker.clone())
            .ok_or_else(|| ConfigError::NotInWorkerCache((*to).encode_base64()))
    }

    /// Returns the addresses of all our workers.
    pub fn our_workers(&self, myself: &PublicKey) -> Result<Vec<WorkerInfo>, ConfigError> {
        let res = self
            .workers
            .iter()
            .find_map(
                |v| match_opt::match_opt!(v, (name, authority) if name == myself => authority),
            )
            .ok_or_else(|| ConfigError::NotInWorkerCache((*myself).encode_base64()))?
            .0
            .values()
            .cloned()
            .collect();
        Ok(res)
    }

    /// Returns the addresses of all known workers.
    pub fn all_workers(&self) -> Vec<(NetworkPublicKey, Multiaddr)> {
        self.workers
            .iter()
            .flat_map(|(_, w)| {
                w.0.values()
                    .map(|w| (w.name.clone(), w.worker_address.clone()))
            })
            .collect()
    }

    /// Returns the addresses of all workers with a specific id except the ones
    /// of the authority specified by `myself`.
    pub fn others_workers_by_id(
        &self,
        myself: &PublicKey,
        id: &WorkerId,
    ) -> Vec<(PublicKey, WorkerInfo)> {
        self.workers
            .iter()
            .filter(|(name, _)| *name != myself )
            .flat_map(
                |(name, authority)|  authority.0.iter().flat_map(
                    |v| match_opt::match_opt!(v,(worker_id, addresses) if worker_id == id => (name.clone(), addresses.clone()))))
            .collect()
    }

    /// Returns the addresses of all workers that are not of our node.
    pub fn others_workers(&self, myself: &PublicKey) -> Vec<(PublicKey, WorkerInfo)> {
        self.workers
            .iter()
            .filter(|(name, _)| *name != myself)
            .flat_map(|(name, authority)| authority.0.iter().map(|v| (name.clone(), v.1.clone())))
            .collect()
    }

    /// Return the network addresses that are present in the current worker
    /// cache that are from a primary key that are no longer in the
    /// committee. Current committee keys provided as an argument.
    pub fn network_diff(&self, keys: Vec<&PublicKey>) -> HashSet<&Multiaddr> {
        self.workers
            .iter()
            .filter(|(name, _)| !keys.contains(name))
            .flat_map(|(_, authority)| {
                authority
                    .0
                    .values()
                    .map(|address| &address.transactions)
                    .chain(authority.0.values().map(|address| &address.worker_address))
            })
            .collect()
    }
}
