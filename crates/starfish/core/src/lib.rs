// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod authority_node;
mod authority_service;
mod base_committer;
mod block_header;
mod block_manager;
mod block_verifier;
mod commit;
mod commit_consumer;
mod commit_observer;
mod commit_syncer;
mod commit_vote_monitor;
mod context;
mod core;
mod core_thread;
mod dag_state;
mod error;
mod leader_schedule;
mod leader_scoring;
mod leader_timeout;
mod linearizer;
mod metrics;
#[cfg(not(msim))]
mod network;
#[cfg(msim)]
pub mod network;

mod header_synchronizer;
mod stake_aggregator;
mod storage;
mod subscriber;
mod threshold_clock;
#[cfg(not(msim))]
mod transaction;
#[cfg(msim)]
pub mod transaction;
mod transaction_ref;
mod transactions_synchronizer;

mod universal_committer;

#[cfg(test)]
#[path = "tests/randomized_tests.rs"]
mod randomized_tests;

mod commit_solidifier;
mod cordial_knowledge;
mod decoder;
mod encoder;
mod shard_reconstructor;
#[cfg(test)]
mod test_dag;
#[cfg(test)]
mod test_dag_builder;
#[cfg(test)]
mod test_dag_parser;

/// Exported consensus API.
pub use authority_node::ConsensusAuthority;
pub use block_header::{BlockHeaderAPI, BlockRef, Round};
/// Exported API for testing.
pub use block_header::{BlockTimestampMs, TestBlockHeader, Transaction, VerifiedBlockHeader};
pub use commit::{CommitDigest, CommitIndex, CommitRef, CommittedSubDag};
pub use commit_consumer::{CommitConsumer, CommitConsumerMonitor};
pub use context::Clock;
pub use network::tonic_network::to_socket_addr;
#[cfg(msim)]
pub use storage::delete_all_transactions_from_store;
#[cfg(msim)]
pub use transaction::NoopTransactionVerifier;
pub use transaction::{
    BlockStatus, ClientError, TransactionClient, TransactionVerifier, ValidationError,
};
