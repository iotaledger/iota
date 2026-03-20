// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
//
// Binary format for DAG visualizer save/load.
//
// Layout (all multi-byte values are little-endian):
//
//   HEADER:
//     [4B] magic "DAGV"
//     [1B] version (1)
//     [1B] flags (bit 0 = highlightSkipped)
//     [4B] centerRound (i32)
//     [8B] savedAt (f64, ms since epoch)
//
//   COMMITTEE:
//     [4B] epoch (u32)
//     [8B] total_stake (f64)
//     [8B] quorum_threshold (f64)
//     [2B] num_validators (u16)
//     per validator:
//       [1B] index (u8)
//       [1B] hostname_len (u8)  + hostname bytes
//       [8B] stake (f64)
//
//   BLOCKS:
//     [4B] num_blocks (u32)
//     per block:
//       [4B] round (u32)
//       [1B] author (u8)
//       [1B] digest_len (u8) + digest bytes
//       [8B] timestamp_ms (f64)
//       [1B] num_ancestors (u8)
//       per ancestor:
//         [4B] round (u32)
//         [1B] author (u8)
//       [2B] num_acknowledgments (u16)
//       per acknowledgment:
//         [4B] round (u32)
//         [1B] author (u8)
//         [1B] digest_len (u8) + digest bytes
//
//   LEADERS:
//     [4B] num_leaders (u32)
//     per leader:
//       [4B] wave (u32)
//       [4B] leader_round (u32)
//       [1B] leader_authority (u8)
//       [1B] status (u8)
//       [1B] has_digest (0/1)
//       if 1: [1B] digest_len + digest bytes

import type { BlockRefMessage, DagBlockMessage, LeaderInfoMessage, SavedDagView } from '../api/types';

const MAGIC = [0x44, 0x41, 0x47, 0x56]; // "DAGV"
const FORMAT_VERSION = 1;

const textEncoder = new TextEncoder();
const textDecoder = new TextDecoder();

// ── Writer ──────────────────────────────────────────────────────

class BinaryWriter {
  private buf: ArrayBuffer;
  private view: DataView;
  private u8: Uint8Array;
  private pos = 0;

  constructor(initialSize = 64 * 1024) {
    this.buf = new ArrayBuffer(initialSize);
    this.view = new DataView(this.buf);
    this.u8 = new Uint8Array(this.buf);
  }

  private grow(need: number): void {
    if (this.pos + need <= this.buf.byteLength) return;
    const size = Math.max(this.buf.byteLength * 2, this.pos + need);
    const next = new ArrayBuffer(size);
    new Uint8Array(next).set(this.u8);
    this.buf = next;
    this.view = new DataView(this.buf);
    this.u8 = new Uint8Array(this.buf);
  }

  u8w(v: number) { this.grow(1); this.view.setUint8(this.pos, v); this.pos += 1; }
  u16w(v: number) { this.grow(2); this.view.setUint16(this.pos, v, true); this.pos += 2; }
  u32w(v: number) { this.grow(4); this.view.setUint32(this.pos, v, true); this.pos += 4; }
  i32w(v: number) { this.grow(4); this.view.setInt32(this.pos, v, true); this.pos += 4; }
  f64w(v: number) { this.grow(8); this.view.setFloat64(this.pos, v, true); this.pos += 8; }

  bytes(data: Uint8Array) { this.grow(data.length); this.u8.set(data, this.pos); this.pos += data.length; }

  str(s: string) {
    let enc = textEncoder.encode(s);
    if (enc.length > 255) {
      console.warn(`BinaryWriter.str(): string truncated from ${enc.length} to 255 bytes`);
      enc = enc.slice(0, 255);
    }
    this.u8w(enc.length);
    this.bytes(enc);
  }

  result(): ArrayBuffer { return this.buf.slice(0, this.pos); }
}

// ── Reader ──────────────────────────────────────────────────────

class BinaryReader {
  private view: DataView;
  private u8: Uint8Array;
  private pos = 0;

  constructor(buf: ArrayBuffer) {
    this.view = new DataView(buf);
    this.u8 = new Uint8Array(buf);
  }

  u8r(): number { const v = this.view.getUint8(this.pos); this.pos += 1; return v; }
  u16r(): number { const v = this.view.getUint16(this.pos, true); this.pos += 2; return v; }
  u32r(): number { const v = this.view.getUint32(this.pos, true); this.pos += 4; return v; }
  i32r(): number { const v = this.view.getInt32(this.pos, true); this.pos += 4; return v; }
  f64r(): number { const v = this.view.getFloat64(this.pos, true); this.pos += 8; return v; }

  bytesr(len: number): Uint8Array {
    const d = this.u8.slice(this.pos, this.pos + len);
    this.pos += len;
    return d;
  }

  strr(): string {
    const len = this.u8r();
    return textDecoder.decode(this.bytesr(len));
  }
}

// ── Encode ──────────────────────────────────────────────────────

export function encodeDagView(saved: SavedDagView): ArrayBuffer {
  const w = new BinaryWriter();
  const numValidators = saved.committee.validators.length;

  // Header
  for (const b of MAGIC) w.u8w(b);
  w.u8w(FORMAT_VERSION);
  w.u8w(saved.highlightSkipped ? 1 : 0);
  w.i32w(saved.viewport.centerRound);
  w.f64w(new Date(saved.savedAt).getTime());

  // Committee
  w.u32w(saved.committee.epoch);
  w.f64w(saved.committee.total_stake);
  w.f64w(saved.committee.quorum_threshold);
  w.u16w(numValidators);
  for (const v of saved.committee.validators) {
    w.u8w(v.index);
    w.str(v.hostname);
    w.f64w(v.stake);
  }

  // Blocks
  w.u32w(saved.blocks.length);
  for (const block of saved.blocks) {
    w.u32w(block.round);
    w.u8w(block.author);
    w.str(block.digest);
    w.f64w(block.timestamp_ms);
    w.u8w(block.ancestors.length);
    for (const ancestor of block.ancestors) {
      w.u32w(ancestor.round);
      w.u8w(ancestor.author);
      // Skip ancestor digest — not needed for visualization
    }
    const acknowledgments = block.acknowledgments ?? [];
    w.u16w(acknowledgments.length);
    for (const acknowledgment of acknowledgments) {
      w.u32w(acknowledgment.round);
      w.u8w(acknowledgment.author);
      w.str(acknowledgment.digest);
    }
  }

  // Leaders
  w.u32w(saved.leaders.length);
  for (const leader of saved.leaders) {
    w.u32w(leader.wave);
    w.u32w(leader.leader_round);
    w.u8w(leader.leader_authority);
    w.u8w(leader.status);
    if (leader.block_digest) {
      w.u8w(1);
      w.str(leader.block_digest);
    } else {
      w.u8w(0);
    }
  }

  return w.result();
}

// ── Decode ──────────────────────────────────────────────────────

export function decodeDagView(buf: ArrayBuffer): SavedDagView {
  const r = new BinaryReader(buf);

  // Header
  for (let i = 0; i < 4; i++) {
    if (r.u8r() !== MAGIC[i]) throw new Error('Invalid file: bad magic bytes');
  }
  const version = r.u8r();
  if (version !== FORMAT_VERSION) throw new Error(`Unsupported format version: ${version}`);
  const flags = r.u8r();
  const centerRound = r.i32r();
  const savedAtMs = r.f64r();

  // Committee
  const epoch = r.u32r();
  const total_stake = r.f64r();
  const quorum_threshold = r.f64r();
  const numValidators = r.u16r();
  const validators = [];
  for (let i = 0; i < numValidators; i++) {
    const index = r.u8r();
    const hostname = r.strr();
    const stake = r.f64r();
    validators.push({ index, hostname, stake });
  }

  // Blocks
  const numBlocks = r.u32r();
  const blocks: DagBlockMessage[] = [];
  for (let i = 0; i < numBlocks; i++) {
    const round = r.u32r();
    const author = r.u8r();
    const digest = r.strr();
    const timestamp_ms = r.f64r();
    const numAncestors = r.u8r();
    const ancestors = [];
    for (let j = 0; j < numAncestors; j++) {
      ancestors.push({ round: r.u32r(), author: r.u8r(), digest: '' });
    }
    const numAcks = r.u16r();
    const acknowledgments: BlockRefMessage[] = [];
    for (let j = 0; j < numAcks; j++) {
      acknowledgments.push({ round: r.u32r(), author: r.u8r(), digest: r.strr() });
    }
    blocks.push({ round, author, digest, timestamp_ms, ancestors, acknowledgments });
  }

  // Leaders
  const numLeaders = r.u32r();
  const leaders: LeaderInfoMessage[] = [];
  for (let i = 0; i < numLeaders; i++) {
    const wave = r.u32r();
    const leader_round = r.u32r();
    const leader_authority = r.u8r();
    const status = r.u8r();
    const block_digest = r.u8r() === 1 ? r.strr() : null;
    leaders.push({ wave, leader_round, leader_authority, status, block_digest });
  }

  return {
    version: 1,
    savedAt: new Date(savedAtMs).toISOString(),
    committee: { epoch, total_stake, quorum_threshold, validators },
    blocks,
    leaders,
    viewport: { centerRound },
    highlightSkipped: (flags & 1) !== 0,
  };
}

/** Check if a buffer starts with the DAGV magic bytes. */
export function isDagViewBinary(buf: ArrayBuffer): boolean {
  if (buf.byteLength < 4) return false;
  const u8 = new Uint8Array(buf, 0, 4);
  return u8[0] === MAGIC[0] && u8[1] === MAGIC[1] && u8[2] === MAGIC[2] && u8[3] === MAGIC[3];
}
