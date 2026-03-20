// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { useCallback, useEffect, useRef, useState } from 'react';
import { fetchDag, fetchEpochs, fetchStatus } from './api/client';
import type { CommitteeMessage, EpochInfo, SavedDagView } from './api/types';
import { decodeDagView, encodeDagView, isDagViewBinary } from './binary/dagViewFormat';
import type { Theme } from './pixi/colors';
import { setActiveTheme } from './pixi/colors';
import { ControlPanel } from './components/ControlPanel';
import { DagCanvas } from './components/DagCanvas';
import { Legend } from './components/Legend';
import { StatusBar } from './components/StatusBar';
import { StatsPanel } from './components/StatsPanel';
import { useCommittee } from './hooks/useCommittee';
import { roundFromKey, useDagData } from './hooks/useDagData';
import { VISIBLE_ROUNDS } from './pixi/DagRenderer';
import type { DagRenderer } from './pixi/DagRenderer';

export default function App() {
  const { committee, loading, error } = useCommittee();
  const dagData = useDagData();
  const [selectedEpoch, setSelectedEpoch] = useState<number | undefined>(undefined);
  const [highlightSkipped, setHighlightSkipped] = useState(false);
  const [searchMatchCount, setSearchMatchCount] = useState(0);
  const [overrideCommittee, setOverrideCommittee] = useState<CommitteeMessage | null>(null);
  const [viewLoading, setViewLoading] = useState(false);
  const [theme, setTheme] = useState<Theme>(() => {
    const saved = (localStorage.getItem('dag-theme') as Theme) ?? 'dark';
    setActiveTheme(saved);
    document.documentElement.setAttribute('data-theme', saved);
    return saved;
  });
  const [epochs, setEpochs] = useState<EpochInfo[]>([]);
  const rendererRef = useRef<DagRenderer | null>(null);
  const urlAppliedRef = useRef(false);

  const activeCommittee = overrideCommittee ?? committee;

  // --- Empty state detection ---
  const [showEmptyWarning, setShowEmptyWarning] = useState(false);
  useEffect(() => {
    if (dagData.blocks.size > 0 || overrideCommittee) {
      setShowEmptyWarning(false);
      return;
    }
    const timer = setTimeout(() => {
      if (dagData.blocks.size === 0) setShowEmptyWarning(true);
    }, 5000);
    return () => clearTimeout(timer);
  }, [dagData.version, dagData.blocks.size, overrideCommittee]);

  // --- Parse URL state on mount ---
  const [urlRound, setUrlRound] = useState<number | null>(null);

  useEffect(() => {
    const hash = window.location.hash.slice(1);
    if (!hash) return;
    const params = new URLSearchParams(hash);
    if (params.has('r')) setUrlRound(Number(params.get('r')));
    if (params.get('s') === '1') setHighlightSkipped(true);
  }, []);

  // --- Theme ---
  useEffect(() => {
    document.documentElement.setAttribute('data-theme', theme);
    localStorage.setItem('dag-theme', theme);
    rendererRef.current?.setTheme(theme);
  }, [theme]);

  const handleToggleTheme = useCallback(() => {
    setTheme((t) => (t === 'dark' ? 'light' : 'dark'));
  }, []);

  // --- Fetch available epochs ---
  useEffect(() => {
    fetchEpochs().then(setEpochs).catch(() => {});
  }, []);

  // --- Apply URL state after data loads ---
  useEffect(() => {
    if (urlAppliedRef.current || urlRound === null) return;
    if (dagData.version === 0) return;
    const renderer = rendererRef.current;
    if (!renderer) return;

    urlAppliedRef.current = true;
    dagData.setPaused(true);

    // Fetch blocks around the target round, then position the viewport
    const half = Math.floor(VISIBLE_ROUNDS / 2);
    const from = Math.max(1, urlRound - half);
    const to = urlRound + half;
    dagData.fetchWindow(from, to).then(() => {
      requestAnimationFrame(() => {
        renderer.setView(urlRound);
        if (highlightSkipped) renderer.setHighlightSkippedOnly(true);
      });
    });
  }, [dagData.version, urlRound, highlightSkipped]);

  // --- Write URL state when paused ---
  useEffect(() => {
    if (!dagData.paused) {
      if (window.location.hash) {
        window.history.replaceState(null, '', window.location.pathname);
      }
      return;
    }
    const view = rendererRef.current?.getView();
    if (!view) return;
    const params = new URLSearchParams();
    params.set('r', String(view.centerRound));
    if (highlightSkipped) params.set('s', '1');
    window.history.replaceState(null, '', '#' + params.toString());
  }, [dagData.paused, highlightSkipped]);

  // Refs for values needed inside the wheel-shift callback (avoids stale closures)
  const selectedEpochRef = useRef(selectedEpoch);
  selectedEpochRef.current = selectedEpoch;
  const overrideCommitteeRef = useRef(overrideCommittee);
  overrideCommitteeRef.current = overrideCommittee;
  const wheelFetchTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Cleanup debounce timeout on unmount
  useEffect(() => {
    return () => {
      if (wheelFetchTimeoutRef.current) {
        clearTimeout(wheelFetchTimeoutRef.current);
      }
    };
  }, []);

  const handleRendererReady = useCallback((renderer: DagRenderer) => {
    rendererRef.current = renderer;
    renderer.setTheme(theme);

    // Debounced fetch when scrolling with the mouse wheel
    renderer.onWheelShift(() => {
      if (wheelFetchTimeoutRef.current) clearTimeout(wheelFetchTimeoutRef.current);
      wheelFetchTimeoutRef.current = setTimeout(() => {
        if (overrideCommitteeRef.current) return;
        const r = rendererRef.current;
        if (!r) return;
        const { minRound, maxRound } = r.getVisibleRoundRange();
        const from = Math.max(1, minRound);
        dagData.fetchWindow(from, maxRound, selectedEpochRef.current).catch(() => {});
      }, 200);
    });
  }, [theme, dagData]);

  const handleShiftView = useCallback((delta: number) => {
    const renderer = rendererRef.current;
    if (!renderer) return;
    renderer.shiftView(delta);

    if (!overrideCommittee) {
      const { minRound, maxRound } = renderer.getVisibleRoundRange();
      const from = Math.max(1, minRound);
      dagData.fetchWindow(from, maxRound, selectedEpoch).catch(() => {});
    }
  }, [dagData, selectedEpoch, overrideCommittee]);

  const handleGoToRound = useCallback((round: number) => {
    const renderer = rendererRef.current;
    if (!renderer) return;

    // Exit imported view mode — "Go" is an explicit user action to fetch from the backend
    if (overrideCommittee) {
      setOverrideCommittee(null);
    }

    renderer.goToRound(round);
    const { minRound, maxRound } = renderer.getVisibleRoundRange();
    dagData.fetchWindow(Math.max(1, minRound), maxRound, selectedEpoch).catch(() => {});
  }, [selectedEpoch, dagData, overrideCommittee]);

  const handleToggleHighlightSkipped = useCallback((enabled: boolean) => {
    setHighlightSkipped(enabled);
    rendererRef.current?.setHighlightSkippedOnly(enabled);
  }, []);

  const handleExport = useCallback(() => {
    rendererRef.current?.exportPNG();
  }, []);

  const handleEquivocationClick = useCallback(() => {
    const renderer = rendererRef.current;
    if (!renderer || dagData.equivocations.size === 0) return;

    // Find the latest equivocation key
    let latestKey = 0;
    for (const key of dagData.equivocations) {
      if (key > latestKey) latestKey = key;
    }

    dagData.setPaused(true);
    renderer.goToRound(roundFromKey(latestKey));
    renderer.pinByKey(latestKey);
  }, [dagData]);

  const handleSearchDigest = useCallback((digest: string) => {
    const renderer = rendererRef.current;
    if (!renderer) return;
    if (digest.length >= 2) {
      const count = renderer.highlightByDigest(digest);
      setSearchMatchCount(count);
    } else {
      renderer.clearSearchHighlights();
      setSearchMatchCount(0);
    }
  }, []);

  const handleTogglePause = useCallback(async (paused: boolean) => {
    if (!paused && (selectedEpoch !== undefined || overrideCommittee !== null)) {
      // Resuming from epoch or imported view — reset and catch up to live
      setSelectedEpoch(undefined);
      setOverrideCommittee(null);
      dagData.importData([], []);
      dagData.setPaused(false);
      try {
        const status = await fetchStatus();
        const toRound = status.highest_accepted_round;
        const fromRound = Math.max(1, toRound - VISIBLE_ROUNDS);
        await dagData.fetchWindow(fromRound, toRound);
      } catch {
        // WS will catch up
      }
      return;
    }
    dagData.setPaused(paused);
  }, [dagData, selectedEpoch, overrideCommittee]);

  const handleEpochChange = useCallback(async (epoch: number | undefined) => {
    setSelectedEpoch(epoch);

    if (epoch === undefined) {
      // Back to live — reset and catch up
      dagData.importData([], []);
      dagData.setPaused(false);
      try {
        const status = await fetchStatus();
        const toRound = status.highest_accepted_round;
        const fromRound = Math.max(1, toRound - VISIBLE_ROUNDS);
        await dagData.fetchWindow(fromRound, toRound);
      } catch {
        // WS will catch up
      }
      return;
    }

    const epochInfo = epochs.find((e) => e.epoch === epoch);
    if (!epochInfo) return;

    try {
      const toRound = epochInfo.to_round;
      const fromRound = Math.max(epochInfo.from_round, toRound - VISIBLE_ROUNDS);
      const dagWindow = await fetchDag(fromRound, toRound, epoch);
      dagData.importData(dagWindow.blocks, dagWindow.leaders);
      // Two rAF frames: PixiJS needs one frame to process the imported data
      // and update the scene graph before viewport positioning can be applied.
      requestAnimationFrame(() => {
        requestAnimationFrame(() => {
          rendererRef.current?.snapToView(toRound);
        });
      });
    } catch (err) {
      console.error('Failed to load epoch:', err);
    }
  }, [epochs, dagData]);

  const handleSaveView = useCallback(() => {
    const comm = overrideCommittee ?? committee;
    if (!comm) return;
    const view = rendererRef.current?.getView() ?? { centerRound: 0 };
    const saved: SavedDagView = {
      version: 1,
      savedAt: new Date().toISOString(),
      committee: comm,
      blocks: [...dagData.blocks.values()],
      leaders: [...dagData.leaders.values()],
      viewport: view,
      highlightSkipped,
    };
    const binary = encodeDagView(saved);
    const blob = new Blob([binary], { type: 'application/octet-stream' });
    const link = document.createElement('a');
    link.download = `dag-view-${Date.now()}.dagv`;
    link.href = URL.createObjectURL(blob);
    document.body.appendChild(link);
    link.click();
    document.body.removeChild(link);
    URL.revokeObjectURL(link.href);
  }, [committee, overrideCommittee, dagData, highlightSkipped]);

  const applyLoadedView = useCallback((saved: SavedDagView) => {
    setOverrideCommittee(saved.committee);
    setHighlightSkipped(saved.highlightSkipped);
    dagData.importData(saved.blocks, saved.leaders);

    // Two rAF frames: PixiJS needs one frame to process the imported data
    // and update the scene graph before viewport positioning can be applied.
    requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        const renderer = rendererRef.current;
        if (renderer) {
          renderer.setView(saved.viewport.centerRound);
          renderer.setHighlightSkippedOnly(saved.highlightSkipped);
        }
        setViewLoading(false);
      });
    });
  }, [dagData]);

  const handleLoadView = useCallback(
    (file: File) => {
      setViewLoading(true);
      dagData.setPaused(true);

      const reader = new FileReader();
      reader.onload = (e) => {
        try {
          const buf = e.target!.result as ArrayBuffer;

          if (!isDagViewBinary(buf)) {
            throw new Error('Invalid file: not a .dagv file');
          }
          const saved = decodeDagView(buf);

          applyLoadedView(saved);
        } catch (err) {
          console.error('Failed to load saved view:', err);
          setViewLoading(false);
        }
      };
      reader.readAsArrayBuffer(file);
    },
    [dagData, applyLoadedView],
  );

  // Keyboard shortcuts — use a ref so the handler doesn't re-register on every data update
  const dagDataRef = useRef(dagData);
  dagDataRef.current = dagData;

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Ignore when focused on an input element
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLSelectElement) return;

      const current = dagDataRef.current;
      switch (e.key) {
        case ' ':
          e.preventDefault();
          current.setPaused(!current.paused);
          break;
        case 'ArrowLeft':
          e.preventDefault();
          rendererRef.current?.shiftView(e.shiftKey ? -10 : -1);
          break;
        case 'ArrowRight':
          e.preventDefault();
          rendererRef.current?.shiftView(e.shiftKey ? 10 : 1);
          break;
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, []);

  const numAuthorities = activeCommittee?.validators.length ?? 0;

  // Warn if BlockKey encoding limit is exceeded
  useEffect(() => {
    if (numAuthorities >= 1000) {
      console.warn(
        `BlockKey encoding supports max 999 authorities, but committee has ${numAuthorities}. Visualization may show incorrect data.`,
      );
    }
  }, [numAuthorities]);

  if (loading && !overrideCommittee) {
    return (
      <div className="h-screen flex items-center justify-center dag-label">
        Loading committee data...
      </div>
    );
  }

  if (error && !overrideCommittee) {
    return (
      <div className="h-screen flex items-center justify-center dag-status-bad">
        Error: {error}
      </div>
    );
  }

  return (
    <div className="h-screen flex flex-col">
      <StatusBar
        status={dagData.status}
        connected={dagData.connected}
        justReconnected={dagData.justReconnected}
        leaders={dagData.leaders}
        equivocationCount={dagData.equivocations.size}
        onEquivocationClick={handleEquivocationClick}
      />
      <div className="relative flex-1 overflow-hidden">
        <DagCanvas
          dagData={dagData}
          committee={activeCommittee}
          onRendererReady={handleRendererReady}
          disableEviction={overrideCommittee !== null}
        />
        {viewLoading && (
          <div className="absolute inset-0 flex items-center justify-center z-20" style={{ backgroundColor: 'rgba(0,0,0,0.5)' }}>
            <div className="dag-panel border rounded-xl px-8 py-6 text-center">
              <div className="text-lg font-semibold" style={{ color: 'var(--dag-text)' }}>
                Loading saved view…
              </div>
            </div>
          </div>
        )}
        {showEmptyWarning && (
          <div className="absolute inset-0 flex items-center justify-center pointer-events-none z-10">
            <div className="dag-panel border rounded-xl px-8 py-6 text-center max-w-md pointer-events-auto">
              <div className="text-lg font-semibold mb-2" style={{ color: 'var(--dag-text)' }}>
                No DAG data received
              </div>
              <div className="text-sm" style={{ color: 'var(--dag-text-muted)' }}>
                {dagData.connected
                  ? 'The backend is connected but the validator is not sending any blocks. Verify that the validator connection is alive.'
                  : 'Cannot connect to the backend server. Check that dag-visualizer-server is running and accessible.'}
              </div>
            </div>
          </div>
        )}
        <ControlPanel
          paused={dagData.paused}
          onTogglePause={handleTogglePause}
          onShiftView={handleShiftView}
          onGoToRound={handleGoToRound}
          selectedEpoch={selectedEpoch}
          onEpochChange={handleEpochChange}
          epochs={epochs}
          highlightSkipped={highlightSkipped}
          onToggleHighlightSkipped={handleToggleHighlightSkipped}
          onExport={handleExport}
          onSearchDigest={handleSearchDigest}
          searchMatchCount={searchMatchCount}
          onSaveView={handleSaveView}
          onLoadView={handleLoadView}
          theme={theme}
          onToggleTheme={handleToggleTheme}
        />
        <StatsPanel dagData={dagData} numAuthorities={numAuthorities} />
        <Legend />
        <div className="absolute bottom-2 left-1/2 -translate-x-1/2 text-xs pointer-events-none" style={{ color: 'var(--dag-text-muted)' }}>
          Scroll to navigate rounds
        </div>
      </div>
    </div>
  );
}
