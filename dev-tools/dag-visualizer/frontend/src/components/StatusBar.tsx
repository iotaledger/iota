// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import type { LeaderInfoMessage, StatusMessage } from '../api/types';
import { LEADER_COMMITTED, LEADER_SKIPPED } from '../api/types';

interface StatusBarProps {
  status: StatusMessage | null;
  connected: boolean;
  justReconnected?: boolean;
  leaders?: Map<number, LeaderInfoMessage>;
  equivocationCount?: number;
  onEquivocationClick?: () => void;
}

export function StatusBar({ status, connected, justReconnected, leaders, equivocationCount = 0, onEquivocationClick }: StatusBarProps) {
  let committed = 0;
  let skipped = 0;
  if (leaders) {
    for (const [, leader] of leaders) {
      if (leader.status === LEADER_COMMITTED) committed++;
      else if (leader.status === LEADER_SKIPPED) skipped++;
    }
  }

  return (
    <div className="dag-panel flex items-center gap-6 px-4 py-2 border-b text-sm">
      <div className="flex items-center gap-2">
        <div
          className={`w-2.5 h-2.5 rounded-full ${connected ? 'bg-green-500' : 'bg-red-500'}`}
        />
        <span className="dag-label">
          {connected ? 'Connected' : 'Disconnected'}
        </span>
      </div>

      {status && (
        <>
          <div className="flex items-center gap-2">
            <span className="dag-label">Highest Round:</span>
            <span className="font-mono dag-value">
              {status.highest_accepted_round}
            </span>
          </div>

          <div className="flex items-center gap-2">
            <span className="dag-label">Last Commit Round:</span>
            <span className="font-mono dag-value">
              {status.last_commit_round}
            </span>
          </div>

          <div className="flex items-center gap-2">
            <span className="dag-label">Commit Index:</span>
            <span className="font-mono dag-value">
              {status.last_commit_index}
            </span>
          </div>
        </>
      )}

      {leaders && leaders.size > 0 && (
        <div className="flex items-center gap-3">
          <span className="dag-label">Leaders:</span>
          <span className="font-mono dag-status-good">{committed}</span>
          <span className="dag-label">/</span>
          <span className="font-mono dag-status-bad">{skipped}</span>
        </div>
      )}

      {equivocationCount > 0 && (
        <button
          onClick={onEquivocationClick}
          className="flex items-center gap-2 px-2 py-0.5 dag-alert-bad border rounded animate-pulse cursor-pointer transition-opacity"
        >
          <span className="font-medium text-xs">
            {equivocationCount} Equivocation{equivocationCount > 1 ? 's' : ''} Detected
          </span>
        </button>
      )}

      {justReconnected && (
        <div className="flex items-center gap-2 px-2 py-0.5 dag-alert-warn border rounded">
          <span className="font-medium text-xs">
            Reconnected — some blocks may be missing
          </span>
        </div>
      )}

      <div className="ml-auto text-xs" style={{ color: 'var(--dag-text-dim)' }}>
        Starfish DAG Visualizer
      </div>
    </div>
  );
}
