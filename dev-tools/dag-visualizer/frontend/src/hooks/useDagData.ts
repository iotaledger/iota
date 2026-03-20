// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { connectWebSocket, fetchDag, fetchStatus } from '../api/client';
import type { DagBlockMessage, DagVisualizerEvent, LeaderInfoMessage, StatusMessage } from '../api/types';
import {
  EVENT_BLOCK_ACCEPTED,
  EVENT_LAGGED,
  EVENT_LEADER_DECIDED,
  EVENT_ROUND_ADVANCED,
  LEADER_COMMITTED,
} from '../api/types';

export type BlockKey = number;

/** Numeric key encoding: `round * 1000 + author`. Supports up to 1000 validators. */
export function makeBlockKey(round: number, author: number): BlockKey {
  return round * 1000 + author;
}

export function roundFromKey(key: BlockKey): number {
  return Math.floor(key / 1000);
}

export function authorFromKey(key: BlockKey): number {
  return key % 1000;
}

export interface UseDagDataResult {
  blocks: Map<BlockKey, DagBlockMessage>;
  leaders: Map<number, LeaderInfoMessage>;
  status: StatusMessage | null;
  connected: boolean;
  paused: boolean;
  setPaused: (paused: boolean) => void;
  /** Increments on every data change — use as a dependency to trigger re-renders. */
  version: number;
  /** Block keys added since the last render consumed them. */
  newBlockKeys: BlockKey[];
  /** Leader rounds added/updated since the last render consumed them. */
  newLeaderRounds: number[];
  /** Equivocating block keys detected (same round+author, different digest). */
  equivocations: Set<BlockKey>;
  /** New equivocation keys since the last render consumed them. */
  newEquivocationKeys: BlockKey[];
  /** True briefly after a WebSocket reconnect (may have gaps in data). */
  justReconnected: boolean;
  /** Increments when importData() replaces all data — signals renderer reset. */
  importVersion: number;
  /** Replace all data with an imported snapshot (pauses automatically). */
  importData: (blocks: DagBlockMessage[], leaders: LeaderInfoMessage[]) => void;
  /** Fetch a round range (optionally from a specific epoch) and merge into state. */
  fetchWindow: (from: number, to: number, epoch?: number) => Promise<void>;
  /** Evict blocks and leaders outside the given round range from the data store. */
  evictRange: (keepMin: number, keepMax: number) => void;
}

const INITIAL_WINDOW_SIZE = 50;
/** Max rounds kept in memory. Older rounds are evicted. */
const MAX_RETAINED_ROUNDS = 80;

export function useDagData(): UseDagDataResult {
  // Mutable data — never cloned, mutated in place
  const blocksRef = useRef(new Map<BlockKey, DagBlockMessage>());
  const leadersRef = useRef(new Map<number, LeaderInfoMessage>());

  // Version counter triggers React re-renders when data changes
  const [version, setVersion] = useState(0);
  const [status, setStatus] = useState<StatusMessage | null>(null);
  const [connected, setConnected] = useState(false);
  const [paused, setPausedRaw] = useState(false);
  const [justReconnected, setJustReconnected] = useState(false);

  const [importVersion, setImportVersion] = useState(0);

  // Wrap setPaused to catch up on missed events when unpausing
  const setPaused = useCallback((value: boolean) => {
    const wasPaused = pausedRef.current;
    setPausedRaw(value);
    pausedRef.current = value;
    // Backfill missed events when switching from paused → unpaused
    if (wasPaused && !value) {
      fetchStatus().then((statusData) => {
        const toRound = statusData.highest_accepted_round;
        const fromRound = Math.max(1, toRound - INITIAL_WINDOW_SIZE);
        maxRoundRef.current = toRound;
        setStatus(statusData);
        fetchWindowRef.current(fromRound, toRound);
      }).catch(() => {
        // WS will catch up
      });
    }
  }, []);

  const pausedRef = useRef(paused);

  const maxRoundRef = useRef(0);
  // Ref to fetchWindow to avoid circular dependency in setPaused callback
  const fetchWindowRef = useRef<(from: number, to: number, epoch?: number) => Promise<void>>(
    async () => {},
  );

  // --- Incremental tracking for consumers ---
  // These arrays are intentionally mutable queues shared with the rendering
  // effect via refs. The producer (WS/REST callbacks) appends new keys; the
  // consumer (DagCanvas render effect) drains them by slicing and resetting
  // length to 0. This avoids cloning the entire block/leader maps on every
  // frame and lets React re-render only via the `version` counter.
  const newBlockKeysRef = useRef<BlockKey[]>([]);
  const newLeaderRoundsRef = useRef<number[]>([]);
  const equivocationsRef = useRef(new Set<BlockKey>());
  const newEquivocationKeysRef = useRef<BlockKey[]>([]);

  // --- Event batching ---
  const pendingBlocksRef = useRef<DagBlockMessage[]>([]);
  const pendingLeadersRef = useRef<LeaderInfoMessage[]>([]);
  const pendingRoundRef = useRef<number | null>(null);
  const flushScheduledRef = useRef(false);

  const evictOldRounds = useCallback(() => {
    const evictBelow = maxRoundRef.current - MAX_RETAINED_ROUNDS;
    if (evictBelow <= 0) return;

    const blocks = blocksRef.current;
    for (const [key, block] of blocks) {
      if (block.round < evictBelow) blocks.delete(key);
    }
    const leaders = leadersRef.current;
    for (const [round] of leaders) {
      if (round < evictBelow) leaders.delete(round);
    }
  }, []);

  const flushEvents = useCallback(() => {
    flushScheduledRef.current = false;

    const newBlocks = pendingBlocksRef.current;
    const newLeaders = pendingLeadersRef.current;
    const newRound = pendingRoundRef.current;
    pendingBlocksRef.current = [];
    pendingLeadersRef.current = [];
    pendingRoundRef.current = null;

    let changed = false;

    if (newBlocks.length > 0) {
      const blocks = blocksRef.current;
      for (const block of newBlocks) {
        if (block.round > maxRoundRef.current) maxRoundRef.current = block.round;
        const key = makeBlockKey(block.round, block.author);
        const existing = blocks.get(key);
        if (existing) {
          // Equivocation: same (round, author) but different digest
          if (existing.digest !== block.digest && !equivocationsRef.current.has(key)) {
            equivocationsRef.current.add(key);
            newEquivocationKeysRef.current.push(key);
          }
        } else {
          newBlockKeysRef.current.push(key);
        }
        blocks.set(key, block);
      }
      evictOldRounds();
      // Keep highest_accepted_round in sync with actual block data
      setStatus((prev) =>
        prev && maxRoundRef.current > prev.highest_accepted_round
          ? { ...prev, highest_accepted_round: maxRoundRef.current }
          : prev,
      );
      changed = true;
    }

    if (newLeaders.length > 0) {
      const leaders = leadersRef.current;
      let newCommits = 0;
      let maxCommitRound = 0;
      for (const leader of newLeaders) {
        leaders.set(leader.leader_round, leader);
        newLeaderRoundsRef.current.push(leader.leader_round);
        if (leader.status === LEADER_COMMITTED) {
          newCommits++;
          if (leader.leader_round > maxCommitRound) maxCommitRound = leader.leader_round;
        }
      }
      if (newCommits > 0) {
        setStatus((prev) => {
          if (!prev) return prev;
          return {
            ...prev,
            last_commit_round: Math.max(prev.last_commit_round, maxCommitRound),
            last_commit_index: prev.last_commit_index + newCommits,
          };
        });
      }
      changed = true;
    }

    if (newRound !== null) {
      setStatus((prev) =>
        prev ? { ...prev, highest_accepted_round: newRound } : prev,
      );
    }

    if (changed) {
      setVersion((v) => v + 1);
    }
  }, [evictOldRounds]);

  const scheduleFlush = useCallback(() => {
    if (flushScheduledRef.current) return;
    flushScheduledRef.current = true;
    requestAnimationFrame(flushEvents);
  }, [flushEvents]);

  const importData = useCallback((importBlocks: DagBlockMessage[], importLeaders: LeaderInfoMessage[]) => {
    const blocks = blocksRef.current;
    const leaders = leadersRef.current;

    blocks.clear();
    leaders.clear();
    equivocationsRef.current.clear();
    newBlockKeysRef.current.length = 0;
    newLeaderRoundsRef.current.length = 0;
    newEquivocationKeysRef.current.length = 0;
    maxRoundRef.current = 0;

    for (const block of importBlocks) {
      const key = makeBlockKey(block.round, block.author);
      blocks.set(key, block);
      newBlockKeysRef.current.push(key);
      if (block.round > maxRoundRef.current) maxRoundRef.current = block.round;
    }

    for (const leader of importLeaders) {
      leaders.set(leader.leader_round, leader);
      newLeaderRoundsRef.current.push(leader.leader_round);
    }

    // Rebuild status from imported data
    let lastCommitRound = 0;
    let lastCommitIndex = 0;
    for (const [, leader] of leaders) {
      if (leader.status === LEADER_COMMITTED) {
        lastCommitIndex++;
        if (leader.leader_round > lastCommitRound) lastCommitRound = leader.leader_round;
      }
    }
    setStatus({
      highest_accepted_round: maxRoundRef.current,
      last_commit_round: lastCommitRound,
      last_commit_index: lastCommitIndex,
      num_authorities: 0,
    });

    setPaused(true);
    setImportVersion((v) => v + 1);
    setVersion((v) => v + 1);
  }, []);

  const evictRange = useCallback((keepMin: number, keepMax: number) => {
    const blocks = blocksRef.current;
    for (const [key, block] of blocks) {
      if (block.round < keepMin || block.round > keepMax) {
        blocks.delete(key);
      }
    }
    const leaders = leadersRef.current;
    for (const [round] of leaders) {
      if (round < keepMin || round > keepMax) {
        leaders.delete(round);
      }
    }
  }, []);

  const fetchWindow = useCallback(async (from: number, to: number, epoch?: number) => {
    try {
      const dagWindow = await fetchDag(from, to, epoch);
      const blocks = blocksRef.current;

      // Evict blocks far outside the fetched range to prevent accumulation
      const evictBuffer = 50;
      const keepMin = from - evictBuffer;
      const keepMax = to + evictBuffer;
      for (const [key, block] of blocks) {
        if (block.round < keepMin || block.round > keepMax) {
          blocks.delete(key);
        }
      }
      const leaders = leadersRef.current;
      for (const [round] of leaders) {
        if (round < keepMin || round > keepMax) {
          leaders.delete(round);
        }
      }

      for (const block of dagWindow.blocks) {
        const key = makeBlockKey(block.round, block.author);
        if (!blocks.has(key)) {
          newBlockKeysRef.current.push(key);
        }
        blocks.set(key, block);
      }
      // Update maxRoundRef to reflect actual data range (not all-time high)
      let maxRound = 0;
      for (const [, block] of blocks) {
        if (block.round > maxRound) maxRound = block.round;
      }
      maxRoundRef.current = maxRound;

      for (const leader of dagWindow.leaders) {
        leaders.set(leader.leader_round, leader);
        newLeaderRoundsRef.current.push(leader.leader_round);
      }
      setVersion((v) => v + 1);
    } catch {
      // silently ignore fetch errors for window requests
    }
  }, []);
  fetchWindowRef.current = fetchWindow;

  useEffect(() => {
    let disposed = false;

    async function init() {
      try {
        const statusData = await fetchStatus();
        if (disposed) return;
        setStatus(statusData);
        maxRoundRef.current = statusData.highest_accepted_round;

        const toRound = statusData.highest_accepted_round;
        const fromRound = Math.max(1, toRound - INITIAL_WINDOW_SIZE);
        await fetchWindow(fromRound, toRound);
      } catch {
        // initial fetch failed, WS will fill in data
      }
    }

    init();

    const handleEvent = (event: DagVisualizerEvent) => {
      switch (event.t) {
        case EVENT_BLOCK_ACCEPTED: {
          if (!pausedRef.current) {
            pendingBlocksRef.current.push({
              round: event.round,
              author: event.author,
              digest: event.digest,
              timestamp_ms: event.timestamp_ms,
              ancestors: event.ancestors,
              acknowledgments: event.acknowledgments,
            });
            scheduleFlush();
          }
          break;
        }
        case EVENT_LEADER_DECIDED: {
          if (!pausedRef.current) {
            pendingLeadersRef.current.push({
              wave: event.wave,
              leader_round: event.leader_round,
              leader_authority: event.leader_authority,
              status: event.status,
              block_digest: event.block_digest,
            });
            scheduleFlush();
          }
          break;
        }
        case EVENT_ROUND_ADVANCED: {
          pendingRoundRef.current = event.round;
          scheduleFlush();
          break;
        }
        case EVENT_LAGGED: {
          // Server told us we missed events — trigger a full refresh
          handleLagged();
          break;
        }
      }
    };

    const handleLagged = async () => {
      try {
        const statusData = await fetchStatus();
        if (disposed) return;
        setStatus(statusData);
        const toRound = statusData.highest_accepted_round;
        const fromRound = Math.max(1, toRound - INITIAL_WINDOW_SIZE);
        maxRoundRef.current = toRound;
        await fetchWindow(fromRound, toRound);
      } catch {
        // fetch failed, next event batch will catch up
      }
    };

    let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
    const handleReconnect = async () => {
      // Just jump to the current position — don't backfill the gap
      setJustReconnected(true);
      if (reconnectTimer) clearTimeout(reconnectTimer);
      reconnectTimer = setTimeout(() => setJustReconnected(false), 5000);
      try {
        const statusData = await fetchStatus();
        if (disposed) return;
        setStatus(statusData);
        const toRound = statusData.highest_accepted_round;
        const fromRound = Math.max(1, toRound - INITIAL_WINDOW_SIZE);
        maxRoundRef.current = toRound;
        await fetchWindow(fromRound, toRound);
      } catch {
        // ignore — WS will catch up
      }
    };

    const cleanup = connectWebSocket(
      (event) => {
        if (!disposed) {
          setConnected(true);
          handleEvent(event);
        }
      },
      handleReconnect,
    );

    return () => {
      disposed = true;
      if (reconnectTimer) clearTimeout(reconnectTimer);
      cleanup();
    };
  }, [fetchWindow, scheduleFlush]);

  return useMemo(() => ({
    blocks: blocksRef.current,
    leaders: leadersRef.current,
    status,
    connected,
    justReconnected,
    paused,
    setPaused,
    version,
    newBlockKeys: newBlockKeysRef.current,
    newLeaderRounds: newLeaderRoundsRef.current,
    equivocations: equivocationsRef.current,
    newEquivocationKeys: newEquivocationKeysRef.current,
    importVersion,
    importData,
    fetchWindow,
    evictRange,
  }), [version, status, connected, paused, justReconnected, importVersion, setPaused, importData, fetchWindow, evictRange]);
}
