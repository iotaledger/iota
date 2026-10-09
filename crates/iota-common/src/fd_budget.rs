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
//!
//! A share is a ceiling its holder will not exceed, not a reservation set aside
//! for it, and nothing enforces one. Two consequences are worth knowing before
//! reading a share as a guarantee. A subsystem that opens fewer descriptors
//! than its share leaves the remainder to whoever asks next, which is what
//! makes the division workable in practice. And a share handed to more than one
//! instance of the same subsystem is worth that many times its size: RocksDB
//! takes [`shares::TYPED_STORE`] for each database it opens, and a node opens
//! several, so what RocksDB may open at once is its share times the number of
//! databases rather than its share. The shares below therefore describe how the
//! limit is meant to be divided in the ordinary case, not a bound that holds
//! when every subsystem reaches its ceiling at once.

use std::sync::OnceLock;

/// The shares the budget is divided into. They are named rather than written
/// as fractions at each call site so that what is left over is visible.
pub mod shares {
    /// RocksDB's open-file cache, for each database that is opened.
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
    "one of each share must account for the whole budget"
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
