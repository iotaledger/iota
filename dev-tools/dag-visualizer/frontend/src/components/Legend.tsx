// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { COLORS, toHex } from '../pixi/colors';

function Circle({ color }: { color: string }) {
  return (
    <svg width="14" height="14" viewBox="0 0 14 14">
      <circle cx="7" cy="7" r="6" fill={color} />
    </svg>
  );
}

function Diamond({ color, stroke }: { color: string; stroke?: string }) {
  return (
    <svg width="14" height="14" viewBox="0 0 14 14">
      <rect
        x="2"
        y="2"
        width="10"
        height="10"
        rx="1"
        fill={color}
        stroke={stroke ?? 'var(--dag-text-dim)'}
        strokeWidth="1"
        transform="rotate(45 7 7)"
      />
    </svg>
  );
}

function Line({ color }: { color: string }) {
  return (
    <svg width="24" height="14" viewBox="0 0 24 14">
      <line x1="2" y1="7" x2="22" y2="7" stroke={color} strokeWidth="2" strokeLinecap="round" />
    </svg>
  );
}

function GlowCircle({ color }: { color: string }) {
  return (
    <svg width="14" height="14" viewBox="0 0 14 14">
      <circle cx="7" cy="7" r="6" fill={color} opacity="0.4" />
      <circle cx="7" cy="7" r="6" fill="none" stroke={color} strokeWidth="2" opacity="0.9" />
    </svg>
  );
}

function LatencyScale({ low, mid, high }: { low: string; mid: string; high: string }) {
  return (
    <svg width="48" height="14" viewBox="0 0 48 14">
      <circle cx="7" cy="7" r="6" fill={low} />
      <circle cx="24" cy="7" r="4.5" fill={mid} />
      <circle cx="41" cy="7" r="3.5" fill={high} />
    </svg>
  );
}

function XMark({ color }: { color: string }) {
  return (
    <svg width="14" height="14" viewBox="0 0 14 14">
      <circle cx="7" cy="7" r="6" fill="none" stroke={color} strokeWidth="1.5" />
      <line x1="4" y1="4" x2="10" y2="10" stroke={color} strokeWidth="1.5" strokeLinecap="round" />
      <line x1="10" y1="4" x2="4" y2="10" stroke={color} strokeWidth="1.5" strokeLinecap="round" />
    </svg>
  );
}

function EquivocationRing({ color }: { color: string }) {
  return (
    <svg width="14" height="14" viewBox="0 0 14 14">
      <circle cx="7" cy="7" r="5.5" fill="none" stroke={color} strokeWidth="2" />
      <circle cx="7" cy="7" r="3" fill={color} opacity="0.3" />
    </svg>
  );
}

function DurationBar({ good, warn, bad }: { good: string; warn: string; bad: string }) {
  return (
    <svg width="32" height="14" viewBox="0 0 32 14">
      <rect x="1" y="4" width="9" height="6" rx="1" fill={good} opacity="0.7" />
      <rect x="12" y="4" width="9" height="6" rx="1" fill={warn} opacity="0.7" />
      <rect x="23" y="4" width="8" height="6" rx="1" fill={bad} opacity="0.7" />
    </svg>
  );
}

function getShapeItems() {
  return [
    { shape: <Circle color={toHex(COLORS.regularBlock)} />, label: 'Regular block' },
    { shape: <Diamond color={toHex(COLORS.committedLeader)} />, label: 'Committed leader' },
    { shape: <Diamond color={toHex(COLORS.skippedLeader)} />, label: 'Skipped leader' },
    { shape: <XMark color={toHex(COLORS.skippedLeader)} />, label: 'Missing block' },
    { shape: <GlowCircle color={toHex(COLORS.edge)} />, label: 'Parent (hover)' },
    { shape: <GlowCircle color={toHex(COLORS.healthWarn)} />, label: 'Stale parent, 2 rounds (hover)' },
    { shape: <GlowCircle color={toHex(COLORS.healthBad)} />, label: 'Very stale parent, 3+ (hover)' },
    { shape: <GlowCircle color={toHex(COLORS.committedGlow)} />, label: 'Acknowledgments (hover)' },
    { shape: <GlowCircle color="#22d3ee" />, label: 'Child (hover)' },
    { shape: <GlowCircle color="#c084fc" />, label: 'Pinned block (click)' },
    { shape: <Line color={toHex(COLORS.commitChain)} />, label: 'Commit chain' },
    { shape: <LatencyScale low={toHex(COLORS.regularBlock)} mid={toHex(COLORS.healthWarn)} high={toHex(COLORS.healthBad)} />, label: 'Low → High latency' },
    { shape: <EquivocationRing color={toHex(COLORS.equivocation)} />, label: 'Equivocation' },
    { shape: <DurationBar good={toHex(COLORS.healthGood)} warn={toHex(COLORS.healthWarn)} bad={toHex(COLORS.healthBad)} />, label: 'Round duration (fast → slow)' },
  ];
}

export function Legend() {
  const [collapsed, setCollapsed] = useState(true);

  return (
    <div className="dag-panel absolute bottom-4 right-4 border rounded-lg px-4 py-3">
      <div className="flex items-center gap-2 mb-1">
        <span className="text-xs font-semibold dag-label uppercase tracking-wider">Legend</span>
        <button
          onClick={() => setCollapsed(!collapsed)}
          className="w-4 h-4 rounded-full text-[10px] leading-none flex items-center justify-center transition-colors"
          style={{ background: 'var(--dag-input-border)', color: 'var(--dag-text)' }}
        >
          {collapsed ? '+' : '\u2212'}
        </button>
      </div>
      {!collapsed && (
        <div className="flex flex-col gap-1.5 mt-1">
          {getShapeItems().map((item) => (
            <div key={item.label} className="flex items-center gap-2">
              {item.shape}
              <span className="text-xs dag-value">{item.label}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
