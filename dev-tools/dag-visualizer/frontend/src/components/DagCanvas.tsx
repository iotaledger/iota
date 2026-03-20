// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef, useState } from 'react';
import type { CommitteeMessage, DagBlockMessage } from '../api/types';
import { LEADER_COMMITTED } from '../api/types';
import type { BlockKey, UseDagDataResult } from '../hooks/useDagData';
import { authorFromKey, makeBlockKey, roundFromKey } from '../hooks/useDagData';
import { DagRenderer, VISIBLE_ROUNDS } from '../pixi/DagRenderer';
import { Tooltip } from './Tooltip';

interface DagCanvasProps {
  dagData: UseDagDataResult;
  committee: CommitteeMessage | null;
  onRendererReady?: (renderer: DagRenderer) => void;
  /** When true, block eviction is disabled (imported snapshot mode). */
  disableEviction?: boolean;
}

interface TooltipState {
  block: DagBlockMessage;
  x: number;
  y: number;
}

/** minRound advances in steps of this size to batch eviction. */
const MIN_ROUND_STEP = 5;

export function DagCanvas({ dagData, committee, onRendererReady, disableEviction }: DagCanvasProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const rendererRef = useRef<DagRenderer | null>(null);
  const renderedBlocksRef = useRef<Set<BlockKey>>(new Set());
  const childrenOfRef = useRef<Map<BlockKey, Set<BlockKey>>>(new Map());
  const [tooltip, setTooltip] = useState<TooltipState | null>(null);
  const [rendererReady, setRendererReady] = useState(false);
  const [propagationKey, setPropagationKey] = useState<BlockKey | null>(null);

  const displayMinRef = useRef(Infinity);
  const displayMaxRef = useRef(-Infinity);
  const prevImportVersionRef = useRef(dagData.importVersion);
  // Stable ref for the callback so the init effect doesn't need to re-run
  const onRendererReadyRef = useRef(onRendererReady);
  onRendererReadyRef.current = onRendererReady;

  const { blocks, leaders, paused, version } = dagData;

  // Initialize renderer
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    let cancelled = false;
    const renderer = new DagRenderer(canvas);
    rendererRef.current = renderer;

    renderer.init().then(() => {
      if (cancelled) return;

      renderer.onBlockHover((block, x, y) => {
        if (renderer.getPinnedBlock()) {
          // Don't change tooltip while a block is pinned
          return;
        }
        if (block) {
          setTooltip({ block, x, y });
        } else {
          setTooltip(null);
        }
      });

      renderer.onBlockClick((block, x, y) => {
        const pinned = renderer.togglePin(block);
        if (pinned) {
          setTooltip({ block, x, y });
        } else {
          setTooltip(null);
        }
        setPropagationKey(pinned ? makeBlockKey(block.round, block.author) : null);
      });

      renderer.onUnpin(() => {
        setTooltip(null);
        setPropagationKey(null);
      });

      setRendererReady(true);
      onRendererReadyRef.current?.(renderer);
    }).catch(console.error);

    const handleResize = () => renderer.resize();
    window.addEventListener('resize', handleResize);

    return () => {
      cancelled = true;
      window.removeEventListener('resize', handleResize);
      renderer.destroy();
      rendererRef.current = null;
      renderedBlocksRef.current.clear();
      childrenOfRef.current.clear();
      displayMinRef.current = Infinity;
      displayMaxRef.current = -Infinity;
      setRendererReady(false);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- init once; onRendererReady accessed via ref
  }, []);

  // Toggle viewport interactivity based on pause state (disabled for imported snapshots)
  useEffect(() => {
    const renderer = rendererRef.current;
    if (!renderer || !rendererReady) return;
    renderer.setInteractive(paused && !disableEviction);
  }, [paused, rendererReady, disableEviction]);

  // When the renderer becomes ready, seed newBlockKeys with any blocks already fetched
  // (fixes race where initial fetch completes before PixiJS init finishes)
  useEffect(() => {
    if (!rendererReady) return;
    // Seed the incremental queues with already-fetched data. This handles the
    // race where the initial REST fetch completes before PixiJS init finishes.
    // The arrays are intentionally mutable queues drained by the rendering effect.
    if (blocks.size > 0 && dagData.newBlockKeys.length === 0) {
      dagData.newBlockKeys.push(...blocks.keys());
      for (const [round] of leaders) {
        dagData.newLeaderRounds.push(round);
      }
    }
  }, [rendererReady]);

  // Render only NEW blocks/edges/leaders whenever data changes
  useEffect(() => {
    const renderer = rendererRef.current;
    if (!renderer || !rendererReady) return;

    // Reset on import
    if (dagData.importVersion !== prevImportVersionRef.current) {
      prevImportVersionRef.current = dagData.importVersion;
      renderer.reset();
      renderedBlocksRef.current.clear();
      childrenOfRef.current.clear();
      displayMinRef.current = Infinity;
      displayMaxRef.current = -Infinity;
      setPropagationKey(null);
    }

    const numAuthorities = committee?.validators.length ?? 0;
    if (numAuthorities === 0) return;

    // Drain new-data queues
    const newBlockKeys = dagData.newBlockKeys;
    const newLeaderRounds = dagData.newLeaderRounds;
    const blockKeysToProcess = newBlockKeys.slice();
    const leaderRoundsToProcess = newLeaderRounds.slice();
    newBlockKeys.length = 0;
    newLeaderRounds.length = 0;

    // Update display bounds from new blocks
    for (const key of blockKeysToProcess) {
      const block = blocks.get(key);
      if (!block) continue;
      if (block.round < displayMinRef.current) displayMinRef.current = block.round;
      if (block.round > displayMaxRef.current) displayMaxRef.current = block.round;
    }

    // Nothing to render yet (no data at all)
    if (displayMinRef.current === Infinity) return;

    // --- Eviction: skip entirely for imported snapshots (finite data, no refetch) ---
    if (!disableEviction) {
      if (paused) {
        // When paused, evict based on viewport center (±50 rounds)
        const { centerRound } = renderer.getView();
        const buffer = 50;
        const keepMin = centerRound - buffer;
        const keepMax = centerRound + buffer;

        // Evict from data store via the dedicated method (avoids direct mutation)
        dagData.evictRange(keepMin, keepMax);

        // Evict from renderer
        renderer.evictOutside(keepMin, keepMax);

        for (const key of renderedBlocksRef.current) {
          if (!blocks.has(key)) {
            renderedBlocksRef.current.delete(key);
            childrenOfRef.current.delete(key);
          }
        }

        displayMinRef.current = Math.max(displayMinRef.current, keepMin);
        displayMaxRef.current = Math.min(displayMaxRef.current, keepMax);
      } else {
        // In live mode, evict rounds outside the visible window
        const dataMax = displayMaxRef.current;
        const keepRounds = VISIBLE_ROUNDS + 20;
        const targetMin = dataMax - keepRounds;
        if (targetMin > displayMinRef.current + MIN_ROUND_STEP) {
          displayMinRef.current = targetMin;
          renderer.evictBefore(targetMin);

          for (const key of renderedBlocksRef.current) {
            if (!blocks.has(key)) {
              renderedBlocksRef.current.delete(key);
              childrenOfRef.current.delete(key);
            }
          }
        }
      }
    }

    // If no new data to process, stop here (eviction above still ran)
    if (blockKeysToProcess.length === 0 && leaderRoundsToProcess.length === 0) return;

    const hostnames = committee?.validators.map((v) => v.hostname) ?? [];
    const stakes = committee?.validators.map((v) => v.stake);
    const totalStake = committee?.total_stake;
    renderer.drawGrid(numAuthorities, displayMinRef.current, displayMaxRef.current, hostnames, stakes, totalStake);

    // Add new blocks and build reverse index
    for (const key of blockKeysToProcess) {
      const block = blocks.get(key);
      if (!block || renderedBlocksRef.current.has(key)) continue;

      for (const ancestor of block.ancestors) {
        const ancestorKey = makeBlockKey(ancestor.round, ancestor.author);
        let children = childrenOfRef.current.get(ancestorKey);
        if (!children) {
          children = new Set();
          childrenOfRef.current.set(ancestorKey, children);
        }
        children.add(key);
      }

      const leaderInfo = leaders.get(block.round);
      const isLeader = leaderInfo !== undefined && leaderInfo.leader_authority === block.author;
      const leaderStatus = isLeader ? leaderInfo!.status : 0;
      renderer.addBlock(block, isLeader, leaderStatus);
      renderedBlocksRef.current.add(key);
    }

    // Update leader statuses
    for (const round of leaderRoundsToProcess) {
      const leaderInfo = leaders.get(round);
      if (leaderInfo) {
        renderer.updateLeaderStatus(
          leaderInfo.leader_round,
          leaderInfo.leader_authority,
          leaderInfo.status,
        );
      }
    }

    // Mark missing slots, update health bars and round durations
    renderer.markMissingSlots(displayMaxRef.current, numAuthorities);
    renderer.updateHealthBars();
    renderer.updateRoundDurations();

    // Process new equivocations
    const newEquivKeys = dagData.newEquivocationKeys.slice();
    dagData.newEquivocationKeys.length = 0;
    for (const key of newEquivKeys) {
      renderer.markEquivocation(roundFromKey(key), authorFromKey(key));
    }

    // In live mode: snap viewport so latest round is at the bottom edge
    if (!paused) {
      renderer.snapToView(displayMaxRef.current);
    }
  }, [version, committee, paused, rendererReady, disableEviction]);

  // Click overlay: commit sub-DAG for committed leaders, propagation heatmap for others
  useEffect(() => {
    const renderer = rendererRef.current;
    if (!renderer || !rendererReady) return;

    if (propagationKey === null) {
      renderer.clearPropagationHeatmap();
      return;
    }

    const block = blocks.get(propagationKey);
    if (!block) {
      renderer.clearPropagationHeatmap();
      return;
    }

    // Check if this is a committed leader → show acknowledged blocks
    const leaderInfo = leaders.get(block.round);
    const leaderBlock = blocks.get(propagationKey);
    const isCommittedLeader =
      leaderInfo &&
      leaderInfo.leader_authority === block.author &&
      leaderInfo.status === LEADER_COMMITTED &&
      leaderBlock &&
      leaderBlock.acknowledgments &&
      leaderBlock.acknowledgments.length > 0;

    if (isCommittedLeader) {
      const subDagKeys: number[] = [];
      for (const ack of leaderBlock!.acknowledgments) {
        subDagKeys.push(makeBlockKey(ack.round, ack.author));
      }
      renderer.showCommitSubDag(subDagKeys);
      return;
    }

    // Regular block: show propagation heatmap
    const children = childrenOfRef.current.get(propagationKey);
    if (!children || children.size === 0) {
      renderer.clearPropagationHeatmap();
      return;
    }

    const references: Array<{ round: number; author: number; deltaMs: number }> = [];
    for (const childKey of children) {
      const childBlock = blocks.get(childKey);
      if (childBlock) {
        references.push({
          round: childBlock.round,
          author: childBlock.author,
          deltaMs: Math.max(0, childBlock.timestamp_ms - block.timestamp_ms),
        });
      }
    }
    renderer.showPropagationHeatmap(references);
  }, [propagationKey, rendererReady]);

  return (
    <div className="relative w-full h-full overflow-hidden">
      <canvas ref={canvasRef} className="w-full h-full block" />
      {tooltip && (
        <Tooltip block={tooltip.block} x={tooltip.x} y={tooltip.y} leaders={leaders} committee={committee} />
      )}
    </div>
  );
}
