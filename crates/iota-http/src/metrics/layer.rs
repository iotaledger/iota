// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use bytes::Bytes;
use http::{Method, Request, Response, StatusCode};
use http_body::{Body, Frame, SizeHint};
use pin_project_lite::pin_project;
use tower::{Layer, Service};

use crate::metrics::{ConnectionHandle, connection::RequestGuard};

/// Tower layer that counts each request on the [`ConnectionHandle`] found in
/// the request extensions, and wraps the response body. A request without a
/// [`ConnectionHandle`] is not counted, and the frames of its response pass
/// on unchanged.
///
/// A request ends when the response future and the response body are done and
/// the transport has written the last chunk of the response to the socket or
/// copied it into its buffer. A response that does not fit in the HTTP/2
/// window of the client stays in flight until the client reads it.
#[derive(Clone)]
pub(crate) struct TrackRequestsLayer;

impl TrackRequestsLayer {
    pub(crate) fn new() -> Self {
        Self
    }
}

impl<S> Layer<S> for TrackRequestsLayer {
    type Service = TrackRequests<S>;

    fn layer(&self, inner: S) -> Self::Service {
        TrackRequests { inner }
    }
}

/// The service made by [`TrackRequestsLayer`].
#[derive(Clone)]
pub(crate) struct TrackRequests<S> {
    inner: S,
}

impl<S, ReqBody, ResBody> Service<Request<ReqBody>> for TrackRequests<S>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>>,
    ResBody: Body<Data = Bytes>,
{
    type Response = Response<TrackedResponseBody<ResBody>>;
    type Error = S::Error;
    type Future = TrackedFuture<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: Request<ReqBody>) -> Self::Future {
        // Only an HTTP/1 connection becomes a tunnel: in HTTP/2 a CONNECT is one
        // stream, and the connection goes on with ordinary requests.
        let is_http1_connect =
            request.method() == Method::CONNECT && request.version() <= http::Version::HTTP_11;
        let guard = request
            .extensions()
            .get::<ConnectionHandle>()
            .map(|connection| connection.start_request(request.version()));
        TrackedFuture {
            inner: self.inner.call(request),
            guard,
            is_http1_connect,
        }
    }
}

pin_project! {
    /// The response future of [`TrackRequests`].
    pub(crate) struct TrackedFuture<F> {
        #[pin]
        inner: F,
        guard: Option<RequestGuard>,
        is_http1_connect: bool,
    }
}

impl<F, B, E> Future for TrackedFuture<F>
where
    F: Future<Output = Result<Response<B>, E>>,
    B: Body,
{
    type Output = Result<Response<TrackedResponseBody<B>>, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        match this.inner.poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Ok(response)) => {
                let upgrade = response.status() == StatusCode::SWITCHING_PROTOCOLS
                    || (*this.is_http1_connect && response.status().is_success());
                let guard = this.guard.take().map(|mut guard| {
                    guard.mark_response_ready();
                    if upgrade {
                        guard.mark_upgraded();
                    }
                    Arc::new(guard)
                });
                Poll::Ready(Ok(
                    response.map(|inner| TrackedResponseBody { inner, guard })
                ))
            }
            Poll::Ready(Err(error)) => Poll::Ready(Err(error)),
        }
    }
}

pin_project! {
    /// The response body of [`TrackRequests`]. It gives each data chunk to the
    /// transport wrapped, so the count sees when the transport drops it.
    pub(crate) struct TrackedResponseBody<B: Body> {
        #[pin]
        inner: B,
        guard: Option<Arc<RequestGuard>>,
    }
}

impl<B> Body for TrackedResponseBody<B>
where
    B: Body<Data = Bytes>,
{
    type Data = Bytes;
    type Error = B::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, Self::Error>>> {
        let mut this = self.project();
        let frame = match this.inner.as_mut().poll_frame(cx) {
            Poll::Ready(Some(Ok(frame))) => frame,
            Poll::Ready(None) => {
                this.guard.take();
                return Poll::Ready(None);
            }
            other => return other,
        };
        let frame = match this.guard.as_ref() {
            Some(guard) => frame.map_data(|bytes| guard.wrap_chunk(bytes)),
            None => frame,
        };
        if this.inner.is_end_stream() {
            this.guard.take();
        }
        Poll::Ready(Some(Ok(frame)))
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> SizeHint {
        self.inner.size_hint()
    }
}
