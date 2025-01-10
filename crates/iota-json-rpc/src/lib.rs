// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{env, net::SocketAddr, str::FromStr};

use axum::{
    body::Body,
    routing::{get, post},
};
pub use balance_changes::*;
use hyper::{
    Method, Request,
    header::{HeaderName, HeaderValue},
};
pub use iota_config::node::ServerType;
use iota_core::traffic_controller::metrics::TrafficControllerMetrics;
use iota_json_rpc_api::{
    CLIENT_SDK_TYPE_HEADER, CLIENT_SDK_VERSION_HEADER, CLIENT_TARGET_API_VERSION_HEADER,
};
use iota_open_rpc::{Module, Project};
use iota_types::traffic_control::{PolicyConfig, RemoteFirewallConfig};
use jsonrpsee::{Extensions, RpcModule, types::ErrorObjectOwned};
pub use object_changes::*;
use prometheus::Registry;
use tokio::runtime::Handle;
use tokio_util::sync::CancellationToken;
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    trace::TraceLayer,
};
use tracing::{debug, info};

use crate::{
    axum_router::{json_rpc_handler, ws::ws_json_rpc_upgrade},
    error::Error,
    metrics::MetricsLogger,
    routing_layer::RpcRouter,
};

pub mod authority_state;
pub mod axum_router;
mod balance_changes;
pub mod bridge_api;
pub mod coin_api;
pub mod error;
pub mod governance_api;
pub mod indexer_api;
pub mod logger;
mod metrics;
pub mod move_utils;
mod object_changes;
pub mod read_api;
mod routing_layer;
pub mod transaction_builder_api;
pub mod transaction_execution_api;

pub const APP_NAME_HEADER: &str = "app-name";

pub const MAX_REQUEST_SIZE: u32 = 2 << 30;

pub struct JsonRpcServerBuilder {
    module: RpcModule<()>,
    rpc_doc: Project,
    registry: Registry,
    policy_config: Option<PolicyConfig>,
    firewall_config: Option<RemoteFirewallConfig>,
}

pub fn iota_rpc_doc(version: &str) -> Project {
    Project::new(
        version,
        "IOTA JSON-RPC",
        "IOTA JSON-RPC API for interaction with Iota Full node. Make RPC calls using https://api.NETWORK.iota.cafe:443, where NETWORK is the network you want to use (testnet, devnet, mainnet). By default, local networks use port 9000.",
        "IOTA Foundation",
        "https://iota.org",
        "info@iota.org",
        "Apache-2.0",
        "https://raw.githubusercontent.com/iotaledger/iota/main/LICENSE",
    )
}

impl JsonRpcServerBuilder {
    pub fn new(
        version: &str,
        prometheus_registry: &Registry,
        policy_config: Option<PolicyConfig>,
        firewall_config: Option<RemoteFirewallConfig>,
    ) -> Self {
        Self {
            module: RpcModule::new(()),
            rpc_doc: iota_rpc_doc(version),
            registry: prometheus_registry.clone(),
            policy_config,
            firewall_config,
        }
    }

    pub fn register_module<T: IotaRpcModule>(&mut self, module: T) -> Result<(), Error> {
        self.rpc_doc.add_module(T::rpc_doc_module());
        Ok(self.module.merge(module.rpc())?)
    }

    fn cors() -> Result<CorsLayer, Error> {
        let acl = match env::var("ACCESS_CONTROL_ALLOW_ORIGIN") {
            Ok(value) => {
                let allow_hosts = value
                    .split(',')
                    .map(HeaderValue::from_str)
                    .collect::<Result<Vec<_>, _>>()?;
                AllowOrigin::list(allow_hosts)
            }
            _ => AllowOrigin::any(),
        };
        info!(?acl);

        let cors = CorsLayer::new()
            // Allow `POST` when accessing the resource
            .allow_methods([Method::POST])
            // Allow requests from any origin
            .allow_origin(acl)
            .allow_headers([
                hyper::header::CONTENT_TYPE,
                HeaderName::from_static(CLIENT_SDK_TYPE_HEADER),
                HeaderName::from_static(CLIENT_SDK_VERSION_HEADER),
                HeaderName::from_static(CLIENT_TARGET_API_VERSION_HEADER),
                HeaderName::from_static(APP_NAME_HEADER),
            ]);
        Ok(cors)
    }

    fn trace_layer() -> TraceLayer<
        tower_http::classify::SharedClassifier<tower_http::classify::ServerErrorsAsFailures>,
        impl tower_http::trace::MakeSpan<Body> + Clone,
    > {
        TraceLayer::new_for_http().make_span_with(|request: &Request<Body>| {
            let request_id = request
                .headers()
                .get("x-req-id")
                .and_then(|v| v.to_str().ok())
                .map(tracing::field::display);

            tracing::info_span!("json-rpc-request", "x-req-id" = request_id)
        })
    }

    pub async fn to_router(&self, server_type: ServerType) -> Result<axum::Router, Error> {
        let routing = self.rpc_doc.method_routing.clone();

        let disable_routing = env::var("DISABLE_BACKWARD_COMPATIBILITY")
            .ok()
            .and_then(|v| bool::from_str(&v).ok())
            .unwrap_or_default();
        info!(
            "Compatibility method routing {}.",
            if disable_routing {
                "disabled"
            } else {
                "enabled"
            }
        );
        let rpc_router = RpcRouter::new(routing, disable_routing);

        let rpc_docs = self.rpc_doc.clone();
        let mut module = self.module.clone();
        module.register_method("rpc.discover", move |_, _, _| {
            Result::<_, ErrorObjectOwned>::Ok(rpc_docs.clone())
        })?;
        let methods_names = module.method_names().collect::<Vec<_>>();

        let metrics_logger = MetricsLogger::new(&self.registry, &methods_names);
        let traffic_controller_metrics = TrafficControllerMetrics::new(&self.registry);

        let middleware = tower::ServiceBuilder::new()
            .layer(Self::trace_layer())
            .layer(Self::cors()?);

        let service = crate::axum_router::JsonRpcService::new(
            module.into(),
            rpc_router,
            metrics_logger,
            self.firewall_config.clone(),
            self.policy_config.clone(),
            traffic_controller_metrics,
            Extensions::new(),
        );

        let mut router = axum::Router::new();

        match server_type {
            ServerType::WebSocket => {
                router = router
                    .route("/", get(ws_json_rpc_upgrade))
                    .route("/subscribe", get(ws_json_rpc_upgrade));
            }
            ServerType::Http => {
                router = router
                    .route("/", post(json_rpc_handler))
                    .route("/json-rpc", post(json_rpc_handler))
                    .route("/public", post(json_rpc_handler));
            }
            ServerType::Both => {
                router = router
                    .route("/", post(json_rpc_handler))
                    .route("/", get(ws_json_rpc_upgrade))
                    .route("/subscribe", get(ws_json_rpc_upgrade))
                    .route("/json-rpc", post(json_rpc_handler))
                    .route("/public", post(json_rpc_handler));
            }
        }

        let app = router.with_state(service).layer(middleware);

        info!("Available JSON-RPC methods : {methods_names:?}");

        Ok(app)
    }

    pub async fn start(
        self,
        listen_address: SocketAddr,
        custom_runtime: Option<Handle>,
        server_type: ServerType,
        cancel: Option<CancellationToken>,
    ) -> Result<ServerHandle, Error> {
        let app = self.to_router(server_type).await?;

        let listener = tokio::net::TcpListener::bind(listen_address)
            .await
            .map_err(|e| {
                Error::Unexpected(format!("invalid listen address {listen_address}: {e}"))
            })?;

        let addr = listener.local_addr().map_err(|e| {
            Error::Unexpected(format!("invalid listen address {listen_address}: {e}"))
        })?;

        let fut = async move {
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await
            .unwrap();
            if let Some(cancel) = cancel {
                // Signal that the server is shutting down, so other tasks can clean-up.
                cancel.cancel();
            }
        };
        let handle = if let Some(custom_runtime) = custom_runtime {
            debug!("Spawning server with custom runtime");
            custom_runtime.spawn(fut)
        } else {
            tokio::spawn(fut)
        };

        let handle = ServerHandle {
            handle: ServerHandleInner::Axum(handle),
        };
        info!(local_addr =? addr, "Iota JSON-RPC server listening on {addr}");
        Ok(handle)
    }
}

pub struct ServerHandle {
    handle: ServerHandleInner,
}

impl ServerHandle {
    pub async fn stopped(self) {
        match self.handle {
            ServerHandleInner::Axum(handle) => handle.await.unwrap(),
        }
    }
}

enum ServerHandleInner {
    Axum(tokio::task::JoinHandle<()>),
}

pub trait IotaRpcModule
where
    Self: Sized,
{
    fn rpc(self) -> RpcModule<Self>;
    fn rpc_doc_module() -> Module;
}
