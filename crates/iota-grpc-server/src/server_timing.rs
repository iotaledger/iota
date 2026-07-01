// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Tower [`Layer`] that runs each request inside a server-timing context, so
//! handlers can record timings via [`iota_metrics::add_server_timing`]. The
//! collected timings are returned in the `Server-Timing` response header.

use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use iota_metrics::{finish_and_set_server_timing_header, with_new_server_timing};
use tower::{Layer, Service};

/// Tower [`Layer`] that creates a server-timing context per request.
#[derive(Clone)]
pub(crate) struct ServerTimingLayer;

impl<S> Layer<S> for ServerTimingLayer {
    type Service = ServerTimingService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        ServerTimingService { inner }
    }
}

/// Tower [`Service`] wrapper that runs the inner service inside a
/// server-timing context.
#[derive(Clone)]
pub(crate) struct ServerTimingService<S> {
    inner: S,
}

impl<S, ReqBody, ResBody> Service<http::Request<ReqBody>> for ServerTimingService<S>
where
    S: Service<http::Request<ReqBody>, Response = http::Response<ResBody>> + Clone + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Send + 'static,
    ReqBody: Send + 'static,
    ResBody: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<S::Response, S::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: http::Request<ReqBody>) -> Self::Future {
        // Tower contract: `self.inner` is the ready one (poll_ready set it up),
        // so call it and leave the clone for the next request.
        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);

        Box::pin(with_new_server_timing(async move {
            let mut response = inner.call(req).await?;
            finish_and_set_server_timing_header(response.headers_mut());
            Ok(response)
        }))
    }
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;

    use http::{Request, Response};
    use iota_metrics::{add_server_timing, get_server_timing, server_timing_header_key};
    use tower::{ServiceExt, service_fn};

    use super::*;

    #[tokio::test]
    async fn creates_timing_scope_and_sets_header() {
        let service = ServerTimingLayer.layer(service_fn(|_req: Request<()>| async {
            assert!(
                get_server_timing().is_some(),
                "handler must run inside a server-timing context"
            );
            add_server_timing("handler");
            Ok::<_, Infallible>(Response::new(()))
        }));

        let response = service.oneshot(Request::new(())).await.unwrap();

        let header = response
            .headers()
            .get(server_timing_header_key())
            .expect("server-timing header must be set")
            .to_str()
            .unwrap();
        assert!(header.contains("handler;dur="), "header: {header}");
        assert!(header.contains("finish_request;dur="), "header: {header}");
    }
}
