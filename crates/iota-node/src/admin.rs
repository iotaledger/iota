// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{net::SocketAddr, str::FromStr, sync::Arc};

use axum::{
    Router,
    extract::{Query, RawQuery, State},
    http::StatusCode,
    response::{IntoResponse as _, Response},
    routing::{get, post},
};
use base64::Engine;
use humantime::parse_duration;
use iota_core::authority::AuthorityState;
use iota_sdk_types::RandomnessRound;
use iota_types::{
    base_types::AuthorityName,
    crypto::{RandomnessPartialSignature, RandomnessSignature},
    error::{IotaError, IotaResult},
    traffic_control::{AccessRuleList, BlockReason, DefaultAction, TrafficControlReconfigParams},
};
use serde::{Deserialize, Serialize};
use telemetry_subscribers::{TelemetryError, TracingHandle};
use tokio::sync::oneshot;
use tracing::info;

use crate::IotaNode;

// Example commands:
//
// Set buffer stake for current epoch 2 to 1500 basis points:
//
//   $ curl -X POST 'http://127.0.0.1:1337/set-override-buffer-stake?buffer_bps=1500&epoch=2'
//
// Clear buffer stake override for current epoch 2, use
// ProtocolConfig::buffer_stake_for_protocol_upgrade_bps:
//
//   $ curl -X POST 'http://127.0.0.1:1337/clear-override-buffer-stake?epoch=2'
//
// Vote to close epoch 2 early
//
//   $ curl -X POST 'http://127.0.0.1:1337/force-close-epoch?epoch=2'
//
// View current all capabilities from all authorities that have been received by
// this node:
//
//   $ curl 'http://127.0.0.1:1337/capabilities'
//
// View the node config (private keys will be masked):
//
//   $ curl 'http://127.0.0.1:1337/node-config'
//
// Set a time-limited tracing config. After the duration expires, tracing will
// be disabled automatically.
//
//   $ curl -X POST 'http://127.0.0.1:1337/enable-tracing?filter=info&duration=10s'
//
// Reset tracing to the TRACE_FILTER env var.
//
//   $ curl -X POST 'http://127.0.0.1:1337/reset-tracing'
//
// Get the node's randomness partial signatures for round 123.
//
//  $ curl 'http://127.0.0.1:1337/randomness-partial-sigs?round=123'
//
// Inject a randomness partial signature from another node, bypassing validity
// checks.
//
//  $ curl 'http://127.0.0.1:1337/randomness-inject-partial-sigs?authority_name=hexencodedname&round=123&sigs=base64encodedsigs'
//
// Inject a full signature from another node, bypassing validity checks.
//
//  $ curl 'http://127.0.0.1:1337/randomness-inject-full-sig?round=123&sigs=base64encodedsig'
//
// Reconfigure traffic control policy
//
//  $ curl -X POST 'http://127.0.0.1:1337/traffic-control?error_threshold=100&spam_threshold=100&dry_run=true&connection_blocklist_ttl_sec=60&proxy_blocklist_ttl_sec=60'
//
// Set the simulation gas limit; turning it on takes both values
//
//  $ curl -X POST 'http://127.0.0.1:1337/traffic-control?simulation_gas_per_sec=1000000&simulation_burst_gas=20000000'
//
// Set the default action of the access rules, `limit` or `deny`
//
//  $ curl -X POST 'http://127.0.0.1:1337/traffic-control?default_action=deny'
//
// View the traffic control settings in force, as JSON
//
//  $ curl 'http://127.0.0.1:1337/traffic-control'
//
// View the traffic control settings in force in the shape of `policy-config`,
// as JSON
//
//  $ curl 'http://127.0.0.1:1337/traffic-control/config'
//
// View, add and remove access rules, where `list` is `allow` or `deny` and
// `client` an IP address or a CIDR network
//
//  $ curl 'http://127.0.0.1:1337/traffic-control/rules'
//  $ curl -X POST 'http://127.0.0.1:1337/traffic-control/rules/add?list=allow&client=198.51.100.0/24'
//  $ curl -X POST 'http://127.0.0.1:1337/traffic-control/rules/remove?list=allow&client=198.51.100.0/24'
//
// View the blocked clients, at most `limit` of them (200 by default)
//
//  $ curl 'http://127.0.0.1:1337/traffic-control/blocklist?limit=100'
//
// Block a client for `ttl_sec` seconds, and unblock it
//
//  $ curl -X POST 'http://127.0.0.1:1337/traffic-control/block?client=203.0.113.9&ttl_sec=3600'
//  $ curl -X POST 'http://127.0.0.1:1337/traffic-control/unblock?client=203.0.113.9'
//
// View up to `top` clients with the lowest simulation gas balances, and up to
// `top` blocked clients, 200 of each by default
//
//  $ curl 'http://127.0.0.1:1337/traffic-control/clients?top=20'
//
// Every traffic control change lasts until the node restarts.
//
// View the Prometheus metrics filter: the directives currently in effect and
// the startup directives (config with the METRICS_FILTER env var merged over
// it) that a reset restores.
//
//   $ curl 'http://127.0.0.1:1337/metrics/filters'
//
// Set the runtime override for the metrics the /metrics endpoint exposes.
// Patterns may be group names from the `metrics.groups` config section
// (`default` and `hardware` included) or raw METRICS_FILTER-style patterns.
// The override is merged over the startup directives: an override directive
// replaces the startup directive with the same pattern, and otherwise the
// most specific matching pattern decides each metric, whichever source it
// came from. A bare level (e.g. `trace`) replaces the startup directives
// entirely, exposing everything up to that level; `default=LEVEL` changes
// only the level for the metrics no other directive matches. Each POST
// starts from the startup directives again rather than stacking on the
// previous override.
//
//   $ curl -X POST 'http://127.0.0.1:1337/metrics/filters?filter=consensus=off,typed_store=warn'
//
// Drop the runtime override, restoring the startup configuration.
//
//   $ curl -X POST 'http://127.0.0.1:1337/metrics/filters/reset'

const LOGGING_ROUTE: &str = "/logging";
const TRACING_ROUTE: &str = "/enable-tracing";
const TRACING_RESET_ROUTE: &str = "/reset-tracing";
const SET_BUFFER_STAKE_ROUTE: &str = "/set-override-buffer-stake";
const CLEAR_BUFFER_STAKE_ROUTE: &str = "/clear-override-buffer-stake";
const FORCE_CLOSE_EPOCH: &str = "/force-close-epoch";
const CAPABILITIES: &str = "/capabilities";
const NODE_CONFIG: &str = "/node-config";
const RANDOMNESS_PARTIAL_SIGS_ROUTE: &str = "/randomness-partial-sigs";
const RANDOMNESS_INJECT_PARTIAL_SIGS_ROUTE: &str = "/randomness-inject-partial-sigs";
const RANDOMNESS_INJECT_FULL_SIG_ROUTE: &str = "/randomness-inject-full-sig";
const FLAMEGRAPH_ROUTE: &str = "/flamegraph";
const TRAFFIC_CONTROL: &str = "/traffic-control";
const TRAFFIC_CONTROL_CONFIG: &str = "/traffic-control/config";
const TRAFFIC_CONTROL_RULES: &str = "/traffic-control/rules";
const TRAFFIC_CONTROL_RULES_ADD: &str = "/traffic-control/rules/add";
const TRAFFIC_CONTROL_RULES_REMOVE: &str = "/traffic-control/rules/remove";
const TRAFFIC_CONTROL_BLOCKLIST: &str = "/traffic-control/blocklist";
const TRAFFIC_CONTROL_BLOCK: &str = "/traffic-control/block";
const TRAFFIC_CONTROL_UNBLOCK: &str = "/traffic-control/unblock";
const TRAFFIC_CONTROL_CLIENTS: &str = "/traffic-control/clients";

/// How many blocked clients `/traffic-control/blocklist` lists by default.
const DEFAULT_BLOCKLIST_LIMIT: usize = 200;
/// How many clients `/traffic-control/clients` lists by default, of each
/// kind.
const DEFAULT_CLIENTS_TOP: usize = 200;
/// Ends the response to every traffic control change.
const UNTIL_RESTART: &str = "The change lasts until the node restarts.\n";
const METRICS_FILTER_ROUTE: &str = "/metrics/filters";
const METRICS_FILTER_RESET_ROUTE: &str = "/metrics/filters/reset";

struct AppState {
    node: Arc<IotaNode>,
    tracing_handle: TracingHandle,
}

pub async fn run_admin_server(
    node: Arc<IotaNode>,
    socket_address: SocketAddr,
    tracing_handle: TracingHandle,
) {
    let filter = tracing_handle.get_log().unwrap();

    let app_state = AppState {
        node,
        tracing_handle,
    };

    let app = Router::new()
        .route(LOGGING_ROUTE, get(get_filter))
        .route(CAPABILITIES, get(capabilities))
        .route(NODE_CONFIG, get(node_config))
        .route(LOGGING_ROUTE, post(set_filter))
        .route(
            SET_BUFFER_STAKE_ROUTE,
            post(set_override_protocol_upgrade_buffer_stake),
        )
        .route(
            CLEAR_BUFFER_STAKE_ROUTE,
            post(clear_override_protocol_upgrade_buffer_stake),
        )
        .route(FORCE_CLOSE_EPOCH, post(force_close_epoch))
        .route(TRACING_ROUTE, post(enable_tracing))
        .route(TRACING_RESET_ROUTE, post(reset_tracing))
        .route(RANDOMNESS_PARTIAL_SIGS_ROUTE, get(randomness_partial_sigs))
        .route(
            RANDOMNESS_INJECT_PARTIAL_SIGS_ROUTE,
            post(randomness_inject_partial_sigs),
        )
        .route(
            RANDOMNESS_INJECT_FULL_SIG_ROUTE,
            post(randomness_inject_full_sig),
        )
        .route(FLAMEGRAPH_ROUTE, get(flamegraph))
        .route(TRAFFIC_CONTROL, post(traffic_control))
        .route(TRAFFIC_CONTROL, get(traffic_control_status))
        .route(TRAFFIC_CONTROL_CONFIG, get(traffic_control_config))
        .route(TRAFFIC_CONTROL_RULES, get(traffic_control_rules))
        .route(TRAFFIC_CONTROL_RULES_ADD, post(add_traffic_control_rule))
        .route(
            TRAFFIC_CONTROL_RULES_REMOVE,
            post(remove_traffic_control_rule),
        )
        .route(TRAFFIC_CONTROL_BLOCKLIST, get(traffic_control_blocklist))
        .route(TRAFFIC_CONTROL_BLOCK, post(block_traffic_control_client))
        .route(
            TRAFFIC_CONTROL_UNBLOCK,
            post(unblock_traffic_control_client),
        )
        .route(TRAFFIC_CONTROL_CLIENTS, get(traffic_control_clients))
        .route(METRICS_FILTER_ROUTE, get(get_metrics_filter))
        .route(METRICS_FILTER_ROUTE, post(set_metrics_filter))
        .route(METRICS_FILTER_RESET_ROUTE, post(reset_metrics_filter))
        .with_state(Arc::new(app_state));

    info!(
        filter =% filter,
        address =% socket_address,
        "starting admin server"
    );

    let listener = tokio::net::TcpListener::bind(&socket_address)
        .await
        .unwrap();
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}

#[derive(Deserialize)]
struct EnableTracing {
    // These params change the filter, and reset it after the duration expires.
    filter: Option<String>,
    duration: Option<String>,

    // Change the trace output file (if file output was enabled at program start)
    trace_file: Option<String>,

    // Change the tracing sample rate
    sample_rate: Option<f64>,
}

async fn enable_tracing(
    State(state): State<Arc<AppState>>,
    query: Query<EnableTracing>,
) -> (StatusCode, String) {
    let Query(EnableTracing {
        filter,
        duration,
        trace_file,
        sample_rate,
    }) = query;

    let mut response = Vec::new();

    if let Some(sample_rate) = sample_rate {
        state.tracing_handle.update_sampling_rate(sample_rate);
        response.push(format!("sample rate set to {sample_rate:?}"));
    }

    if let Some(trace_file) = trace_file {
        if let Err(err) = state.tracing_handle.update_trace_file(&trace_file) {
            response.push(format!("can't update trace file: {err:?}"));
            return (StatusCode::BAD_REQUEST, response.join("\n"));
        } else {
            response.push(format!("trace file set to {trace_file:?}"));
        }
    }

    let Some(filter) = filter else {
        return (StatusCode::OK, response.join("\n"));
    };

    // Duration is required if filter is set
    let Some(duration) = duration else {
        response.push("can't update filter: missing duration".into());
        return (StatusCode::BAD_REQUEST, response.join("\n"));
    };

    let Ok(duration) = parse_duration(&duration) else {
        response.push("can't update filter: invalid duration".into());
        return (StatusCode::BAD_REQUEST, response.join("\n"));
    };

    match state.tracing_handle.update_trace_filter(&filter, duration) {
        Ok(()) => {
            response.push(format!("filter set to {filter:?}"));
            response.push(format!("filter will be reset after {duration:?}"));
            (StatusCode::OK, response.join("\n"))
        }
        Err(TelemetryError::TracingDisabled) => {
            response.push("can't update filter: tracing is not enabled. to enable it, run the node with 'TRACE_FILTER' set.".into());
            (StatusCode::NOT_IMPLEMENTED, response.join("\n"))
        }
        Err(err) => {
            response.push(format!("can't update filter: {err:?}"));
            (StatusCode::BAD_REQUEST, response.join("\n"))
        }
    }
}

async fn reset_tracing(State(state): State<Arc<AppState>>) -> (StatusCode, String) {
    match state.tracing_handle.reset_trace() {
        Ok(()) => (
            StatusCode::OK,
            "tracing filter reset to TRACE_FILTER env var".into(),
        ),
        Err(TelemetryError::TracingDisabled) => (
            StatusCode::NOT_IMPLEMENTED,
            "tracing is not enabled. to enable it, run the node with 'TRACE_FILTER' set.".into(),
        ),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    }
}

async fn get_filter(State(state): State<Arc<AppState>>) -> (StatusCode, String) {
    match state.tracing_handle.get_log() {
        Ok(filter) => (StatusCode::OK, filter),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    }
}

async fn set_filter(
    State(state): State<Arc<AppState>>,
    new_filter: String,
) -> (StatusCode, String) {
    match state.tracing_handle.update_log(&new_filter) {
        Ok(()) => {
            info!(filter =% new_filter, "Log filter updated");
            (StatusCode::OK, "".into())
        }
        Err(err) => (StatusCode::BAD_REQUEST, err.to_string()),
    }
}

async fn capabilities(State(state): State<Arc<AppState>>) -> (StatusCode, String) {
    let epoch_store = state.node.state().load_epoch_store_one_call_per_task();

    let mut output = String::new();
    let capabilities = epoch_store.get_capabilities_v1();
    for capability in capabilities.unwrap_or_default() {
        output.push_str(&format!("{capability:?}\n"));
    }

    (StatusCode::OK, output)
}

async fn node_config(State(state): State<Arc<AppState>>) -> (StatusCode, String) {
    let node_config = &state.node.config;

    // Note private keys will be masked
    (StatusCode::OK, format!("{node_config:#?}\n"))
}

#[derive(Deserialize)]
struct Epoch {
    epoch: u64,
}

async fn clear_override_protocol_upgrade_buffer_stake(
    State(state): State<Arc<AppState>>,
    epoch: Query<Epoch>,
) -> (StatusCode, String) {
    let Query(Epoch { epoch }) = epoch;

    match state
        .node
        .clear_override_protocol_upgrade_buffer_stake(epoch)
    {
        Ok(()) => (
            StatusCode::OK,
            "protocol upgrade buffer stake cleared\n".to_string(),
        ),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    }
}

#[derive(Deserialize)]
struct SetBufferStake {
    buffer_bps: u64,
    epoch: u64,
}

async fn set_override_protocol_upgrade_buffer_stake(
    State(state): State<Arc<AppState>>,
    buffer_state: Query<SetBufferStake>,
) -> (StatusCode, String) {
    let Query(SetBufferStake { buffer_bps, epoch }) = buffer_state;

    match state
        .node
        .set_override_protocol_upgrade_buffer_stake(epoch, buffer_bps)
    {
        Ok(()) => (
            StatusCode::OK,
            format!("protocol upgrade buffer stake set to '{buffer_bps}'\n"),
        ),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    }
}

async fn force_close_epoch(
    State(state): State<Arc<AppState>>,
    epoch: Query<Epoch>,
) -> (StatusCode, String) {
    let Query(Epoch {
        epoch: expected_epoch,
    }) = epoch;
    let epoch_store = state.node.state().load_epoch_store_one_call_per_task();
    let actual_epoch = epoch_store.epoch();
    if actual_epoch != expected_epoch {
        let err = IotaError::WrongEpoch {
            expected_epoch,
            actual_epoch,
        };
        return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string());
    }

    match state.node.close_epoch(&epoch_store).await {
        Ok(()) => (
            StatusCode::OK,
            "close_epoch() called successfully\n".to_string(),
        ),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    }
}

#[derive(Deserialize)]
struct Round {
    round: u64,
}

async fn randomness_partial_sigs(
    State(state): State<Arc<AppState>>,
    round: Query<Round>,
) -> (StatusCode, String) {
    let Query(Round { round }) = round;

    let (tx, rx) = oneshot::channel();
    state
        .node
        .randomness_handle()
        .admin_get_partial_signatures(RandomnessRound::new(round), tx);

    let sigs = match rx.await {
        Ok(sigs) => sigs,
        Err(err) => return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    };

    let output = format!(
        "{}\n",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(sigs)
    );

    (StatusCode::OK, output)
}

#[derive(Deserialize)]
struct PartialSigsToInject {
    hex_authority_name: String,
    round: u64,
    base64_sigs: String,
}

async fn randomness_inject_partial_sigs(
    State(state): State<Arc<AppState>>,
    args: Query<PartialSigsToInject>,
) -> (StatusCode, String) {
    let Query(PartialSigsToInject {
        hex_authority_name,
        round,
        base64_sigs,
    }) = args;

    let authority_name = match AuthorityName::from_str(hex_authority_name.as_str()) {
        Ok(authority_name) => authority_name,
        Err(err) => return (StatusCode::BAD_REQUEST, err.to_string()),
    };

    let sigs: Vec<u8> = match base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(base64_sigs) {
        Ok(sigs) => sigs,
        Err(err) => return (StatusCode::BAD_REQUEST, err.to_string()),
    };

    let sigs: Vec<RandomnessPartialSignature> = match bcs::from_bytes(&sigs) {
        Ok(sigs) => sigs,
        Err(err) => return (StatusCode::BAD_REQUEST, err.to_string()),
    };

    let (tx_result, rx_result) = oneshot::channel();
    state
        .node
        .randomness_handle()
        .admin_inject_partial_signatures(
            authority_name,
            RandomnessRound::new(round),
            sigs,
            tx_result,
        );

    match rx_result.await {
        Ok(Ok(())) => (StatusCode::OK, "partial signatures injected\n".to_string()),
        Ok(Err(e)) => (StatusCode::BAD_REQUEST, e.to_string()),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

#[derive(Deserialize)]
struct FullSigToInject {
    round: u64,
    base64_sig: String,
}

async fn randomness_inject_full_sig(
    State(state): State<Arc<AppState>>,
    args: Query<FullSigToInject>,
) -> (StatusCode, String) {
    let Query(FullSigToInject { round, base64_sig }) = args;

    let sig: Vec<u8> = match base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(base64_sig) {
        Ok(sig) => sig,
        Err(err) => return (StatusCode::BAD_REQUEST, err.to_string()),
    };

    let sig: RandomnessSignature = match bcs::from_bytes(&sig) {
        Ok(sig) => sig,
        Err(err) => return (StatusCode::BAD_REQUEST, err.to_string()),
    };

    let (tx_result, rx_result) = oneshot::channel();
    state.node.randomness_handle().admin_inject_full_signature(
        RandomnessRound::new(round),
        sig,
        tx_result,
    );

    match rx_result.await {
        Ok(Ok(())) => (StatusCode::OK, "full signature injected\n".to_string()),
        Ok(Err(e)) => (StatusCode::BAD_REQUEST, e.to_string()),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

#[derive(Deserialize)]
struct Flamegraph {
    /// Toggle SVG response, otherwise return nested set model for Grafana.
    #[serde(default)]
    svg: bool,
    /// SVG width in pixels (when missing or set to 0 will default to 1920).
    #[serde(default)]
    width: usize,
    /// Select still running call graphs.
    #[serde(default)]
    running: bool,
    /// Select already completed call graphs.
    #[serde(default)]
    completed: bool,
    /// Select call graph with the given ID.
    #[serde(default)]
    graph_id: String,
    /// Use memory allocations as span measure rather than duration.
    #[serde(default)]
    mem: bool,
}

async fn flamegraph(State(state): State<Arc<AppState>>, query: Query<Flamegraph>) -> Response {
    if let Some(sub) = state.tracing_handle.get_flamegraph() {
        let Query(Flamegraph {
            svg,
            width,
            mut running,
            mut completed,
            graph_id,
            mem,
        }) = query;
        if !running && !completed {
            running = true;
            completed = true;
        }
        if svg {
            #[cfg(not(all(feature = "flamegraph-alloc", nightly)))]
            {
                if mem {
                    return (
                        StatusCode::BAD_REQUEST,
                        "memory flamegraphs are not supported (re-run iota-node with 'flamegraph-alloc' feature enabled and on nightly Rust toolchain)",
                    )
                        .into_response();
                }
            }

            // draw an svg
            let width = if width == 0 { Some(1920) } else { Some(width) };
            let config = telemetry_subscribers::flamegraph::SvgConfig {
                width,
                #[cfg(all(feature = "flamegraph-alloc", nightly))]
                measure_mem: mem,
                ..Default::default()
            };
            let svg = if !graph_id.is_empty() {
                sub.get_svg(&graph_id, running, completed, &config)
            } else {
                sub.get_combined_svg("iota-node", running, completed, &config)
            };
            if let Some(svg) = svg {
                (
                    [(
                        axum::http::header::CONTENT_TYPE,
                        axum::http::header::HeaderValue::from_static("image/svg+xml"),
                    )],
                    svg.into_string(),
                )
                    .into_response()
            } else {
                (StatusCode::NOT_FOUND, "Flamegraphs not found\n").into_response()
            }
        } else {
            // default nested set model for grafana
            let nested_frames = if !graph_id.is_empty() {
                sub.get_nested_set(&graph_id, running, completed)
            } else {
                sub.get_nested_sets("iota-node", running, completed)
            };
            if !nested_frames.is_empty() {
                axum::Json(nested_frames).into_response()
            } else {
                (StatusCode::NOT_FOUND, "Flamegraphs not found\n").into_response()
            }
        }
    } else {
        (
            StatusCode::NOT_FOUND,
            "Flamegraphs are not enabled (re-run iota-node with TRACE_FLAMEGRAPH=1)\n",
        )
            .into_response()
    }
}

async fn traffic_control(
    State(state): State<Arc<AppState>>,
    Query(params): Query<TrafficControlReconfigParams>,
) -> (StatusCode, String) {
    reconfigure_traffic_control(&state.node.state(), params).await
}

async fn reconfigure_traffic_control(
    state: &AuthorityState,
    params: TrafficControlReconfigParams,
) -> (StatusCode, String) {
    let sets_default_deny = params.default_action == Some(DefaultAction::Deny);
    match state.reconfigure_traffic_control(params).await {
        Ok(updated_state) => (
            StatusCode::OK,
            format!(
                "Traffic control configured with:\n\
                 Error threshold: {:?}\n\
                 Spam threshold: {:?}\n\
                 Dry run: {:?}\n\
                 Connection blocklist TTL sec: {:?}\n\
                 Proxy blocklist TTL sec: {:?}\n\
                 Simulation gas per sec: {:?}\n\
                 Simulation burst gas: {:?}\n\
                 Default action: {:?}\n\
                 {}{UNTIL_RESTART}",
                updated_state.error_threshold,
                updated_state.spam_threshold,
                updated_state.dry_run,
                updated_state.connection_blocklist_ttl_sec,
                updated_state.proxy_blocklist_ttl_sec,
                updated_state.simulation_gas_per_sec,
                updated_state.simulation_burst_gas,
                updated_state.default_action,
                if sets_default_deny {
                    DEFAULT_DENY_WARNING
                } else {
                    ""
                },
            ),
        ),
        Err(err) => error_response(err),
    }
}

const DEFAULT_DENY_WARNING: &str = "Warning: outside dry run, the default action deny refuses \
    every client that no allow rule names. On a validator this also refuses the other nodes \
    that send it requests. With `client-id-source: socket-addr`, add allow rules for them first. \
    With `x-forwarded-for`, allow rules cannot admit them: their requests carry no \
    x-forwarded-for header, so the node resolves no client to match.\n";

/// A JSON response with `result`, or the error.
fn json_response<T: Serialize>(result: IotaResult<T>) -> Response {
    match result {
        Ok(value) => axum::Json(value).into_response(),
        Err(err) => error_response(err).into_response(),
    }
}

/// A text response to a change, or the error.
fn change_response(result: IotaResult<String>) -> (StatusCode, String) {
    match result {
        Ok(message) => (StatusCode::OK, format!("{message}\n{UNTIL_RESTART}")),
        Err(err) => error_response(err),
    }
}

/// `400 Bad Request` for a request the node rejects, otherwise `500`.
fn error_response(err: IotaError) -> (StatusCode, String) {
    let status = if matches!(err, IotaError::InvalidAdminRequest(_)) {
        StatusCode::BAD_REQUEST
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    };
    (status, format!("{err}\n"))
}

/// The traffic control settings in force, as JSON. Refuses a query, since a
/// `GET` that carries the settings of a `POST` would change nothing.
async fn traffic_control_status(
    State(state): State<Arc<AppState>>,
    RawQuery(query): RawQuery,
) -> Response {
    traffic_control_status_response(&state.node.state(), query)
}

fn traffic_control_status_response(state: &AuthorityState, query: Option<String>) -> Response {
    if query.is_some_and(|query| !query.is_empty()) {
        return (
            StatusCode::BAD_REQUEST,
            "GET /traffic-control takes no query; use POST to change the settings\n".to_string(),
        )
            .into_response();
    }
    json_response(state.traffic_control_status())
}

async fn traffic_control_config(State(state): State<Arc<AppState>>) -> Response {
    traffic_control_config_response(&state.node.state())
}

fn traffic_control_config_response(state: &AuthorityState) -> Response {
    json_response(state.traffic_control_policy_config())
}

async fn traffic_control_rules(State(state): State<Arc<AppState>>) -> Response {
    traffic_control_rules_response(&state.node.state())
}

fn traffic_control_rules_response(state: &AuthorityState) -> Response {
    json_response(state.traffic_control_access_rules())
}

#[derive(Debug, Deserialize)]
struct AccessRuleChange {
    list: AccessRuleList,
    client: String,
}

/// The name of `list` in the admin API.
fn list_name(list: AccessRuleList) -> &'static str {
    match list {
        AccessRuleList::Allow => "allow",
        AccessRuleList::Deny => "deny",
    }
}

async fn add_traffic_control_rule(
    State(state): State<Arc<AppState>>,
    Query(change): Query<AccessRuleChange>,
) -> (StatusCode, String) {
    add_traffic_control_rule_response(&state.node.state(), change)
}

fn add_traffic_control_rule_response(
    state: &AuthorityState,
    AccessRuleChange { list, client }: AccessRuleChange,
) -> (StatusCode, String) {
    let list_name = list_name(list);
    change_response(
        state
            .add_traffic_control_access_rule(list, &client)
            .map(|added| match added {
                true => format!("Added {client} to the {list_name} rules."),
                false => format!("The {list_name} rules already hold {client}."),
            }),
    )
}

async fn remove_traffic_control_rule(
    State(state): State<Arc<AppState>>,
    Query(change): Query<AccessRuleChange>,
) -> (StatusCode, String) {
    remove_traffic_control_rule_response(&state.node.state(), change)
}

fn remove_traffic_control_rule_response(
    state: &AuthorityState,
    AccessRuleChange { list, client }: AccessRuleChange,
) -> (StatusCode, String) {
    let list_name = list_name(list);
    change_response(
        state
            .remove_traffic_control_access_rule(list, &client)
            .map(|removed| match removed {
                true => format!("Removed {client} from the {list_name} rules."),
                false => format!("The {list_name} rules do not hold {client}."),
            }),
    )
}

#[derive(Debug, Deserialize)]
struct ListingLimit {
    limit: Option<usize>,
}

async fn traffic_control_blocklist(
    State(state): State<Arc<AppState>>,
    Query(limit): Query<ListingLimit>,
) -> Response {
    traffic_control_blocklist_response(&state.node.state(), limit)
}

fn traffic_control_blocklist_response(
    state: &AuthorityState,
    ListingLimit { limit }: ListingLimit,
) -> Response {
    json_response(state.traffic_control_blocklist(limit.unwrap_or(DEFAULT_BLOCKLIST_LIMIT)))
}

#[derive(Debug, Deserialize)]
struct ManualBlock {
    client: String,
    ttl_sec: u64,
}

async fn block_traffic_control_client(
    State(state): State<Arc<AppState>>,
    Query(block): Query<ManualBlock>,
) -> (StatusCode, String) {
    block_traffic_control_client_response(&state.node.state(), block)
}

fn block_traffic_control_client_response(
    state: &AuthorityState,
    ManualBlock { client, ttl_sec }: ManualBlock,
) -> (StatusCode, String) {
    change_response(
        state
            .block_traffic_control_client(&client, ttl_sec)
            .map(|block| {
                let served = if state.traffic_control_dry_run().unwrap_or(false) {
                    " Dry run is on, so the node still serves it."
                } else {
                    ""
                };
                if block.reason == BlockReason::Manual && block.seconds_left <= ttl_sec {
                    format!(
                        "Blocked {} for {} seconds.{served}",
                        block.client, block.seconds_left
                    )
                } else {
                    format!(
                        "{} keeps the longer block in force: {} seconds left, with the reason \
                         {}.{served}",
                        block.client, block.seconds_left, block.reason
                    )
                }
            }),
    )
}

#[derive(Debug, Deserialize)]
struct Client {
    client: String,
}

async fn unblock_traffic_control_client(
    State(state): State<Arc<AppState>>,
    Query(client): Query<Client>,
) -> (StatusCode, String) {
    unblock_traffic_control_client_response(&state.node.state(), client)
}

fn unblock_traffic_control_client_response(
    state: &AuthorityState,
    Client { client }: Client,
) -> (StatusCode, String) {
    change_response(state.unblock_traffic_control_client(&client).map(
        |unblocked| match unblocked {
            true => format!("Unblocked {client}."),
            false => format!(
                "{client} had no block in force; what the spam and error policies counted \
                     for it and its simulation gas balance were reset."
            ),
        },
    ))
}

#[derive(Debug, Deserialize)]
struct Top {
    top: Option<usize>,
}

async fn traffic_control_clients(
    State(state): State<Arc<AppState>>,
    Query(top): Query<Top>,
) -> Response {
    traffic_control_clients_response(&state.node.state(), top)
}

fn traffic_control_clients_response(state: &AuthorityState, Top { top }: Top) -> Response {
    json_response(state.traffic_control_client_usage(top.unwrap_or(DEFAULT_CLIENTS_TOP)))
}

async fn get_metrics_filter(State(state): State<Arc<AppState>>) -> (StatusCode, String) {
    let filter = state.node.registry_service().filter();
    (
        StatusCode::OK,
        format!(
            "metrics exposure filter:\n\
             current: {}\n\n\n\
             (startup: {})\n",
            filter.filter_string(),
            filter.startup_filter_string(),
        ),
    )
}

#[derive(Deserialize)]
struct MetricsFilterUpdate {
    filter: String,
}

async fn set_metrics_filter(
    State(state): State<Arc<AppState>>,
    Query(MetricsFilterUpdate { filter }): Query<MetricsFilterUpdate>,
) -> (StatusCode, String) {
    let new_filter = filter.trim();
    match state.node.registry_service().set_runtime_filter(new_filter) {
        Ok(()) => {
            info!(filter =% new_filter, "Metrics filter updated");
            (
                StatusCode::OK,
                format!("metrics filter set to {new_filter:?}\n"),
            )
        }
        Err(err) => (StatusCode::BAD_REQUEST, format!("{err}\n")),
    }
}

async fn reset_metrics_filter(State(state): State<Arc<AppState>>) -> (StatusCode, String) {
    state.node.registry_service().reset_runtime_filter();
    info!("Metrics filter reset to startup configuration");
    (
        StatusCode::OK,
        "metrics filter reset to startup configuration\n".into(),
    )
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::{body::to_bytes, extract::Query, http::Uri};
    use iota_core::authority::test_authority_builder::TestAuthorityBuilder;
    use iota_types::traffic_control::{
        AccessRules, BlockedClients, ClientUsage, PolicyConfig, TrafficControlStatus,
    };
    use serde::de::DeserializeOwned;

    use super::*;

    fn query<T: DeserializeOwned>(uri: &str) -> T {
        Query::<T>::try_from_uri(&uri.parse::<Uri>().unwrap())
            .unwrap()
            .0
    }

    async fn body(response: Response) -> (StatusCode, String) {
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        (status, String::from_utf8(body.to_vec()).unwrap())
    }

    async fn json<T: DeserializeOwned>(response: Response) -> T {
        let (status, body) = body(response).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        serde_json::from_str(&body).unwrap()
    }

    async fn authority(policy_config: Option<PolicyConfig>) -> Arc<AuthorityState> {
        let builder = TestAuthorityBuilder::new();
        match policy_config {
            Some(policy_config) => builder.with_policy_config(policy_config),
            None => builder,
        }
        .build()
        .await
    }

    #[test]
    fn the_traffic_control_routes_read_their_queries() {
        let change: AccessRuleChange =
            query("/traffic-control/rules/add?list=deny&client=192.0.2.0/24");
        assert_eq!(change.list, AccessRuleList::Deny);
        assert_eq!(change.client, "192.0.2.0/24");
        let block: ManualBlock = query("/traffic-control/block?client=203.0.113.9&ttl_sec=3600");
        assert_eq!(
            (block.client.as_str(), block.ttl_sec),
            ("203.0.113.9", 3600)
        );
        let client: Client = query("/traffic-control/unblock?client=203.0.113.9");
        assert_eq!(client.client, "203.0.113.9");
        assert_eq!(
            query::<ListingLimit>("/traffic-control/blocklist").limit,
            None
        );
        assert_eq!(query::<Top>("/traffic-control/clients?top=5").top, Some(5));
        let params: TrafficControlReconfigParams =
            query("/traffic-control?default_action=deny&simulation_burst_gas=20");
        assert_eq!(params.default_action, Some(DefaultAction::Deny));
        assert_eq!(params.simulation_burst_gas, Some(20));
    }

    #[tokio::test]
    async fn every_route_rejects_a_request_while_traffic_control_is_off() {
        let state = authority(None).await;
        let mut responses = vec![
            reconfigure_traffic_control(&state, TrafficControlReconfigParams::default()).await,
            add_traffic_control_rule_response(&state, query("/?list=allow&client=192.0.2.1")),
            remove_traffic_control_rule_response(&state, query("/?list=allow&client=192.0.2.1")),
            block_traffic_control_client_response(&state, query("/?client=192.0.2.1&ttl_sec=9")),
            unblock_traffic_control_client_response(&state, query("/?client=192.0.2.1")),
        ];
        for response in [
            traffic_control_status_response(&state, None),
            traffic_control_config_response(&state),
            traffic_control_rules_response(&state),
            traffic_control_blocklist_response(&state, query("/")),
            traffic_control_clients_response(&state, query("/")),
        ] {
            responses.push(body(response).await);
        }
        for (status, body) in responses {
            assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
            assert!(body.contains("not configured"), "{body}");
        }
    }

    #[tokio::test]
    async fn the_routes_change_and_report_traffic_control() {
        let state = authority(Some(PolicyConfig {
            dry_run: false,
            ..Default::default()
        }))
        .await;

        let status: TrafficControlStatus =
            json(traffic_control_status_response(&state, None)).await;
        assert_eq!(status.default_action, DefaultAction::Limit);
        let refused = traffic_control_status_response(&state, Some("dry_run=true".to_string()));
        assert_eq!(refused.status(), StatusCode::BAD_REQUEST);
        let config: PolicyConfig = json(traffic_control_config_response(&state)).await;
        assert!(!config.dry_run);

        let (status, body) =
            reconfigure_traffic_control(&state, query("/?default_action=deny")).await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.contains(DEFAULT_DENY_WARNING), "{body}");
        for advice in [
            "With `client-id-source: socket-addr`, add allow rules for them first.",
            "With `x-forwarded-for`, allow rules cannot admit them",
        ] {
            assert!(body.contains(advice), "{body}");
        }
        assert!(body.ends_with(UNTIL_RESTART), "{body}");
        let status: TrafficControlStatus =
            json(traffic_control_status_response(&state, None)).await;
        assert_eq!(status.default_action, DefaultAction::Deny);
        let (_, body) = reconfigure_traffic_control(&state, query("/?default_action=limit")).await;
        assert!(!body.contains(DEFAULT_DENY_WARNING), "{body}");

        let (status, body) =
            add_traffic_control_rule_response(&state, query("/?list=allow&client=198.51.100.7"));
        assert_eq!(status, StatusCode::OK);
        assert!(
            body.starts_with("Added 198.51.100.7 to the allow rules."),
            "{body}"
        );
        assert!(body.ends_with(UNTIL_RESTART), "{body}");
        let rules: AccessRules = json(traffic_control_rules_response(&state)).await;
        assert_eq!(rules.allow, ["198.51.100.7/32"]);
        let (_, body) =
            remove_traffic_control_rule_response(&state, query("/?list=allow&client=198.51.100.7"));
        assert!(
            body.starts_with("Removed 198.51.100.7 from the allow rules."),
            "{body}"
        );

        let (status, body) = block_traffic_control_client_response(
            &state,
            query("/?client=203.0.113.9&ttl_sec=3600"),
        );
        assert_eq!(status, StatusCode::OK);
        assert!(body.starts_with("Blocked 203.0.113.9 for "), "{body}");
        assert!(!body.contains("Dry run is on"), "{body}");
        let (_, body) =
            block_traffic_control_client_response(&state, query("/?client=203.0.113.9&ttl_sec=60"));
        assert!(
            body.starts_with("203.0.113.9 keeps the longer block in force: "),
            "{body}"
        );
        let blocklist: BlockedClients =
            json(traffic_control_blocklist_response(&state, query("/"))).await;
        assert_eq!(blocklist.clients.len(), 1);
        assert_eq!(blocklist.clients[0].reason, BlockReason::Manual);
        let usage: ClientUsage = json(traffic_control_clients_response(&state, query("/"))).await;
        assert_eq!(usage.blocked, blocklist);

        let (status, body) =
            unblock_traffic_control_client_response(&state, query("/?client=203.0.113.9"));
        assert_eq!(status, StatusCode::OK);
        assert!(body.starts_with("Unblocked 203.0.113.9."), "{body}");
        let blocklist: BlockedClients = json(traffic_control_blocklist_response(
            &state,
            query("/?limit=10"),
        ))
        .await;
        assert_eq!(blocklist, BlockedClients::default());
    }

    #[tokio::test]
    async fn the_traffic_control_routes_change_what_the_node_serves() {
        let state = authority(Some(PolicyConfig {
            dry_run: false,
            ..Default::default()
        }))
        .await;
        let controller = state.traffic_controller.clone().unwrap();
        let client = "192.0.2.1".parse().unwrap();
        let served = || controller.check(&Some(client), &None).is_served();
        assert!(served());

        add_traffic_control_rule_response(&state, query("/?list=deny&client=192.0.2.0/24"));
        assert!(!served());
        remove_traffic_control_rule_response(&state, query("/?list=deny&client=192.0.2.0/24"));
        assert!(served());

        block_traffic_control_client_response(&state, query("/?client=192.0.2.1&ttl_sec=60"));
        assert!(!served());
        // An allow rule wins over the block.
        add_traffic_control_rule_response(&state, query("/?list=allow&client=192.0.2.1"));
        assert!(served());
        remove_traffic_control_rule_response(&state, query("/?list=allow&client=192.0.2.1"));
        assert!(!served());
        unblock_traffic_control_client_response(&state, query("/?client=192.0.2.1"));
        assert!(served());

        reconfigure_traffic_control(&state, query("/?default_action=deny")).await;
        assert!(!served());
        reconfigure_traffic_control(&state, query("/?default_action=limit")).await;
        assert!(served());
    }

    #[tokio::test]
    async fn a_manual_block_under_dry_run_says_the_node_still_serves_the_client() {
        let state = authority(Some(PolicyConfig {
            dry_run: true,
            ..Default::default()
        }))
        .await;
        let (status, body) =
            block_traffic_control_client_response(&state, query("/?client=203.0.113.9&ttl_sec=60"));
        assert_eq!(status, StatusCode::OK);
        assert!(
            body.contains("Dry run is on, so the node still serves it."),
            "{body}"
        );
    }

    #[tokio::test]
    async fn a_rejected_change_answers_bad_request() {
        let state = authority(Some(PolicyConfig::default())).await;
        let (status, body) =
            block_traffic_control_client_response(&state, query("/?client=203.0.113.9&ttl_sec=0"));
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        let (status, _) = error_response(IotaError::Unknown("failed".to_string()));
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    }
}
