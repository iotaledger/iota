// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { useMemo, useState } from 'react';
import { LEADER_COMMITTED, LEADER_SKIPPED } from '../api/types';
import type { UseDagDataResult } from '../hooks/useDagData';

interface StatsPanelProps {
  dagData: UseDagDataResult;
  numAuthorities: number;
}

export function StatsPanel({ dagData, numAuthorities }: StatsPanelProps) {
  const [collapsed, setCollapsed] = useState(true);

  const stats = useMemo(() => {
    const { blocks, leaders } = dagData;

    // Block production rate
    let minRound = Infinity;
    let maxRound = -Infinity;
    for (const [, block] of blocks) {
      if (block.round < minRound) minRound = block.round;
      if (block.round > maxRound) maxRound = block.round;
    }
    const totalRounds = isFinite(minRound) ? maxRound - minRound + 1 : 0;
    const totalSlots = totalRounds * numAuthorities;
    const blockRate = totalSlots > 0 ? (blocks.size / totalSlots) * 100 : 0;

    // Average round duration from block timestamps
    const roundTimestamps = new Map<number, number[]>();
    for (const [, block] of blocks) {
      const arr = roundTimestamps.get(block.round) ?? [];
      arr.push(block.timestamp_ms);
      roundTimestamps.set(block.round, arr);
    }
    const roundMedians = new Map<number, number>();
    for (const [round, timestamps] of roundTimestamps) {
      timestamps.sort((a, b) => a - b);
      roundMedians.set(round, timestamps[Math.floor(timestamps.length / 2)]);
    }
    const sortedRounds = [...roundMedians.keys()].sort((a, b) => a - b);
    let totalDelta = 0;
    let deltaCount = 0;
    for (let i = 1; i < sortedRounds.length; i++) {
      const delta = roundMedians.get(sortedRounds[i])! - roundMedians.get(sortedRounds[i - 1])!;
      if (delta > 0) {
        totalDelta += delta;
        deltaCount++;
      }
    }
    const avgRoundDuration = deltaCount > 0 ? totalDelta / deltaCount : 0;

    // Leader stats
    let committed = 0;
    let skipped = 0;
    for (const [, leader] of leaders) {
      if (leader.status === LEADER_COMMITTED) committed++;
      else if (leader.status === LEADER_SKIPPED) skipped++;
    }
    const totalDecided = committed + skipped;
    const commitRate = totalDecided > 0 ? (committed / totalDecided) * 100 : 0;
    const skipRate = totalDecided > 0 ? (skipped / totalDecided) * 100 : 0;

    return {
      totalRounds,
      blockRate,
      avgRoundDuration,
      commitRate,
      skipRate,
      totalBlocks: blocks.size,
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [dagData.version, numAuthorities]);

  return (
    <div className="dag-panel absolute bottom-4 left-4 border rounded-lg px-4 py-3">
      <div className="flex items-center gap-2 mb-1">
        <span className="text-xs font-semibold dag-label uppercase tracking-wider">Statistics</span>
        <button
          onClick={() => setCollapsed(!collapsed)}
          className="w-4 h-4 rounded-full text-[10px] leading-none flex items-center justify-center transition-colors"
          style={{ background: 'var(--dag-input-border)', color: 'var(--dag-text)' }}
        >
          {collapsed ? '+' : '\u2212'}
        </button>
      </div>
      {!collapsed && (
        <div className="grid grid-cols-2 gap-x-6 gap-y-1 text-xs">
          <span className="dag-label">Block Rate:</span>
          <span className="font-mono dag-value">{stats.blockRate.toFixed(1)}%</span>
          <span className="dag-label">Avg Round:</span>
          <span className="font-mono dag-value">{stats.avgRoundDuration.toFixed(0)}ms</span>
          <span className="dag-label">Commit Rate:</span>
          <span className="font-mono dag-status-good">{stats.commitRate.toFixed(1)}%</span>
          <span className="dag-label">Skip Rate:</span>
          <span className="font-mono dag-status-bad">{stats.skipRate.toFixed(1)}%</span>
          <span className="dag-label">Rounds:</span>
          <span className="font-mono dag-value">{stats.totalRounds}</span>
          <span className="dag-label">Blocks:</span>
          <span className="font-mono dag-value">{stats.totalBlocks}</span>
        </div>
      )}
    </div>
  );
}
