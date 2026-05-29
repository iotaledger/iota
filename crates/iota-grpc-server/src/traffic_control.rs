// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Tower [`Layer`] that wires the shared [`TrafficController`] into the
//! gRPC server.
//!
//! The fullnode-facing gRPC API exposes the same class of traffic that the
//! JSON-RPC API protects, including transaction execution endpoints. We share
//! one controller across both surfaces so the same blocklist and tally apply
//! to a given client IP regardless of which API it hits.
//!
//! The layer extracts the client IP from `TcpConnectInfo` (added to request
//! extensions by tonic per connection) or from the `x-forwarded-for` header,
//! depending on [`ClientIdSource`], calls
//! [`TrafficController::check`][check] before dispatching, then calls
//! [`tally`][tally] with a weight derived from the response's gRPC status
//! code.
//!
//! [check]: iota_core::traffic_controller::TrafficController::check
//! [tally]: iota_core::traffic_controller::TrafficController::tally

use std::{
    future::Future,
    net::{IpAddr, SocketAddr},
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::SystemTime,
};

use iota_core::traffic_controller::{TrafficController, parse_ip, policies::TrafficTally};
use iota_types::traffic_control::{ClientIdSource, Weight};
use tonic::{Code, Status, transport::server::TcpConnectInfo};
use tower::{Layer, Service};
use tracing::error;

/// Header name used for forwarded client identification.
const X_FORWARDED_FOR: &str = "x-forwarded-for";

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
        let client = extract_client_ip(&req, &self.client_id_source);

        // Tower contract: the cloned service is the one ready to call, so swap
        // the clone in and keep the previously-ready inner for this request.
        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);

        Box::pin(async move {
            if !traffic_controller.check(&client, &None).await {
                let response = Status::resource_exhausted("Too many requests").into_http();
                tally(&traffic_controller, client, Code::ResourceExhausted);
                return Ok(response);
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
        // Match the JSON-RPC layer: count every request equally as spam.
        // A future refinement could weight transaction-execution paths more
        // heavily than reads.
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

fn extract_client_ip<B>(req: &http::Request<B>, source: &ClientIdSource) -> Option<IpAddr> {
    match source {
        ClientIdSource::SocketAddr => req
            .extensions()
            .get::<TcpConnectInfo>()
            .and_then(|info| info.remote_addr())
            .map(|addr: SocketAddr| addr.ip()),
        ClientIdSource::XForwardedFor(num_hops) => {
            let header = req
                .headers()
                .get(X_FORWARDED_FOR)
                .or_else(|| req.headers().get("X-Forwarded-For"))?;
            let value = header
                .to_str()
                .map_err(|e| error!("Invalid UTF-8 in x-forwarded-for header: {e:?}"))
                .ok()?;
            let contents: Vec<&str> = value.split(',').map(str::trim).collect();
            if *num_hops == 0 {
                error!(
                    "x-forwarded-for: 0 hops specified. Contents: {contents:?}. Set a nonzero hop count or use socket-addr."
                );
                return None;
            }
            if contents.len() < *num_hops {
                error!(
                    "x-forwarded-for header value of {contents:?} contains {} values but {num_hops} hops were specified.",
                    contents.len()
                );
                return None;
            }
            contents
                .get(contents.len() - num_hops)
                .and_then(|ip| parse_ip(ip))
        }
    }
}
