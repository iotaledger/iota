// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Axum HTTP/WebSocket server for the DAG visualizer.
//! Serves REST endpoints (binary) and WebSocket for real-time events.

use std::{net::SocketAddr, sync::Arc};

use axum::{
    Router,
    extract::{
        Query, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    response::IntoResponse,
    routing::get,
};
use serde::Deserialize;
use tokio::sync::broadcast;
use tower_http::cors::CorsLayer;
use tracing::{info, warn};

/// Maximum rounds returned in a single DAG window request.
const MAX_DAG_WINDOW: u32 = 50;

use crate::{snapshot::build_dag_window, storage::StorageManager, types::*};

/// Shared state for the Axum server.
#[derive(Clone)]
pub struct AppState {
    pub storage: Arc<StorageManager>,
    pub event_tx: broadcast::Sender<DagVisualizerEvent>,
}

/// Query parameters for the DAG window endpoint.
#[derive(Deserialize)]
struct DagQuery {
    epoch: Option<u64>,
    from_round: Option<u32>,
    to_round: Option<u32>,
}

/// Return an `application/octet-stream` response from raw bytes.
fn binary_response(body: Vec<u8>) -> impl IntoResponse {
    (
        [(
            http::header::CONTENT_TYPE,
            http::HeaderValue::from_static("application/octet-stream"),
        )],
        body,
    )
}

/// `GET /api/v1/committee` — committee info (binary).
async fn get_committee(State(state): State<AppState>) -> impl IntoResponse {
    let committee = state
        .storage
        .current_epoch_store()
        .and_then(|store| store.get_committee())
        .map(|c| CommitteeMessage {
            epoch: c.epoch,
            total_stake: c.total_stake,
            quorum_threshold: c.quorum_threshold,
            validators: c
                .validators
                .iter()
                .map(|v| ValidatorMessage {
                    index: v.index,
                    hostname: v.hostname.clone(),
                    stake: v.stake,
                })
                .collect(),
        })
        .unwrap_or(CommitteeMessage {
            epoch: 0,
            total_stake: 0,
            quorum_threshold: 0,
            validators: vec![],
        });
    let mut buf = Vec::new();
    committee.encode_binary(&mut buf);
    binary_response(buf)
}

/// Compute the (from, to) range for a DAG window query.
fn compute_dag_range(
    last_round: u32,
    from_round: Option<u32>,
    to_round: Option<u32>,
) -> (u32, u32) {
    let to = to_round.unwrap_or(last_round);
    let from = from_round.unwrap_or_else(|| to.saturating_sub(MAX_DAG_WINDOW)).min(to);
    let from = if to - from > MAX_DAG_WINDOW {
        to - MAX_DAG_WINDOW
    } else {
        from
    };
    (from, to)
}

/// `GET /api/v1/dag?from_round=X&to_round=Y[&epoch=N]` — windowed DAG snapshot (binary).
async fn get_dag(
    State(state): State<AppState>,
    Query(query): Query<DagQuery>,
) -> impl IntoResponse {
    let empty = DagWindowMessage {
        from_round: 0,
        to_round: 0,
        highest_accepted_round: 0,
        last_commit_round: 0,
        blocks: vec![],
        leaders: vec![],
    };

    let store = if let Some(epoch) = query.epoch {
        match state.storage.get_epoch(epoch) {
            Some(s) => s,
            None => {
                let mut buf = Vec::new();
                empty.encode_binary(&mut buf);
                return binary_response(buf);
            }
        }
    } else {
        match state.storage.current_epoch_store() {
            Some(s) => s,
            None => {
                let mut buf = Vec::new();
                empty.encode_binary(&mut buf);
                return binary_response(buf);
            }
        }
    };

    let last_round = store.get_last_round();
    let (from, to) = compute_dag_range(last_round, query.from_round, query.to_round);
    let window = build_dag_window(&store, from, to);
    let mut buf = Vec::new();
    window.encode_binary(&mut buf);
    binary_response(buf)
}

/// `GET /api/v1/status` — current status (binary, 16 bytes).
async fn get_status(State(state): State<AppState>) -> impl IntoResponse {
    let status = state
        .storage
        .current_epoch_store()
        .and_then(|store| store.get_status())
        .map(|s| StatusMessage {
            highest_accepted_round: s.highest_accepted_round,
            last_commit_index: s.last_commit_index,
            last_commit_round: s.last_commit_round,
            num_authorities: s.num_authorities,
        })
        .unwrap_or(StatusMessage {
            highest_accepted_round: 0,
            last_commit_index: 0,
            last_commit_round: 0,
            num_authorities: 0,
        });
    let mut buf = Vec::new();
    status.encode_binary(&mut buf);
    binary_response(buf)
}

/// `GET /api/v1/epochs` — available epochs (binary).
async fn get_epochs(State(state): State<AppState>) -> impl IntoResponse {
    let epochs: Vec<EpochInfo> = state
        .storage
        .list_epochs()
        .into_iter()
        .map(|(epoch, from, to)| EpochInfo {
            epoch,
            from_round: from,
            to_round: to,
        })
        .collect();
    let mut buf = Vec::new();
    encode_epochs_binary(&epochs, &mut buf);
    binary_response(buf)
}

/// `GET /api/v1/ws` — WebSocket upgrade for real-time events.
async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws(socket, state))
}

/// Handle a single WebSocket connection — forward broadcast events as binary.
async fn handle_ws(mut socket: WebSocket, state: AppState) {
    let mut rx = state.event_tx.subscribe();
    let mut ping_interval = tokio::time::interval(std::time::Duration::from_secs(5));
    ping_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            result = rx.recv() => {
                match result {
                    Ok(event) => {
                        let mut buf = Vec::new();
                        event.encode_binary(&mut buf);
                        if socket.send(Message::Binary(buf.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!("DAG visualizer WebSocket lagged, skipped {n} events");
                        // Notify the client so it can trigger a full refresh
                        let mut buf = Vec::new();
                        encode_lagged_event(n, &mut buf);
                        if socket.send(Message::Binary(buf.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }
            _ = ping_interval.tick() => {
                if socket.send(Message::Ping(vec![].into())).await.is_err() {
                    break;
                }
            }
            msg = socket.recv() => {
                // Client disconnected or sent close frame
                if msg.is_none() {
                    break;
                }
            }
        }
    }
}

/// Start the HTTP/WebSocket server.
pub async fn start_http_server(
    listen_addr: SocketAddr,
    storage: Arc<StorageManager>,
    event_tx: broadcast::Sender<DagVisualizerEvent>,
) {
    let state = AppState { storage, event_tx };

    let app = Router::new()
        .route("/api/v1/committee", get(get_committee))
        .route("/api/v1/dag", get(get_dag))
        .route("/api/v1/status", get(get_status))
        .route("/api/v1/epochs", get(get_epochs))
        .route("/api/v1/ws", get(ws_handler))
        // All endpoints are read-only (GET + WebSocket) and intentionally
        // unauthenticated — this is a publicly-hosted visualization tool.
        .layer(
            CorsLayer::new()
                .allow_origin(tower_http::cors::Any)
                .allow_methods([http::Method::GET])
                .allow_headers(tower_http::cors::Any),
        )
        .with_state(state);

    info!("DAG visualizer HTTP server listening on http://{listen_addr}");
    let listener = match tokio::net::TcpListener::bind(listen_addr).await {
        Ok(l) => l,
        Err(e) => {
            warn!("Failed to bind DAG visualizer HTTP server to {listen_addr}: {e}");
            return;
        }
    };
    if let Err(e) = axum::serve(listener, app).await {
        warn!("DAG visualizer HTTP server error: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_defaults_to_last_50() {
        assert_eq!(compute_dag_range(100, None, None), (50, 100));
    }

    #[test]
    fn range_explicit_from_to() {
        assert_eq!(compute_dag_range(100, Some(20), Some(40)), (20, 40));
    }

    #[test]
    fn range_from_clamped_to_to() {
        assert_eq!(compute_dag_range(100, Some(60), Some(40)), (40, 40));
    }

    #[test]
    fn range_clamped_to_max_window() {
        assert_eq!(compute_dag_range(200, Some(0), Some(200)), (150, 200));
    }

    #[test]
    fn range_zero_last_round() {
        assert_eq!(compute_dag_range(0, None, None), (0, 0));
    }
}
