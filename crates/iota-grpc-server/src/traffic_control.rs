// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Tower [`Layer`] that wires the shared [`TrafficController`] into the
//! gRPC server.
//!
//! The layer extracts the client IP via and calls the traffic controller
//! with a weight derived from the response's gRPC status code.
//!
//! [check]: iota_core::traffic_controller::TrafficController::check
//! [tally]: iota_core::traffic_controller::TrafficController::tally

use std::{
    future::Future,
    net::IpAddr,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::SystemTime,
};

use iota_core::traffic_controller::{
    ClientIpStatus, TrafficController, get_client_ip, policies::TrafficTally,
};
use iota_types::traffic_control::{ClientIdSource, Weight};
use tonic::{Code, Status, transport::server::TcpConnectInfo};
use tower::{Layer, Service};
use tracing::warn;

/// Tower [`Layer`] that integrates the shared traffic controller.
#[derive(Clone)]
pub struct TrafficControlLayer {
    traffic_controller: Arc<TrafficController>,
    client_id_source: ClientIdSource,
}

impl TrafficControlLayer {
    pub fn new(
        traffic_controller: Arc<TrafficController>,
        client_id_source: ClientIdSource,
    ) -> Self {
        Self {
            traffic_controller,
            client_id_source,
        }
    }
}

impl<S> Layer<S> for TrafficControlLayer {
    type Service = TrafficControlService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        TrafficControlService {
            inner,
            traffic_controller: self.traffic_controller.clone(),
            client_id_source: self.client_id_source.clone(),
        }
    }
}

/// Tower [`Service`] wrapper that gates requests through the traffic
/// controller.
#[derive(Clone)]
pub struct TrafficControlService<S> {
    inner: S,
    traffic_controller: Arc<TrafficController>,
    client_id_source: ClientIdSource,
}

impl<S, ReqBody, ResBody> Service<http::Request<ReqBody>> for TrafficControlService<S>
where
    S: Service<http::Request<ReqBody>, Response = http::Response<ResBody>> + Clone + Send + 'static,
    S::Future: Send + 'static,
    ReqBody: Send + 'static,
    ResBody: Default + Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<S::Response, S::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: http::Request<ReqBody>) -> Self::Future {
        let traffic_controller = self.traffic_controller.clone();
        let remote_addr = req
            .extensions()
            .get::<TcpConnectInfo>()
            .and_then(|info| info.remote_addr());
        let client = match get_client_ip(req.headers(), remote_addr, &self.client_id_source) {
            ClientIpStatus::Ok(ip) => Some(ip),
            other => {
                warn!(
                    "Skipping traffic controller request handling for client {remote_addr:?}: {other:?}"
                );
                None
            }
        };

        // Tower contract: `self.inner` is the ready one (poll_ready set it up),
        // so call it and leave the clone for the next request.
        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);

        Box::pin(async move {
            if !traffic_controller.check(&client, &None).await {
                return Ok(Status::resource_exhausted("Too many requests").into_http());
            }

            let result = inner.call(req).await;
            if let Ok(response) = &result {
                let code =
                    Status::from_header_map(response.headers()).map_or(Code::Ok, |s| s.code());
                tally(&traffic_controller, client, code);
            }
            result
        })
    }
}

fn tally(traffic_controller: &TrafficController, client: Option<IpAddr>, code: Code) {
    let error_info = if matches!(code, Code::Ok) {
        None
    } else {
        Some((normalize(code), format!("{code:?}")))
    };
    traffic_controller.tally(TrafficTally {
        direct: client,
        through_fullnode: None,
        error_info,
        spam_weight: Weight::one(),
        timestamp: SystemTime::now(),
    });
}

/// Map a gRPC status code to a tally weight.
///
/// Mirrors the conservative shape of the JSON-RPC `normalize`: only obvious
/// client-side mistakes contribute to the error-policy budget. Server-side
/// failures (`Internal`, `Unavailable`, `DataLoss`) must not count, or a
/// flaky backend would auto-block legitimate clients.
fn normalize(code: Code) -> Weight {
    match code {
        Code::InvalidArgument
        | Code::FailedPrecondition
        | Code::OutOfRange
        | Code::Unauthenticated
        | Code::PermissionDenied => Weight::one(),
        _ => Weight::zero(),
    }
}
