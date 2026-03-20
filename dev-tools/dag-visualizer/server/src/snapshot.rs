// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Converts stored data from RocksDB into binary types for the REST API.

use crate::{
    storage::{EpochStore, StoredBlock, StoredLeader},
    types::*,
};

/// Build a DAG window response from storage.
pub fn build_dag_window(store: &EpochStore, from_round: u32, to_round: u32) -> DagWindowMessage {
    let blocks_stored = store.get_blocks_in_range(from_round, to_round);
    let leaders_stored = store.get_leaders_in_range(from_round, to_round);

    let blocks: Vec<DagBlockMessage> = blocks_stored.iter().map(stored_block_to_msg).collect();
    let leaders: Vec<LeaderInfoMessage> = leaders_stored.iter().map(stored_leader_to_msg).collect();

    // Use global status from the EpochStore rather than deriving values from
    // the queried window, which would only reflect the windowed subset.
    let status = store.get_status();
    let highest_accepted_round = status.as_ref().map_or(0, |s| s.highest_accepted_round);
    let last_commit_round = status.as_ref().map_or(0, |s| s.last_commit_round);

    DagWindowMessage {
        from_round,
        to_round,
        highest_accepted_round,
        last_commit_round,
        blocks,
        leaders,
    }
}

/// Convert a stored block to its binary representation.
fn stored_block_to_msg(block: &StoredBlock) -> DagBlockMessage {
    DagBlockMessage {
        round: block.round,
        author: block.author,
        digest: short_digest(&hex::encode(block.digest)),
        timestamp_ms: block.timestamp_ms,
        ancestors: block
            .ancestors
            .iter()
            .map(|(round, author)| BlockRefMessage {
                round: *round,
                author: *author,
                // Ancestor digests are not stored (one block per slot),
                // use empty string.
                digest: String::new(),
            })
            .collect(),
        acknowledgments: block
            .acknowledgments
            .iter()
            .map(|(round, author, digest)| BlockRefMessage {
                round: *round,
                author: *author,
                digest: short_digest(&hex::encode(digest)),
            })
            .collect(),
    }
}

/// Convert a stored leader to its binary representation.
fn stored_leader_to_msg(leader: &StoredLeader) -> LeaderInfoMessage {
    LeaderInfoMessage {
        wave: leader.wave,
        leader_round: leader.leader_round,
        leader_authority: leader.leader_authority,
        status: leader.status,
        block_digest: leader.block_digest.map(|d| short_digest(&hex::encode(d))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{EpochStore, StoredBlock, StoredLeader, StoredStatus};

    fn open_test_store(dir: &std::path::Path) -> EpochStore {
        EpochStore::open_for_test(dir)
    }

    fn make_block(round: u32, author: u8) -> StoredBlock {
        StoredBlock {
            round,
            author,
            digest: [round as u8; 32],
            timestamp_ms: round as u64 * 1000,
            ancestors: if round > 1 {
                vec![(round - 1, author)]
            } else {
                vec![]
            },
            acknowledgments: vec![],
        }
    }

    fn make_leader(leader_round: u32, status: u8) -> StoredLeader {
        StoredLeader {
            wave: leader_round / 2,
            leader_round,
            leader_authority: 0,
            status,
            block_digest: if status == LEADER_COMMITTED {
                Some([leader_round as u8; 32])
            } else {
                None
            },
        }
    }

    #[tokio::test]
    async fn build_dag_window_empty() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_test_store(dir.path());
        let window = build_dag_window(&store, 0, 10);

        assert_eq!(window.from_round, 0);
        assert_eq!(window.to_round, 10);
        assert_eq!(window.highest_accepted_round, 0);
        assert_eq!(window.last_commit_round, 0);
        assert!(window.blocks.is_empty());
        assert!(window.leaders.is_empty());
    }

    #[tokio::test]
    async fn build_dag_window_with_blocks() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_test_store(dir.path());
        for round in 1..=4 {
            store.insert_block(&make_block(round, 0));
        }
        store.insert_leader(&make_leader(1, LEADER_COMMITTED));
        store.set_status(&StoredStatus {
            highest_accepted_round: 4,
            last_commit_index: 1,
            last_commit_round: 1,
            num_authorities: 4,
        });

        let window = build_dag_window(&store, 1, 4);
        assert_eq!(window.blocks.len(), 4);
        assert_eq!(window.highest_accepted_round, 4);
        assert_eq!(window.last_commit_round, 1);
        // Digests should be 6 hex chars
        for block in &window.blocks {
            assert_eq!(block.digest.len(), DIGEST_SHORT_LEN);
        }
    }

    #[tokio::test]
    async fn build_dag_window_filters_by_range() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_test_store(dir.path());
        for round in 1..=10 {
            store.insert_block(&make_block(round, 0));
        }

        let window = build_dag_window(&store, 5, 8);
        let rounds: Vec<u32> = window.blocks.iter().map(|b| b.round).collect();
        assert_eq!(rounds, vec![5, 6, 7, 8]);
    }

    #[tokio::test]
    async fn leader_status_committed_vs_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_test_store(dir.path());
        store.insert_leader(&make_leader(4, LEADER_COMMITTED));
        store.insert_leader(&make_leader(6, LEADER_SKIPPED));
        store.set_status(&StoredStatus {
            highest_accepted_round: 10,
            last_commit_index: 1,
            last_commit_round: 4,
            num_authorities: 4,
        });

        let window = build_dag_window(&store, 1, 10);
        assert_eq!(window.leaders.len(), 2);

        let committed: Vec<_> = window
            .leaders
            .iter()
            .filter(|l| l.status == LEADER_COMMITTED)
            .collect();
        let skipped: Vec<_> = window
            .leaders
            .iter()
            .filter(|l| l.status == LEADER_SKIPPED)
            .collect();
        assert_eq!(committed.len(), 1);
        assert_eq!(skipped.len(), 1);
        assert_eq!(committed[0].leader_round, 4);
        assert_eq!(skipped[0].leader_round, 6);
        // last_commit_round reflects global status, not just the window
        assert_eq!(window.last_commit_round, 4);
    }

    #[tokio::test]
    async fn block_digest_is_short_hex() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_test_store(dir.path());
        store.insert_block(&make_block(1, 0));

        let window = build_dag_window(&store, 1, 1);
        assert_eq!(window.blocks.len(), 1);
        let digest = &window.blocks[0].digest;
        assert_eq!(digest.len(), DIGEST_SHORT_LEN);
        assert!(digest.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
