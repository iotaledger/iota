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
    sync::{Arc, Mutex, PoisonError},
    task::{Context, Poll, ready},
    time::Duration,
};

use bytes::Bytes;
use futures::future::BoxFuture;
use http_body::Frame;
use pin_project_lite::pin_project;
use tokio::{sync::Semaphore, task::AbortHandle};
use tonic::{
    Status,
    body::Body,
    codegen::http::{Request, Response},
    server::NamedService,
};
use tower::Service;

type BoxError = Box<dyn std::error::Error + Send + Sync>;

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

/// Largest data frame `ReclaimableBody` hands to the transport. Whatever has
/// been handed over sits in the stream's send buffer out of reach, so a small
/// frame keeps what a reclaim cannot free small.
const MAX_FRAME_BYTES: usize = 64 * 1024;

/// What a `ReclaimableBody` gives up at its deadline.
struct Held<B, D> {
    inner: B,
    /// Rest of the data frame being handed over in `MAX_FRAME_BYTES` pieces.
    pending: Bytes,
    _until_deadline: Option<D>,
}

pin_project! {
    /// Response body that gives up its contents when a deadline passes before
    /// it has been sent. The transport polls a body only when the receiver
    /// grants it window, so a receiver that stops reading leaves the body
    /// parked, together with everything it owns; the deadline takes the inner
    /// body and `until_deadline` away from under it, and any later poll yields
    /// an error. `until_end` stays until the body itself is dropped, once the
    /// stream has been written, reset or abandoned.
    pub struct ReclaimableBody<B, D, E> {
        held: Arc<Mutex<Option<Held<B, D>>>>,
        _until_end: Option<E>,
        timer: Option<AbortHandle>,
    }

    impl<B, D, E> PinnedDrop for ReclaimableBody<B, D, E> {
        fn drop(this: Pin<&mut Self>) {
            if let Some(timer) = &this.timer {
                timer.abort();
            }
        }
    }
}

impl<B, D, E> ReclaimableBody<B, D, E>
where
    B: Send + 'static,
    D: Send + 'static,
{
    /// A body with no deadline; `until_deadline` and `until_end` are then both
    /// released when it is dropped.
    pub fn new(inner: B, until_deadline: Option<D>, until_end: Option<E>) -> Self {
        Self {
            held: Arc::new(Mutex::new(Some(Held {
                inner,
                pending: Bytes::new(),
                _until_deadline: until_deadline,
            }))),
            _until_end: until_end,
            timer: None,
        }
    }

    /// A body that gives up `inner` and `until_deadline` once `deadline` has
    /// passed, calling `on_reclaim` when it does.
    pub fn with_deadline(
        inner: B,
        until_deadline: Option<D>,
        until_end: Option<E>,
        deadline: Duration,
        on_reclaim: impl FnOnce() + Send + 'static,
    ) -> Self {
        let mut body = Self::new(inner, until_deadline, until_end);
        let held = body.held.clone();
        let timer = tokio::spawn(async move {
            tokio::time::sleep(deadline).await;
            let reclaimed = held.lock().unwrap_or_else(PoisonError::into_inner).take();
            if reclaimed.is_some() {
                on_reclaim();
            }
        });
        body.timer = Some(timer.abort_handle());
        body
    }
}

impl<B, D, E> http_body::Body for ReclaimableBody<B, D, E>
where
    B: http_body::Body<Data = Bytes> + Unpin,
    B::Error: Into<BoxError>,
{
    type Data = Bytes;
    type Error = BoxError;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let this = self.project();
        let mut held = this.held.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(state) = held.as_mut() else {
            return Poll::Ready(Some(Err(Box::new(Status::deadline_exceeded(
                "response was not read in time",
            )))));
        };
        if state.pending.is_empty() {
            match ready!(Pin::new(&mut state.inner).poll_frame(cx)) {
                None => {
                    // The stream is over; nothing is left to reclaim.
                    if let Some(timer) = this.timer.take() {
                        timer.abort();
                    }
                    return Poll::Ready(None);
                }
                Some(Err(error)) => return Poll::Ready(Some(Err(error.into()))),
                Some(Ok(frame)) => match frame.into_data() {
                    Ok(data) => state.pending = data,
                    Err(frame) => {
                        if let Some(timer) = this.timer.take() {
                            timer.abort();
                        }
                        return Poll::Ready(Some(Ok(frame)));
                    }
                },
            }
        }
        let piece = state
            .pending
            .split_to(state.pending.len().min(MAX_FRAME_BYTES));
        // While a reclaim can still happen, hand over a copy: a slice would keep
        // the whole buffer it came from alive in the send buffer.
        let piece = if this.timer.is_some() {
            Bytes::copy_from_slice(&piece)
        } else {
            piece
        };
        Poll::Ready(Some(Ok(Frame::data(piece))))
    }

    fn is_end_stream(&self) -> bool {
        let held = self.held.lock().unwrap_or_else(PoisonError::into_inner);
        held.as_ref()
            .is_some_and(|state| state.pending.is_empty() && state.inner.is_end_stream())
    }

    fn size_hint(&self) -> http_body::SizeHint {
        let held = self.held.lock().unwrap_or_else(PoisonError::into_inner);
        match held.as_ref() {
            Some(state) => {
                let pending = state.pending.len() as u64;
                let mut hint = state.inner.size_hint();
                // Upper first: `set_lower` asserts the new lower bound is within it.
                if let Some(upper) = hint.upper() {
                    hint.set_upper(upper.saturating_add(pending));
                }
                hint.set_lower(hint.lower().saturating_add(pending));
                hint
            }
            None => http_body::SizeHint::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

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

    const DEADLINE: Duration = Duration::from_millis(30);

    fn flag() -> (Arc<AtomicBool>, DropFlag) {
        let raised = Arc::new(AtomicBool::new(false));
        (raised.clone(), DropFlag(raised))
    }

    /// A reclaim callback and the flag it raises when called.
    fn report() -> (Arc<AtomicBool>, impl FnOnce() + Send + 'static) {
        let reported = Arc::new(AtomicBool::new(false));
        let raise = reported.clone();
        (reported, move || raise.store(true, Ordering::SeqCst))
    }

    /// Reads one frame, or the end of the body.
    async fn next_frame<B: http_body::Body + Unpin>(
        body: &mut B,
    ) -> Option<Result<Frame<B::Data>, B::Error>> {
        std::future::poll_fn(|cx| Pin::new(&mut *body).poll_frame(cx)).await
    }

    #[tokio::test]
    async fn reclaimable_body_hands_over_frames_of_bounded_size() {
        let inner = http_body_util::Full::new(Bytes::from(vec![7u8; MAX_FRAME_BYTES * 2 + 5]));
        let mut body = ReclaimableBody::<_, DropFlag, DropFlag>::new(inner, None, None);

        let mut sizes = Vec::new();
        let mut remaining = vec![body.size_hint().exact()];
        while let Some(frame) = next_frame(&mut body).await {
            sizes.push(frame.unwrap().into_data().unwrap().len());
            remaining.push(body.size_hint().exact());
        }
        assert_eq!(sizes, [MAX_FRAME_BYTES, MAX_FRAME_BYTES, 5]);
        let frame = MAX_FRAME_BYTES as u64;
        assert_eq!(
            remaining,
            [Some(2 * frame + 5), Some(frame + 5), Some(5), Some(0)]
        );
    }

    #[tokio::test]
    async fn the_deadline_reclaims_the_body_and_keeps_the_end_guard() {
        let (reclaimed_deadline, until_deadline) = flag();
        let (reclaimed_end, until_end) = flag();
        let (reported, on_reclaim) = report();
        let mut body = ReclaimableBody::with_deadline(
            PendingBody,
            Some(until_deadline),
            Some(until_end),
            DEADLINE,
            on_reclaim,
        );

        tokio::time::sleep(DEADLINE * 3).await;
        assert!(reclaimed_deadline.load(Ordering::SeqCst));
        assert!(reported.load(Ordering::SeqCst));
        assert!(!reclaimed_end.load(Ordering::SeqCst));
        // The transport, once the peer lets it poll again, gets an error.
        let error = next_frame(&mut body).await.unwrap().unwrap_err();
        assert_eq!(
            Status::from_error(error).code(),
            tonic::Code::DeadlineExceeded
        );

        drop(body);
        assert!(reclaimed_end.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn a_body_read_to_the_end_is_never_reclaimed() {
        let (reclaimed_deadline, until_deadline) = flag();
        let (reported, on_reclaim) = report();
        let mut body = ReclaimableBody::<_, _, DropFlag>::with_deadline(
            Body::default(),
            Some(until_deadline),
            None,
            DEADLINE,
            on_reclaim,
        );

        assert!(next_frame(&mut body).await.is_none());
        tokio::time::sleep(DEADLINE * 3).await;
        assert!(!reclaimed_deadline.load(Ordering::SeqCst));
        assert!(!reported.load(Ordering::SeqCst));

        drop(body);
        assert!(reclaimed_deadline.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn dropping_a_reclaimable_body_stops_its_deadline() {
        let (reported, on_reclaim) = report();
        let body = ReclaimableBody::<_, DropFlag, DropFlag>::with_deadline(
            PendingBody,
            None,
            None,
            DEADLINE,
            on_reclaim,
        );

        drop(body);
        tokio::time::sleep(DEADLINE * 3).await;
        assert!(!reported.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn the_deadline_frees_the_buffer_behind_a_piece_already_handed_over() {
        let response = Bytes::from(vec![7u8; MAX_FRAME_BYTES * 2]);
        let mut body = ReclaimableBody::<_, DropFlag, DropFlag>::with_deadline(
            http_body_util::Full::new(response.clone()),
            None,
            None,
            DEADLINE,
            || {},
        );
        // The transport holds on to the piece while the peer is not reading.
        let piece = next_frame(&mut body)
            .await
            .unwrap()
            .unwrap()
            .into_data()
            .unwrap();

        tokio::time::sleep(DEADLINE * 3).await;
        assert!(
            response.is_unique(),
            "the handed-over piece must not keep the response buffer alive"
        );
        drop((piece, body));
    }
}
