// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use axum::{
    extract::Path,
    response::IntoResponse,
    routing::{get, post},
    Extension, Json, Router,
};
use clap::Parser;
use http::{Method, StatusCode};
use std::{net::SocketAddr, sync::Arc};
use iota_cluster_test::{
    cluster::{Cluster, LocalNewCluster},
    config::{ClusterTestOpt, Env},
    faucet::{FaucetClient, FaucetClientFactory},
};
use iota_faucet::{
    BatchFaucetResponse, BatchStatusFaucetResponse, FaucetError, FaucetRequest, FaucetResponse,
    FixedAmountRequest,
};
use tower::ServiceBuilder;
use tower_http::cors::{Any, CorsLayer};
use uuid::Uuid;

/// Start a Iota validator and fullnode for easy testing.
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Config directory that will be used to store network config, node db, keystore
    /// iota genesis -f --with-faucet generates a genesis config that can be used to start this process.
    /// Example: iota-test-validator --config-dir ~/.iota/iota_config
    /// We can use any config dir that is generated by the iota genesis.
    #[clap(short, long)]
    config_dir: Option<std::path::PathBuf>,
    /// Port to start the Fullnode RPC server on
    #[clap(long, default_value = "9000")]
    fullnode_rpc_port: u16,

    /// Port to start the Iota faucet on
    #[clap(long, default_value = "9123")]
    faucet_port: u16,

    /// Host to start the GraphQl server on
    #[clap(long, default_value = "127.0.0.1")]
    graphql_host: String,

    /// Port to start the GraphQl server on
    /// Explicitly setting this enables the server
    #[clap(long)]
    graphql_port: Option<u16>,

    /// Port to start the Indexer RPC server on
    #[clap(long, default_value = "9124")]
    indexer_rpc_port: u16,

    /// Port for the Indexer Postgres DB
    /// 5432 is the default port for postgres on Mac
    #[clap(long, default_value = "5432")]
    pg_port: u16,

    /// Hostname for the Indexer Postgres DB
    #[clap(long, default_value = "localhost")]
    pg_host: String,

    /// DB name for the Indexer Postgres DB
    #[clap(long, default_value = "iota_indexer")]
    pg_db_name: String,

    /// DB username for the Indexer Postgres DB
    #[clap(long, default_value = "postgres")]
    pg_user: String,

    /// DB password for the Indexer Postgres DB
    #[clap(long, default_value = "postgrespw")]
    pg_password: String,

    /// The duration for epochs (defaults to one minute)
    #[clap(long, default_value = "60000")]
    epoch_duration_ms: u64,

    /// if we should run indexer
    #[clap(long)]
    pub with_indexer: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let (_guard, _filter_handle) = telemetry_subscribers::TelemetryConfig::new()
        .with_env()
        .init();

    let args = Args::parse();
    let Args {
        config_dir,
        fullnode_rpc_port,
        graphql_host,
        graphql_port,
        indexer_rpc_port,
        pg_port,
        pg_host,
        pg_db_name,
        pg_user,
        pg_password,
        epoch_duration_ms,
        faucet_port,
        with_indexer,
    } = args;

    // We don't pass epoch duration if we have a genesis config.
    let epoch_duration_ms = if config_dir.is_some() {
        None
    } else {
        Some(epoch_duration_ms)
    };

    if graphql_port.is_none() {
        println!("Graphql port not provided. Graphql service will not run.")
    }
    if !with_indexer {
        println!("`with_indexer` flag unset. Indexer service will not run.")
    }

    let cluster_config = ClusterTestOpt {
        env: Env::NewLocal,

        fullnode_address: Some(format!("0.0.0.0:{}", fullnode_rpc_port)),
        indexer_address: with_indexer.then_some(format!("0.0.0.0:{}", indexer_rpc_port)),
        pg_address: Some(format!(
            "postgres://{pg_user}:{pg_password}@{pg_host}:{pg_port}/{pg_db_name}"
        )),
        faucet_address: Some(format!("127.0.0.1:{}", faucet_port)),
        epoch_duration_ms,
        config_dir,
        graphql_address: graphql_port.map(|p| format!("{}:{}", graphql_host, p)),
    };

    println!("Starting Iota validator with config: {:#?}", cluster_config);
    let cluster = LocalNewCluster::start(&cluster_config).await?;

    println!("Fullnode RPC URL: {}", cluster.fullnode_url());

    if with_indexer {
        println!(
            "Indexer RPC URL: {}",
            cluster.indexer_url().clone().unwrap_or_default()
        );
    }

    start_faucet(&cluster, faucet_port).await?;

    Ok(())
}

struct AppState {
    faucet: Arc<dyn FaucetClient + Sync + Send>,
}

async fn start_faucet(cluster: &LocalNewCluster, port: u16) -> Result<()> {
    let faucet = FaucetClientFactory::new_from_cluster(cluster).await;

    let app_state = Arc::new(AppState { faucet });

    let cors = CorsLayer::new()
        .allow_methods(vec![Method::GET, Method::POST])
        .allow_headers(Any)
        .allow_origin(Any);

    let app = Router::new()
        .route("/", get(health))
        .route("/gas", post(faucet_request))
        .route("/v1/gas", post(faucet_batch_request))
        .route("/v1/status/:task_id", get(request_status))
        .layer(
            ServiceBuilder::new()
                .layer(cors)
                .layer(Extension(app_state))
                .into_inner(),
        );

    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    println!("Faucet URL: http://{}", addr);

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}

/// basic handler that responds with a static string
async fn health() -> &'static str {
    "OK"
}

async fn faucet_request(
    Extension(state): Extension<Arc<AppState>>,
    Json(payload): Json<FaucetRequest>,
) -> impl IntoResponse {
    let result = match payload {
        FaucetRequest::FixedAmountRequest(FixedAmountRequest { recipient }) => {
            state.faucet.request_iota_coins(recipient).await
        }
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(FaucetResponse::from(FaucetError::Internal(
                    "Input Error.".to_string(),
                ))),
            )
        }
    };

    if !result.transferred_gas_objects.is_empty() {
        (StatusCode::CREATED, Json(result))
    } else {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(result))
    }
}

async fn faucet_batch_request(
    Extension(state): Extension<Arc<AppState>>,
    Json(payload): Json<FaucetRequest>,
) -> impl IntoResponse {
    let result = match payload {
        FaucetRequest::FixedAmountRequest(FixedAmountRequest { recipient }) => {
            state.faucet.batch_request_iota_coins(recipient).await
        }
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(BatchFaucetResponse::from(FaucetError::Internal(
                    "Input Error.".to_string(),
                ))),
            )
        }
    };
    if result.task.is_some() {
        (StatusCode::CREATED, Json(result))
    } else {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(result))
    }
}

async fn request_status(
    Extension(state): Extension<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match Uuid::parse_str(&id) {
        Ok(task_id) => {
            let status = state.faucet.get_batch_send_status(task_id).await;
            (StatusCode::CREATED, Json(status))
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(BatchStatusFaucetResponse::from(FaucetError::Internal(
                e.to_string(),
            ))),
        ),
    }
}
