// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { Application, Container, Graphics, Text, TextStyle, TextureSource } from 'pixi.js';
import { Viewport } from 'pixi-viewport';
import { LEADER_COMMITTED, LEADER_SKIPPED } from '../api/types';
import type { DagBlockMessage } from '../api/types';
import { makeBlockKey, roundFromKey } from '../hooks/useDagData';
import { createBlockGraphic, createMissingSlotGraphic } from './BlockGraphic';
import { drawGridOverlay } from './GridOverlay';
import type { Theme } from './colors';
import { COLORS, lerpColor, setActiveTheme } from './colors';

const HEALTH_BAR_WIDTH = 8;
const HEALTH_BAR_HEIGHT = 16;

export const CELL_WIDTH = 50;
export const CELL_HEIGHT = 50;
/** Fixed number of rounds visible at all times. */
export const VISIBLE_ROUNDS = 50;

interface BlockEntry {
  graphic: Graphics;
  block: DagBlockMessage;
  isLeader: boolean;
  leaderStatus: number;
  latencyMs?: number;
}

type BlockHoverCallback = (
  block: DagBlockMessage | null,
  screenX: number,
  screenY: number,
) => void;
type BlockClickCallback = (block: DagBlockMessage, screenX: number, screenY: number) => void;

export class DagRenderer {
  private app: Application;
  private viewport!: Viewport;
  private gridLayer!: Container;
  private roundDurationLayer!: Container;
  private missingSlotLayer!: Container;
  private edgeLayer!: Container;
  private commitChainLayer!: Container;
  private propagationLayer!: Container;
  private nodeLayer!: Container;
  private equivocationLayer!: Container;
  private labelLayer!: Container;
  private labelBg!: Graphics;
  private labelTexts: Text[] = [];
  private labelStakeTexts: Text[] = [];
  private healthBars: Graphics[] = [];

  private blockMap = new Map<number, BlockEntry>();
  private missingSlotGraphics = new Map<number, Graphics>();
  private equivocationGraphics = new Map<number, Graphics>();
  private roundDurationGraphics = new Map<number, Graphics>();
  private propagationGraphics: Graphics[] = [];
  private searchLayer!: Container;
  private searchGraphics: Graphics[] = [];
  private validatorStakes?: number[];
  private totalStake?: number;
  private numAuthorities = 0;
  /** Per-round average timestamp (ms) — used to compute per-block latency. */
  private roundAvgTs = new Map<number, { avg: number; count: number }>();
  /** When true, dim all blocks except skipped leaders. */
  private highlightSkippedOnly = false;
  /** Committed leaders sorted by round — used to draw the commit chain. */
  private committedLeaders: { round: number; author: number }[] = [];
  /** Tracks the highest round for which missing slots have been scanned. */
  private missingSlotScannedUpTo = 0;
  /** Last hostnames passed to createLabels — needed for theme rebuilds. */
  private lastHostnames: string[] = [];

  /** Bounds of the last grid draw (with padding). Grid only redraws when exceeded. */
  private gridDrawnMin = Infinity;
  private gridDrawnMax = -Infinity;
  private gridDrawnAuthorities = 0;

  private hoverCallbacks: BlockHoverCallback[] = [];
  private clickCallbacks: BlockClickCallback[] = [];
  private unpinCallbacks: (() => void)[] = [];

  /** Temporary graphics added during hover (edge overlays + block glows). */
  private hoverGraphics: Graphics[] = [];
  /** Block whose highlights are pinned (persists after pointerleave). */
  private pinnedBlock: DagBlockMessage | null = null;
  /** Graphics for the pinned block's highlights (separate from hover). */
  private pinnedGraphics: Graphics[] = [];
  /** Purple ring drawn around the pinned block. */
  private pinnedRing: Graphics | null = null;
  /** Reverse index: parent blockKey → set of child blockKeys. */
  private childrenOf = new Map<number, Set<number>>();

  private wheelEnabled = false;
  private wheelAccumulator = 0;
  private wheelShiftCallbacks: ((deltaRounds: number) => void)[] = [];

  private canvas: HTMLCanvasElement;
  private initialized = false;
  private destroyed = false;
  private unpinTickerFn: (() => void) | null = null;

  constructor(canvas: HTMLCanvasElement) {
    this.canvas = canvas;
    this.app = new Application();
  }

  async init(): Promise<void> {
    if (this.destroyed) return;

    // Enable mipmaps so text textures stay sharp when the viewport scales them down.
    TextureSource.defaultOptions.autoGenerateMipmaps = true;

    await this.app.init({
      canvas: this.canvas,
      resizeTo: this.canvas.parentElement ?? undefined,
      background: COLORS.background,
      antialias: true,
      resolution: window.devicePixelRatio || 1,
      autoDensity: true,
      preserveDrawingBuffer: true,
    });

    this.viewport = new Viewport({
      screenWidth: this.canvas.clientWidth,
      screenHeight: this.canvas.clientHeight,
      worldWidth: 5000,
      worldHeight: 2000,
      events: this.app.renderer.events,
    });

    // No interactive plugins — we handle scrolling manually via handleWheel.

    this.app.stage.addChild(this.viewport);

    this.gridLayer = new Container();
    this.roundDurationLayer = new Container();
    this.missingSlotLayer = new Container();
    this.edgeLayer = new Container();
    this.commitChainLayer = new Container();
    this.propagationLayer = new Container();
    this.nodeLayer = new Container();
    this.equivocationLayer = new Container();
    this.searchLayer = new Container();

    this.viewport.addChild(this.gridLayer);
    this.viewport.addChild(this.roundDurationLayer);
    this.viewport.addChild(this.missingSlotLayer);
    this.viewport.addChild(this.edgeLayer);
    this.viewport.addChild(this.commitChainLayer);
    this.viewport.addChild(this.propagationLayer);
    this.viewport.addChild(this.nodeLayer);
    this.viewport.addChild(this.equivocationLayer);
    this.viewport.addChild(this.searchLayer);

    // Label layer sits on app.stage (not viewport) so it stays fixed on screen
    this.labelLayer = new Container();
    this.labelBg = new Graphics();
    this.labelLayer.addChild(this.labelBg);
    this.app.stage.addChild(this.labelLayer);

    // Update label positions whenever viewport moves
    this.viewport.on('moved', () => this.updateLabelPositions());

    // Check every frame whether the pinned block has left the visible area
    this.unpinTickerFn = () => this.unpinIfOutOfView();
    this.app.ticker.add(this.unpinTickerFn);

    // Custom wheel handler: scroll = navigate rounds, ctrl+scroll = zoom
    this.canvas.addEventListener('wheel', this.handleWheel, { passive: false });

    this.initialized = true;
  }

  /** Absolute world position — never changes once placed. */
  private blockPosition(round: number, author: number): { x: number; y: number } {
    return {
      x: author * CELL_WIDTH + CELL_WIDTH / 2,
      y: round * CELL_HEIGHT + CELL_HEIGHT / 2,
    };
  }

  private clearHighlights(): void {
    for (const g of this.hoverGraphics) {
      g.parent?.removeChild(g);
      g.destroy();
    }
    this.hoverGraphics = [];
  }

  private clearPinnedHighlights(): void {
    for (const g of this.pinnedGraphics) {
      g.parent?.removeChild(g);
      g.destroy();
    }
    this.pinnedGraphics = [];
    if (this.pinnedRing) {
      this.pinnedRing.parent?.removeChild(this.pinnedRing);
      this.pinnedRing.destroy();
      this.pinnedRing = null;
    }
    this.pinnedBlock = null;
  }

  /** Draw highlights for a block and store them as pinned graphics. */
  private pinBlock(block: DagBlockMessage): void {
    this.clearPinnedHighlights();
    this.pinnedBlock = block;
    // Temporarily redirect hoverGraphics so highlightBlock populates pinnedGraphics
    const saved = this.hoverGraphics;
    this.hoverGraphics = [];
    this.highlightBlock(block);
    this.pinnedGraphics = this.hoverGraphics;
    this.hoverGraphics = saved;

    // Draw a bright purple highlight on the pinned block
    const pos = this.blockPosition(block.round, block.author);
    this.pinnedRing = new Graphics();
    this.pinnedRing.eventMode = 'none';
    this.pinnedRing.circle(0, 0, 26);
    this.pinnedRing.fill({ color: 0xc084fc, alpha: 0.4 });
    this.pinnedRing.circle(0, 0, 26);
    this.pinnedRing.stroke({ color: 0xc084fc, width: 4.5, alpha: 0.95 });
    this.pinnedRing.circle(0, 0, 32);
    this.pinnedRing.stroke({ color: 0xc084fc, width: 2.5, alpha: 0.6 });
    this.pinnedRing.position.set(pos.x, pos.y);
    this.nodeLayer.addChild(this.pinnedRing);
  }

  /** Toggle pin state for a block. Returns the pinned block or null if unpinned. */
  togglePin(block: DagBlockMessage): DagBlockMessage | null {
    const key = makeBlockKey(block.round, block.author);
    const pinnedKey = this.pinnedBlock
      ? makeBlockKey(this.pinnedBlock.round, this.pinnedBlock.author)
      : null;

    if (pinnedKey === key) {
      this.clearPinnedHighlights();
      return null;
    }
    this.pinBlock(block);
    return block;
  }

  getPinnedBlock(): DagBlockMessage | null {
    return this.pinnedBlock;
  }

  /** Clear pin + notify if the pinned block is no longer in the visible viewport. */
  private unpinIfOutOfView(): void {
    if (!this.pinnedBlock) return;
    const { minRound, maxRound } = this.getVisibleRoundRange();
    if (this.pinnedBlock.round < minRound || this.pinnedBlock.round > maxRound) {
      this.clearPinnedHighlights();
      for (const cb of this.unpinCallbacks) cb();
    }
  }

  /** Pin a block by its numeric key (round * 1000 + author). Returns true if found and pinned. */
  pinByKey(key: number): boolean {
    const entry = this.blockMap.get(key);
    if (!entry) return false;
    this.pinBlock(entry.block);
    return true;
  }

  private drawHoverGlow(x: number, y: number, color: number): void {
    const glow = new Graphics();
    glow.eventMode = 'none';
    // Filled disc — strong enough to tint the block
    glow.circle(0, 0, 26);
    glow.fill({ color, alpha: 0.4 });
    // Inner ring — thick and bright
    glow.circle(0, 0, 26);
    glow.stroke({ color, width: 4.5, alpha: 0.95 });
    // Outer ring
    glow.circle(0, 0, 32);
    glow.stroke({ color, width: 2.5, alpha: 0.6 });
    glow.position.set(x, y);
    this.nodeLayer.addChild(glow);
    this.hoverGraphics.push(glow);
  }

  private highlightBlock(block: DagBlockMessage): void {
    this.clearHighlights();

    const blockKey = makeBlockKey(block.round, block.author);

    // Track ancestor keys so we don't double-draw acknowledgments for them
    const ancestorKeys = new Set<number>();

    // 1) Glows on ALL direct ancestors, colored by round gap
    for (const ref of block.ancestors) {
      const ancestorEntry = this.blockMap.get(makeBlockKey(ref.round, ref.author));
      if (!ancestorEntry) continue;

      ancestorKeys.add(makeBlockKey(ref.round, ref.author));
      const gap = block.round - ref.round;
      const color = gap >= 3 ? COLORS.healthBad : gap === 2 ? COLORS.healthWarn : COLORS.edge;
      const fromPos = this.blockPosition(ref.round, ref.author);
      this.drawHoverGlow(fromPos.x, fromPos.y, color);
    }

    // 2) Glows on acknowledged blocks
    if (block.acknowledgments) {
      for (const ack of block.acknowledgments) {
        const key = makeBlockKey(ack.round, ack.author);
        if (ancestorKeys.has(key)) continue;
        const entry = this.blockMap.get(key);
        if (!entry) continue;

        const fromPos = this.blockPosition(ack.round, ack.author);
        this.drawHoverGlow(fromPos.x, fromPos.y, COLORS.committedGlow);
      }
    }

    // 3) Glows on children (blocks that reference this block)
    const children = this.childrenOf.get(blockKey);
    if (children) {
      for (const childKey of children) {
        const childEntry = this.blockMap.get(childKey);
        if (!childEntry) continue;
        const childPos = this.blockPosition(childEntry.block.round, childEntry.block.author);
        this.drawHoverGlow(childPos.x, childPos.y, 0x22d3ee);
      }
    }
  }

  private attachBlockEvents(graphic: Graphics, block: DagBlockMessage): void {
    graphic.on('pointerenter', (e) => {
      const globalPos = e.global;
      for (const cb of this.hoverCallbacks) {
        cb(block, globalPos.x, globalPos.y);
      }
      this.highlightBlock(block);
    });

    graphic.on('pointerleave', () => {
      this.clearHighlights();
      if (!this.pinnedBlock) {
        for (const cb of this.hoverCallbacks) {
          cb(null, 0, 0);
        }
      }
    });

    graphic.on('pointertap', (e) => {
      const globalPos = e.global;
      for (const cb of this.clickCallbacks) {
        cb(block, globalPos.x, globalPos.y);
      }
    });
  }

  addBlock(block: DagBlockMessage, isLeader: boolean, leaderStatus: number): void {
    if (!this.initialized) return;

    const key = makeBlockKey(block.round, block.author);
    if (this.blockMap.has(key)) return;

    // Update round median timestamp
    this.updateRoundAvgTs(block.round, block.timestamp_ms);

    // Compute latency: delta between this block's timestamp and the
    // average timestamp of its ancestors' round (previous round).
    let latencyMs: number | undefined;
    if (block.ancestors.length > 0) {
      const prevRound = block.round - 1;
      const prevRoundTs = this.roundAvgTs.get(prevRound);
      if (prevRoundTs !== undefined) {
        latencyMs = Math.max(0, block.timestamp_ms - prevRoundTs.avg);
      }
    }

    const pos = this.blockPosition(block.round, block.author);
    const graphic = createBlockGraphic(isLeader, leaderStatus, latencyMs);
    graphic.position.set(pos.x, pos.y);
    graphic.eventMode = 'static';
    graphic.cursor = 'pointer';

    // Apply skipped-leader filter dimming
    if (this.highlightSkippedOnly) {
      const isSkippedLeader = isLeader && leaderStatus === LEADER_SKIPPED;
      if (!isSkippedLeader) {
        graphic.alpha = 0.15;
      }
    }

    // Remove missing-slot marker if one exists at this position
    const missingGraphic = this.missingSlotGraphics.get(key);
    if (missingGraphic) {
      this.missingSlotLayer.removeChild(missingGraphic);
      missingGraphic.destroy();
      this.missingSlotGraphics.delete(key);
    }

    const entry: BlockEntry = { graphic, block, isLeader, leaderStatus, latencyMs };
    this.blockMap.set(key, entry);

    // Build reverse index: register this block as a child of each ancestor
    for (const ancestor of block.ancestors) {
      const ancestorKey = makeBlockKey(ancestor.round, ancestor.author);
      let ch = this.childrenOf.get(ancestorKey);
      if (!ch) {
        ch = new Set();
        this.childrenOf.set(ancestorKey, ch);
      }
      ch.add(key);
    }

    this.attachBlockEvents(graphic, block);
    this.nodeLayer.addChild(graphic);
  }

  /** Incrementally update the running average timestamp for a round. */
  private updateRoundAvgTs(round: number, timestampMs: number): void {
    const existing = this.roundAvgTs.get(round);
    if (existing === undefined) {
      this.roundAvgTs.set(round, { avg: timestampMs, count: 1 });
    } else {
      // Running average: newAvg = (oldAvg * count + newValue) / (count + 1)
      const newCount = existing.count + 1;
      const newAvg = (existing.avg * existing.count + timestampMs) / newCount;
      this.roundAvgTs.set(round, { avg: newAvg, count: newCount });
    }
  }

  updateLeaderStatus(round: number, author: number, status: number): void {
    if (!this.initialized) return;

    const key = makeBlockKey(round, author);
    const entry = this.blockMap.get(key);
    if (!entry) return;
    if (entry.isLeader && entry.leaderStatus === status) return;

    entry.leaderStatus = status;
    entry.isLeader = true;

    const pos = entry.graphic.position;
    const newGraphic = createBlockGraphic(true, status, entry.latencyMs);
    newGraphic.position.set(pos.x, pos.y);
    newGraphic.eventMode = 'static';
    newGraphic.cursor = 'pointer';

    if (this.highlightSkippedOnly) {
      newGraphic.alpha = status === LEADER_SKIPPED ? 1 : 0.15;
    }

    this.attachBlockEvents(newGraphic, entry.block);

    entry.graphic.removeAllListeners();
    this.nodeLayer.removeChild(entry.graphic);
    entry.graphic.destroy();
    this.nodeLayer.addChild(newGraphic);
    entry.graphic = newGraphic;

    // Track committed leaders for the commit chain
    if (status === LEADER_COMMITTED) {
      this.insertCommittedLeader(round, author);
      this.appendCommitChainSegment();
    }
  }

  /** Toggle skipped-leader highlighting. When active, all non-skipped blocks are dimmed. */
  setHighlightSkippedOnly(enabled: boolean): void {
    if (this.highlightSkippedOnly === enabled) return;
    this.highlightSkippedOnly = enabled;

    for (const [, entry] of this.blockMap) {
      const isSkippedLeader = entry.isLeader && entry.leaderStatus === LEADER_SKIPPED;
      entry.graphic.alpha = enabled && !isSkippedLeader ? 0.15 : 1;
    }

    // Also dim missing slots and commit chain when filtering
    for (const [, g] of this.missingSlotGraphics) {
      g.alpha = enabled ? 0.1 : 1;
    }
    this.commitChainLayer.alpha = enabled ? 0.15 : 1;
  }

  /** Switch color theme. Redraws background, grid, labels, and label panel. */
  setTheme(theme: Theme): void {
    if (!this.initialized) return;
    setActiveTheme(theme);
    this.app.renderer.background.color = COLORS.background;

    // Redraw grid with new colors
    if (this.gridDrawnAuthorities > 0) {
      drawGridOverlay(
        this.gridLayer,
        this.gridDrawnAuthorities,
        this.gridDrawnMin,
        this.gridDrawnMax,
        CELL_WIDTH,
        CELL_HEIGHT,
      );
    }

    // Recreate labels with new text color
    if (this.lastHostnames.length > 0) {
      this.createLabels(this.lastHostnames);
    }
  }

  // ── Missing slot indicators ──────────────────────────────────────────

  /** Mark empty (round, author) slots as missing for rounds up to maxRound - 2. */
  markMissingSlots(maxRound: number, numAuthorities: number): void {
    if (!this.initialized || numAuthorities === 0) return;

    // Only scan rounds that are settled (at least 2 behind the frontier)
    const scanUpTo = maxRound - 2;
    const scanFrom = Math.max(this.missingSlotScannedUpTo + 1, this.gridDrawnMin);
    if (scanFrom > scanUpTo) return;

    for (let r = scanFrom; r <= scanUpTo; r++) {
      for (let a = 0; a < numAuthorities; a++) {
        const key = makeBlockKey(r, a);
        if (this.blockMap.has(key) || this.missingSlotGraphics.has(key)) continue;

        const pos = this.blockPosition(r, a);
        const g = createMissingSlotGraphic();
        g.position.set(pos.x, pos.y);
        if (this.highlightSkippedOnly) g.alpha = 0.1;
        this.missingSlotLayer.addChild(g);
        this.missingSlotGraphics.set(key, g);
      }
    }
    this.missingSlotScannedUpTo = scanUpTo;
  }

  // ── Commit chain visualization ─────────────────────────────────────

  private insertCommittedLeader(round: number, author: number): void {
    const leaders = this.committedLeaders;
    // Fast path: leaders typically arrive in order, so append to the end
    if (leaders.length === 0 || round > leaders[leaders.length - 1].round) {
      leaders.push({ round, author });
      return;
    }
    // Duplicate check for the last element (common case for replays)
    if (round === leaders[leaders.length - 1].round) return;
    // Fallback: binary search insert for out-of-order arrivals
    let lo = 0;
    let hi = leaders.length;
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      if (leaders[mid].round < round) lo = mid + 1;
      else hi = mid;
    }
    if (lo < leaders.length && leaders[lo].round === round) return;
    leaders.splice(lo, 0, { round, author });
  }

  /** Append a single segment to the commit chain (from second-to-last to last leader). */
  private appendCommitChainSegment(): void {
    if (this.committedLeaders.length < 2) return;

    const from = this.committedLeaders[this.committedLeaders.length - 2];
    const to = this.committedLeaders[this.committedLeaders.length - 1];
    const fromPos = this.blockPosition(from.round, from.author);
    const toPos = this.blockPosition(to.round, to.author);

    const g = new Graphics();
    const midY = (fromPos.y + toPos.y) / 2;
    g.moveTo(fromPos.x, fromPos.y);
    g.bezierCurveTo(fromPos.x, midY, toPos.x, midY, toPos.x, toPos.y);
    g.stroke({ color: COLORS.commitChain, width: 5, alpha: 0.6 });
    this.commitChainLayer.addChild(g);
  }

  /** Full rebuild of the commit chain (used after eviction). */
  private redrawCommitChain(): void {
    for (let i = this.commitChainLayer.children.length - 1; i >= 0; i--) {
      this.commitChainLayer.children[i].destroy(true);
    }

    if (this.committedLeaders.length < 2) return;

    const g = new Graphics();
    for (let i = 0; i < this.committedLeaders.length - 1; i++) {
      const from = this.committedLeaders[i];
      const to = this.committedLeaders[i + 1];
      const fromPos = this.blockPosition(from.round, from.author);
      const toPos = this.blockPosition(to.round, to.author);

      const midY = (fromPos.y + toPos.y) / 2;
      g.moveTo(fromPos.x, fromPos.y);
      g.bezierCurveTo(fromPos.x, midY, toPos.x, midY, toPos.x, toPos.y);
    }
    g.stroke({ color: COLORS.commitChain, width: 5, alpha: 0.6 });
    this.commitChainLayer.addChild(g);
  }

  // ── Equivocation markers ─────────────────────────────────────────

  /** Draw a red warning ring around a slot where equivocation was detected. */
  markEquivocation(round: number, author: number): void {
    if (!this.initialized) return;
    const key = makeBlockKey(round, author);
    if (this.equivocationGraphics.has(key)) return;

    const pos = this.blockPosition(round, author);
    const g = new Graphics();
    g.eventMode = 'none';
    // Bright filled disc
    g.circle(0, 0, 26);
    g.fill({ color: COLORS.equivocation, alpha: 0.35 });
    // Inner ring — thick and bright
    g.circle(0, 0, 26);
    g.stroke({ color: COLORS.equivocation, width: 4.5, alpha: 0.95 });
    // Outer ring
    g.circle(0, 0, 32);
    g.stroke({ color: COLORS.equivocation, width: 2.5, alpha: 0.6 });
    g.position.set(pos.x, pos.y);
    this.equivocationLayer.addChild(g);
    this.equivocationGraphics.set(key, g);
  }

  // ── Round duration overlay ──────────────────────────────────────

  /** Draw thin colored bars to the left of each round row showing round duration. */
  updateRoundDurations(): void {
    if (!this.initialized || this.roundAvgTs.size < 2) return;

    const rounds = [...this.roundAvgTs.keys()].sort((a, b) => a - b);
    for (let i = 1; i < rounds.length; i++) {
      const round = rounds[i];
      if (this.roundDurationGraphics.has(round)) continue;

      const prevTs = this.roundAvgTs.get(rounds[i - 1])!.avg;
      const currTs = this.roundAvgTs.get(round)!.avg;
      const durationMs = currTs - prevTs;
      if (durationMs <= 0) continue;

      // Green < ~125ms, yellow around 250ms, red > 500ms
      const t = Math.min(durationMs / 500, 1);
      const color = t < 0.5
        ? lerpColor(COLORS.healthGood, COLORS.healthWarn, t * 2)
        : lerpColor(COLORS.healthWarn, COLORS.healthBad, (t - 0.5) * 2);

      const y = round * CELL_HEIGHT + 2;
      const g = new Graphics();
      g.rect(-12, y, 6, CELL_HEIGHT - 4);
      g.fill({ color, alpha: 0.6 });
      this.roundDurationLayer.addChild(g);
      this.roundDurationGraphics.set(round, g);
    }
  }

  // ── Block propagation heatmap ───────────────────────────────────

  /** Show colored rings on blocks that reference the given block, colored by time delta. */
  showPropagationHeatmap(
    references: Array<{ round: number; author: number; deltaMs: number }>,
  ): void {
    this.clearPropagationHeatmap();
    if (references.length === 0) return;

    const maxDelta = Math.max(...references.map((r) => r.deltaMs), 1);

    for (const ref of references) {
      const t = ref.deltaMs / maxDelta;
      const color = lerpColor(COLORS.healthGood, COLORS.healthBad, t);
      const pos = this.blockPosition(ref.round, ref.author);

      const g = new Graphics();
      g.circle(0, 0, 20);
      g.fill({ color, alpha: 0.25 });
      g.circle(0, 0, 20);
      g.stroke({ color, width: 2.5, alpha: 0.85 });
      g.position.set(pos.x, pos.y);
      this.propagationLayer.addChild(g);
      this.propagationGraphics.push(g);
    }
  }

  clearPropagationHeatmap(): void {
    for (const g of this.propagationGraphics) {
      g.parent?.removeChild(g);
      g.destroy();
    }
    this.propagationGraphics = [];
  }

  // ── Commit sub-DAG highlight ─────────────────────────────────────

  /** Highlight blocks that were committed by a specific leader. */
  showCommitSubDag(blockKeys: number[]): void {
    this.clearPropagationHeatmap();
    for (const key of blockKeys) {
      const entry = this.blockMap.get(key);
      if (!entry) continue;
      const pos = this.blockPosition(entry.block.round, entry.block.author);
      const g = new Graphics();
      g.circle(0, 0, 18);
      g.fill({ color: COLORS.committedGlow, alpha: 0.2 });
      g.circle(0, 0, 18);
      g.stroke({ color: COLORS.committedGlow, width: 2, alpha: 0.7 });
      g.position.set(pos.x, pos.y);
      this.propagationLayer.addChild(g);
      this.propagationGraphics.push(g);
    }
  }

  // ── Search by digest ───────────────────────────────────────────

  /** Highlight all blocks whose digest contains the given substring. Returns match count. */
  highlightByDigest(partialDigest: string): number {
    this.clearSearchHighlights();
    if (!partialDigest || partialDigest.length < 2) return 0;

    const needle = partialDigest.toLowerCase();
    let count = 0;
    let firstMatch: { round: number } | null = null;

    for (const [, entry] of this.blockMap) {
      if (entry.block.digest.toLowerCase().includes(needle)) {
        const pos = this.blockPosition(entry.block.round, entry.block.author);
        const g = new Graphics();
        g.circle(0, 0, 22);
        g.stroke({ color: COLORS.searchHighlight, width: 3, alpha: 0.9 });
        g.position.set(pos.x, pos.y);
        this.searchLayer.addChild(g);
        this.searchGraphics.push(g);
        if (!firstMatch) firstMatch = { round: entry.block.round };
        count++;
      }
    }

    if (firstMatch) {
      this.goToRound(firstMatch.round);
    }
    return count;
  }

  clearSearchHighlights(): void {
    for (const g of this.searchGraphics) {
      g.parent?.removeChild(g);
      g.destroy();
    }
    this.searchGraphics = [];
  }

  // ── View state ─────────────────────────────────────────────────

  /** Get the current viewport center round. */
  getView(): { centerRound: number } {
    if (!this.initialized) return { centerRound: 0 };
    const center = this.viewport.center;
    return {
      centerRound: Math.round((center.y - CELL_HEIGHT / 2) / CELL_HEIGHT),
    };
  }

  /** Get the actual visible round range (accounting for the top panel). */
  getVisibleRoundRange(): { minRound: number; maxRound: number } {
    if (!this.initialized) return { minRound: 0, maxRound: 0 };
    const scale = this.viewport.scale.y;
    const panelHeight = DagRenderer.LABEL_PANEL_HEIGHT;
    const topWorldY = this.viewport.corner.y + panelHeight / scale;
    const bottomWorldY = this.viewport.corner.y + this.canvas.clientHeight / scale;
    return {
      minRound: Math.floor(topWorldY / CELL_HEIGHT),
      maxRound: Math.ceil(bottomWorldY / CELL_HEIGHT),
    };
  }

  /** Set the viewport to a specific center round. */
  setView(centerRound: number): void {
    if (!this.initialized) return;
    this.goToRound(centerRound);
  }

  // ── Reset ──────────────────────────────────────────────────────

  /** Clear all rendered data (used when importing a saved view). */
  reset(): void {
    if (!this.initialized) return;

    for (const [, entry] of this.blockMap) {
      entry.graphic.removeAllListeners();
      this.nodeLayer.removeChild(entry.graphic);
      entry.graphic.destroy();
    }
    for (const [, g] of this.missingSlotGraphics) {
      this.missingSlotLayer.removeChild(g);
      g.destroy();
    }
    for (const [, g] of this.equivocationGraphics) {
      this.equivocationLayer.removeChild(g);
      g.destroy();
    }
    for (const [, g] of this.roundDurationGraphics) {
      this.roundDurationLayer.removeChild(g);
      g.destroy();
    }
    for (let i = this.gridLayer.children.length - 1; i >= 0; i--) {
      this.gridLayer.children[i].destroy(true);
    }
    for (let i = this.commitChainLayer.children.length - 1; i >= 0; i--) {
      this.commitChainLayer.children[i].destroy(true);
    }

    this.blockMap.clear();
    this.missingSlotGraphics.clear();
    this.equivocationGraphics.clear();
    this.roundDurationGraphics.clear();
    this.clearPropagationHeatmap();
    this.clearSearchHighlights();
    this.clearPinnedHighlights();
    this.childrenOf.clear();
    this.roundAvgTs.clear();
    this.committedLeaders = [];
    this.missingSlotScannedUpTo = 0;
    this.gridDrawnMin = Infinity;
    this.gridDrawnMax = -Infinity;
    this.gridDrawnAuthorities = 0;
  }

  // ── Export ──────────────────────────────────────────────────────

  /** Capture the visible canvas area as a PNG and trigger a download. */
  exportPNG(): void {
    if (!this.initialized) return;
    const dataUrl = this.canvas.toDataURL('image/png');
    const link = document.createElement('a');
    link.download = `dag-snapshot-${Date.now()}.png`;
    link.href = dataUrl;
    document.body.appendChild(link);
    link.click();
    document.body.removeChild(link);
  }

  // ── Per-validator health bars ──────────────────────────────────────

  /** Recompute and redraw health bars based on current block data. */
  updateHealthBars(): void {
    if (!this.initialized || this.numAuthorities === 0) return;

    // Determine visible round range
    const minRound = this.gridDrawnMin;
    const maxRound = this.missingSlotScannedUpTo;
    const totalRounds = maxRound - minRound + 1;
    if (totalRounds <= 0) return;

    // Count blocks per authority in the visible range
    const counts = new Array<number>(this.numAuthorities).fill(0);
    for (const [, entry] of this.blockMap) {
      const r = entry.block.round;
      if (r >= minRound && r <= maxRound) {
        counts[entry.block.author]++;
      }
    }

    // Ensure we have one health bar graphic per authority
    while (this.healthBars.length < this.numAuthorities) {
      const bar = new Graphics();
      this.healthBars.push(bar);
      this.labelLayer.addChild(bar);
    }
    // Remove excess
    while (this.healthBars.length > this.numAuthorities) {
      const bar = this.healthBars.pop()!;
      bar.destroy(true);
    }

    const panelHeight = DagRenderer.LABEL_PANEL_HEIGHT;
    const barY = panelHeight - HEALTH_BAR_HEIGHT - 4;

    for (let a = 0; a < this.numAuthorities; a++) {
      const rate = counts[a] / totalRounds;
      const color = rate >= 0.9 ? COLORS.healthGood : rate >= 0.7 ? COLORS.healthWarn : COLORS.healthBad;

      const worldX = a * CELL_WIDTH + CELL_WIDTH / 2;
      const screenX = this.viewport.toScreen(worldX, 0).x;

      const bar = this.healthBars[a];
      bar.clear();
      // Background
      bar.rect(screenX - HEALTH_BAR_WIDTH / 2, barY, HEALTH_BAR_WIDTH, HEALTH_BAR_HEIGHT);
      bar.fill({ color: COLORS.healthBg, alpha: 0.8 });
      // Fill (from bottom)
      const fillHeight = HEALTH_BAR_HEIGHT * rate;
      if (fillHeight > 0) {
        bar.rect(screenX - HEALTH_BAR_WIDTH / 2, barY + HEALTH_BAR_HEIGHT - fillHeight, HEALTH_BAR_WIDTH, fillHeight);
        bar.fill({ color, alpha: 0.9 });
      }
    }
  }

  private static readonly GRID_PADDING = 30;
  private static readonly LABEL_PANEL_HEIGHT = 150;
  /** World-space left margin for round/wave label columns. */
  private static readonly LEFT_MARGIN = 280;

  /** Reposition fixed authority labels to match viewport's horizontal pan/zoom. */
  private updateLabelPositions(): void {
    if (!this.initialized || this.labelTexts.length === 0) return;

    const scale = this.viewport.scale.x;
    const panelHeight = DagRenderer.LABEL_PANEL_HEIGHT;
    const cellScreenWidth = CELL_WIDTH * scale;

    // Hide labels when columns are too narrow to read
    const showLabels = cellScreenWidth > 8;
    const showStakes = cellScreenWidth > 16;

    for (let i = 0; i < this.labelTexts.length; i++) {
      const worldX = i * CELL_WIDTH + CELL_WIDTH / 2;
      const screenX = this.viewport.toScreen(worldX, 0).x;
      this.labelTexts[i].position.set(screenX, panelHeight - 58);
      this.labelTexts[i].visible = showLabels;
      if (i < this.labelStakeTexts.length) {
        this.labelStakeTexts[i].position.set(screenX, panelHeight - 42);
        this.labelStakeTexts[i].visible = showStakes;
      }
    }

    // Redraw background panel — horizontal strip at top
    this.labelBg.clear();
    this.labelBg.rect(0, 0, this.canvas.clientWidth, panelHeight);
    this.labelBg.fill({ color: COLORS.background, alpha: 1.0 });

    // Reposition health bars to match
    this.updateHealthBars();
  }

  private createLabels(hostnames: string[]): void {
    this.lastHostnames = hostnames;
    // Remove old labels (keep labelBg at index 0)
    for (const text of this.labelTexts) {
      text.destroy(true);
    }
    for (const text of this.labelStakeTexts) {
      text.destroy(true);
    }
    this.labelTexts = [];
    this.labelStakeTexts = [];

    for (let a = 0; a < this.numAuthorities; a++) {
      const hostname = hostnames[a] ?? `V${a}`;
      const displayName = hostname.length > 12 ? hostname.slice(0, 12) + '..' : hostname;
      const label = new Text({ text: displayName, style: new TextStyle({
        fontSize: 12,
        fill: COLORS.labelText,
        fontFamily: 'system-ui, sans-serif',
      }) });
      label.anchor.set(0.5, 0.5);
      label.rotation = -Math.PI / 2;
      this.labelTexts.push(label);
      this.labelLayer.addChild(label);

      if (this.validatorStakes && this.totalStake) {
        const pct = ((this.validatorStakes[a] / this.totalStake) * 100).toFixed(1);
        const stakeLabel = new Text({ text: `${pct}%`, style: new TextStyle({
          fontSize: 12,
          fill: COLORS.labelText,
          fontFamily: 'system-ui, sans-serif',
        }) });
        stakeLabel.anchor.set(0.5, 0.5);
        stakeLabel.rotation = -Math.PI / 2;
        this.labelStakeTexts.push(stakeLabel);
        this.labelLayer.addChild(stakeLabel);
      }
    }

    this.updateLabelPositions();
  }

  drawGrid(
    numAuthorities: number, minRound: number, maxRound: number, hostnames: string[],
    stakes?: number[], totalStake?: number,
  ): void {
    if (!this.initialized) return;

    const authoritiesChanged = numAuthorities !== this.numAuthorities;
    this.numAuthorities = numAuthorities;
    if (stakes) {
      this.validatorStakes = stakes;
      this.totalStake = totalStake;
    }

    const needsGridRedraw =
      numAuthorities !== this.gridDrawnAuthorities ||
      minRound < this.gridDrawnMin ||
      maxRound > this.gridDrawnMax;

    if (needsGridRedraw) {
      const paddedMax = maxRound + DagRenderer.GRID_PADDING;
      this.gridDrawnMin = minRound;
      this.gridDrawnMax = paddedMax;
      this.gridDrawnAuthorities = numAuthorities;

      drawGridOverlay(
        this.gridLayer,
        numAuthorities,
        minRound,
        paddedMax,
        CELL_WIDTH,
        CELL_HEIGHT,
      );
    }

    if (authoritiesChanged || this.labelTexts.length === 0) {
      this.createLabels(hostnames);
    }

    const worldWidth = numAuthorities * CELL_WIDTH + 60;
    const worldHeight = (this.gridDrawnMax + 2) * CELL_HEIGHT;
    this.viewport.resize(
      this.canvas.clientWidth,
      this.canvas.clientHeight,
      worldWidth,
      worldHeight,
    );
  }

  /** Compute the fixed scale that fits VISIBLE_ROUNDS vertically and all validators + left labels horizontally. */
  private computeScale(): number {
    const panelHeight = DagRenderer.LABEL_PANEL_HEIGHT;
    const availableHeight = this.canvas.clientHeight - panelHeight;
    const totalVisualWidth = this.numAuthorities * CELL_WIDTH + DagRenderer.LEFT_MARGIN;
    const hScale = this.numAuthorities > 0
      ? this.canvas.clientWidth / totalVisualWidth
      : 1;
    const vScale = availableHeight / (VISIBLE_ROUNDS * CELL_HEIGHT);
    return Math.min(hScale, vScale);
  }

  /** Horizontal corner that centers the grid + left labels on screen. */
  private centeredCornerX(scale: number): number {
    const totalWorldWidth = this.numAuthorities * CELL_WIDTH;
    const leftMargin = DagRenderer.LEFT_MARGIN;
    // Center the visual range [-leftMargin, totalWorldWidth]
    return (-leftMargin + totalWorldWidth - this.canvas.clientWidth / scale) / 2;
  }

  /** Position the viewport so `bottomRound` aligns with the screen bottom, centered horizontally. */
  snapToView(bottomRound: number): void {
    if (!this.initialized) return;

    const scale = this.computeScale();
    this.viewport.scale.set(scale, scale);

    const bottomWorldY = (bottomRound + 1) * CELL_HEIGHT;
    const cornerY = bottomWorldY - this.canvas.clientHeight / scale;
    this.viewport.moveCorner(this.centeredCornerX(scale), cornerY);
    this.updateLabelPositions();
  }

  /** Center the viewport on a given round (for pause-mode "go to round"). */
  goToRound(round: number): void {
    if (!this.initialized) return;

    const scale = this.computeScale();
    this.viewport.scale.set(scale, scale);

    const centerY = round * CELL_HEIGHT + CELL_HEIGHT / 2;
    const cornerY = centerY - this.canvas.clientHeight / scale / 2;
    this.viewport.moveCorner(this.centeredCornerX(scale), cornerY);
    this.updateLabelPositions();
  }

  /** Shift the viewport by a number of rounds (for pause-mode navigation). */
  shiftView(deltaRounds: number): void {
    if (!this.initialized) return;
    const scale = this.viewport.scale.x;
    const newCornerY = Math.max(0, this.viewport.corner.y + deltaRounds * CELL_HEIGHT);
    this.viewport.moveCorner(
      this.centeredCornerX(scale),
      newCornerY,
    );
    this.updateLabelPositions();
  }

  /** Custom wheel handler: scroll = navigate rounds. */
  private handleWheel = (e: WheelEvent): void => {
    e.preventDefault();
    if (!this.wheelEnabled) return;

    let dy = e.deltaY;
    if (e.deltaMode === 1) dy *= 40;
    else if (e.deltaMode === 2) dy *= 800;

    this.wheelAccumulator += dy;
    const threshold = 50;
    const rounds = Math.trunc(this.wheelAccumulator / threshold);
    if (rounds !== 0) {
      this.wheelAccumulator -= rounds * threshold;
      this.shiftView(rounds);
      for (const cb of this.wheelShiftCallbacks) {
        cb(rounds);
      }
    }
  };

  /** Register a callback for wheel-based view shifts (used for data fetching). */
  onWheelShift(callback: (deltaRounds: number) => void): void {
    this.wheelShiftCallbacks = [callback];
  }

  /** Enable or disable wheel-scroll navigation (paused mode only). */
  setInteractive(enabled: boolean): void {
    if (!this.initialized) return;
    this.wheelEnabled = enabled;
  }

  /** Remove all block data and Graphics for rounds before the given round. */
  evictBefore(round: number): void {
    for (const [key, entry] of this.blockMap) {
      if (entry.block.round < round) {
        entry.graphic.removeAllListeners();
        this.nodeLayer.removeChild(entry.graphic);
        entry.graphic.destroy();
        this.blockMap.delete(key);
        this.childrenOf.delete(key);
      }
    }

    for (const r of this.roundAvgTs.keys()) {
      if (r < round) this.roundAvgTs.delete(r);
    }

    // Evict missing slot markers
    for (const [key, g] of this.missingSlotGraphics) {
      if (roundFromKey(key) < round) {
        this.missingSlotLayer.removeChild(g);
        g.destroy();
        this.missingSlotGraphics.delete(key);
      }
    }

    // Evict equivocation markers
    for (const [key, g] of this.equivocationGraphics) {
      if (roundFromKey(key) < round) {
        this.equivocationLayer.removeChild(g);
        g.destroy();
        this.equivocationGraphics.delete(key);
      }
    }

    // Evict round duration bars
    for (const [r, g] of this.roundDurationGraphics) {
      if (r < round) {
        this.roundDurationLayer.removeChild(g);
        g.destroy();
        this.roundDurationGraphics.delete(r);
      }
    }

    // Evict old committed leaders and redraw chain
    const oldLen = this.committedLeaders.length;
    this.committedLeaders = this.committedLeaders.filter((l) => l.round >= round);
    if (this.committedLeaders.length !== oldLen) {
      this.redrawCommitChain();
    }
  }

  /** Remove all block data and Graphics for rounds outside [keepMin, keepMax]. */
  evictOutside(keepMin: number, keepMax: number): void {
    for (const [key, entry] of this.blockMap) {
      const r = entry.block.round;
      if (r < keepMin || r > keepMax) {
        entry.graphic.removeAllListeners();
        this.nodeLayer.removeChild(entry.graphic);
        entry.graphic.destroy();
        this.blockMap.delete(key);
        this.childrenOf.delete(key);
      }
    }

    for (const r of this.roundAvgTs.keys()) {
      if (r < keepMin || r > keepMax) this.roundAvgTs.delete(r);
    }

    for (const [key, g] of this.missingSlotGraphics) {
      const r = roundFromKey(key);
      if (r < keepMin || r > keepMax) {
        this.missingSlotLayer.removeChild(g);
        g.destroy();
        this.missingSlotGraphics.delete(key);
      }
    }

    for (const [key, g] of this.equivocationGraphics) {
      const r = roundFromKey(key);
      if (r < keepMin || r > keepMax) {
        this.equivocationLayer.removeChild(g);
        g.destroy();
        this.equivocationGraphics.delete(key);
      }
    }

    for (const [r, g] of this.roundDurationGraphics) {
      if (r < keepMin || r > keepMax) {
        this.roundDurationLayer.removeChild(g);
        g.destroy();
        this.roundDurationGraphics.delete(r);
      }
    }

    const oldLen = this.committedLeaders.length;
    this.committedLeaders = this.committedLeaders.filter(
      (l) => l.round >= keepMin && l.round <= keepMax,
    );
    if (this.committedLeaders.length !== oldLen) {
      this.redrawCommitChain();
    }

    this.missingSlotScannedUpTo = Math.max(0, keepMin - 1);
  }

  onBlockHover(callback: BlockHoverCallback): void {
    this.hoverCallbacks = [callback];
  }

  onBlockClick(callback: BlockClickCallback): void {
    this.clickCallbacks = [callback];
  }

  /** Register a callback fired when a pinned block is evicted from the view. */
  onUnpin(callback: () => void): void {
    this.unpinCallbacks = [callback];
  }

  resize(): void {
    if (!this.initialized) return;
    this.viewport.resize(
      this.canvas.clientWidth,
      this.canvas.clientHeight,
      this.viewport.worldWidth,
      this.viewport.worldHeight,
    );
    // Recompute scale and recenter after window resize
    const scale = this.computeScale();
    this.viewport.scale.set(scale, scale);
    const cornerX = this.centeredCornerX(scale);
    this.viewport.moveCorner(cornerX, this.viewport.corner.y);
    this.updateLabelPositions();
  }

  destroy(): void {
    this.destroyed = true;
    this.canvas.removeEventListener('wheel', this.handleWheel);
    if (this.unpinTickerFn) {
      this.app.ticker.remove(this.unpinTickerFn);
      this.unpinTickerFn = null;
    }
    this.hoverCallbacks = [];
    this.clickCallbacks = [];
    this.wheelShiftCallbacks = [];
    this.blockMap.clear();
    this.missingSlotGraphics.clear();
    this.equivocationGraphics.clear();
    this.roundDurationGraphics.clear();
    this.clearPropagationHeatmap();
    this.clearSearchHighlights();
    this.clearPinnedHighlights();
    this.childrenOf.clear();
    this.roundAvgTs.clear();
    this.committedLeaders = [];
    this.healthBars = [];
    this.labelTexts = [];
    this.labelStakeTexts = [];
    if (this.initialized) {
      this.app.destroy(true);
    }
  }
}
