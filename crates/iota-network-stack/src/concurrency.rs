// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Per-service concurrency limiting for gRPC servers.
//!
//! Unlike [`tower::limit::GlobalConcurrencyLimitLayer`] applied around a whole
//! server, [`ServiceConcurrencyLimit`] bounds the in-flight requests of a
//! single gRPC service, so services sharing one listener cannot crowd each
//! other out of admission slots.

use std::{
    convert::Infallible,
    sync::Arc,
    task::{Context, Poll},
};

use futures::future::BoxFuture;
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
    pub fn new(inner: S, limit: usize, load_shed: bool) -> Self {
        Self {
            inner,
            // Clamp: `Semaphore::new` panics above `MAX_PERMITS`, and
            // effectively-unlimited configs multiply large values by the CPU
            // core count.
            semaphore: Arc::new(Semaphore::new(limit.min(Semaphore::MAX_PERMITS))),
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

#[cfg(test)]
mod tests {
    use std::time::Duration;

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
            1,
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
            1,
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
        let saturated = ServiceConcurrencyLimit::new(blocking.clone(), 1, true);
        let other = ServiceConcurrencyLimit::new(blocking, 1, true);

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
}
