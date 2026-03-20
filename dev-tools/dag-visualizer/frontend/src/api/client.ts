// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import type { CommitteeMessage, DagVisualizerEvent, DagWindowMessage, EpochInfo, StatusMessage } from './types';
import { decodeCommittee, decodeDagEvent, decodeDagWindow, decodeEpochs, decodeStatus } from './codec';

const BASE_URL = window.location.origin;

export async function fetchCommittee(): Promise<CommitteeMessage> {
  const res = await fetch(`${BASE_URL}/api/v1/committee`);
  if (!res.ok) throw new Error(`Failed to fetch committee: ${res.statusText}`);
  return decodeCommittee(await res.arrayBuffer());
}

export async function fetchDag(fromRound: number, toRound: number, epoch?: number): Promise<DagWindowMessage> {
  let url = `${BASE_URL}/api/v1/dag?from_round=${fromRound}&to_round=${toRound}`;
  if (epoch !== undefined) url += `&epoch=${epoch}`;
  const res = await fetch(url);
  if (!res.ok) throw new Error(`Failed to fetch DAG: ${res.statusText}`);
  return decodeDagWindow(await res.arrayBuffer());
}

export async function fetchEpochs(): Promise<EpochInfo[]> {
  const res = await fetch(`${BASE_URL}/api/v1/epochs`);
  if (!res.ok) throw new Error(`Failed to fetch epochs: ${res.statusText}`);
  return decodeEpochs(await res.arrayBuffer());
}

export async function fetchStatus(): Promise<StatusMessage> {
  const res = await fetch(`${BASE_URL}/api/v1/status`);
  if (!res.ok) throw new Error(`Failed to fetch status: ${res.statusText}`);
  return decodeStatus(await res.arrayBuffer());
}

/** If no message (including pings) arrives within this period, assume connection is dead. */
const HEARTBEAT_TIMEOUT_MS = 15_000;

export function connectWebSocket(
  onMessage: (event: DagVisualizerEvent) => void,
  onReconnect?: () => void,
): () => void {
  let activeWs: WebSocket | null = null;
  let disposed = false;
  let retryDelay = 1000;
  let hasConnected = false;
  let heartbeatTimer: ReturnType<typeof setTimeout> | null = null;
  let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  const maxRetryDelay = 30000;

  function clearTimers() {
    if (heartbeatTimer) { clearTimeout(heartbeatTimer); heartbeatTimer = null; }
    if (reconnectTimer) { clearTimeout(reconnectTimer); reconnectTimer = null; }
  }

  function scheduleReconnect() {
    if (disposed || reconnectTimer) return;
    reconnectTimer = setTimeout(() => {
      reconnectTimer = null;
      retryDelay = Math.min(retryDelay * 2, maxRetryDelay);
      connect();
    }, retryDelay);
  }

  function connect() {
    if (disposed) return;

    // Close any lingering previous connection
    if (activeWs) {
      const old = activeWs;
      activeWs = null;
      old.onopen = null;
      old.onmessage = null;
      old.onclose = null;
      old.onerror = null;
      old.close();
    }

    const wsProtocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const wsUrl = `${wsProtocol}//${window.location.host}/api/v1/ws`;
    const sock = new WebSocket(wsUrl);
    sock.binaryType = 'arraybuffer';
    activeWs = sock;

    sock.onopen = () => {
      if (sock !== activeWs) return; // stale
      retryDelay = 1000;
      // Start heartbeat
      if (heartbeatTimer) clearTimeout(heartbeatTimer);
      heartbeatTimer = setTimeout(function tick() {
        if (sock === activeWs) sock.close();
      }, HEARTBEAT_TIMEOUT_MS);

      if (hasConnected) {
        onReconnect?.();
      }
      hasConnected = true;
    };

    sock.onmessage = (event) => {
      if (sock !== activeWs) return;
      // Reset heartbeat on any incoming frame
      if (heartbeatTimer) clearTimeout(heartbeatTimer);
      heartbeatTimer = setTimeout(() => {
        if (sock === activeWs) sock.close();
      }, HEARTBEAT_TIMEOUT_MS);

      if (!(event.data instanceof ArrayBuffer)) return; // ignore non-binary (e.g. pong)
      let data: DagVisualizerEvent;
      try {
        data = decodeDagEvent(event.data);
      } catch {
        return; // ignore malformed messages
      }
      onMessage(data);
    };

    sock.onclose = () => {
      if (sock !== activeWs) return; // stale — already replaced
      activeWs = null;
      if (heartbeatTimer) { clearTimeout(heartbeatTimer); heartbeatTimer = null; }
      scheduleReconnect();
    };

    sock.onerror = () => {
      // onerror is always followed by onclose — just let onclose handle reconnect
    };
  }

  connect();

  return () => {
    disposed = true;
    clearTimers();
    if (activeWs) {
      const old = activeWs;
      activeWs = null;
      old.onopen = null;
      old.onmessage = null;
      old.onclose = null;
      old.onerror = null;
      old.close();
    }
  };
}
