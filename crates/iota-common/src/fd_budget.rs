// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Divides the file descriptors this process may open between the subsystems
//! that consume them in bulk.
//!
//! Every socket, database file and log file counts against one per-process
//! ceiling. When it is reached `accept` fails and files stop opening, for every
//! subsystem at once and not only the one that took the last descriptor — so
//! the shares below exist to keep any one of them from doing that to the
//! others.
//!
//! The ceiling itself belongs to the deployment, not to this repository: it is
//! the hard limit the service manager or container grants. What this module
//! does is raise the process to that ceiling rather than accepting the
//! conventional soft default, and then hand out shares of the result.

use std::sync::OnceLock;

/// The shares the budget is divided into. They are named rather than written
/// as fractions at each call site so that what is left over is visible, and
/// they are checked below to sum to the whole.
pub mod shares {
    /// RocksDB's open-file cache.
    pub const TYPED_STORE: u64 = 1;
    /// Each listener that bounds the connections it serves. Two do today: the
    /// validator gRPC and the JSON-RPC interfaces.
    pub const LISTENER: u64 = 1;
    /// Everything that is not budgeted: the consensus and peer-to-peer
    /// networks, state sync, log files, and headroom for the fact that none of
    /// these shares are enforced.
    pub const UNBUDGETED: u64 = 5;

    /// What the shares above are shares of.
    pub const TOTAL: u64 = TYPED_STORE + 2 * LISTENER + UNBUDGETED;
}

const _: () = assert!(
    shares::TYPED_STORE + 2 * shares::LISTENER + shares::UNBUDGETED == shares::TOTAL,
    "the file descriptor shares must add up to the whole budget"
);

/// Used where the platform reports no limit, which is Windows only.
const UNKNOWN_FD_LIMIT: u64 = 16384;

static FD_LIMIT: OnceLock<u64> = OnceLock::new();

/// Raises this process to the highest number of open files it is allowed, and
/// returns it.
///
/// The raise happens once however often this is called, and the soft limit is
/// only ever raised as far as the hard limit, so this widens what the
/// deployment already permits rather than overriding it.
pub fn fd_limit() -> u64 {
    *FD_LIMIT.get_or_init(|| {
        let limit = fdlimit::raise_fd_limit().unwrap_or(UNKNOWN_FD_LIMIT);
        tracing::info!("This process may open up to {limit} files");
        limit
    })
}

/// How many file descriptors the given share is worth.
///
/// ```
/// use iota_common::fd_budget::{budget_for, shares};
///
/// let listener = budget_for(shares::LISTENER);
/// assert!(listener > 0);
/// ```
pub fn budget_for(share: u64) -> usize {
    ((fd_limit() / shares::TOTAL) * share).max(1) as usize
}
