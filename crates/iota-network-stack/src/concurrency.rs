// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Per-service concurrency limiting for gRPC servers.
//!
//! Unlike [`tower::limit::GlobalConcurrencyLimitLayer`] applied around a whole
//! server, [`ServiceConcurrencyLimit`] bounds the in-flight requests of a
//! single gRPC service, so services sharing one listener cannot crowd each
//! other out of admission slots.
//!
//! Tower's per-service [`tower::limit::ConcurrencyLimitLayer`] cannot be used
//! here: its `ConcurrencyLimit` wrapper does not implement tonic's
//! [`NamedService`], which `Routes::add_service` requires for routing, and
//! shedding through tower's `LoadShed` surfaces as a `BoxError`, incompatible
//! with the router's `Error = Infallible` bound — over-limit requests must be
//! answered in-band with a gRPC `RESOURCE_EXHAUSTED` response instead.

use std::{
    convert::Infallible,
    num::NonZeroUsize,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use futures::future::BoxFuture;
use pin_project_lite::pin_project;
use tokio::sync::Semaphore;
use tonic::{
    body::Body,
    codegen::http::{Request, Response},
    server::NamedService,
};
use tower::Service;

/// Bounds the number of concurrent in-flight requests to the wrapped gRPC
/// service, independently of any other service registered on the same server.
///
/// With `load_shed` enabled, requests over the limit are rejected immediately
/// with gRPC `RESOURCE_EXHAUSTED`; otherwise they wait for a slot to free up.
/// Clones share the same limit.
#[derive(Clone)]
pub struct ServiceConcurrencyLimit<S> {
    inner: S,
    semaphore: Arc<Semaphore>,
    load_shed: bool,
}

impl<S> ServiceConcurrencyLimit<S> {
    pub fn new(inner: S, limit: NonZeroUsize, load_shed: bool) -> Self {
        Self {
            inner,
            // Clamp: `Semaphore::new` panics above `MAX_PERMITS`, and
            // effectively-unlimited configs multiply large values by the CPU
            // core count.
            semaphore: Arc::new(Semaphore::new(limit.get().min(Semaphore::MAX_PERMITS))),
            load_shed,
        }
    }
}

impl<S> Service<Request<Body>> for ServiceConcurrencyLimit<S>
where
    S: Service<Request<Body>, Response = Response<Body>, Error = Infallible>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
{
    type Response = Response<Body>;
    type Error = Infallible;
    type Future = BoxFuture<'static, Result<Response<Body>, Infallible>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: Request<Body>) -> Self::Future {
        // Admission happens here rather than in `poll_ready` (where
        // `tower::limit::ConcurrencyLimit` acquires its permit): a shed
        // request must be answered with a gRPC response, and `poll_ready`
        // can only signal Ready or Pending — converting Pending into an
        // error via an outer load-shed layer is ruled out by the router's
        // `Error = Infallible` bound. Readiness-based acquisition would
        // also buy no upstream backpressure: the axum router dispatches
        // every request on a fresh clone of this service.
        //
        // Take the instance that was driven to readiness and leave the clone
        // for later calls, as `poll_ready` readiness does not transfer to
        // clones.
        let clone = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, clone);
        let semaphore = self.semaphore.clone();
        let load_shed = self.load_shed;

        Box::pin(async move {
            // The permit is held until the response future resolves, mirroring
            // `tower::limit::ConcurrencyLimit`.
            let _permit = if load_shed {
                match semaphore.try_acquire_owned() {
                    Ok(permit) => permit,
                    Err(_) => {
                        return Ok(tonic::Status::resource_exhausted(
                            "service concurrency limit reached",
                        )
                        .into_http());
                    }
                }
            } else {
                semaphore
                    .acquire_owned()
                    .await
                    .expect("the semaphore is never closed")
            };
            inner.call(request).await
        })
    }
}

impl<S: NamedService> NamedService for ServiceConcurrencyLimit<S> {
    const NAME: &'static str = S::NAME;
}

pin_project! {
    /// Response body owning a guard, typically a semaphore permit, that is
    /// released when the body is dropped: once the response has been written,
    /// reset, or abandoned.
    pub struct PermitGuardedBody<B, G> {
        #[pin]
        inner: B,
        _guard: Option<G>,
    }
}

impl<B, G> PermitGuardedBody<B, G> {
    /// `guard` is `None` when the caller holds no permit for this response.
    pub fn new(inner: B, guard: Option<G>) -> Self {
        Self {
            inner,
            _guard: guard,
        }
    }
}

impl<B, G> http_body::Body for PermitGuardedBody<B, G>
where
    B: http_body::Body,
{
    type Data = B::Data;
    type Error = B::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
        self.project().inner.poll_frame(cx)
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
        sync::atomic::{AtomicBool, Ordering},
        time::Duration,
    };

    use http_body::Body as _;
    use tower::ServiceExt;

    use super::*;

    /// Inner service whose responses only complete once `release` is
    /// notified, keeping requests in flight for as long as the test needs.
    #[derive(Clone)]
    struct BlockingService {
        release: Arc<tokio::sync::Notify>,
    }

    impl Service<Request<Body>> for BlockingService {
        type Response = Response<Body>;
        type Error = Infallible;
        type Future = BoxFuture<'static, Result<Response<Body>, Infallible>>;

        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, _request: Request<Body>) -> Self::Future {
            let release = self.release.clone();
            Box::pin(async move {
                release.notified().await;
                Ok(Response::new(Body::default()))
            })
        }
    }

    fn request() -> Request<Body> {
        Request::new(Body::default())
    }

    #[tokio::test]
    async fn load_shedding_rejects_requests_over_the_limit() {
        let release = Arc::new(tokio::sync::Notify::new());
        let service = ServiceConcurrencyLimit::new(
            BlockingService {
                release: release.clone(),
            },
            NonZeroUsize::MIN,
            true,
        );

        let in_flight = tokio::spawn(service.clone().oneshot(request()));
        tokio::task::yield_now().await;

        let shed = service.clone().oneshot(request()).await.unwrap();
        assert_eq!(
            shed.headers().get("grpc-status").unwrap(),
            &(tonic::Code::ResourceExhausted as i32).to_string()
        );

        release.notify_one();
        let response = in_flight.await.unwrap().unwrap();
        assert!(response.headers().get("grpc-status").is_none());
    }

    #[tokio::test]
    async fn without_load_shedding_requests_over_the_limit_wait() {
        let release = Arc::new(tokio::sync::Notify::new());
        let service = ServiceConcurrencyLimit::new(
            BlockingService {
                release: release.clone(),
            },
            NonZeroUsize::MIN,
            false,
        );

        let first = tokio::spawn(service.clone().oneshot(request()));
        tokio::task::yield_now().await;

        let mut second = tokio::spawn(service.clone().oneshot(request()));
        let waiting = tokio::time::timeout(Duration::from_millis(50), &mut second).await;
        assert!(waiting.is_err(), "second request should wait for a slot");

        release.notify_one();
        first.await.unwrap().unwrap();
        release.notify_one();
        second.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn limits_are_independent_per_service() {
        let release = Arc::new(tokio::sync::Notify::new());
        let blocking = BlockingService {
            release: release.clone(),
        };
        let saturated = ServiceConcurrencyLimit::new(blocking.clone(), NonZeroUsize::MIN, true);
        let other = ServiceConcurrencyLimit::new(blocking, NonZeroUsize::MIN, true);

        let in_flight = tokio::spawn(saturated.clone().oneshot(request()));
        tokio::task::yield_now().await;

        // The other service has its own semaphore and still admits requests.
        let admitted = tokio::spawn(other.oneshot(request()));
        tokio::task::yield_now().await;

        release.notify_waiters();
        assert!(
            in_flight
                .await
                .unwrap()
                .unwrap()
                .headers()
                .get("grpc-status")
                .is_none()
        );
        assert!(
            admitted
                .await
                .unwrap()
                .unwrap()
                .headers()
                .get("grpc-status")
                .is_none()
        );
    }

    /// Guard recording whether it has been dropped.
    struct DropFlag(Arc<AtomicBool>);

    impl Drop for DropFlag {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    /// Body that never yields a frame, standing in for a response still being
    /// streamed.
    struct PendingBody;

    impl http_body::Body for PendingBody {
        type Data = bytes::Bytes;
        type Error = Infallible;

        fn poll_frame(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
            Poll::Pending
        }
    }

    #[tokio::test]
    async fn the_guard_survives_the_end_of_the_stream() {
        let dropped = Arc::new(AtomicBool::new(false));
        let mut body = PermitGuardedBody::new(Body::default(), Some(DropFlag(dropped.clone())));

        let end = std::future::poll_fn(|cx| Pin::new(&mut body).poll_frame(cx)).await;
        assert!(end.is_none());
        assert!(!dropped.load(Ordering::SeqCst));

        drop(body);
        assert!(dropped.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn dropping_an_unfinished_body_releases_the_guard() {
        let dropped = Arc::new(AtomicBool::new(false));
        let mut body = PermitGuardedBody::new(PendingBody, Some(DropFlag(dropped.clone())));

        let polled = tokio::time::timeout(
            Duration::from_millis(10),
            std::future::poll_fn(|cx| Pin::new(&mut body).poll_frame(cx)),
        )
        .await;
        assert!(polled.is_err());

        drop(body);
        assert!(dropped.load(Ordering::SeqCst));
    }
}
