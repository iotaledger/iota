// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { Graphics } from 'pixi.js';
import { LEADER_COMMITTED, LEADER_SKIPPED } from '../api/types';
import { COLORS, lerpColor } from './colors';

export function getBlockColor(isLeader: boolean, status: number): number {
  if (!isLeader) return COLORS.regularBlock;
  switch (status) {
    case LEADER_COMMITTED:
      return COLORS.committedLeader;
    case LEADER_SKIPPED:
    default:
      return COLORS.skippedLeader;
  }
}

/**
 * Map a latency value (ms) to a scale factor and alpha for visual encoding.
 * 0 ms → scale 1.0, alpha 1.0 (prompt block)
 * ≥ threshold → scale 0.6, alpha 0.4 (late block)
 */
function latencyVisuals(latencyMs: number): { scale: number; alpha: number } {
  const LATENCY_THRESHOLD_MS = 5000;
  const t = Math.min(latencyMs / LATENCY_THRESHOLD_MS, 1);
  return {
    scale: 1.0 - t * 0.4,   // 1.0 → 0.6
    alpha: 1.0 - t * 0.6,   // 1.0 → 0.4
  };
}

/**
 * Map latency to a color: grey → yellow → red.
 * 0 ms = grey (0x6b7280), midpoint = yellow (0xeab308), threshold = red (0xef4444).
 */
function latencyColor(latencyMs: number): number {
  const LATENCY_THRESHOLD_MS = 5000;
  const t = Math.min(latencyMs / LATENCY_THRESHOLD_MS, 1);
  const GREY = 0x6b7280;
  const YELLOW = 0xeab308;
  const RED = 0xef4444;
  if (t <= 0.5) {
    return lerpColor(GREY, YELLOW, t * 2);
  }
  return lerpColor(YELLOW, RED, (t - 0.5) * 2);
}

/** Circle with X marker for a slot where no block was produced. */
export function createMissingSlotGraphic(): Graphics {
  const g = new Graphics();
  const color = 0xef4444;
  const radius = 14;
  g.circle(0, 0, radius);
  g.stroke({ color, width: 1.5 });
  const s = 6;
  g.moveTo(-s, -s);
  g.lineTo(s, s);
  g.moveTo(s, -s);
  g.lineTo(-s, s);
  g.stroke({ color, width: 2 });
  return g;
}

/**
 * Create a block graphic. When `latencyMs` is provided (>= 0), the block
 * size and opacity reflect how late the block arrived relative to its round.
 */
export function createBlockGraphic(
  isLeader: boolean,
  status: number,
  latencyMs?: number,
): Graphics {
  const g = new Graphics();
  const baseRadius = 14;
  const { scale, alpha } = latencyMs !== undefined && latencyMs >= 0
    ? latencyVisuals(latencyMs)
    : { scale: 1, alpha: 1 };
  const radius = baseRadius * scale;

  if (isLeader) {
    const color = getBlockColor(isLeader, status);
    const size = radius;
    g.moveTo(0, -size);
    g.lineTo(size, 0);
    g.lineTo(0, size);
    g.lineTo(-size, 0);
    g.closePath();
    g.fill({ color, alpha });
    g.stroke({ color: 0xffffff, width: 1.5, alpha: 0.4 * alpha });
  } else {
    const color = latencyMs !== undefined && latencyMs >= 0
      ? latencyColor(latencyMs)
      : COLORS.regularBlock;
    g.circle(0, 0, radius);
    g.fill({ color, alpha });
  }

  return g;
}
