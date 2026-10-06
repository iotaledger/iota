// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The sizes of the responses of the methods that return one message.

use std::{
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use bytes::{Buf, Bytes};
use http_body::{Body, Frame, SizeHint};
use pin_project_lite::pin_project;

use crate::metrics::GrpcServerMetrics;

/// The methods that return a stream of messages. Every other known method
/// returns one message.
pub(super) const STREAMING_METHODS: [&str; 4] = [
    "/iota.grpc.v1.ledger_service.LedgerService/GetObjects",
    "/iota.grpc.v1.ledger_service.LedgerService/GetTransactions",
    "/iota.grpc.v1.ledger_service.LedgerService/GetCheckpoint",
    "/iota.grpc.v1.ledger_service.LedgerService/StreamCheckpoints",
];

/// Reads the length of the first message of a gRPC response body. The message
/// starts with a 5-byte prefix: a compression flag and the length.
#[derive(Default)]
struct MessageLengthReader {
    prefix: [u8; MessageLengthReader::PREFIX_LEN],
    filled: usize,
}

impl MessageLengthReader {
    /// One byte of compression flag and four bytes of message length.
    const PREFIX_LEN: usize = 5;

    /// Copies the start of `chunk` into the prefix, up to its length.
    fn read(&mut self, chunk: &[u8]) {
        let missing = Self::PREFIX_LEN - self.filled;
        let copied = chunk.len().min(missing);
        self.prefix[self.filled..self.filled + copied].copy_from_slice(&chunk[..copied]);
        self.filled += copied;
    }

    fn is_done(&self) -> bool {
        self.filled == Self::PREFIX_LEN
    }

    /// The length of the message. `None` if the prefix is not complete or the
    /// message is compressed: the length of a compressed message is its
    /// compressed size.
    fn length(&self) -> Option<u32> {
        if !self.is_done() {
            return None;
        }
        let mut prefix = &self.prefix[..];
        let compressed = prefix.get_u8() != 0;
        let length = prefix.get_u32();
        (!compressed).then_some(length)
    }
}

/// Wraps the body of `response` so that the size of its message is recorded.
/// `metrics` is `None` for a method that returns a stream: its response passes
/// unchanged. See [`SizeRecordingBody`].
pub(super) fn record_size(
    response: http::Response<tonic::body::Body>,
    metrics: Option<Arc<GrpcServerMetrics>>,
) -> http::Response<tonic::body::Body> {
    let Some(metrics) = metrics else {
        return response;
    };
    response.map(|body| tonic::body::Body::new(SizeRecordingBody::new(body, metrics)))
}

pin_project! {
    /// The body of a response. It records the size of the first message, which
    /// it reads from the prefix of the message, and passes all frames on
    /// unchanged.
    struct SizeRecordingBody<B> {
        #[pin]
        inner: B,
        metrics: Arc<GrpcServerMetrics>,
        reader: Option<MessageLengthReader>,
    }
}

impl<B> SizeRecordingBody<B> {
    fn new(inner: B, metrics: Arc<GrpcServerMetrics>) -> Self {
        Self {
            inner,
            metrics,
            reader: Some(MessageLengthReader::default()),
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
        let data = match &poll {
            Poll::Ready(Some(Ok(frame))) => frame.data_ref(),
            _ => None,
        };
        if let Some(data) = data {
            read_length(this.reader, this.metrics, data);
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

/// Reads the start of a body chunk while the length of the message is not known
/// yet, and records the length once it is.
fn read_length(reader: &mut Option<MessageLengthReader>, metrics: &GrpcServerMetrics, data: &[u8]) {
    let Some(length_reader) = reader else {
        return;
    };
    length_reader.read(data);
    if !length_reader.is_done() {
        return;
    }
    if let Some(length) = length_reader.length() {
        metrics.unary_response_bytes.observe(f64::from(length));
    }
    *reader = None;
}

#[cfg(test)]
mod tests {
    use iota_grpc_types::v1::service_methods::ALL_METHOD_PATHS;
    use prometheus_filtered::Registry;

    use super::*;

    const UNARY_METHODS: [&str; 10] = [
        "/iota.grpc.v1.ledger_service.LedgerService/GetHealth",
        "/iota.grpc.v1.ledger_service.LedgerService/GetServiceInfo",
        "/iota.grpc.v1.ledger_service.LedgerService/GetEpoch",
        "/iota.grpc.v1.move_package_service.MovePackageService/ListPackageVersions",
        "/iota.grpc.v1.state_service.StateService/ListDynamicFields",
        "/iota.grpc.v1.state_service.StateService/ListOwnedObjects",
        "/iota.grpc.v1.state_service.StateService/GetCoinInfo",
        "/iota.grpc.v1.transaction_execution_service.TransactionExecutionService/ExecuteTransactions",
        "/iota.grpc.v1.transaction_execution_service.TransactionExecutionService/SimulateTransactions",
        "/iota.grpc.v1.transaction_execution_service.TransactionExecutionService/ViewFunctionCalls",
    ];

    /// A new method has to be listed as unary or as streaming.
    #[test]
    fn every_known_method_is_classified_as_unary_or_streaming() {
        let classified: std::collections::BTreeSet<&str> =
            UNARY_METHODS.into_iter().chain(STREAMING_METHODS).collect();
        assert_eq!(
            classified.len(),
            UNARY_METHODS.len() + STREAMING_METHODS.len()
        );
        assert_eq!(
            classified,
            ALL_METHOD_PATHS
                .into_iter()
                .collect::<std::collections::BTreeSet<_>>()
        );
    }

    #[test]
    fn the_length_is_big_endian_and_comes_from_a_prefix_in_several_chunks() {
        let mut reader = MessageLengthReader::default();
        reader.read(&[0, 0]);
        assert!(!reader.is_done());
        assert_eq!(reader.length(), None);
        reader.read(&[1, 2]);
        reader.read(&[3, 9, 9]);
        assert!(reader.is_done());
        assert_eq!(reader.length(), Some(0x0001_0203));
    }

    #[test]
    fn a_compressed_message_has_no_length() {
        let mut reader = MessageLengthReader::default();
        reader.read(&[1, 0, 0, 0, 10]);
        assert!(reader.is_done());
        assert_eq!(reader.length(), None);
    }

    fn prefixed(length: u32, compressed: bool) -> Vec<u8> {
        let mut message = vec![u8::from(compressed)];
        message.extend_from_slice(&length.to_be_bytes());
        message.extend(std::iter::repeat_n(7, length as usize));
        message
    }

    fn registry_with_metrics() -> (Registry, Arc<GrpcServerMetrics>) {
        let registry = Registry::new();
        (
            registry.clone(),
            Arc::new(GrpcServerMetrics::new(&registry)),
        )
    }

    async fn drain<B: Body<Data = Bytes> + Unpin>(mut body: B) {
        use std::future::poll_fn;
        while poll_fn(|cx| Pin::new(&mut body).poll_frame(cx))
            .await
            .is_some()
        {}
    }

    fn observed(registry: &Registry) -> (u64, f64) {
        let hist = iota_metrics::test_utils::MetricsReader::new(registry)
            .histogram_totals("node_grpc_unary_response_bytes", &[]);
        (hist.count, hist.sum)
    }

    type Frames =
        futures::stream::Iter<std::vec::IntoIter<Result<Frame<Bytes>, std::convert::Infallible>>>;

    fn chunked(message: &[u8], sizes: &[usize]) -> http_body_util::StreamBody<Frames> {
        let mut frames = Vec::new();
        let mut rest = message;
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

    #[tokio::test]
    async fn the_size_is_the_length_in_the_prefix_of_the_first_message() {
        let (registry, metrics) = registry_with_metrics();
        let message = prefixed(100_000, false);
        drain(SizeRecordingBody::new(
            chunked(&message, &[40_000, 40_000]),
            metrics,
        ))
        .await;
        assert_eq!(observed(&registry), (1, 100_000.0));
    }

    #[tokio::test]
    async fn a_prefix_split_over_chunks_is_read() {
        let (registry, metrics) = registry_with_metrics();
        let message = prefixed(300, false);
        drain(SizeRecordingBody::new(
            chunked(&message, &[2, 1, 2]),
            metrics,
        ))
        .await;
        assert_eq!(observed(&registry), (1, 300.0));
    }

    #[tokio::test]
    async fn a_response_with_two_messages_is_observed_once() {
        let (registry, metrics) = registry_with_metrics();
        let mut message = prefixed(10, false);
        message.extend(prefixed(20, false));
        drain(SizeRecordingBody::new(chunked(&message, &[]), metrics)).await;
        assert_eq!(observed(&registry), (1, 10.0));
    }

    #[tokio::test]
    async fn a_compressed_message_and_a_stream_are_not_observed() {
        let (registry, metrics) = registry_with_metrics();
        let compressed = prefixed(10, true);
        drain(SizeRecordingBody::new(chunked(&compressed, &[]), metrics)).await;
        drain(chunked(&prefixed(10, false), &[])).await;
        assert_eq!(observed(&registry), (0, 0.0));
    }

    #[tokio::test]
    async fn a_response_without_data_is_not_observed() {
        let (registry, metrics) = registry_with_metrics();
        drain(SizeRecordingBody::new(chunked(&[], &[]), metrics)).await;
        assert_eq!(observed(&registry), (0, 0.0));
    }
}
