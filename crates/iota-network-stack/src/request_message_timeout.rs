// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Deadline for a gRPC request message to arrive after its headers.

use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll, ready},
    time::Duration,
};

use http::Request;
use pin_project_lite::pin_project;
use tokio::time::Sleep;
use tonic::{Status, server::NamedService};
use tower::Service;

type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// Fails a request whose body has not fully arrived within `timeout` of its
/// headers with `DEADLINE_EXCEEDED`, so a peer cannot keep a request open by
/// withholding its message. Requests on `exempt_paths` pass through untouched;
/// they carry client streams that stay open by design.
#[derive(Debug, Clone)]
pub struct RequestMessageTimeout<S> {
    inner: S,
    timeout: Duration,
    exempt_paths: &'static [&'static str],
}

impl<S> RequestMessageTimeout<S> {
    pub fn new(inner: S, timeout: Duration, exempt_paths: &'static [&'static str]) -> Self {
        Self {
            inner,
            timeout,
            exempt_paths,
        }
    }
}

impl<S, B> Service<Request<B>> for RequestMessageTimeout<S>
where
    S: Service<Request<TimedRequestBody<B>>>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: Request<B>) -> Self::Future {
        let timeout = (!self.exempt_paths.contains(&request.uri().path())).then_some(self.timeout);
        self.inner
            .call(request.map(|body| TimedRequestBody::new(body, timeout)))
    }
}

impl<S: NamedService> NamedService for RequestMessageTimeout<S> {
    const NAME: &'static str = S::NAME;
}

pin_project! {
    /// Request body that yields `DEADLINE_EXCEEDED` in place of its next frame
    /// once its deadline has passed.
    pub struct TimedRequestBody<B> {
        #[pin]
        inner: B,
        #[pin]
        deadline: Option<Sleep>,
    }
}

impl<B> TimedRequestBody<B> {
    fn new(inner: B, timeout: Option<Duration>) -> Self {
        Self {
            inner,
            deadline: timeout.map(tokio::time::sleep),
        }
    }
}

impl<B> http_body::Body for TimedRequestBody<B>
where
    B: http_body::Body,
    B::Error: Into<BoxError>,
{
    type Data = B::Data;
    type Error = BoxError;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
        let this = self.project();
        if let Poll::Ready(frame) = this.inner.poll_frame(cx) {
            return Poll::Ready(frame.map(|result| result.map_err(Into::into)));
        }
        if let Some(deadline) = this.deadline.as_pin_mut() {
            ready!(deadline.poll(cx));
            return Poll::Ready(Some(Err(Box::new(Status::deadline_exceeded(
                "request message did not arrive in time",
            )))));
        }
        Poll::Pending
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> http_body::SizeHint {
        self.inner.size_hint()
    }
}

#[cfg(test)]
mod tests {
    use std::{
        convert::Infallible,
        future::{Ready, ready},
    };

    use bytes::Bytes;
    use http_body_util::{BodyExt, Full};

    use super::*;

    const TIMEOUT: Duration = Duration::from_millis(50);
    const EXEMPT: &[&str] = &["/pkg.Svc/Stream"];

    /// A body whose next frame never arrives.
    struct Withheld;

    impl http_body::Body for Withheld {
        type Data = Bytes;
        type Error = Infallible;

        fn poll_frame(
            self: Pin<&mut Self>,
            _: &mut Context<'_>,
        ) -> Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
            Poll::Pending
        }
    }

    /// Hands the request body back, so a test can drive it as tonic would.
    #[derive(Clone)]
    struct Echo;

    impl<B> Service<Request<B>> for Echo {
        type Response = B;
        type Error = Infallible;
        type Future = Ready<Result<B, Infallible>>;

        fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, request: Request<B>) -> Self::Future {
            ready(Ok(request.into_body()))
        }
    }

    fn request<B>(path: &str, body: B) -> Request<B> {
        Request::builder().uri(path).body(body).unwrap()
    }

    #[tokio::test]
    async fn withheld_message_fails_with_deadline_exceeded() {
        let mut service = RequestMessageTimeout::new(Echo, TIMEOUT, EXEMPT);
        let body = service
            .call(request("/pkg.Svc/Unary", Withheld))
            .await
            .unwrap();
        let error = body.collect().await.unwrap_err();
        assert_eq!(
            Status::from_error(error).code(),
            tonic::Code::DeadlineExceeded
        );
    }

    #[tokio::test]
    async fn withheld_message_on_an_exempt_path_stays_open() {
        let mut service = RequestMessageTimeout::new(Echo, TIMEOUT, EXEMPT);
        let body = service
            .call(request("/pkg.Svc/Stream", Withheld))
            .await
            .unwrap();
        assert!(
            tokio::time::timeout(TIMEOUT * 3, body.collect())
                .await
                .is_err(),
            "an exempt request must not be cut by the deadline"
        );
    }

    #[tokio::test]
    async fn message_that_arrives_in_time_passes_through() {
        let mut service = RequestMessageTimeout::new(Echo, TIMEOUT, EXEMPT);
        let body = service
            .call(request(
                "/pkg.Svc/Unary",
                Full::new(Bytes::from_static(b"message")),
            ))
            .await
            .unwrap();
        assert_eq!(body.collect().await.unwrap().to_bytes(), "message");
    }
}
