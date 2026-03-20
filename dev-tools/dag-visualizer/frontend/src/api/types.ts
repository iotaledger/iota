// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
//
// NOTE: These types must stay in sync with the server's types.rs
// (dev-tools/dag-visualizer/server/src/types.rs). The server uses u8 for
// author fields; here they are `number`.
export interface BlockRefMessage {
  round: number;
  author: number;
  /**
   * Short hex digest. May be an empty string for ancestor references loaded
   * from storage (only `round` + `author` are persisted per ancestor slot).
   */
  digest: string;
}

export interface DagBlockMessage {
  round: number;
  author: number;
  digest: string;
  timestamp_ms: number;
  ancestors: BlockRefMessage[];
  /** Block refs acknowledged by this block at accept time. */
  acknowledgments: BlockRefMessage[];
}

/** Leader status: 0 = committed, 1 = skipped */
export const LEADER_COMMITTED = 0;
export const LEADER_SKIPPED = 1;

export interface LeaderInfoMessage {
  wave: number;
  leader_round: number;
  leader_authority: number;
  status: number;
  block_digest: string | null;
}

export interface ValidatorMessage {
  index: number;
  hostname: string;
  stake: number;
}

export interface CommitteeMessage {
  epoch: number;
  total_stake: number;
  quorum_threshold: number;
  validators: ValidatorMessage[];
}

export interface DagWindowMessage {
  from_round: number;
  to_round: number;
  highest_accepted_round: number;
  last_commit_round: number;
  blocks: DagBlockMessage[];
  leaders: LeaderInfoMessage[];
}

export interface StatusMessage {
  highest_accepted_round: number;
  last_commit_index: number;
  last_commit_round: number;
  num_authorities: number;
}

export interface EpochInfo {
  epoch: number;
  from_round: number;
  to_round: number;
}

/** Event type discriminants matching backend `"t"` field */
export const EVENT_BLOCK_ACCEPTED = 0;
export const EVENT_LEADER_DECIDED = 1;
export const EVENT_ROUND_ADVANCED = 2;
/** Sent when the WebSocket client falls behind the broadcast channel. */
export const EVENT_LAGGED = 3;

export type DagVisualizerEvent =
  | {
      t: typeof EVENT_BLOCK_ACCEPTED;
      round: number;
      author: number;
      digest: string;
      timestamp_ms: number;
      ancestors: BlockRefMessage[];
      acknowledgments: BlockRefMessage[];
    }
  | {
      t: typeof EVENT_LEADER_DECIDED;
      wave: number;
      leader_round: number;
      leader_authority: number;
      status: number;
      block_digest: string | null;
    }
  | {
      t: typeof EVENT_ROUND_ADVANCED;
      round: number;
    }
  | {
      t: typeof EVENT_LAGGED;
      missed: number;
    };

/** Serializable snapshot of the entire visualizer state for save/load/share. */
export interface SavedDagView {
  version: 1;
  savedAt: string;
  committee: CommitteeMessage;
  blocks: DagBlockMessage[];
  leaders: LeaderInfoMessage[];
  viewport: { centerRound: number; scale?: number };
  /** @deprecated — zoom is no longer configurable; kept optional for backwards compat. */
  windowSize?: number;
  highlightSkipped: boolean;
}
