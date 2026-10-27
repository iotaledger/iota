// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll, ready},
};

use http::{Request, Response};
use http_body::{Body, Frame, SizeHint};
use pin_project_lite::pin_project;
use tower::Service;

use crate::metrics::{ConnectionMetrics, connection::RequestGuard};

/// Tower service that counts the requests of one connection on its
/// [`ConnectionMetrics`]. Without them, it passes requests and responses on
/// unchanged.
///
/// A request ends when the response future is done and the transport has
/// taken the last frame of the response body, or has dropped the body. The
/// transport takes the next frame only when the data before it fits in the
/// window of the client (HTTP/2) or in the write buffer of the transport
/// (HTTP/1). So a response that does not fit stays in flight until the client
/// reads it, except for its last frame: a response of one data frame can end
/// before the client reads it.
#[derive(Clone)]
pub(crate) struct TrackRequests<S> {
    inner: S,
    connection: Option<ConnectionMetrics>,
}

impl<S> TrackRequests<S> {
    pub(crate) fn new(inner: S, connection: Option<ConnectionMetrics>) -> Self {
        Self { inner, connection }
    }
}

impl<S, ReqBody, ResBody> Service<Request<ReqBody>> for TrackRequests<S>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>>,
{
    type Response = Response<TrackedResponseBody<ResBody>>;
    type Error = S::Error;
    type Future = TrackedFuture<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: Request<ReqBody>) -> Self::Future {
        let guard = self
            .connection
            .as_ref()
            .map(|connection| connection.track_request());
        TrackedFuture {
            inner: self.inner.call(request),
            guard,
        }
    }
}

pin_project! {
    /// The response future of [`TrackRequests`].
    pub(crate) struct TrackedFuture<F> {
        #[pin]
        inner: F,
        guard: Option<RequestGuard>,
    }
}

impl<F, B, E> Future for TrackedFuture<F>
where
    F: Future<Output = Result<Response<B>, E>>,
{
    type Output = Result<Response<TrackedResponseBody<B>>, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        match this.inner.poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Ok(response)) => {
                let guard = this.guard.take().map(|mut guard| {
                    guard.record_response_ready();
                    guard
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
    /// The response body of [`TrackRequests`]. It holds the request until the
    /// transport has taken its last frame or drops it.
    pub(crate) struct TrackedResponseBody<B> {
        #[pin]
        inner: B,
        guard: Option<RequestGuard>,
    }
}

impl<B: Body> Body for TrackedResponseBody<B> {
    type Data = B::Data;
    type Error = B::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let mut this = self.project();
        let frame = ready!(this.inner.as_mut().poll_frame(cx));
        if frame.is_none() || this.inner.is_end_stream() {
            this.guard.take();
        }
        Poll::Ready(frame)
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> SizeHint {
        self.inner.size_hint()
    }
}
