// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The sizes of the messages of the gRPC responses.

use std::{
    pin::Pin,
    task::{Context, Poll},
};

use bytes::{Buf, Bytes};
use http_body::{Body, Frame, SizeHint};
use pin_project_lite::pin_project;
use prometheus_filtered::Histogram;

/// One byte of compression flag and four bytes of message length.
const PREFIX_LEN: usize = 5;

/// Reads the messages of a gRPC response body as the body passes. Each message
/// starts with a prefix of a compression flag and the length of the message.
#[derive(Default)]
struct MessageReader {
    prefix: [u8; PREFIX_LEN],
    filled: usize,
    /// The bytes of the current message still to come after its prefix.
    remaining: usize,
}

impl MessageReader {
    /// Reads the next `chunk` of the body and calls `on_message` with the
    /// length of each message whose prefix ends in it. A compressed message is
    /// skipped: the length in its prefix is its compressed size.
    fn read(&mut self, mut chunk: &[u8], mut on_message: impl FnMut(u32)) {
        while !chunk.is_empty() {
            if self.remaining > 0 {
                let skipped = chunk.len().min(self.remaining);
                self.remaining -= skipped;
                chunk = &chunk[skipped..];
                continue;
            }
            let copied = chunk.len().min(PREFIX_LEN - self.filled);
            self.prefix[self.filled..self.filled + copied].copy_from_slice(&chunk[..copied]);
            self.filled += copied;
            chunk = &chunk[copied..];
            if self.filled == PREFIX_LEN {
                self.filled = 0;
                let mut prefix = &self.prefix[..];
                let compressed = prefix.get_u8() != 0;
                let length = prefix.get_u32();
                self.remaining = length as usize;
                if !compressed {
                    on_message(length);
                }
            }
        }
    }
}

/// Wraps the body of `response` so that the size of each of its messages is
/// recorded in `sizes`. See [`SizeRecordingBody`].
pub(super) fn record_sizes(
    response: http::Response<tonic::body::Body>,
    sizes: Histogram,
) -> http::Response<tonic::body::Body> {
    response.map(|body| tonic::body::Body::new(SizeRecordingBody::new(body, sizes)))
}

pin_project! {
    /// The body of a response. It records the size of each message, which it
    /// reads from the prefix of the message, and passes all frames on
    /// unchanged.
    struct SizeRecordingBody<B> {
        #[pin]
        inner: B,
        sizes: Histogram,
        reader: MessageReader,
    }
}

impl<B> SizeRecordingBody<B> {
    fn new(inner: B, sizes: Histogram) -> Self {
        Self {
            inner,
            sizes,
            reader: MessageReader::default(),
        }
    }
}

impl<B> Body for SizeRecordingBody<B>
where
    B: Body<Data = Bytes>,
{
    type Data = Bytes;
    type Error = B::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, Self::Error>>> {
        let this = self.project();
        let poll = this.inner.poll_frame(cx);
        if let Poll::Ready(Some(Ok(frame))) = &poll {
            if let Some(data) = frame.data_ref() {
                let sizes = &*this.sizes;
                this.reader
                    .read(data, |length| sizes.observe(f64::from(length)));
            }
        }
        poll
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> SizeHint {
        self.inner.size_hint()
    }
}

#[cfg(test)]
mod tests {
    use prometheus_filtered::Registry;

    use super::*;
    use crate::metrics::GrpcServerMetrics;

    fn prefixed(length: u32, compressed: bool) -> Vec<u8> {
        let mut message = vec![u8::from(compressed)];
        message.extend_from_slice(&length.to_be_bytes());
        message.extend(std::iter::repeat_n(7, length as usize));
        message
    }

    type Frames =
        futures::stream::Iter<std::vec::IntoIter<Result<Frame<Bytes>, std::convert::Infallible>>>;

    /// A body that sends `body` in chunks of `sizes`, and the rest in one more
    /// chunk.
    fn chunked(body: &[u8], sizes: &[usize]) -> http_body_util::StreamBody<Frames> {
        let mut frames = Vec::new();
        let mut rest = body;
        for size in sizes {
            let (head, tail) = rest.split_at((*size).min(rest.len()));
            frames.push(Ok(Frame::data(Bytes::copy_from_slice(head))));
            rest = tail;
        }
        if !rest.is_empty() {
            frames.push(Ok(Frame::data(Bytes::copy_from_slice(rest))));
        }
        http_body_util::StreamBody::new(futures::stream::iter(frames))
    }

    /// Passes `body` in chunks of `sizes` through a [`SizeRecordingBody`] and
    /// returns the count and the sum of the observed sizes.
    async fn observe(body: &[u8], sizes: &[usize]) -> (u64, f64) {
        use std::future::poll_fn;

        let registry = Registry::new();
        let histogram = GrpcServerMetrics::new(&registry)
            .response_message_bytes
            .with_label_values(&["method"]);
        let mut recording = SizeRecordingBody::new(chunked(body, sizes), histogram);
        while poll_fn(|cx| Pin::new(&mut recording).poll_frame(cx))
            .await
            .is_some()
        {}
        let totals = iota_metrics::test_utils::MetricsReader::new(&registry)
            .histogram_totals("node_grpc_response_message_bytes", &[]);
        (totals.count, totals.sum)
    }

    #[tokio::test]
    async fn the_size_is_the_big_endian_length_in_the_prefix() {
        assert_eq!(
            observe(&prefixed(0x0001_0203, false), &[]).await,
            (1, 66_051.0)
        );
    }

    #[tokio::test]
    async fn a_message_over_several_chunks_is_observed_once() {
        let message = prefixed(100_000, false);
        assert_eq!(observe(&message, &[40_000, 40_000]).await, (1, 100_000.0));
    }

    #[tokio::test]
    async fn a_prefix_split_over_chunks_is_read() {
        assert_eq!(observe(&prefixed(300, false), &[2, 1, 2]).await, (1, 300.0));
    }

    #[tokio::test]
    async fn each_message_of_a_stream_is_observed() {
        let mut body = prefixed(10, false);
        body.extend(prefixed(0, false));
        body.extend(prefixed(20, false));
        assert_eq!(observe(&body, &[]).await, (3, 30.0));
        assert_eq!(observe(&body, &[3, 9, 1, 5, 4]).await, (3, 30.0));
    }

    #[tokio::test]
    async fn a_compressed_message_is_skipped_by_its_length() {
        let mut body = prefixed(10, true);
        body.extend(prefixed(20, false));
        assert_eq!(observe(&body, &[7]).await, (1, 20.0));
    }

    #[tokio::test]
    async fn a_response_without_data_is_not_observed() {
        assert_eq!(observe(&[], &[]).await, (0, 0.0));
    }
}
