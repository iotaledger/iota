// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use axum::{Extension, Router, routing::get};
use iota_metrics::{METRICS_ROUTE, RegistryService};
use prometheus::{IntGauge, Registry, register_int_gauge_with_registry};
use tokio_util::sync::CancellationToken;
use tracing::info;

pub(crate) struct IotaNamesMetrics {
    pub total_name_records: IntGauge,
}

impl IotaNamesMetrics {
    pub fn new(registry: &Registry) -> Self {
        Self {
            total_name_records: register_int_gauge_with_registry!(
                "total_name_records",
                "The total number of name records in the registry",
                registry,
            )
            .unwrap(),
        }
    }
}

pub(crate) struct PrometheusServer {
    registry_service: RegistryService,
}

impl PrometheusServer {
    pub(crate) fn new() -> Self {
        Self {
            registry_service: RegistryService::new(Registry::new()),
        }
    }

    pub(crate) async fn start(&self, token: CancellationToken) -> anyhow::Result<()> {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 9184);
        info!("Starting prometheus metrics at {addr}");

        let app = Router::new()
            .route(METRICS_ROUTE, get(iota_metrics::metrics))
            .layer(Extension(self.registry_service.clone()));

        let listener = tokio::net::TcpListener::bind(&addr).await?;
        async fn shutdown_signal(token: CancellationToken) {
            token.cancelled().await;
        }
        axum::serve(listener, app.into_make_service())
            .with_graceful_shutdown(shutdown_signal(token))
            .await?;

        Ok(())
    }

    pub(crate) fn registry(&self) -> Registry {
        self.registry_service.default_registry()
    }
}
