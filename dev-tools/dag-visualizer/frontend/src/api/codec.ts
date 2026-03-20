// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/**
 * Binary codec for the DAG visualizer wire format.
 *
 * All multi-byte integers are little-endian. u64 values are encoded as f64
 * (safe for integers up to 2^53). Strings use a 1-byte length prefix + UTF-8.
 */

import type {
  BlockRefMessage,
  CommitteeMessage,
  DagBlockMessage,
  DagVisualizerEvent,
  DagWindowMessage,
  EpochInfo,
  LeaderInfoMessage,
  StatusMessage,
  ValidatorMessage,
} from './types';

const LE = true; // littleEndian

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/** Read a length-prefixed UTF-8 string and return [value, newOffset]. */
function readStr(view: DataView, buf: Uint8Array, offset: number): [string, number] {
  const len = view.getUint8(offset);
  offset += 1;
  const bytes = buf.subarray(offset, offset + len);
  const str = new TextDecoder().decode(bytes);
  return [str, offset + len];
}

/** Read a BlockRef and return [ref, newOffset]. */
function readBlockRef(view: DataView, buf: Uint8Array, offset: number): [BlockRefMessage, number] {
  const round = view.getUint32(offset, LE);
  offset += 4;
  const author = view.getUint16(offset, LE);
  offset += 2;
  let digest: string;
  [digest, offset] = readStr(view, buf, offset);
  return [{ round, author, digest }, offset];
}

/** Read a DagBlockMessage (without the type byte) and return [block, newOffset]. */
function readBlock(view: DataView, buf: Uint8Array, offset: number): [DagBlockMessage, number] {
  const round = view.getUint32(offset, LE);
  offset += 4;
  const author = view.getUint16(offset, LE);
  offset += 2;
  let digest: string;
  [digest, offset] = readStr(view, buf, offset);
  const timestamp_ms = view.getFloat64(offset, LE);
  offset += 8;

  const ancestorCount = view.getUint16(offset, LE);
  offset += 2;
  const ancestors: BlockRefMessage[] = [];
  for (let i = 0; i < ancestorCount; i++) {
    let ref_: BlockRefMessage;
    [ref_, offset] = readBlockRef(view, buf, offset);
    ancestors.push(ref_);
  }

  const ackCount = view.getUint16(offset, LE);
  offset += 2;
  const acknowledgments: BlockRefMessage[] = [];
  for (let i = 0; i < ackCount; i++) {
    let ref_: BlockRefMessage;
    [ref_, offset] = readBlockRef(view, buf, offset);
    acknowledgments.push(ref_);
  }

  return [{ round, author, digest, timestamp_ms, ancestors, acknowledgments }, offset];
}

/** Read a LeaderInfoMessage (without the type byte) and return [leader, newOffset]. */
function readLeader(view: DataView, buf: Uint8Array, offset: number): [LeaderInfoMessage, number] {
  const wave = view.getUint32(offset, LE);
  offset += 4;
  const leader_round = view.getUint32(offset, LE);
  offset += 4;
  const leader_authority = view.getUint16(offset, LE);
  offset += 2;
  const status = view.getUint8(offset);
  offset += 1;
  const hasDigest = view.getUint8(offset);
  offset += 1;
  let block_digest: string | null = null;
  if (hasDigest === 1) {
    [block_digest, offset] = readStr(view, buf, offset);
  }
  return [{ wave, leader_round, leader_authority, status, block_digest }, offset];
}

// ---------------------------------------------------------------------------
// Public decoders
// ---------------------------------------------------------------------------

/** Decode a single WebSocket binary event. */
export function decodeDagEvent(data: ArrayBuffer): DagVisualizerEvent {
  const buf = new Uint8Array(data);
  const view = new DataView(data);
  let offset = 0;

  const type_ = view.getUint8(offset);
  offset += 1;

  switch (type_) {
    case 0: {
      // BlockAccepted
      const [block, _] = readBlock(view, buf, offset);
      return {
        t: 0,
        round: block.round,
        author: block.author,
        digest: block.digest,
        timestamp_ms: block.timestamp_ms,
        ancestors: block.ancestors,
        acknowledgments: block.acknowledgments,
      };
    }
    case 1: {
      // LeaderDecided
      const [leader, _] = readLeader(view, buf, offset);
      return {
        t: 1,
        wave: leader.wave,
        leader_round: leader.leader_round,
        leader_authority: leader.leader_authority,
        status: leader.status,
        block_digest: leader.block_digest,
      };
    }
    case 2: {
      // RoundAdvanced
      const round = view.getUint32(offset, LE);
      return { t: 2, round };
    }
    case 3: {
      // Lagged
      const missed = view.getFloat64(offset, LE);
      return { t: 3, missed };
    }
    default:
      throw new Error(`Unknown DAG event type: ${type_}`);
  }
}

/** Decode `GET /api/v1/committee` binary response. */
export function decodeCommittee(data: ArrayBuffer): CommitteeMessage {
  const buf = new Uint8Array(data);
  const view = new DataView(data);
  let offset = 0;

  const epoch = view.getFloat64(offset, LE);
  offset += 8;
  const total_stake = view.getFloat64(offset, LE);
  offset += 8;
  const quorum_threshold = view.getFloat64(offset, LE);
  offset += 8;
  const validatorCount = view.getUint16(offset, LE);
  offset += 2;

  const validators: ValidatorMessage[] = [];
  for (let i = 0; i < validatorCount; i++) {
    const index = view.getUint8(offset);
    offset += 1;
    const stake = view.getFloat64(offset, LE);
    offset += 8;
    let hostname: string;
    [hostname, offset] = readStr(view, buf, offset);
    validators.push({ index, hostname, stake });
  }

  return { epoch, total_stake, quorum_threshold, validators };
}

/** Decode `GET /api/v1/status` binary response (16 bytes fixed). */
export function decodeStatus(data: ArrayBuffer): StatusMessage {
  const view = new DataView(data);
  return {
    highest_accepted_round: view.getUint32(0, LE),
    last_commit_index: view.getUint32(4, LE),
    last_commit_round: view.getUint32(8, LE),
    num_authorities: view.getUint32(12, LE),
  };
}

/** Decode `GET /api/v1/epochs` binary response. */
export function decodeEpochs(data: ArrayBuffer): EpochInfo[] {
  const view = new DataView(data);
  let offset = 0;

  const count = view.getUint16(offset, LE);
  offset += 2;

  const epochs: EpochInfo[] = [];
  for (let i = 0; i < count; i++) {
    const epoch = view.getFloat64(offset, LE);
    offset += 8;
    const from_round = view.getUint32(offset, LE);
    offset += 4;
    const to_round = view.getUint32(offset, LE);
    offset += 4;
    epochs.push({ epoch, from_round, to_round });
  }
  return epochs;
}

/** Decode `GET /api/v1/dag` binary response. */
export function decodeDagWindow(data: ArrayBuffer): DagWindowMessage {
  const buf = new Uint8Array(data);
  const view = new DataView(data);
  let offset = 0;

  const from_round = view.getUint32(offset, LE);
  offset += 4;
  const to_round = view.getUint32(offset, LE);
  offset += 4;
  const highest_accepted_round = view.getUint32(offset, LE);
  offset += 4;
  const last_commit_round = view.getUint32(offset, LE);
  offset += 4;

  const blockCount = view.getUint32(offset, LE);
  offset += 4;
  const blocks: DagBlockMessage[] = [];
  for (let i = 0; i < blockCount; i++) {
    let block: DagBlockMessage;
    [block, offset] = readBlock(view, buf, offset);
    blocks.push(block);
  }

  const leaderCount = view.getUint32(offset, LE);
  offset += 4;
  const leaders: LeaderInfoMessage[] = [];
  for (let i = 0; i < leaderCount; i++) {
    let leader: LeaderInfoMessage;
    [leader, offset] = readLeader(view, buf, offset);
    leaders.push(leader);
  }

  return { from_round, to_round, highest_accepted_round, last_commit_round, blocks, leaders };
}
