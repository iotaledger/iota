// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { Container, Graphics, Text, TextStyle } from 'pixi.js';
import { COLORS } from './colors';

/** Consensus wave length — leader round is at wave * WAVE_LENGTH + 1. */
const WAVE_LENGTH = 3;

function roundLabelStyle(): TextStyle {
  return new TextStyle({
    fontSize: 48,
    fill: COLORS.labelText,
    fontFamily: 'system-ui, sans-serif',
  });
}

function waveLabelStyle(): TextStyle {
  return new TextStyle({
    fontSize: 40,
    fill: COLORS.waveLabel,
    fontFamily: 'system-ui, sans-serif',
    fontWeight: 'bold',
  });
}

/** Return the wave number for a given round. */
function waveForRound(round: number): number {
  return round > 0 ? Math.floor((round - 1) / WAVE_LENGTH) : 0;
}

export function drawGridOverlay(
  container: Container,
  numAuthorities: number,
  minRound: number,
  maxRound: number,
  cellWidth: number,
  cellHeight: number,
): void {
  for (let i = container.children.length - 1; i >= 0; i--) {
    container.children[i].destroy(true);
  }

  const gridWidth = numAuthorities * cellWidth;
  const yStart = minRound * cellHeight;
  const yEnd = (maxRound + 1) * cellHeight;

  // Wave bands — horizontal strips per round
  const bands = new Graphics();
  for (let r = minRound; r <= maxRound; r++) {
    const y = r * cellHeight;
    const wave = waveForRound(r);
    const color = wave % 2 === 0 ? COLORS.waveBandA : COLORS.waveBandB;
    bands.rect(0, y, gridWidth, cellHeight);
    bands.fill({ color, alpha: 0.5 });
  }
  container.addChild(bands);

  // Wave boundary lines + wave number labels
  const waveBoundaries = new Graphics();
  for (let r = minRound; r <= maxRound; r++) {
    const leaderRound = waveForRound(r) * WAVE_LENGTH + 1;
    if (r === leaderRound) {
      const y = r * cellHeight;
      waveBoundaries.moveTo(0, y);
      waveBoundaries.lineTo(gridWidth, y);

      // Wave number label — left column (further from grid)
      const wave = waveForRound(r);
      const waveCenterY = y + (WAVE_LENGTH * cellHeight) / 2;
      const label = new Text({ text: `W${wave}`, style: waveLabelStyle() });
      label.anchor.set(1, 0.5);
      label.position.set(-150, waveCenterY);
      container.addChild(label);
    }
  }
  waveBoundaries.stroke({ color: COLORS.waveBoundary, width: 1.5, alpha: 0.5 });
  container.addChild(waveBoundaries);

  // Vertical grid lines — per validator column
  const vertLines = new Graphics();
  for (let a = 0; a <= numAuthorities; a++) {
    const x = a * cellWidth;
    vertLines.moveTo(x, yStart);
    vertLines.lineTo(x, yEnd);
  }
  vertLines.stroke({ color: COLORS.gridLine, width: 1, alpha: 0.3 });
  container.addChild(vertLines);

  // Horizontal grid lines — per round row
  const horizLines = new Graphics();
  for (let r = minRound; r <= maxRound + 1; r++) {
    const y = r * cellHeight;
    horizLines.moveTo(0, y);
    horizLines.lineTo(gridWidth, y);
  }
  horizLines.stroke({ color: COLORS.gridLine, width: 1, alpha: 0.3 });
  container.addChild(horizLines);

  // Round number labels — to the left, skip some when there are many rounds
  const totalRounds = maxRound - minRound + 1;
  const labelEvery = totalRounds > 100 ? 5 : totalRounds > 50 ? 2 : 1;
  for (let r = minRound; r <= maxRound; r++) {
    if (labelEvery > 1 && r % labelEvery !== 0) continue;
    const y = r * cellHeight + cellHeight / 2;
    const label = new Text({ text: String(r), style: roundLabelStyle() });
    label.anchor.set(1, 0.5);
    label.position.set(-20, y);
    container.addChild(label);
  }
}
