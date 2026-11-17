// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::time::Duration;

use anyhow::anyhow;
use futures::Future;
use iota_json_rpc_api::error_object_from_rpc;
use jsonrpsee::{
    MethodKind,
    core::{ClientError as RpcError, RpcResult},
    types::Params,
};
use tracing::{Instrument, Span, error, info};

use crate::error::RpcInterimResult;

/// Defines a logger with callbacks during the RPC request life-cycle. The primary use case for this
/// is to collect timings for a larger metrics collection solution.
pub trait Logger: Send + Sync + Clone + 'static {
    /// Intended to carry timestamp of a request, for example
    /// `std::time::Instant`. How the trait measures time, if at all, is
    /// entirely up to the implementation.
    type Instant: std::fmt::Debug + Send + Sync + Copy;

    /// Called when a new JSON-RPC request comes to the server.
    fn on_request(&self) -> Self::Instant;

    /// Called on each JSON-RPC method call, batch requests will trigger
    /// `on_call` multiple times.
    fn on_call(&self, method_name: &str, params: Params, kind: MethodKind);

    /// Called on each JSON-RPC method completion, batch requests will trigger
    /// `on_result` multiple times.
    fn on_result(
        &self,
        method_name: &str,
        success: bool,
        error_code: Option<i32>,
        started_at: Self::Instant,
    );
}

impl Logger for () {
    type Instant = ();

    fn on_request(&self) -> Self::Instant {}

    fn on_call(&self, _: &str, _: Params, _: MethodKind) {}

    fn on_result(&self, _: &str, _: bool, _: Option<i32>, _: Self::Instant) {}
}

impl<A, B> Logger for (A, B)
where
    A: Logger,
    B: Logger,
{
    type Instant = (A::Instant, B::Instant);

    fn on_request(&self) -> Self::Instant {
        (self.0.on_request(), self.1.on_request())
    }

    fn on_call(&self, method_name: &str, params: Params, kind: MethodKind) {
        self.0.on_call(method_name, params.clone(), kind);
        self.1.on_call(method_name, params, kind);
    }

    fn on_result(
        &self,
        method_name: &str,
        success: bool,
        error_code: Option<i32>,
        started_at: Self::Instant,
    ) {
        self.0
            .on_result(method_name, success, error_code, started_at.0);
        self.1
            .on_result(method_name, success, error_code, started_at.1);
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
                let anyhow_error = anyhow!("{e:?}");

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
