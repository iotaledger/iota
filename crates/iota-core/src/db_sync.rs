// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Switches that make chosen RocksDB batch writes sync the write-ahead log
//! (WAL) to disk, so that these writes survive a power loss and not only a
//! process crash.
//!
//! The switches are separate only so that the cost of each can be measured
//! on its own. Once that is measured on validators with a disk each, they are
//! to be merged into fewer settings or removed.
//!
//! Why some writes need a sync: a node keeps several RocksDB databases (the
//! epoch, perpetual and checkpoint databases), each with its own WAL. A write
//! that returned is safe from a process crash, because the operating system
//! still holds it. After a power loss, each database keeps its writes up to
//! some point and loses the rest, independently of the others. So when a
//! node writes A to one database and then B to another and relies on A being
//! on disk whenever B is, a power loss can keep B and lose A. Syncing A
//! prevents that. A synced write also puts every earlier write to the same
//! database on disk.
//!
//! Each switch syncs the first write, A, of such a pair:
//!
//! - `IOTA_DB_SYNC_HANDLER_OBJECT_STATE`: the batch of handler object state rows that the
//!   checkpoint executor writes to the epoch database just before the checkpoint's transaction
//!   outputs go to the perpetual database. With the switch off, a power loss can keep the outputs
//!   and lose the rows. The executor does not run those transactions again, so the rows are never
//!   written again. A committee validator's post-consensus validation can then decide differently
//!   from the other validators, and the node has to be restored from a snapshot.
//! - `IOTA_DB_SYNC_CHECKPOINT_OUTPUTS`: the checkpoint's transaction outputs (objects, effects and
//!   transactions, perpetual database). Two later writes rely on them: the quarantine flush (epoch
//!   database), which writes the consensus output of the checkpoint's commits and moves the point
//!   the consensus handler replays from on restart past those commits; and the highest executed
//!   checkpoint (checkpoint database). With the switch off, a power loss can keep either of these
//!   two writes and lose the outputs:
//!   - if the quarantine flush is kept, the consensus handler does not replay the checkpoint's
//!     commits, so on a committee validator that keeps the handler object state nothing runs those
//!     transactions again. Checkpoint execution waits until the node switches to executing synced
//!     checkpoints at the end of the epoch.
//!   - if the highest executed checkpoint is kept but the quarantine flush is not, the consensus
//!     handler replays the commits and flushes them at once, because their checkpoint counts as
//!     executed. The flush derives the commits' rows from their effects, finds none, and fails.
//!     This happens again on every restart.
//! - `IOTA_DB_SYNC_TRANSACTIONS_AND_EFFECTS`: the transactions and effects stored outside
//!   checkpoint execution (perpetual database): those of the checkpoints state sync downloads, and
//!   the change-epoch transaction the checkpoint builder puts in the last checkpoint of the epoch.
//!   Later writes rely on them: the checkpoint contents rows and the highest synced checkpoint
//!   (checkpoint database). With the switch off, a power loss can keep those rows and lose the
//!   transactions and effects. State sync then does not fetch them again, and the checkpoint
//!   executor fails on every restart when it executes that checkpoint from synced data: on a
//!   fullnode always, and on a validator for the last checkpoint of the epoch and after it switches
//!   to executing synced checkpoints.
//!
//! How likely each problem is has not been measured, and it depends on the
//! file system and on when the kernel writes data back. With no switch on, a
//! problem needs the database of B to reach disk ahead of the database of A.
//! That depends on timing, so only some power losses cause one. Turning a
//! switch on removes the problems of its own pair. But because a synced write
//! puts every earlier write to its database on disk, the synced database is
//! then regularly ahead of the others, which makes a problem of another pair
//! more likely than with no switch on when that pair's switch is off:
//! - handler object state on, checkpoint outputs off: the next checkpoint's synced rows also put
//!   the previous checkpoint's quarantine flush on disk, since both are in the epoch database,
//!   often before that checkpoint's outputs reach disk. The stall at a kept quarantine flush
//!   becomes more likely.
//! - checkpoint outputs on, handler object state off: the synced outputs reach disk often before
//!   their rows do. A lost row, the only one of these problems that needs a snapshot to recover,
//!   becomes more likely. The transactions and effects switch has the same effect, since it also
//!   syncs the perpetual database, but on a validator state sync rarely writes.
//!
//! With all three switches on, none of these problems remain.
//!
//! Every switch is off by default, and then its writes use
//! `rocksdb::WriteOptions::default()`. A switch is on when the node config
//! turns it on through [`Switch::enable`], or when its environment variable
//! is set to `1` or `true`. The environment variable takes precedence: once
//! it turns a switch on, the node config cannot turn it off, not even by
//! setting it to false. `rocksdb_synced_writes{db_name, switch}` counts the
//! writes each switch synced.
//!
//! The switches apply to the whole process: turning one on for one node turns
//! it on for every node in the same process, and nothing turns it off again.

use std::sync::{
    LazyLock,
    atomic::{AtomicBool, Ordering},
};

use typed_store::rocks::SyncSwitch;

/// A group of writes that is synced to disk when its switch is on. See the
/// module docs for the writes in each group and the risk when it is off.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Switch {
    HandlerObjectState,
    CheckpointOutputs,
    TransactionsAndEffects,
}

impl Switch {
    /// The value of the `switch` label of `rocksdb_synced_writes` for this
    /// switch.
    pub fn name(self) -> &'static str {
        match self {
            Self::HandlerObjectState => "handler_object_state",
            Self::CheckpointOutputs => "checkpoint_outputs",
            Self::TransactionsAndEffects => "transactions_and_effects",
        }
    }

    /// Turns the switch on for the whole process. Call it before opening the
    /// databases, so that no write of its group is left unsynced.
    pub fn enable(self) {
        self.state().store(true, Ordering::Relaxed);
    }

    /// Whether the writes of this group are synced.
    pub fn is_on(self) -> bool {
        self.state().load(Ordering::Relaxed)
    }

    fn state(self) -> &'static AtomicBool {
        match self {
            Self::HandlerObjectState => &HANDLER_OBJECT_STATE,
            Self::CheckpointOutputs => &CHECKPOINT_OUTPUTS,
            Self::TransactionsAndEffects => &TRANSACTIONS_AND_EFFECTS,
        }
    }
}

impl SyncSwitch for Switch {
    fn is_on(&self) -> bool {
        Switch::is_on(*self)
    }

    fn name(&self) -> &'static str {
        Switch::name(*self)
    }
}

static HANDLER_OBJECT_STATE: LazyLock<AtomicBool> =
    LazyLock::new(|| from_env("IOTA_DB_SYNC_HANDLER_OBJECT_STATE"));
static CHECKPOINT_OUTPUTS: LazyLock<AtomicBool> =
    LazyLock::new(|| from_env("IOTA_DB_SYNC_CHECKPOINT_OUTPUTS"));
static TRANSACTIONS_AND_EFFECTS: LazyLock<AtomicBool> =
    LazyLock::new(|| from_env("IOTA_DB_SYNC_TRANSACTIONS_AND_EFFECTS"));

fn from_env(name: &str) -> AtomicBool {
    AtomicBool::new(parse_switch(std::env::var(name).ok().as_deref()))
}

/// Only `1` and `true` (any case) turn a switch on.
fn parse_switch(value: Option<&str>) -> bool {
    value.is_some_and(|v| {
        let v = v.trim();
        v == "1" || v.eq_ignore_ascii_case("true")
    })
}

#[cfg(test)]
mod tests {
    use super::parse_switch;

    #[test]
    fn parse_switch_is_on_only_for_one_and_true() {
        assert!(!parse_switch(None));
        assert!(!parse_switch(Some("")));
        assert!(!parse_switch(Some("0")));
        assert!(!parse_switch(Some("false")));
        assert!(!parse_switch(Some("yes")));
        assert!(!parse_switch(Some("2")));
        assert!(parse_switch(Some("1")));
        assert!(parse_switch(Some(" true ")));
        assert!(parse_switch(Some("TRUE")));
    }
}
