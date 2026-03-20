// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import type { CommitteeMessage, DagBlockMessage, LeaderInfoMessage } from '../api/types';
import { LEADER_COMMITTED, LEADER_SKIPPED } from '../api/types';

interface TooltipProps {
  block: DagBlockMessage;
  x: number;
  y: number;
  leaders?: Map<number, LeaderInfoMessage>;
  committee?: CommitteeMessage | null;
}

function leaderStatusLabel(status: number): { text: string; color: string } {
  switch (status) {
    case LEADER_COMMITTED:
      return { text: 'Committed', color: 'dag-status-good' };
    case LEADER_SKIPPED:
      return { text: 'Skipped', color: 'dag-status-bad' };
    default:
      return { text: 'Skipped', color: 'dag-status-bad' };
  }
}

export function Tooltip({ block, x, y, leaders, committee }: TooltipProps) {
  const [copied, setCopied] = useState(false);
  const tooltipRef = useRef<HTMLDivElement>(null);
  const copiedTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [pos, setPos] = useState({ left: x + 12, top: y + 12 });
  const timestamp = new Date(block.timestamp_ms).toISOString();
  const shortDigest = block.digest.length > 16
    ? block.digest.slice(0, 8) + '…' + block.digest.slice(-8)
    : block.digest;

  // Clear copied timer on unmount
  useEffect(() => {
    return () => {
      if (copiedTimerRef.current) {
        clearTimeout(copiedTimerRef.current);
      }
    };
  }, []);

  const handleCopyDigest = () => {
    navigator.clipboard.writeText(block.digest).then(() => {
      setCopied(true);
      if (copiedTimerRef.current) clearTimeout(copiedTimerRef.current);
      copiedTimerRef.current = setTimeout(() => setCopied(false), 1500);
    });
  };

  // Check if this block is a leader
  const leaderInfo = leaders?.get(block.round);
  const isLeader = leaderInfo !== undefined && leaderInfo.leader_authority === block.author;
  const statusInfo = isLeader ? leaderStatusLabel(leaderInfo!.status) : null;

  // Resolve author display name from committee data
  const authorName = committee?.validators[block.author]?.hostname ?? block.author;

  // Position the tooltip so it doesn't overflow the container
  useLayoutEffect(() => {
    const el = tooltipRef.current;
    if (!el) return;
    const rect = el.getBoundingClientRect();
    const container = el.offsetParent as HTMLElement | null;
    const containerRect = container?.getBoundingClientRect() ?? {
      right: window.innerWidth,
      bottom: window.innerHeight,
    };

    let left = x + 12;
    let top = y + 12;

    if (rect.bottom > containerRect.bottom) {
      top = y - rect.height - 12;
    }
    if (rect.right > containerRect.right) {
      left = x - rect.width - 12;
    }

    top = Math.max(4, top);
    left = Math.max(4, left);

    setPos({ left, top });
  }, [x, y]);

  // The tooltip container is pointer-events-none so it doesn't interfere with
  // canvas hover detection. The digest copy button opts back in via
  // pointer-events-auto to remain clickable.
  return (
    <div
      ref={tooltipRef}
      className="dag-panel absolute z-50 border rounded-lg px-3 py-2 shadow-xl pointer-events-none"
      style={{
        left: `${pos.left}px`,
        top: `${pos.top}px`,
        maxWidth: '280px',
      }}
    >
      <div className="text-xs space-y-1">
        <div className="flex justify-between gap-4">
          <span className="dag-label">Round:</span>
          <span className="font-mono dag-value">{block.round}</span>
        </div>
        <div className="flex justify-between gap-4">
          <span className="dag-label">Author:</span>
          <span className="font-mono dag-value">{authorName}</span>
        </div>
        <div className="flex justify-between gap-4">
          <span className="dag-label">Digest:</span>
          <button
            onClick={handleCopyDigest}
            className="font-mono dag-value dag-accent-hover cursor-pointer pointer-events-auto transition-colors"
            title="Click to copy full digest"
          >
            {copied ? 'Copied!' : shortDigest}
          </button>
        </div>
        <div className="flex justify-between gap-4">
          <span className="dag-label">Timestamp:</span>
          <span className="font-mono dag-value text-[10px]">{timestamp}</span>
        </div>
        <div className="flex justify-between gap-4">
          <span className="dag-label">Ancestors:</span>
          <span className="font-mono dag-value">{block.ancestors.length}</span>
        </div>
        {isLeader && statusInfo && (
          <>
            <div className="flex justify-between gap-4">
              <span className="dag-label">Leader:</span>
              <span className={`font-mono font-medium ${statusInfo.color}`}>
                {statusInfo.text}
              </span>
            </div>
            <div className="flex justify-between gap-4">
              <span className="dag-label">Wave:</span>
              <span className="font-mono dag-value">{leaderInfo!.wave}</span>
            </div>
          </>
        )}
      </div>
    </div>
  );
}
