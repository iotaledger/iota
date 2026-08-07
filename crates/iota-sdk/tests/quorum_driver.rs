// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    net::Ipv4Addr,
    sync::{Arc, Mutex},
};

use iota_json_rpc_types::{IotaTransactionBlockResponse, IotaTransactionBlockResponseOptions};
use iota_sdk::{IotaClient, IotaClientBuilder};
use iota_sdk_types::{
    Address, ObjectDigest, ObjectId, ObjectReference, Transaction, TransactionDigest, Version,
};
use iota_types::{
    crypto::{AccountKeyPair, get_key_pair},
    quorum_driver_types::ExecuteTransactionRequestType,
    transaction::{TransactionAPI, TransactionEnvelope},
};
use jsonrpsee::{
    RpcModule,
    server::{Server, ServerHandle},
    types::ErrorObjectOwned,
};
use serde_json::{Value, json};

/// What the mock node observed, so a test can assert on the wire format
/// rather than on the response the SDK synthesises.
#[derive(Clone, Debug, Default)]
struct MockState {
    /// The `request_type` argument of every `iota_executeTransactionBlock`
    /// call, as it arrived on the wire. `None` means JSON `null`.
    pub execute_request_types: Vec<Option<String>>,
    /// How many times `iota_getTransactionBlock` was called.
    pub poll_count: usize,
    /// The `digest` argument of every `iota_getTransactionBlock` call, in
    /// call order.
    pub poll_digests: Vec<TransactionDigest>,
    /// How many of the next `iota_getTransactionBlock` calls should still
    /// fail, counting down to zero. Lets a test exercise the SDK's
    /// swallow-and-retry loop instead of always succeeding on the first
    /// poll.
    pub reads_to_fail: usize,
}

struct MockNode {
    pub url: String,
    state: Arc<Mutex<MockState>>,
    // Dropping the handle stops the server.
    _handle: ServerHandle,
}

impl MockNode {
    fn state(&self) -> MockState {
        self.state.lock().unwrap().clone()
    }

    async fn client(&self) -> IotaClient {
        IotaClientBuilder::default()
            .build(&self.url)
            .await
            .expect("mock node handshake failed")
    }
}

/// Starts a node that answers the three methods `IotaClientBuilder::build` and
/// `execute_transaction_block` need, returning `confirmed_local_execution` in
/// the execution response. The first `reads_to_fail` calls to
/// `iota_getTransactionBlock` return an error, so a test can exercise the
/// SDK's retry loop; pass `0` for a node whose reads always succeed.
async fn start_mock_node(
    confirmed_local_execution: Option<bool>,
    reads_to_fail: usize,
) -> MockNode {
    let state = Arc::new(Mutex::new(MockState {
        reads_to_fail,
        ..Default::default()
    }));
    let mut module = RpcModule::new(state.clone());

    module
        .register_method("rpc.discover", |_params, _state, _| {
            json!({
                "info": { "version": "0.0.0" },
                "methods": [
                    { "name": "iota_executeTransactionBlock" },
                    { "name": "iota_getTransactionBlock" },
                ],
            })
        })
        .unwrap();

    module
        .register_method("iota_executeTransactionBlock", move |params, state, _| {
            // Positional params: [tx_bytes, signatures, options, request_type].
            let params: Vec<Value> = params.parse().unwrap();
            let request_type = params.get(3).and_then(|v| v.as_str()).map(|s| s.to_owned());
            state
                .lock()
                .unwrap()
                .execute_request_types
                .push(request_type);

            let mut response = IotaTransactionBlockResponse::new(TransactionDigest::default());
            response.confirmed_local_execution = confirmed_local_execution;
            serde_json::to_value(response).unwrap()
        })
        .unwrap();

    module
        .register_method(
            "iota_getTransactionBlock",
            |params, state, _| -> Result<Value, ErrorObjectOwned> {
                // Positional params: [digest, options].
                let params: Vec<Value> = params.parse().unwrap();
                let digest: TransactionDigest = serde_json::from_value(params[0].clone()).unwrap();

                let mut state = state.lock().unwrap();
                state.poll_count += 1;
                state.poll_digests.push(digest);
                if state.reads_to_fail > 0 {
                    state.reads_to_fail -= 1;
                    return Err(ErrorObjectOwned::owned(
                        -32000,
                        "transaction not yet visible",
                        None::<()>,
                    ));
                }

                let mut response = IotaTransactionBlockResponse::new(TransactionDigest::default());
                // Only the read API populates these two; the execute API
                // always leaves them empty. This is what lets a test tell
                // which response the SDK handed back.
                response.checkpoint = Some(7);
                response.timestamp_ms = Some(1_700_000_000_000);
                Ok(serde_json::to_value(response).unwrap())
            },
        )
        .unwrap();

    let server = Server::builder()
        .build((Ipv4Addr::LOCALHOST, 0))
        .await
        .unwrap();
    let url = format!("http://{}", server.local_addr().unwrap());
    let handle = server.start(module);

    MockNode {
        url,
        state,
        _handle: handle,
    }
}

/// A syntactically valid signed transaction. The mock node never inspects it.
fn sample_transaction() -> TransactionEnvelope {
    let (sender, keypair): (Address, AccountKeyPair) = get_key_pair();
    let (recipient, _): (Address, AccountKeyPair) = get_key_pair();
    let gas = ObjectReference::new(
        ObjectId::random(),
        Version::from_u64(1),
        ObjectDigest::random(),
    );
    let data = Transaction::new_transfer_iota(recipient, sender, Some(1), gas, 1_000_000, 1000);
    TransactionEnvelope::from_data_and_signer(data, vec![&keypair])
}

#[tokio::test]
async fn forwards_wait_for_local_execution_on_the_wire() {
    let node = start_mock_node(Some(true), 0).await;
    let client = node.client().await;

    client
        .quorum_driver_api()
        .execute_transaction_block(
            sample_transaction(),
            IotaTransactionBlockResponseOptions::new().with_effects(),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await
        .unwrap();

    let state = node.state();
    assert_eq!(
        state.execute_request_types,
        vec![Some("WaitForLocalExecution".to_owned())],
        "the SDK must send the caller's request type, not null"
    );
    assert_eq!(
        state.poll_count, 0,
        "a node that confirmed local execution must not be polled"
    );
}

#[tokio::test]
async fn falls_back_to_polling_when_local_execution_is_not_confirmed() {
    // A node that answers `WaitForLocalExecution` without confirming it —
    // an older node, or one that could not execute locally in time.
    let node = start_mock_node(Some(false), 0).await;
    let client = node.client().await;

    let tx = sample_transaction();
    let digest = *tx.digest();
    let response = client
        .quorum_driver_api()
        .execute_transaction_block(
            tx,
            IotaTransactionBlockResponseOptions::new().with_effects(),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await
        .unwrap();

    let state = node.state();
    assert_eq!(
        state.execute_request_types,
        vec![Some("WaitForLocalExecution".to_owned())]
    );
    assert_eq!(state.poll_count, 1, "the fallback poll must still run");
    assert_eq!(
        state.poll_digests,
        vec![digest],
        "the fallback must poll for the transaction it just submitted"
    );
    assert_eq!(
        response.confirmed_local_execution,
        Some(true),
        "a successful fallback reports the same guarantee as the fast path"
    );
    // The poll is a barrier, not a data source. The mock's read
    // response carries a checkpoint; the returned response must not, because
    // it is the execute response.
    assert_eq!(
        response.checkpoint, None,
        "the fallback must return the execute response, not the read response"
    );
    assert_eq!(response.timestamp_ms, None);
}

#[tokio::test]
async fn falls_back_to_polling_when_the_node_omits_the_field() {
    // A node that ignores the request type entirely.
    let node = start_mock_node(None, 0).await;
    let client = node.client().await;

    client
        .quorum_driver_api()
        .execute_transaction_block(
            sample_transaction(),
            IotaTransactionBlockResponseOptions::new().with_effects(),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await
        .unwrap();

    assert_eq!(node.state().poll_count, 1);
}

#[tokio::test]
async fn fallback_retries_past_reads_that_are_not_yet_visible() {
    // A node that does not confirm local execution and whose first two reads
    // also fail — the transaction has not reached the read path yet. The
    // fallback must swallow those errors and keep polling rather than
    // surfacing the first failure to the caller.
    let node = start_mock_node(Some(false), 2).await;
    let client = node.client().await;

    let response = client
        .quorum_driver_api()
        .execute_transaction_block(
            sample_transaction(),
            IotaTransactionBlockResponseOptions::new().with_effects(),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await
        .unwrap();

    let state = node.state();
    assert!(
        state.poll_count > 1,
        "the SDK must retry past failed reads instead of giving up on the first one, got {}",
        state.poll_count
    );
    assert_eq!(response.confirmed_local_execution, Some(true));
}

#[tokio::test]
async fn wait_for_effects_cert_never_polls() {
    let node = start_mock_node(Some(false), 0).await;
    let client = node.client().await;

    let response = client
        .quorum_driver_api()
        .execute_transaction_block(
            sample_transaction(),
            IotaTransactionBlockResponseOptions::new().with_effects(),
            Some(ExecuteTransactionRequestType::WaitForEffectsCert),
        )
        .await
        .unwrap();

    let state = node.state();
    assert_eq!(
        state.execute_request_types,
        vec![Some("WaitForEffectsCert".to_owned())]
    );
    assert_eq!(state.poll_count, 0);
    assert_eq!(
        response.confirmed_local_execution,
        Some(false),
        "WaitForEffectsCert must report Some(false), not the WaitForLocalExecution guarantee"
    );
}

#[tokio::test]
async fn resolves_the_default_request_type_from_the_options() {
    // `with_effects()` resolves to `WaitForLocalExecution`; bare options
    // resolve to `WaitForEffectsCert`.
    let node = start_mock_node(Some(true), 0).await;
    let client = node.client().await;

    client
        .quorum_driver_api()
        .execute_transaction_block(
            sample_transaction(),
            IotaTransactionBlockResponseOptions::new().with_effects(),
            None::<ExecuteTransactionRequestType>,
        )
        .await
        .unwrap();

    client
        .quorum_driver_api()
        .execute_transaction_block(
            sample_transaction(),
            IotaTransactionBlockResponseOptions::new(),
            None::<ExecuteTransactionRequestType>,
        )
        .await
        .unwrap();

    assert_eq!(
        node.state().execute_request_types,
        vec![
            Some("WaitForLocalExecution".to_owned()),
            Some("WaitForEffectsCert".to_owned()),
        ]
    );
}
