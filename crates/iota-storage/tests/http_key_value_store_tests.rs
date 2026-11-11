// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! [`HttpKVStore`] against a real HTTP/2 server on localhost.

#![cfg(not(msim))]

use std::{
    collections::HashMap,
    convert::Infallible,
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::Duration,
};

use bytes::Bytes;
use futures::StreamExt;
use http_body_util::{Full, StreamBody};
use hyper::{
    Request, Response, StatusCode,
    body::{Frame, Incoming},
    service::service_fn,
};
use hyper_util::{
    rt::{TokioExecutor, TokioIo},
    server::conn::auto::Builder,
};
use iota_storage::{
    http_key_value_store::{HttpKVStore, encode_digest},
    key_value_store::TransactionKeyValueStoreTrait,
    key_value_store_metrics::KeyValueStoreMetrics,
};
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    base_types::random_object_ref,
    crypto::{AccountPrivateKey, get_key_pair},
    transaction::TransactionEnvelope,
};
use tokio::net::TcpListener;

fn random_tx() -> TransactionEnvelope {
    let (sender, key): (_, AccountPrivateKey) = get_key_pair();
    TestTransactionBuilder::new(sender, random_object_ref(), 1)
        .transfer(random_object_ref(), sender)
        .build_and_sign(&key)
}

/// The values a test server returns, by path (`tx/<key>`, ...).
type Values = Arc<Mutex<HashMap<String, Vec<u8>>>>;

/// Serves `values` over HTTP/2 without TLS, allowing at most `max_streams` concurrent streams per
/// connection and answering each request after `delay`. A path without a value gets a 404.
async fn serve(values: Values, max_streams: u32, delay: Duration) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let (stream, _) = listener.accept().await.unwrap();
            let values = values.clone();
            tokio::spawn(async move {
                let service = service_fn(move |request: Request<Incoming>| {
                    let values = values.clone();
                    async move {
                        tokio::time::sleep(delay).await;
                        let value = values
                            .lock()
                            .unwrap()
                            .get(request.uri().path().trim_start_matches('/'))
                            .cloned();
                        let response = match value {
                            Some(value) => Response::new(Full::new(Bytes::from(value))),
                            None => Response::builder()
                                .status(StatusCode::NOT_FOUND)
                                .body(Full::new(Bytes::new()))
                                .unwrap(),
                        };
                        Ok::<_, Infallible>(response)
                    }
                });
                let mut builder = Builder::new(TokioExecutor::new());
                builder.http2().max_concurrent_streams(max_streams);
                builder
                    .serve_connection(TokioIo::new(stream), service)
                    .await
                    .ok();
            });
        }
    });
    addr
}

/// Serves `value` for every path over HTTP/2 without TLS, in `parts` parts, waiting `gap` before
/// each part.
async fn serve_in_parts(value: Vec<u8>, parts: usize, gap: Duration) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let parts = value
        .chunks(value.len().div_ceil(parts))
        .map(Bytes::copy_from_slice)
        .collect::<Vec<_>>();
    tokio::spawn(async move {
        loop {
            let (stream, _) = listener.accept().await.unwrap();
            let parts = parts.clone();
            tokio::spawn(async move {
                let service = service_fn(move |_: Request<Incoming>| {
                    let body = futures::stream::iter(parts.clone()).then(move |part| async move {
                        tokio::time::sleep(gap).await;
                        Ok::<_, Infallible>(Frame::data(part))
                    });
                    async move { Ok::<_, Infallible>(Response::new(StreamBody::new(body))) }
                });
                Builder::new(TokioExecutor::new())
                    .serve_connection(TokioIo::new(stream), service)
                    .await
                    .ok();
            });
        }
    });
    addr
}

fn store(addr: SocketAddr) -> HttpKVStore {
    let metrics = KeyValueStoreMetrics::new_for_tests();
    HttpKVStore::new(&format!("http://{addr}"), 1000, metrics).unwrap()
}

#[tokio::test]
async fn gets_beyond_the_server_stream_limit_wait_for_a_slot_instead_of_timing_out() {
    // 8 GETs of 2 s each through 2 streams take 8 s. Without a slot limit, the last GETs would
    // wait for a stream inside the client and pass the 5 s GET timeout.
    let txs = (0..8).map(|_| random_tx()).collect::<Vec<_>>();
    let values = txs
        .iter()
        .map(|tx| {
            (
                format!("tx/{}", encode_digest(tx.digest())),
                bcs::to_bytes(tx).unwrap(),
            )
        })
        .collect();
    let addr = serve(Arc::new(Mutex::new(values)), 2, Duration::from_secs(2)).await;
    let store = store(addr).with_max_in_flight_fetches(2);

    let digests = txs.iter().map(|tx| *tx.digest()).collect::<Vec<_>>();
    let (fetched, _) = store.multi_get(&digests, &[]).await.unwrap();

    assert_eq!(fetched, txs.into_iter().map(Some).collect::<Vec<_>>());
}

#[tokio::test]
async fn a_value_that_keeps_arriving_past_the_read_timeout_is_read() {
    // 4 parts, 2 s apart: the value takes 8 s, more than the 5 s read timeout, but no part waits
    // longer than 2 s.
    let tx = random_tx();
    let addr = serve_in_parts(bcs::to_bytes(&tx).unwrap(), 4, Duration::from_secs(2)).await;

    let (fetched, _) = store(addr).multi_get(&[*tx.digest()], &[]).await.unwrap();

    assert_eq!(fetched, vec![Some(tx)]);
}
