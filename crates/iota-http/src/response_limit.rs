// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! A limit of the responses in progress on each connection.
//!
//! A response is in progress from before its handler runs until h2 has
//! dropped the last data chunk of it: each chunk that the body hands to hyper
//! owns a reference to the slot of the response. hyper drops a response body
//! as soon as its last frame is queued, while h2 still holds the data until
//! the client has read it, so a limit released at the return of the handler
//! or at the drop of the body counts nothing for a response that the client
//! reads slowly or not at all.

use std::{
    future::Future,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Context, Poll},
};

use bytes::Bytes;
use http::{Request, Response, request::Parts};
use pin_project_lite::pin_project;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tower::{Layer, Service};

use crate::metrics::ListenerMetrics;

/// The responses in progress on one connection, and the slots that limit
/// them.
#[derive(Debug)]
struct ConnectionResponses {
    /// `None` when the limit is 0: nothing waits, but the responses are
    /// counted.
    slots: Option<Arc<Semaphore>>,
    in_progress: AtomicUsize,
    waiting: AtomicUsize,
}

/// The response slots of one connection. The server puts a clone into the
/// extensions of every request of the connection while
/// [`Config::concurrency_limit_per_connection`](crate::Config::concurrency_limit_per_connection)
/// is set. A handler that sends items for a long time reads it to take a slot
/// for each item instead of one for its whole response.
#[derive(Clone, Debug)]
pub struct ResponseSlots {
    responses: Arc<ConnectionResponses>,
    metrics: Option<ListenerMetrics>,
}

impl ResponseSlots {
    pub(crate) fn new(limit: usize, metrics: Option<ListenerMetrics>) -> Self {
        Self {
            responses: Arc::new(ConnectionResponses {
                slots: (limit > 0).then(|| Arc::new(Semaphore::new(limit))),
                in_progress: AtomicUsize::new(0),
                waiting: AtomicUsize::new(0),
            }),
            metrics,
        }
    }

    /// Takes a slot of the connection, and waits for one in arrival order if
    /// none is free. With a limit of 0 it returns at once and only counts the
    /// response. If the future is dropped while it waits, it takes nothing;
    /// if it is dropped after it has got a slot but before it returns, it
    /// gives the slot back, and the response is not counted.
    pub async fn take(&self) -> ResponseSlot {
        let permit = match &self.responses.slots {
            None => None,
            Some(slots) => Some(match slots.clone().try_acquire_owned() {
                Ok(permit) => permit,
                Err(_) => {
                    let permit = {
                        let waiting = self.responses.waiting.fetch_add(1, Ordering::Relaxed) + 1;
                        let _waiting = WaitingGuard(&self.responses);
                        if let Some(metrics) = &self.metrics {
                            metrics.record_response_waiting(waiting);
                        }
                        slots
                            .clone()
                            .acquire_owned()
                            .await
                            .expect("the semaphore of a connection is never closed")
                    };
                    // hyper looks for a reset of the stream, and a deadline
                    // layer outside this one checks its deadline, only while
                    // this future is pending. Without this, a request that
                    // its client cancelled while it waited would start once a
                    // slot frees, and run its handler.
                    tokio::task::yield_now().await;
                    permit
                }
            }),
        };
        let in_progress = self.responses.in_progress.fetch_add(1, Ordering::Relaxed) + 1;
        // Built at once, so that its drop takes the count back also if the
        // rest of this function panics.
        let slot = ResponseSlot {
            _permit: permit,
            responses: self.responses.clone(),
        };
        if let Some(metrics) = &self.metrics {
            metrics.record_response_start(in_progress);
        }
        slot
    }

    /// The responses in progress on the connection.
    #[cfg(test)]
    pub(crate) fn in_progress(&self) -> usize {
        self.responses.in_progress.load(Ordering::Relaxed)
    }
}

/// A response slot of a connection. The response that holds it counts as in
/// progress until it is dropped.
#[derive(Debug)]
pub struct ResponseSlot {
    _permit: Option<OwnedSemaphorePermit>,
    responses: Arc<ConnectionResponses>,
}

impl Drop for ResponseSlot {
    fn drop(&mut self) {
        self.responses.in_progress.fetch_sub(1, Ordering::Relaxed);
    }
}

/// Counts a response as waiting until it is dropped, also when the wait is
/// cancelled.
struct WaitingGuard<'a>(&'a ConnectionResponses);

impl Drop for WaitingGuard<'_> {
    fn drop(&mut self) {
        self.0.waiting.fetch_sub(1, Ordering::Relaxed);
    }
}

/// A data chunk of a response, which holds the slot of the response until h2
/// has written its last byte and dropped it.
struct SlottedChunk {
    data: Bytes,
    _slot: Arc<ResponseSlot>,
}

impl AsRef<[u8]> for SlottedChunk {
    fn as_ref(&self) -> &[u8] {
        &self.data
    }
}

/// `data` as a chunk that holds `slot` until the chunk and every copy of it
/// are dropped. A handler that takes slots per item puts each chunk of an
/// item through this.
pub fn chunk_with_slot(data: Bytes, slot: Arc<ResponseSlot>) -> Bytes {
    Bytes::from_owner(SlottedChunk { data, _slot: slot })
}

type TakesSlotsPerItem = Arc<dyn Fn(&Parts) -> bool + Send + Sync>;

/// Tower [`Layer`] that applies the
/// [`concurrency_limit_per_connection`](crate::Config::concurrency_limit_per_connection)
/// of the listener to each request: it takes a slot of the connection before
/// the inner service runs, waits in arrival order if none is free, and gives
/// the slot back when hyper has dropped the response body and h2 has dropped
/// every data chunk of it.
///
/// It reads the [`ResponseSlots`] that the server put into the request
/// extensions; a request without them, from a listener without the limit,
/// passes through unchanged.
#[derive(Clone, Default)]
pub struct ResponseLimitLayer {
    takes_slots_per_item: Option<TakesSlotsPerItem>,
}

impl ResponseLimitLayer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Takes no slot for the requests that `predicate` matches: their handler
    /// takes a slot for each item it sends, with [`ResponseSlots::take`] and
    /// [`chunk_with_slot`], so that a response that stays open for long does
    /// not hold a slot while it is idle.
    pub fn takes_slots_per_item(
        self,
        predicate: impl Fn(&Parts) -> bool + Send + Sync + 'static,
    ) -> Self {
        Self {
            takes_slots_per_item: Some(Arc::new(predicate)),
        }
    }
}

impl<S> Layer<S> for ResponseLimitLayer {
    type Service = ResponseLimitService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        ResponseLimitService {
            inner,
            takes_slots_per_item: self.takes_slots_per_item.clone(),
        }
    }
}

/// Tower [`Service`] of [`ResponseLimitLayer`].
#[derive(Clone)]
pub struct ResponseLimitService<S> {
    inner: S,
    takes_slots_per_item: Option<TakesSlotsPerItem>,
}

impl<S, ReqBody, ResBody> Service<Request<ReqBody>> for ResponseLimitService<S>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>> + Clone + Send + 'static,
    S::Future: Send,
    ReqBody: Send + 'static,
{
    type Response = Response<ResponseLimitBody<ResBody>>;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, S::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        let (parts, body) = req.into_parts();
        let takes_slots_per_item = self
            .takes_slots_per_item
            .as_ref()
            .is_some_and(|predicate| predicate(&parts));
        let slots = parts.extensions.get::<ResponseSlots>().cloned();
        let req = Request::from_parts(parts, body);
        // Tower contract: `self.inner` is the ready one (poll_ready set it up),
        // so call it and leave the clone for the next request.
        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);
        Box::pin(async move {
            let slot = match slots {
                Some(slots) if !takes_slots_per_item => Some(Arc::new(slots.take().await)),
                _ => None,
            };
            let response = inner.call(req).await?;
            Ok(response.map(|inner| ResponseLimitBody { inner, slot }))
        })
    }
}

pin_project! {
    /// Response body of [`ResponseLimitService`]. It holds the slot of its
    /// request, and puts it on each of its data chunks, so the slot is free
    /// only once the body and all its chunks are dropped. A body without a
    /// slot passes its frames on as they are.
    pub struct ResponseLimitBody<B> {
        #[pin]
        inner: B,
        slot: Option<Arc<ResponseSlot>>,
    }
}

impl<B: Default> Default for ResponseLimitBody<B> {
    fn default() -> Self {
        Self {
            inner: B::default(),
            slot: None,
        }
    }
}

impl<B: http_body::Body<Data = Bytes>> http_body::Body for ResponseLimitBody<B> {
    type Data = Bytes;
    type Error = B::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<http_body::Frame<Bytes>, Self::Error>>> {
        let this = self.project();
        let slot = this.slot;
        this.inner.poll_frame(cx).map(|frame| {
            frame.map(|frame| {
                frame.map(|frame| match (slot, frame.into_data()) {
                    (Some(slot), Ok(data)) => {
                        http_body::Frame::data(chunk_with_slot(data, slot.clone()))
                    }
                    (None, Ok(data)) => http_body::Frame::data(data),
                    (_, Err(frame)) => frame,
                })
            })
        })
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
        io,
        sync::{Mutex, atomic::AtomicBool},
        time::Duration,
    };

    use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

    use super::*;

    /// A response body with one data chunk and then trailers. `dropped` is
    /// set when the body is dropped.
    struct OneChunkBody {
        chunk: Option<Bytes>,
        trailers_sent: bool,
        dropped: Arc<AtomicBool>,
    }

    impl OneChunkBody {
        fn new(chunk: Bytes, dropped: Arc<AtomicBool>) -> Self {
            Self {
                chunk: Some(chunk),
                trailers_sent: false,
                dropped,
            }
        }
    }

    impl Drop for OneChunkBody {
        fn drop(&mut self) {
            self.dropped.store(true, Ordering::SeqCst);
        }
    }

    impl http_body::Body for OneChunkBody {
        type Data = Bytes;
        type Error = Infallible;

        fn poll_frame(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<Option<Result<http_body::Frame<Bytes>, Infallible>>> {
            let this = self.get_mut();
            if let Some(chunk) = this.chunk.take() {
                return Poll::Ready(Some(Ok(http_body::Frame::data(chunk))));
            }
            if !this.trailers_sent {
                this.trailers_sent = true;
                return Poll::Ready(Some(Ok(http_body::Frame::trailers(http::HeaderMap::new()))));
            }
            Poll::Ready(None)
        }
    }

    /// A body with a slot of `slots`.
    async fn slotted_body(
        slots: &ResponseSlots,
        chunk_size: usize,
        dropped: Arc<AtomicBool>,
    ) -> ResponseLimitBody<OneChunkBody> {
        ResponseLimitBody {
            inner: OneChunkBody::new(Bytes::from(vec![7u8; chunk_size]), dropped),
            slot: Some(Arc::new(slots.take().await)),
        }
    }

    /// A response keeps its slot after its body is dropped, for as long as a
    /// data chunk of the body lives, and frees it when the last copy of the
    /// last chunk is dropped.
    #[tokio::test]
    async fn a_response_keeps_its_slot_until_its_last_chunk_is_dropped() {
        let slots = ResponseSlots::new(1, None);
        let dropped = Arc::default();
        let mut body = Box::pin(slotted_body(&slots, 1024, dropped).await);

        let frame = std::future::poll_fn(|cx| http_body::Body::poll_frame(body.as_mut(), cx))
            .await
            .unwrap()
            .unwrap();
        let chunk = frame.into_data().expect("a data frame");
        let trailers = std::future::poll_fn(|cx| http_body::Body::poll_frame(body.as_mut(), cx))
            .await
            .unwrap()
            .unwrap();
        assert!(trailers.is_trailers());
        drop(body);
        assert_eq!(slots.in_progress(), 1, "the chunk must hold the slot");

        let copy = chunk.clone();
        drop(chunk);
        assert_eq!(slots.in_progress(), 1, "a copy of the chunk holds it too");
        drop(copy);
        assert_eq!(slots.in_progress(), 0);
    }

    /// A client side of a connection that reads only as many bytes as it is
    /// allowed, so that a test can stall the server's writes at a chosen
    /// point.
    struct ThrottledIo {
        inner: tokio::io::DuplexStream,
        allowed: Arc<AtomicUsize>,
        waker: Arc<Mutex<Option<std::task::Waker>>>,
    }

    impl AsyncRead for ThrottledIo {
        fn poll_read(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut ReadBuf<'_>,
        ) -> Poll<io::Result<()>> {
            let this = self.get_mut();
            let allowed = this.allowed.load(Ordering::SeqCst);
            if allowed == 0 {
                *this.waker.lock().unwrap() = Some(cx.waker().clone());
                return Poll::Pending;
            }
            let mut tmp = vec![0u8; allowed.min(buf.remaining())];
            let mut tmp_buf = ReadBuf::new(&mut tmp);
            std::task::ready!(Pin::new(&mut this.inner).poll_read(cx, &mut tmp_buf))?;
            let read = tmp_buf.filled().len();
            this.allowed.fetch_sub(read, Ordering::SeqCst);
            buf.put_slice(tmp_buf.filled());
            Poll::Ready(Ok(()))
        }
    }

    impl AsyncWrite for ThrottledIo {
        fn poll_write(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<io::Result<usize>> {
            Pin::new(&mut self.get_mut().inner).poll_write(cx, buf)
        }

        fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Pin::new(&mut self.get_mut().inner).poll_flush(cx)
        }

        fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Pin::new(&mut self.get_mut().inner).poll_shutdown(cx)
        }
    }

    /// Waits until `condition` holds, or fails with `what`.
    async fn wait_until(condition: impl Fn() -> bool, what: &str) {
        tokio::time::timeout(Duration::from_secs(10), async {
            while !condition() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap_or_else(|_| panic!("{what}"));
    }

    /// Through real hyper and h2: hyper drops the response body once h2 has
    /// queued its last frame, while h2 still holds the tail of the data
    /// chunk, because the client reads slowly. The response keeps its slot
    /// until h2 has written that tail and dropped the chunk.
    #[tokio::test]
    async fn a_response_keeps_its_slot_until_h2_has_written_its_last_chunk() {
        const CHUNK: usize = 2 * 1024 * 1024;
        const UNREAD: usize = 200 * 1024;
        let slots = ResponseSlots::new(1, None);
        let dropped = Arc::new(AtomicBool::new(false));
        let (client_io, server_io) = tokio::io::duplex(64 * 1024);

        let service = {
            let slots = slots.clone();
            let dropped = dropped.clone();
            hyper::service::service_fn(move |_request| {
                let slots = slots.clone();
                let dropped = dropped.clone();
                async move {
                    Ok::<_, Infallible>(Response::new(slotted_body(&slots, CHUNK, dropped).await))
                }
            })
        };
        tokio::spawn(
            hyper::server::conn::http2::Builder::new(hyper_util::rt::TokioExecutor::new())
                .serve_connection(hyper_util::rt::TokioIo::new(server_io), service),
        );

        let allowed = Arc::new(AtomicUsize::new(CHUNK - UNREAD));
        let waker = Arc::new(Mutex::new(None));
        let client_io = ThrottledIo {
            inner: client_io,
            allowed: allowed.clone(),
            waker: waker.clone(),
        };
        let (send_request, connection) = h2::client::Builder::new()
            .initial_window_size(8 * 1024 * 1024)
            .initial_connection_window_size(8 * 1024 * 1024)
            .handshake::<_, Bytes>(client_io)
            .await
            .unwrap();
        tokio::spawn(connection);
        let mut send_request = send_request.ready().await.unwrap();
        let request = Request::builder()
            .uri("http://localhost/")
            .body(())
            .unwrap();
        let (response, _) = send_request.send_request(request, true).unwrap();
        let response = response.await.unwrap();

        wait_until(
            || dropped.load(Ordering::SeqCst),
            "hyper must drop the body while h2 still holds the tail of the chunk",
        )
        .await;
        assert_eq!(
            slots.in_progress(),
            1,
            "the chunk that h2 still holds must keep the slot"
        );

        allowed.store(usize::MAX, Ordering::SeqCst);
        if let Some(waker) = waker.lock().unwrap().take() {
            waker.wake();
        }
        wait_until(
            || slots.in_progress() == 0,
            "the slot must be free once h2 has written the chunk",
        )
        .await;
        drop(response);
    }
}
