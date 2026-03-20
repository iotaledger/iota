// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef, useState } from 'react';
import type { EpochInfo } from '../api/types';

interface ControlPanelProps {
  paused: boolean;
  onTogglePause: (paused: boolean) => void;
  onShiftView: (deltaRounds: number) => void;
  onGoToRound: (round: number) => void;
  selectedEpoch: number | undefined;
  onEpochChange: (epoch: number | undefined) => void;
  epochs: EpochInfo[];
  highlightSkipped: boolean;
  onToggleHighlightSkipped: (enabled: boolean) => void;
  onExport?: () => void;
  onSearchDigest?: (digest: string) => void;
  searchMatchCount?: number;
  onSaveView?: () => void;
  onLoadView?: (file: File) => void;
  theme?: 'dark' | 'light';
  onToggleTheme?: () => void;
}

export function ControlPanel({
  paused,
  onTogglePause,
  onShiftView,
  onGoToRound,
  selectedEpoch,
  onEpochChange,
  epochs,
  highlightSkipped,
  onToggleHighlightSkipped,
  onExport,
  onSearchDigest,
  searchMatchCount = 0,
  onSaveView,
  onLoadView,
  theme = 'dark',
  onToggleTheme,
}: ControlPanelProps) {
  const [roundInput, setRoundInput] = useState('');
  const [searchInput, setSearchInput] = useState('');
  const fileInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    const timeout = setTimeout(() => {
      onSearchDigest?.(searchInput.length >= 2 ? searchInput : '');
    }, 300);
    return () => clearTimeout(timeout);
  }, [searchInput, onSearchDigest]);

  const handleGo = () => {
    const round = parseInt(roundInput, 10);
    if (!isNaN(round) && round > 0) {
      onGoToRound(round);
    }
  };

  return (
    <div className="dag-panel absolute top-0 left-1/2 -translate-x-1/2 border border-t-0 rounded-b-lg px-3 py-1.5 flex items-center gap-2 whitespace-nowrap z-10">
      {/* Play/Pause */}
      <button
        onClick={() => onTogglePause(!paused)}
        className={`px-2.5 py-1 rounded text-xs font-medium transition-colors ${
          paused
            ? 'bg-green-600 hover:bg-green-500 text-white'
            : 'bg-yellow-600 hover:bg-yellow-500 text-white'
        }`}
      >
        {paused ? 'Resume' : 'Pause'}
      </button>

      {/* Navigation arrows (when paused) */}
      {paused && (
        <>
          <button
            onClick={() => onShiftView(-10)}
            className="dag-btn px-1.5 py-1 rounded text-xs font-medium"
          >
            &#x21D1;
          </button>
          <button
            onClick={() => onShiftView(10)}
            className="dag-btn px-1.5 py-1 rounded text-xs font-medium"
          >
            &#x21D3;
          </button>

          {/* Go to round */}
          <div className="flex items-center gap-1">
            <input
              type="number"
              placeholder="Round"
              value={roundInput}
              onChange={(e) => setRoundInput(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && handleGo()}
              className="w-20 px-1.5 py-1 rounded text-xs focus:outline-none focus:border-blue-500"
            />
            <button
              onClick={handleGo}
              className="dag-btn px-2 py-1 rounded text-xs font-medium"
            >
              Go
            </button>
          </div>
        </>
      )}

      {/* Separator */}
      <div className="dag-separator w-px h-4" />

      {/* Epoch selector */}
      {epochs.length > 0 && (
        <div className="flex items-center gap-1">
          <label className="dag-label text-xs">Epoch:</label>
          <select
            value={selectedEpoch ?? 'latest'}
            onChange={(e) => {
              const val = e.target.value;
              onEpochChange(val === 'latest' ? undefined : Number(val));
            }}
            className="px-1.5 py-1 rounded text-xs focus:outline-none focus:border-blue-500"
          >
            <option value="latest">Latest</option>
            {epochs.map((ep) => (
              <option key={ep.epoch} value={ep.epoch}>
                Epoch {ep.epoch} (r{ep.from_round}-{ep.to_round})
              </option>
            ))}
          </select>
        </div>
      )}

      {/* Skipped filter */}
      <button
        onClick={() => onToggleHighlightSkipped(!highlightSkipped)}
        className={`px-2.5 py-1 rounded text-xs font-medium transition-colors ${
          highlightSkipped
            ? 'bg-red-600 hover:bg-red-500 text-white'
            : 'dag-btn'
        }`}
      >
        {highlightSkipped ? 'Show All' : 'Skipped Only'}
      </button>

      {/* Search */}
      {onSearchDigest && (
        <div className="flex items-center gap-1">
          <input
            type="text"
            placeholder="Search..."
            value={searchInput}
            onChange={(e) => setSearchInput(e.target.value)}
            className="w-24 px-1.5 py-1 rounded text-xs focus:outline-none focus:border-blue-500"
          />
          {searchMatchCount > 0 && (
            <span className="text-xs dag-accent">{searchMatchCount}</span>
          )}
        </div>
      )}

      {/* Separator */}
      <div className="dag-separator w-px h-4" />

      {/* Actions */}
      {onToggleTheme && (
        <button
          onClick={onToggleTheme}
          className="dag-btn px-2 py-1 rounded text-xs font-medium transition-colors"
        >
          {theme === 'dark' ? 'Light' : 'Dark'}
        </button>
      )}
      {onSaveView && (
        <button
          onClick={onSaveView}
          className="dag-btn px-2 py-1 rounded text-xs font-medium transition-colors"
        >
          Save
        </button>
      )}
      {onLoadView && (
        <>
          <input
            ref={fileInputRef}
            type="file"
            accept=".dagv"
            className="hidden"
            onChange={(e) => {
              const file = e.target.files?.[0];
              if (file) onLoadView(file);
              e.target.value = '';
            }}
          />
          <button
            onClick={() => fileInputRef.current?.click()}
            className="dag-btn px-2 py-1 rounded text-xs font-medium transition-colors"
          >
            Load
          </button>
        </>
      )}
      {onExport && (
        <button
          onClick={onExport}
          className="dag-btn px-2 py-1 rounded text-xs font-medium transition-colors"
        >
          PNG
        </button>
      )}
    </div>
  );
}
