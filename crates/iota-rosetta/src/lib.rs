// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung 
// SPDX-License-Identifier: Apache-2.0

use std::{net::SocketAddr, sync::Arc};

use axum::{routing::post, Extension, Router};
use mysten_metrics::spawn_monitored_task;
use once_cell::sync::Lazy;
use iota_sdk::IotaClient;
use tokio::task::JoinHandle;
use tracing::info;

use crate::{
    errors::Error,
    state::{CheckpointBlockProvider, OnlineServerContext},
    types::{Currency, IotaEnv},
};

/// This lib implements the Rosetta online and offline server defined by the [Rosetta API Spec](https://www.rosetta-api.org/docs/Reference.html)
mod account;
mod block;
mod construction;
mod errors;
mod network;
pub mod operations;
mod state;
pub mod types;

pub static IOTA: Lazy<Currency> = Lazy::new(|| Currency {
    symbol: "IOTA".to_string(),
    decimals: 9,
});

pub struct RosettaOnlineServer {
    env: IotaEnv,
    context: OnlineServerContext,
}

impl RosettaOnlineServer {
    pub fn new(env: IotaEnv, client: IotaClient) -> Self {
        let blocks = Arc::new(CheckpointBlockProvider::new(client.clone()));
        Self {
            env,
            context: OnlineServerContext::new(client, blocks),
        }
    }

    pub fn serve(self, addr: SocketAddr) -> JoinHandle<hyper::Result<()>> {
        // Online endpoints
        let app = Router::new()
            .route("/account/balance", post(account::balance))
            .route("/account/coins", post(account::coins))
            .route("/block", post(block::block))
            .route("/block/transaction", post(block::transaction))
            .route("/construction/submit", post(construction::submit))
            .route("/construction/metadata", post(construction::metadata))
            .route("/network/status", post(network::status))
            .route("/network/list", post(network::list))
            .route("/network/options", post(network::options))
            .layer(Extension(self.env))
            .with_state(self.context);
        let server = axum::Server::bind(&addr).serve(app.into_make_service());
        info!(
            "Iota Rosetta online server listening on {}",
            server.local_addr()
        );
        spawn_monitored_task!(server)
    }
}

pub struct RosettaOfflineServer {
    env: IotaEnv,
}

impl RosettaOfflineServer {
    pub fn new(env: IotaEnv) -> Self {
        Self { env }
    }

    pub fn serve(self, addr: SocketAddr) -> JoinHandle<hyper::Result<()>> {
        // Online endpoints
        let app = Router::new()
            .route("/construction/derive", post(construction::derive))
            .route("/construction/payloads", post(construction::payloads))
            .route("/construction/combine", post(construction::combine))
            .route("/construction/preprocess", post(construction::preprocess))
            .route("/construction/hash", post(construction::hash))
            .route("/construction/parse", post(construction::parse))
            .route("/network/list", post(network::list))
            .route("/network/options", post(network::options))
            .layer(Extension(self.env));
        let server = axum::Server::bind(&addr).serve(app.into_make_service());
        info!(
            "Iota Rosetta offline server listening on {}",
            server.local_addr()
        );
        spawn_monitored_task!(server)
    }
}
