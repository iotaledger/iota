// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Binary types sent to the browser via REST/WebSocket.
//! These match the frontend's expected API contract.
//!
//! Wire format: all multi-byte integers are little-endian. u64 values are
//! encoded as f64 (safe for integers up to 2^53). Strings use a 1-byte
//! length prefix + UTF-8 bytes.
//!
//! NOTE: These types must stay in sync with the frontend's `types.ts`
//! (`dev-tools/dag-visualizer/frontend/src/api/types.ts`) and the binary
//! codec (`dev-tools/dag-visualizer/frontend/src/api/codec.ts`).

/// Leader status constants.
pub const LEADER_COMMITTED: u8 = 0;
pub const LEADER_SKIPPED: u8 = 1;

/// Max hex chars for digests on the wire.
pub const DIGEST_SHORT_LEN: usize = 6;

/// Truncate a full hex digest to short form for display.
pub fn short_digest(full_hex: &str) -> String {
    full_hex[..full_hex.len().min(DIGEST_SHORT_LEN)].to_string()
}

// ---------------------------------------------------------------------------
// Binary encoding helpers
// ---------------------------------------------------------------------------

/// Write a length-prefixed UTF-8 string: 1-byte len + bytes.
fn write_str(buf: &mut Vec<u8>, s: &str) {
    let bytes = s.as_bytes();
    buf.push(bytes.len() as u8);
    buf.extend_from_slice(bytes);
}

/// Write a u64 as f64 (little-endian).
fn write_u64_as_f64(buf: &mut Vec<u8>, v: u64) {
    buf.extend_from_slice(&(v as f64).to_le_bytes());
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A block reference.
#[derive(Clone, Debug)]
pub struct BlockRefMessage {
    pub round: u32,
    pub author: u8,
    pub digest: String,
}

impl BlockRefMessage {
    /// Encode as: u32 round + u16 author + len-prefixed digest.
    pub fn encode_binary(&self, buf: &mut Vec<u8>) {
        buf.extend_from_slice(&self.round.to_le_bytes());
        buf.extend_from_slice(&(self.author as u16).to_le_bytes());
        write_str(buf, &self.digest);
    }
}

/// A DAG block.
#[derive(Clone, Debug)]
pub struct DagBlockMessage {
    pub round: u32,
    pub author: u8,
    pub digest: String,
    pub timestamp_ms: u64,
    pub ancestors: Vec<BlockRefMessage>,
    pub acknowledgments: Vec<BlockRefMessage>,
}

impl DagBlockMessage {
    /// Encode block (without type byte).
    pub fn encode_binary(&self, buf: &mut Vec<u8>) {
        buf.extend_from_slice(&self.round.to_le_bytes());
        buf.extend_from_slice(&(self.author as u16).to_le_bytes());
        write_str(buf, &self.digest);
        write_u64_as_f64(buf, self.timestamp_ms);

        buf.extend_from_slice(&(self.ancestors.len() as u16).to_le_bytes());
        for a in &self.ancestors {
            a.encode_binary(buf);
        }

        buf.extend_from_slice(&(self.acknowledgments.len() as u16).to_le_bytes());
        for a in &self.acknowledgments {
            a.encode_binary(buf);
        }
    }
}

/// Leader decision information.
#[derive(Clone, Debug)]
pub struct LeaderInfoMessage {
    pub wave: u32,
    pub leader_round: u32,
    pub leader_authority: u8,
    /// 0 = committed, 1 = skipped
    pub status: u8,
    pub block_digest: Option<String>,
}

impl LeaderInfoMessage {
    /// Encode leader (without type byte).
    pub fn encode_binary(&self, buf: &mut Vec<u8>) {
        buf.extend_from_slice(&self.wave.to_le_bytes());
        buf.extend_from_slice(&self.leader_round.to_le_bytes());
        buf.extend_from_slice(&(self.leader_authority as u16).to_le_bytes());
        buf.push(self.status);
        match &self.block_digest {
            Some(d) => {
                buf.push(1);
                write_str(buf, d);
            }
            None => {
                buf.push(0);
            }
        }
    }
}

/// A validator.
#[derive(Clone, Debug)]
pub struct ValidatorMessage {
    pub index: u8,
    pub hostname: String,
    pub stake: u64,
}

/// Committee information.
#[derive(Clone, Debug)]
pub struct CommitteeMessage {
    pub epoch: u64,
    pub total_stake: u64,
    pub quorum_threshold: u64,
    pub validators: Vec<ValidatorMessage>,
}

impl CommitteeMessage {
    pub fn encode_binary(&self, buf: &mut Vec<u8>) {
        write_u64_as_f64(buf, self.epoch);
        write_u64_as_f64(buf, self.total_stake);
        write_u64_as_f64(buf, self.quorum_threshold);
        buf.extend_from_slice(&(self.validators.len() as u16).to_le_bytes());
        for v in &self.validators {
            buf.push(v.index);
            write_u64_as_f64(buf, v.stake);
            write_str(buf, &v.hostname);
        }
    }
}

/// A windowed snapshot of the DAG.
#[derive(Clone, Debug)]
pub struct DagWindowMessage {
    pub from_round: u32,
    pub to_round: u32,
    pub highest_accepted_round: u32,
    pub last_commit_round: u32,
    pub blocks: Vec<DagBlockMessage>,
    pub leaders: Vec<LeaderInfoMessage>,
}

impl DagWindowMessage {
    pub fn encode_binary(&self, buf: &mut Vec<u8>) {
        buf.extend_from_slice(&self.from_round.to_le_bytes());
        buf.extend_from_slice(&self.to_round.to_le_bytes());
        buf.extend_from_slice(&self.highest_accepted_round.to_le_bytes());
        buf.extend_from_slice(&self.last_commit_round.to_le_bytes());

        buf.extend_from_slice(&(self.blocks.len() as u32).to_le_bytes());
        for b in &self.blocks {
            b.encode_binary(buf);
        }

        buf.extend_from_slice(&(self.leaders.len() as u32).to_le_bytes());
        for l in &self.leaders {
            l.encode_binary(buf);
        }
    }
}

/// Status summary.
#[derive(Clone, Debug)]
pub struct StatusMessage {
    pub highest_accepted_round: u32,
    pub last_commit_index: u32,
    pub last_commit_round: u32,
    pub num_authorities: u32,
}

impl StatusMessage {
    /// Encode as 16 bytes fixed: 4× u32 LE.
    pub fn encode_binary(&self, buf: &mut Vec<u8>) {
        buf.extend_from_slice(&self.highest_accepted_round.to_le_bytes());
        buf.extend_from_slice(&self.last_commit_index.to_le_bytes());
        buf.extend_from_slice(&self.last_commit_round.to_le_bytes());
        buf.extend_from_slice(&self.num_authorities.to_le_bytes());
    }
}

/// Epoch information for the /epochs endpoint.
#[derive(Clone, Debug)]
pub struct EpochInfo {
    pub epoch: u64,
    pub from_round: u32,
    pub to_round: u32,
}

/// Encode a list of EpochInfo into binary.
pub fn encode_epochs_binary(epochs: &[EpochInfo], buf: &mut Vec<u8>) {
    buf.extend_from_slice(&(epochs.len() as u16).to_le_bytes());
    for e in epochs {
        write_u64_as_f64(buf, e.epoch);
        buf.extend_from_slice(&e.from_round.to_le_bytes());
        buf.extend_from_slice(&e.to_round.to_le_bytes());
    }
}

/// Event type discriminants.
pub const EVENT_BLOCK_ACCEPTED: u8 = 0;
pub const EVENT_LEADER_DECIDED: u8 = 1;
pub const EVENT_ROUND_ADVANCED: u8 = 2;
pub const EVENT_LAGGED: u8 = 3;

/// Events streamed over WebSocket.
#[derive(Clone, Debug)]
pub enum DagVisualizerEvent {
    BlockAccepted(DagBlockMessage),
    LeaderDecided(LeaderInfoMessage),
    RoundAdvanced { round: u32 },
}

impl DagVisualizerEvent {
    /// Encode to binary wire format.
    pub fn encode_binary(&self, buf: &mut Vec<u8>) {
        match self {
            DagVisualizerEvent::BlockAccepted(block) => {
                buf.push(EVENT_BLOCK_ACCEPTED);
                block.encode_binary(buf);
            }
            DagVisualizerEvent::LeaderDecided(leader) => {
                buf.push(EVENT_LEADER_DECIDED);
                leader.encode_binary(buf);
            }
            DagVisualizerEvent::RoundAdvanced { round } => {
                buf.push(EVENT_ROUND_ADVANCED);
                buf.extend_from_slice(&round.to_le_bytes());
            }
        }
    }
}

/// Encode a "lagged" event (type=3 + f64 missed count).
pub fn encode_lagged_event(missed: u64, buf: &mut Vec<u8>) {
    buf.push(EVENT_LAGGED);
    buf.extend_from_slice(&(missed as f64).to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_digest_truncates() {
        assert_eq!(short_digest("abcdef1234"), "abcdef");
    }

    #[test]
    fn short_digest_short_input() {
        assert_eq!(short_digest("abc"), "abc");
    }

    #[test]
    fn short_digest_empty() {
        assert_eq!(short_digest(""), "");
    }

    // --- Binary encoding helpers ---

    /// Read a length-prefixed string from a buffer at the given offset.
    /// Returns (string, new_offset).
    fn read_str(buf: &[u8], offset: usize) -> (String, usize) {
        let len = buf[offset] as usize;
        let value = std::str::from_utf8(&buf[offset + 1..offset + 1 + len])
            .unwrap()
            .to_string();
        (value, offset + 1 + len)
    }

    fn read_u32(buf: &[u8], offset: usize) -> (u32, usize) {
        let value = u32::from_le_bytes(buf[offset..offset + 4].try_into().unwrap());
        (value, offset + 4)
    }

    fn read_u16(buf: &[u8], offset: usize) -> (u16, usize) {
        let value = u16::from_le_bytes(buf[offset..offset + 2].try_into().unwrap());
        (value, offset + 2)
    }

    fn read_f64(buf: &[u8], offset: usize) -> (f64, usize) {
        let value = f64::from_le_bytes(buf[offset..offset + 8].try_into().unwrap());
        (value, offset + 8)
    }

    fn read_block_ref(buf: &[u8], offset: usize) -> (BlockRefMessage, usize) {
        let (round, offset) = read_u32(buf, offset);
        let (author, offset) = read_u16(buf, offset);
        let (digest, offset) = read_str(buf, offset);
        (
            BlockRefMessage {
                round,
                author: author as u8,
                digest,
            },
            offset,
        )
    }

    #[test]
    fn block_ref_binary_roundtrip() {
        let block_ref = BlockRefMessage {
            round: 42,
            author: 3,
            digest: "abcdef".to_string(),
        };
        let mut buf = Vec::new();
        block_ref.encode_binary(&mut buf);

        let (decoded, end) = read_block_ref(&buf, 0);
        assert_eq!(end, buf.len());
        assert_eq!(decoded.round, 42);
        assert_eq!(decoded.author, 3);
        assert_eq!(decoded.digest, "abcdef");
    }

    #[test]
    fn status_binary_is_16_bytes() {
        let status = StatusMessage {
            highest_accepted_round: 100,
            last_commit_index: 50,
            last_commit_round: 98,
            num_authorities: 4,
        };
        let mut buf = Vec::new();
        status.encode_binary(&mut buf);
        assert_eq!(buf.len(), 16);

        let (highest_accepted_round, off) = read_u32(&buf, 0);
        let (last_commit_index, off) = read_u32(&buf, off);
        let (last_commit_round, off) = read_u32(&buf, off);
        let (num_authorities, _) = read_u32(&buf, off);
        assert_eq!(highest_accepted_round, 100);
        assert_eq!(last_commit_index, 50);
        assert_eq!(last_commit_round, 98);
        assert_eq!(num_authorities, 4);
    }

    #[test]
    fn event_block_accepted_binary_roundtrip() {
        let block = DagBlockMessage {
            round: 5,
            author: 2,
            digest: "abcdef".to_string(),
            timestamp_ms: 1000,
            ancestors: vec![BlockRefMessage {
                round: 4,
                author: 1,
                digest: "111111".to_string(),
            }],
            acknowledgments: vec![BlockRefMessage {
                round: 3,
                author: 1,
                digest: "aabbcc".to_string(),
            }],
        };
        let event = DagVisualizerEvent::BlockAccepted(block);
        let mut buf = Vec::new();
        event.encode_binary(&mut buf);

        // Type byte
        assert_eq!(buf[0], EVENT_BLOCK_ACCEPTED);
        let off = 1;

        // Block fields
        let (round, off) = read_u32(&buf, off);
        assert_eq!(round, 5);
        let (author, off) = read_u16(&buf, off);
        assert_eq!(author, 2);
        let (digest, off) = read_str(&buf, off);
        assert_eq!(digest, "abcdef");
        let (timestamp, off) = read_f64(&buf, off);
        assert_eq!(timestamp, 1000.0);

        // Ancestors
        let (ancestor_count, off) = read_u16(&buf, off);
        assert_eq!(ancestor_count, 1);
        let (ancestor, off) = read_block_ref(&buf, off);
        assert_eq!(ancestor.round, 4);
        assert_eq!(ancestor.author, 1);
        assert_eq!(ancestor.digest, "111111");

        // Acknowledgments
        let (acknowledgment_count, off) = read_u16(&buf, off);
        assert_eq!(acknowledgment_count, 1);
        let (acknowledgment, off) = read_block_ref(&buf, off);
        assert_eq!(acknowledgment.round, 3);
        assert_eq!(acknowledgment.author, 1);
        assert_eq!(acknowledgment.digest, "aabbcc");

        assert_eq!(off, buf.len());
    }

    #[test]
    fn event_leader_decided_binary_roundtrip() {
        let leader = LeaderInfoMessage {
            wave: 3,
            leader_round: 6,
            leader_authority: 1,
            status: 0,
            block_digest: Some("fedcba".to_string()),
        };
        let event = DagVisualizerEvent::LeaderDecided(leader);
        let mut buf = Vec::new();
        event.encode_binary(&mut buf);

        assert_eq!(buf[0], EVENT_LEADER_DECIDED);
        let off = 1;

        let (wave, off) = read_u32(&buf, off);
        assert_eq!(wave, 3);
        let (leader_round, off) = read_u32(&buf, off);
        assert_eq!(leader_round, 6);
        let (leader_authority, off) = read_u16(&buf, off);
        assert_eq!(leader_authority, 1);
        assert_eq!(buf[off], 0); // status
        let off = off + 1;
        assert_eq!(buf[off], 1); // has_digest
        let off = off + 1;
        let (digest, off) = read_str(&buf, off);
        assert_eq!(digest, "fedcba");

        assert_eq!(off, buf.len());
    }

    #[test]
    fn event_leader_decided_no_digest() {
        let leader = LeaderInfoMessage {
            wave: 1,
            leader_round: 2,
            leader_authority: 0,
            status: 1, // skipped
            block_digest: None,
        };
        let event = DagVisualizerEvent::LeaderDecided(leader);
        let mut buf = Vec::new();
        event.encode_binary(&mut buf);

        assert_eq!(buf[0], EVENT_LEADER_DECIDED);
        let off = 1 + 4 + 4 + 2 + 1; // skip to has_digest
        assert_eq!(buf[off], 0); // has_digest = 0
        assert_eq!(off + 1, buf.len()); // nothing after
    }

    #[test]
    fn event_round_advanced_binary() {
        let event = DagVisualizerEvent::RoundAdvanced { round: 42 };
        let mut buf = Vec::new();
        event.encode_binary(&mut buf);

        assert_eq!(buf.len(), 5); // 1 type + 4 round
        assert_eq!(buf[0], EVENT_ROUND_ADVANCED);
        let (round, _) = read_u32(&buf, 1);
        assert_eq!(round, 42);
    }

    #[test]
    fn lagged_event_binary() {
        let mut buf = Vec::new();
        encode_lagged_event(123, &mut buf);

        assert_eq!(buf.len(), 9); // 1 type + 8 f64
        assert_eq!(buf[0], EVENT_LAGGED);
        let (missed, _) = read_f64(&buf, 1);
        assert_eq!(missed, 123.0);
    }

    #[test]
    fn committee_binary_roundtrip() {
        let committee = CommitteeMessage {
            epoch: 5,
            total_stake: 10000,
            quorum_threshold: 6667,
            validators: vec![
                ValidatorMessage {
                    index: 0,
                    hostname: "node-0".to_string(),
                    stake: 5000,
                },
                ValidatorMessage {
                    index: 1,
                    hostname: "node-1".to_string(),
                    stake: 5000,
                },
            ],
        };
        let mut buf = Vec::new();
        committee.encode_binary(&mut buf);

        let off = 0;
        let (epoch, off) = read_f64(&buf, off);
        assert_eq!(epoch, 5.0);
        let (total_stake, off) = read_f64(&buf, off);
        assert_eq!(total_stake, 10000.0);
        let (quorum_threshold, off) = read_f64(&buf, off);
        assert_eq!(quorum_threshold, 6667.0);
        let (validator_count, off) = read_u16(&buf, off);
        assert_eq!(validator_count, 2);

        // Validator 0
        assert_eq!(buf[off], 0);
        let off = off + 1;
        let (stake, off) = read_f64(&buf, off);
        assert_eq!(stake, 5000.0);
        let (hostname, off) = read_str(&buf, off);
        assert_eq!(hostname, "node-0");

        // Validator 1
        assert_eq!(buf[off], 1);
        let off = off + 1;
        let (stake, off) = read_f64(&buf, off);
        assert_eq!(stake, 5000.0);
        let (hostname, off) = read_str(&buf, off);
        assert_eq!(hostname, "node-1");

        assert_eq!(off, buf.len());
    }

    #[test]
    fn epochs_binary_roundtrip() {
        let epochs = vec![
            EpochInfo {
                epoch: 1,
                from_round: 1,
                to_round: 100,
            },
            EpochInfo {
                epoch: 2,
                from_round: 101,
                to_round: 200,
            },
        ];
        let mut buf = Vec::new();
        encode_epochs_binary(&epochs, &mut buf);

        let off = 0;
        let (count, off) = read_u16(&buf, off);
        assert_eq!(count, 2);

        let (epoch_1, off) = read_f64(&buf, off);
        assert_eq!(epoch_1, 1.0);
        let (from_round_1, off) = read_u32(&buf, off);
        assert_eq!(from_round_1, 1);
        let (to_round_1, off) = read_u32(&buf, off);
        assert_eq!(to_round_1, 100);

        let (epoch_2, off) = read_f64(&buf, off);
        assert_eq!(epoch_2, 2.0);
        let (from_round_2, off) = read_u32(&buf, off);
        assert_eq!(from_round_2, 101);
        let (to_round_2, off) = read_u32(&buf, off);
        assert_eq!(to_round_2, 200);

        assert_eq!(off, buf.len());
    }

    #[test]
    fn dag_window_binary_roundtrip() {
        let window = DagWindowMessage {
            from_round: 10,
            to_round: 20,
            highest_accepted_round: 20,
            last_commit_round: 18,
            blocks: vec![DagBlockMessage {
                round: 15,
                author: 1,
                digest: "abc123".to_string(),
                timestamp_ms: 5000,
                ancestors: vec![],
                acknowledgments: vec![],
            }],
            leaders: vec![LeaderInfoMessage {
                wave: 2,
                leader_round: 13,
                leader_authority: 0,
                status: 0,
                block_digest: Some("def456".to_string()),
            }],
        };
        let mut buf = Vec::new();
        window.encode_binary(&mut buf);

        let off = 0;
        let (from_round, off) = read_u32(&buf, off);
        assert_eq!(from_round, 10);
        let (to_round, off) = read_u32(&buf, off);
        assert_eq!(to_round, 20);
        let (highest_accepted_round, off) = read_u32(&buf, off);
        assert_eq!(highest_accepted_round, 20);
        let (last_commit_round, off) = read_u32(&buf, off);
        assert_eq!(last_commit_round, 18);

        // 1 block
        let (block_count, off) = read_u32(&buf, off);
        assert_eq!(block_count, 1);
        let (block_round, off) = read_u32(&buf, off);
        assert_eq!(block_round, 15);
        let (block_author, off) = read_u16(&buf, off);
        assert_eq!(block_author, 1);
        let (block_digest, off) = read_str(&buf, off);
        assert_eq!(block_digest, "abc123");
        let (timestamp, off) = read_f64(&buf, off);
        assert_eq!(timestamp, 5000.0);
        let (num_ancestors, off) = read_u16(&buf, off);
        assert_eq!(num_ancestors, 0);
        let (num_acknowledgments, off) = read_u16(&buf, off);
        assert_eq!(num_acknowledgments, 0);

        // 1 leader
        let (leader_count, off) = read_u32(&buf, off);
        assert_eq!(leader_count, 1);
        let (wave, off) = read_u32(&buf, off);
        assert_eq!(wave, 2);
        let (leader_round, off) = read_u32(&buf, off);
        assert_eq!(leader_round, 13);
        let (leader_authority, off) = read_u16(&buf, off);
        assert_eq!(leader_authority, 0);
        assert_eq!(buf[off], 0); // status
        let off = off + 1;
        assert_eq!(buf[off], 1); // has_digest
        let off = off + 1;
        let (leader_digest, off) = read_str(&buf, off);
        assert_eq!(leader_digest, "def456");

        assert_eq!(off, buf.len());
    }
}
