// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Tower [`Layer`] that wires the shared [`TrafficController`] into the
//! gRPC server.
//!
//! The layer extracts the client IP and tallies each request against the
//! traffic controller, deriving the error weight from the response's gRPC
//! status code. A batch API's items are invisible at this level: their results
//! ride in the response body — embedded in a unary response or produced lazily
//! by a stream — while the layer only inspects the response status, so a batch
//! would count as a single request regardless of how many items it carries.
//! Handlers report each item through the [`TallyHandle`] request extension: an
//! item counts as one request for the spam policy, and a per-item client error
//! additionally feeds the error policy. Handlers that account eagerly from the
//! request's item count (the streaming reads, whose items are produced lazily)
//! report each item as `Code::Ok`, so they contribute to the spam policy only.
//!
//! [check]: iota_core::traffic_controller::TrafficController::check
//! [tally]: iota_core::traffic_controller::TrafficController::tally

use std::{
    future::Future,
    net::IpAddr,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
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

        // A batch handler sets this through its `TallyHandle` once it has
        // accounted for each item, so the layer skips its default
        // one-tally-per-request accounting below.
        let per_item_accounted = Arc::new(AtomicBool::new(false));
        let mut req = req;
        req.extensions_mut().insert(TallyHandle {
            traffic_controller: traffic_controller.clone(),
            client,
            per_item_accounted: per_item_accounted.clone(),
        });

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
                // Batch handlers account for each embedded item themselves; only
                // tally the request here when a handler didn't.
                if !per_item_accounted.load(Ordering::Relaxed) {
                    let code =
                        Status::from_header_map(response.headers()).map_or(Code::Ok, |s| s.code());
                    tally(&traffic_controller, client, code);
                }
            }
            result
        })
    }
}

/// Request extension through which a batch handler reports each per-item result
/// to the traffic controller.
///
/// A batch API's items are invisible to the transport-level layer: their
/// per-item errors are embedded in an otherwise successful response, and a
/// batch would otherwise count as a single request no matter how many items it
/// carries. Reporting each item makes it count as one request for the spam
/// policy and, on a client error, feeds the error policy — so batching cannot
/// dilute a client's request rate or hide its errors.
#[derive(Clone)]
pub struct TallyHandle {
    traffic_controller: Arc<TrafficController>,
    client: Option<IpAddr>,
    per_item_accounted: Arc<AtomicBool>,
}

impl TallyHandle {
    /// Account for one item of a batch response: count it as one request for
    /// the spam policy and, if `code` is a client error, feed the error policy.
    /// Passing `Code::Ok` (e.g. when accounting eagerly from a request's item
    /// count) contributes to the spam policy only.
    ///
    /// Signals the layer to skip its default per-request tally, so a batch is
    /// charged per item rather than once.
    pub fn tally_item(&self, code: Code) {
        self.per_item_accounted.store(true, Ordering::Relaxed);
        tally(&self.traffic_controller, self.client, code);
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
