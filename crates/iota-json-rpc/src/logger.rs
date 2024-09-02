// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{net::SocketAddr, time::Duration};

use anyhow::anyhow;
use futures::Future;
use iota_json_rpc_api::error_object_from_rpc;
use jsonrpsee::{
    core::{ClientError as RpcError, RpcResult},
    server::HttpRequest,
    types::Params,
    MethodKind,
};
use tracing::{error, info, Instrument, Span};

use crate::error::RpcInterimResult;

/// The transport protocol used to send or receive a call or request.
#[derive(Debug, Copy, Clone)]
pub enum TransportProtocol {
    /// HTTP transport.
    Http,
    /// WebSocket transport.
    WebSocket,
}

impl std::fmt::Display for TransportProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Http => "http",
            Self::WebSocket => "websocket",
        };

        write!(f, "{s}")
    }
}

/// Defines a logger specifically for WebSocket connections with callbacks
/// during the RPC request life-cycle. The primary use case for this is to
/// collect timings for a larger metrics collection solution.
pub trait Logger: Send + Sync + Clone + 'static {
    /// Intended to carry timestamp of a request, for example
    /// `std::time::Instant`. How the trait measures time, if at all, is
    /// entirely up to the implementation.
    type Instant: std::fmt::Debug + Send + Sync + Copy;

    /// Called when a new client connects
    fn on_connect(&self, _remote_addr: SocketAddr, _request: &HttpRequest, _t: TransportProtocol);

    /// Called when a new JSON-RPC request comes to the server.
    fn on_request(&self, transport: TransportProtocol) -> Self::Instant;

    /// Called on each JSON-RPC method call, batch requests will trigger
    /// `on_call` multiple times.
    fn on_call(
        &self,
        method_name: &str,
        params: Params,
        kind: MethodKind,
        transport: TransportProtocol,
    );

    /// Called on each JSON-RPC method completion, batch requests will trigger
    /// `on_result` multiple times.
    fn on_result(
        &self,
        method_name: &str,
        success: bool,
        error_code: Option<i32>,
        started_at: Self::Instant,
        transport: TransportProtocol,
    );

    /// Called once the JSON-RPC request is finished and response is sent to the
    /// output buffer.
    fn on_response(&self, result: &str, started_at: Self::Instant, transport: TransportProtocol);

    /// Called when a client disconnects
    fn on_disconnect(&self, _remote_addr: SocketAddr, transport: TransportProtocol);
}

impl Logger for () {
    type Instant = ();

    fn on_connect(&self, _: SocketAddr, _: &HttpRequest, _p: TransportProtocol) -> Self::Instant {}

    fn on_request(&self, _p: TransportProtocol) -> Self::Instant {}

    fn on_call(&self, _: &str, _: Params, _: MethodKind, _p: TransportProtocol) {}

    fn on_result(&self, _: &str, _: bool, _: Option<i32>, _: Self::Instant, _p: TransportProtocol) {
    }

    fn on_response(&self, _: &str, _: Self::Instant, _p: TransportProtocol) {}

    fn on_disconnect(&self, _: SocketAddr, _p: TransportProtocol) {}
}

impl<A, B> Logger for (A, B)
where
    A: Logger,
    B: Logger,
{
    type Instant = (A::Instant, B::Instant);

    fn on_connect(
        &self,
        remote_addr: std::net::SocketAddr,
        request: &HttpRequest,
        transport: TransportProtocol,
    ) {
        self.0.on_connect(remote_addr, request, transport);
        self.1.on_connect(remote_addr, request, transport);
    }

    fn on_request(&self, transport: TransportProtocol) -> Self::Instant {
        (self.0.on_request(transport), self.1.on_request(transport))
    }

    fn on_call(
        &self,
        method_name: &str,
        params: Params,
        kind: MethodKind,
        transport: TransportProtocol,
    ) {
        self.0.on_call(method_name, params.clone(), kind, transport);
        self.1.on_call(method_name, params, kind, transport);
    }

    fn on_result(
        &self,
        method_name: &str,
        success: bool,
        error_code: Option<i32>,
        started_at: Self::Instant,
        transport: TransportProtocol,
    ) {
        self.0
            .on_result(method_name, success, error_code, started_at.0, transport);
        self.1
            .on_result(method_name, success, error_code, started_at.1, transport);
    }

    fn on_response(&self, result: &str, started_at: Self::Instant, transport: TransportProtocol) {
        self.0.on_response(result, started_at.0, transport);
        self.1.on_response(result, started_at.1, transport);
    }

    fn on_disconnect(&self, remote_addr: SocketAddr, transport: TransportProtocol) {
        self.0.on_disconnect(remote_addr, transport);
        self.1.on_disconnect(remote_addr, transport);
    }
}

pub(crate) trait FutureWithTracing<O>: Future<Output = RpcInterimResult<O>> {
    fn trace(self) -> impl Future<Output = RpcResult<O>>
    where
        Self: Sized,
    {
        self.trace_timeout(Duration::from_secs(1))
    }

    fn trace_timeout(self, timeout: Duration) -> impl Future<Output = RpcResult<O>>
    where
        Self: Sized,
    {
        async move {
            let start = std::time::Instant::now();
            let interim_result: RpcInterimResult<_> = self.await;
            let elapsed = start.elapsed();
            let result = interim_result.map_err(|e| {
                let anyhow_error = anyhow!("{:?}", e);

                let rpc_error: RpcError = e.into();
                if !matches!(rpc_error, RpcError::Call(_)) {
                    error!(error=?anyhow_error);
                }
                error_object_from_rpc(rpc_error)
            });

            if elapsed > timeout {
                info!(?elapsed, "RPC took longer than threshold to complete.");
            }
            result
        }
        .instrument(Span::current())
    }
}
impl<F: Future<Output = RpcInterimResult<O>>, O> FutureWithTracing<O> for F {}
