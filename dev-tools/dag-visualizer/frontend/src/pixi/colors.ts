// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
export type Theme = 'dark' | 'light';

export interface ThemeColors {
  committedLeader: number;
  skippedLeader: number;
  regularBlock: number;
  missingSlot: number;
  edge: number;
  committedGlow: number;
  uncommittedGlow: number;
  commitChain: number;
  gridLine: number;
  waveBandA: number;
  waveBandB: number;
  waveBoundary: number;
  waveLabel: number;
  healthGood: number;
  healthWarn: number;
  healthBad: number;
  healthBg: number;
  background: number;
  equivocation: number;
  searchHighlight: number;
  labelText: number;
}

const DARK_COLORS: ThemeColors = {
  committedLeader: 0x22c55e,
  skippedLeader: 0xef4444,
  regularBlock: 0x6b7280,
  missingSlot: 0x374151,
  edge: 0x94a3b8,
  committedGlow: 0x4ade80,
  uncommittedGlow: 0xf87171,
  commitChain: 0x22c55e,
  gridLine: 0x1e293b,
  waveBandA: 0x0f172a,
  waveBandB: 0x1a2332,
  waveBoundary: 0x475569,
  waveLabel: 0x64748b,
  healthGood: 0x22c55e,
  healthWarn: 0xeab308,
  healthBad: 0xef4444,
  healthBg: 0x1e293b,
  background: 0x0f172a,
  equivocation: 0xff2222,
  searchHighlight: 0x60a5fa,
  labelText: 0x94a3b8,
};

const LIGHT_COLORS: ThemeColors = {
  committedLeader: 0x0d9668,
  skippedLeader: 0xd94040,
  regularBlock: 0x4a6fa5,
  missingSlot: 0xc4d0de,
  edge: 0x7a9abb,
  committedGlow: 0x34d399,
  uncommittedGlow: 0xf87171,
  commitChain: 0x0d9668,
  gridLine: 0xdce4ee,
  waveBandA: 0xf5f7fb,
  waveBandB: 0xeaeff8,
  waveBoundary: 0xafc0d4,
  waveLabel: 0x7a8ea6,
  healthGood: 0x0d9668,
  healthWarn: 0xe28c0a,
  healthBad: 0xd94040,
  healthBg: 0xdce4ee,
  background: 0xf5f7fb,
  equivocation: 0xd94040,
  searchHighlight: 0x3b82f6,
  labelText: 0x35506e,
};

export const THEME_COLORS: Record<Theme, ThemeColors> = { dark: DARK_COLORS, light: LIGHT_COLORS };

/** Mutable reference to the active color set. Defaults to dark. */
export let COLORS: ThemeColors = DARK_COLORS;

export function setActiveTheme(theme: Theme): void {
  COLORS = THEME_COLORS[theme];
}

/** Convert a PixiJS numeric color to a CSS hex string. */
export function toHex(color: number): string {
  return '#' + color.toString(16).padStart(6, '0');
}

/** Linearly interpolate between two hex colors. `t` clamped to [0, 1]. */
export function lerpColor(c1: number, c2: number, t: number): number {
  const ct = Math.max(0, Math.min(1, t));
  const r1 = (c1 >> 16) & 0xff, g1 = (c1 >> 8) & 0xff, b1 = c1 & 0xff;
  const r2 = (c2 >> 16) & 0xff, g2 = (c2 >> 8) & 0xff, b2 = c2 & 0xff;
  const r = Math.round(r1 + (r2 - r1) * ct);
  const g = Math.round(g1 + (g2 - g1) * ct);
  const b = Math.round(b1 + (b2 - b1) * ct);
  return (r << 16) | (g << 8) | b;
}
