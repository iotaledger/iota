// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::time::Duration;

use futures::StreamExt;
use iota_grpc_types::{
    field::FieldMaskUtil,
    read_masks::GET_CHECKPOINT_READ_MASK,
    v1::{
        filter as grpc_filter,
        ledger_service::{
            CheckpointData, GetCheckpointRequest, StreamCheckpointsRequest, checkpoint_data,
            ledger_service_client::LedgerServiceClient,
        },
        types as grpc_types,
    },
};
use iota_macros::sim_test;
use iota_types::transaction::CallArg;
use prost_types::FieldMask;
use tokio::time::timeout;

use crate::utils::{
    BASICS_PACKAGE, CLOCK_ACCESS_FUNCTION, CLOCK_MODULE, FieldPresenceChecker, NFT_MINTED_EVENT,
    NFT_MODULE, NFT_PACKAGE, assert_field_presence, assert_tonic_error,
    comma_separated_field_mask_to_paths, publish_example_package, setup_grpc_test,
};

/// `sequence_number` is always populated by the server and not controlled by
/// the read_mask, so field-presence checks must ignore it.
const CHECKPOINT_IGNORED_FIELDS: &[&str] = &["sequence_number"];

/// `json_contents` is populated only when the server can resolve the Move
/// event's type information, which is not guaranteed for system or
/// test-fixture events. Skip it in field-presence checks so tests stay stable
/// regardless of which events happen to land in the checkpoint.
const EVENT_IGNORED_FIELDS: &[&str] = &["json_contents"];

/// Paths applicable to one `CheckpointData` payload variant.
///
/// `wildcard = true` means the caller requested every field (via a bare
/// `"checkpoint"` / `"transactions"` / `"events"` entry in the read mask).
/// The concrete field list is then expanded at validation time from the
/// checker's `top_level_fields()`, keeping the field list authoritative in
/// one place (the `impl_field_presence_checker!` macro calls in `v1/mod.rs`).
#[derive(Default)]
struct PayloadPaths<'a> {
    wildcard: bool,
    paths: Vec<&'a str>,
}

/// Categorize read-mask paths under `CheckpointData`'s top-level payload
/// fields (`checkpoint.*`, `transactions.*`, `events.*`). Bare prefixes
/// record a wildcard to be expanded at validation time.
fn split_checkpoint_paths<'a>(
    expected: &[&'a str],
) -> (PayloadPaths<'a>, PayloadPaths<'a>, PayloadPaths<'a>) {
    let mut checkpoint = PayloadPaths::default();
    let mut transactions = PayloadPaths::default();
    let mut events = PayloadPaths::default();

    for path in expected {
        if let Some(sub) = path.strip_prefix("checkpoint.") {
            checkpoint.paths.push(sub);
        } else if *path == "checkpoint" {
            checkpoint.wildcard = true;
        } else if let Some(sub) = path.strip_prefix("transactions.") {
            transactions.paths.push(sub);
        } else if *path == "transactions" {
            transactions.wildcard = true;
        } else if let Some(sub) = path.strip_prefix("events.") {
            events.paths.push(sub);
        } else if *path == "events" {
            events.wildcard = true;
        } else {
            panic!(
                "split_checkpoint_paths: unrecognized CheckpointData path {path:?} — \
                 valid top-level prefixes are \"checkpoint\", \"transactions\", \"events\""
            );
        }
    }

    (checkpoint, transactions, events)
}

/// Resolve a `PayloadPaths` bucket to the concrete field list for
/// [`assert_field_presence`]. A wildcard expands to every top-level field of
/// `checker` (minus `ignored`); otherwise the explicit dotted sub-paths are
/// returned as-is.
fn resolve_payload_paths<'a>(
    payload: &PayloadPaths<'a>,
    checker: &dyn FieldPresenceChecker,
    ignored: &[&str],
) -> Vec<&'a str> {
    if payload.wildcard {
        checker
            .top_level_fields()
            .iter()
            .copied()
            .filter(|f| !ignored.contains(f))
            .collect()
    } else {
        payload.paths.clone()
    }
}

/// Validate a single `CheckpointData` payload against the expected field
/// presence under each top-level `CheckpointData` payload type. Returns `true`
/// when an `EndMarker` is observed (signalling the end of a checkpoint's
/// data).
fn validate_checkpoint_payload(
    data: &CheckpointData,
    checkpoint_paths: &PayloadPaths<'_>,
    transactions_paths: &PayloadPaths<'_>,
    events_paths: &PayloadPaths<'_>,
    expected_end_marker_seq: &mut Option<u64>,
    scenario: &str,
) -> bool {
    match data.payload.as_ref() {
        Some(checkpoint_data::Payload::Checkpoint(checkpoint)) => {
            assert!(
                expected_end_marker_seq.is_none(),
                "{scenario}: received Checkpoint before previous EndMarker"
            );
            *expected_end_marker_seq = checkpoint.sequence_number;
            let paths =
                resolve_payload_paths(checkpoint_paths, checkpoint, CHECKPOINT_IGNORED_FIELDS);
            assert_field_presence(
                checkpoint,
                &paths,
                CHECKPOINT_IGNORED_FIELDS,
                &format!("{scenario}: checkpoint payload"),
            );
            false
        }
        Some(checkpoint_data::Payload::ExecutedTransactions(txs)) => {
            for (idx, tx) in txs.executed_transactions.iter().enumerate() {
                let paths = resolve_payload_paths(transactions_paths, tx, &[]);
                assert_field_presence(
                    tx,
                    &paths,
                    &[],
                    &format!("{scenario}: transactions payload (tx {idx})"),
                );
            }
            false
        }
        Some(checkpoint_data::Payload::Events(events)) => {
            for (idx, event) in events.events.iter().enumerate() {
                let paths = resolve_payload_paths(events_paths, event, EVENT_IGNORED_FIELDS);
                assert_field_presence(
                    event,
                    &paths,
                    EVENT_IGNORED_FIELDS,
                    &format!("{scenario}: events payload (event {idx})"),
                );
            }
            false
        }
        Some(checkpoint_data::Payload::EndMarker(marker)) => {
            assert_eq!(
                marker.sequence_number,
                expected_end_marker_seq.take(),
                "{scenario}: EndMarker sequence_number should match Checkpoint"
            );
            true
        }
        Some(checkpoint_data::Payload::Progress(_)) => {
            panic!("{scenario}: unexpected Progress payload outside filter_checkpoints mode");
        }
        Some(_) => panic!("{scenario}: unknown CheckpointData payload variant"),
        None => panic!("{scenario}: unexpected empty payload"),
    }
}

/// Helper to issue a `GetCheckpointRequest` and validate every payload in the
/// response stream against `expected_field_mask_paths` (paths relative to
/// `CheckpointData`). Returns all received messages.
///
/// Asserts that the stream produces exactly one `Checkpoint` payload followed
/// by optional `ExecutedTransactions` / `Events` payloads and terminates with
/// a single `EndMarker`.
async fn assert_get_checkpoint_request(
    ledger_client: &mut LedgerServiceClient<iota_grpc_client::InterceptedChannel>,
    request: GetCheckpointRequest,
    expected_field_mask_paths: &[&str],
    scenario: &str,
) -> Vec<CheckpointData> {
    let (checkpoint_paths, transactions_paths, events_paths) =
        split_checkpoint_paths(expected_field_mask_paths);

    let mut stream = ledger_client
        .get_checkpoint(request)
        .await
        .unwrap()
        .into_inner();

    let mut messages = Vec::new();
    let mut saw_end_marker = false;
    let mut pending_end_marker_seq: Option<u64> = None;

    while let Some(data) = stream.next().await {
        let data = data.unwrap();
        let ended = validate_checkpoint_payload(
            &data,
            &checkpoint_paths,
            &transactions_paths,
            &events_paths,
            &mut pending_end_marker_seq,
            scenario,
        );
        messages.push(data);
        if ended {
            saw_end_marker = true;
            break;
        }
    }

    assert!(saw_end_marker, "{scenario}: expected EndMarker payload");
    assert!(
        stream.next().await.is_none(),
        "{scenario}: stream should be exhausted after EndMarker"
    );

    messages
}

/// Helper to issue a `StreamCheckpointsRequest` and validate every payload
/// across the full range of expected checkpoints. Asserts that exactly
/// `expected_checkpoint_count` checkpoints are returned, each with a matching
/// `EndMarker`.
async fn assert_stream_checkpoints_request(
    ledger_client: &mut LedgerServiceClient<iota_grpc_client::InterceptedChannel>,
    request: StreamCheckpointsRequest,
    expected_field_mask_paths: &[&str],
    expected_checkpoint_count: usize,
    scenario: &str,
) -> Vec<CheckpointData> {
    let (checkpoint_paths, transactions_paths, events_paths) =
        split_checkpoint_paths(expected_field_mask_paths);

    let mut stream = ledger_client
        .stream_checkpoints(request)
        .await
        .unwrap()
        .into_inner();

    let mut messages = Vec::new();
    let mut checkpoint_count = 0usize;
    let mut pending_end_marker_seq: Option<u64> = None;

    while let Some(data) = stream.next().await {
        let data = data.unwrap();
        let ended = validate_checkpoint_payload(
            &data,
            &checkpoint_paths,
            &transactions_paths,
            &events_paths,
            &mut pending_end_marker_seq,
            &format!("{scenario} (checkpoint #{checkpoint_count})"),
        );
        messages.push(data);
        if ended {
            checkpoint_count += 1;
            if checkpoint_count >= expected_checkpoint_count {
                break;
            }
        }
    }

    assert_eq!(
        checkpoint_count, expected_checkpoint_count,
        "{scenario}: expected {expected_checkpoint_count} checkpoints, got {checkpoint_count}"
    );

    messages
}

#[sim_test]
async fn get_checkpoint_readmask_scenarios() {
    let (_test_cluster, client) = setup_grpc_test(Some(2), None).await;
    let mut ledger_client = client.ledger_service_client();

    type TestCase<'a> = (&'a str, Option<FieldMask>, Vec<&'a str>);
    let test_cases: Vec<TestCase> = vec![
        (
            "default readmask",
            None,
            comma_separated_field_mask_to_paths(GET_CHECKPOINT_READ_MASK),
        ),
        (
            "empty readmask",
            Some(FieldMask::from_paths(&[] as &[&str])),
            vec![],
        ),
        (
            "checkpoint.summary only",
            Some(FieldMask::from_paths(["checkpoint.summary"])),
            vec!["checkpoint.summary"],
        ),
        (
            "checkpoint.contents only",
            Some(FieldMask::from_paths(["checkpoint.contents"])),
            vec!["checkpoint.contents"],
        ),
        (
            "checkpoint wildcard",
            Some(FieldMask::from_paths(["checkpoint"])),
            vec!["checkpoint"],
        ),
        (
            "partial readmask (summary.digest only)",
            Some(FieldMask::from_paths(["checkpoint.summary.digest"])),
            vec!["checkpoint.summary.digest"],
        ),
        (
            "transactions partial (transaction.digest + timestamp)",
            Some(FieldMask::from_paths([
                "transactions.transaction.digest",
                "transactions.timestamp",
            ])),
            vec!["transactions.transaction.digest", "transactions.timestamp"],
        ),
        (
            "full readmask",
            Some(FieldMask::from_paths([
                "checkpoint",
                "transactions",
                "events",
            ])),
            vec!["checkpoint", "transactions", "events"],
        ),
    ];

    for (scenario, mask, expected_paths) in test_cases {
        let mut request = GetCheckpointRequest::default().with_sequence_number(0);
        if let Some(mask) = mask {
            request = request.with_read_mask(mask);
        }
        assert_get_checkpoint_request(&mut ledger_client, request, &expected_paths, scenario).await;
    }
}

#[sim_test]
async fn get_checkpoint_by_digest() {
    let (_test_cluster, client) = setup_grpc_test(Some(2), None).await;
    let mut ledger_client = client.ledger_service_client();

    // Step 1: fetch genesis (sequence 0) to learn its summary digest.
    let by_seq = assert_get_checkpoint_request(
        &mut ledger_client,
        GetCheckpointRequest::default()
            .with_sequence_number(0)
            .with_read_mask(FieldMask::from_paths(["checkpoint.summary.digest"])),
        &["checkpoint.summary.digest"],
        "by-sequence for digest lookup",
    )
    .await;

    let digest = by_seq
        .iter()
        .find_map(|msg| match msg.payload.as_ref() {
            Some(checkpoint_data::Payload::Checkpoint(cp)) => {
                cp.summary.as_ref().and_then(|s| s.digest.clone())
            }
            _ => None,
        })
        .expect("genesis checkpoint should have a summary digest");

    // Step 2: fetch the same checkpoint by digest and confirm the sequence
    // number round-trips.
    let by_digest = assert_get_checkpoint_request(
        &mut ledger_client,
        GetCheckpointRequest::default()
            .with_digest(digest)
            .with_read_mask(FieldMask::from_paths(["checkpoint.summary.digest"])),
        &["checkpoint.summary.digest"],
        "by-digest lookup",
    )
    .await;

    let seq = by_digest
        .iter()
        .find_map(|msg| match msg.payload.as_ref() {
            Some(checkpoint_data::Payload::Checkpoint(cp)) => cp.sequence_number,
            _ => None,
        })
        .expect("by-digest response should include a sequence number");
    assert_eq!(seq, 0, "by-digest: expected genesis sequence number");
}

#[sim_test]
async fn get_checkpoint_latest() {
    let (_test_cluster, client) = setup_grpc_test(Some(2), None).await;
    let mut ledger_client = client.ledger_service_client();

    let messages = assert_get_checkpoint_request(
        &mut ledger_client,
        GetCheckpointRequest::default()
            .with_latest(true)
            .with_read_mask(FieldMask::from_paths(["checkpoint.summary"])),
        &["checkpoint.summary"],
        "latest checkpoint",
    )
    .await;

    let seq = messages
        .iter()
        .find_map(|msg| match msg.payload.as_ref() {
            Some(checkpoint_data::Payload::Checkpoint(cp)) => cp.sequence_number,
            _ => None,
        })
        .expect("latest response should include a sequence number");
    assert!(
        seq >= 2,
        "latest sequence should be >= the waited-for checkpoint (2), got {seq}"
    );
}

#[sim_test]
async fn get_checkpoint_nonexistent() {
    let (_test_cluster, client) = setup_grpc_test(Some(2), None).await;
    let mut ledger_client = client.ledger_service_client();

    // Sequence-number lookups: the server opens the stream successfully and
    // yields the NotFound error as the first stream item (the sequence
    // number isn't resolved until the stream starts reading the checkpoint).
    assert_sequence_number_not_found(&mut ledger_client, 999_999_999).await;

    // Probe `latest + 100` by first resolving the current tip through the same
    // streaming endpoint.  An empty read_mask keeps the payload minimal; the
    // server always populates `sequence_number` regardless of the mask.
    let latest_messages = assert_get_checkpoint_request(
        &mut ledger_client,
        GetCheckpointRequest::default()
            .with_latest(true)
            .with_read_mask(FieldMask::from_paths(&[] as &[&str])),
        &[],
        "fetch latest for not-found probe",
    )
    .await;
    let latest = latest_messages
        .iter()
        .find_map(|msg| match msg.payload.as_ref() {
            Some(checkpoint_data::Payload::Checkpoint(cp)) => cp.sequence_number,
            _ => None,
        })
        .expect("latest checkpoint should have sequence_number");
    assert_sequence_number_not_found(&mut ledger_client, latest + 100).await;

    // Digest lookups: the server resolves the digest to a sequence number
    // upfront, so an unknown digest is returned as the initial response
    // error instead.
    let bogus_digest = grpc_types::Digest::default().with_digest(vec![0u8; 32]);
    let result = ledger_client
        .get_checkpoint(GetCheckpointRequest::default().with_digest(bogus_digest))
        .await;
    assert_tonic_error(result, tonic::Code::NotFound, "bogus digest");
}

async fn assert_sequence_number_not_found(
    ledger_client: &mut LedgerServiceClient<iota_grpc_client::InterceptedChannel>,
    sequence_number: u64,
) {
    let mut stream = ledger_client
        .get_checkpoint(GetCheckpointRequest::default().with_sequence_number(sequence_number))
        .await
        .expect("initial response should succeed for sequence-number lookup")
        .into_inner();
    let first = stream
        .next()
        .await
        .expect("stream should yield the NotFound error");
    assert_tonic_error(
        first,
        tonic::Code::NotFound,
        &format!("sequence number {sequence_number}"),
    );
}

#[sim_test]
async fn stream_checkpoints_readmask_scenarios() {
    let (_test_cluster, client) = setup_grpc_test(Some(2), None).await;
    let mut ledger_client = client.ledger_service_client();

    type TestCase<'a> = (&'a str, Option<FieldMask>, Vec<&'a str>);
    let test_cases: Vec<TestCase> = vec![
        (
            "stream default readmask",
            None,
            comma_separated_field_mask_to_paths(GET_CHECKPOINT_READ_MASK),
        ),
        (
            "stream empty readmask",
            Some(FieldMask::from_paths(&[] as &[&str])),
            vec![],
        ),
        (
            "stream checkpoint wildcard",
            Some(FieldMask::from_paths(["checkpoint"])),
            vec!["checkpoint"],
        ),
        (
            "stream transactions partial (effects.digest + timestamp)",
            Some(FieldMask::from_paths([
                "transactions.effects.digest",
                "transactions.timestamp",
            ])),
            vec!["transactions.effects.digest", "transactions.timestamp"],
        ),
        (
            "stream full readmask",
            Some(FieldMask::from_paths([
                "checkpoint",
                "transactions",
                "events",
            ])),
            vec!["checkpoint", "transactions", "events"],
        ),
    ];

    for (scenario, mask, expected_paths) in test_cases {
        let mut request = StreamCheckpointsRequest::default()
            .with_start_sequence_number(0)
            .with_end_sequence_number(2);
        if let Some(mask) = mask {
            request = request.with_read_mask(mask);
        }
        assert_stream_checkpoints_request(
            &mut ledger_client,
            request,
            &expected_paths,
            3, // checkpoints 0, 1, 2
            scenario,
        )
        .await;
    }
}

/// Verify the server closes a bounded `[start, end]` stream after delivering
/// the final `EndMarker`. Guards against two regressions: over-delivery past
/// `end_sequence_number`, and failure to terminate the stream.
#[sim_test]
async fn stream_checkpoints_terminates_on_bounded_range() {
    let (_test_cluster, client) = setup_grpc_test(Some(2), None).await;
    let mut ledger_client = client.ledger_service_client();

    let mut stream = ledger_client
        .stream_checkpoints(
            StreamCheckpointsRequest::default()
                .with_start_sequence_number(0)
                .with_end_sequence_number(2),
        )
        .await
        .unwrap()
        .into_inner();

    let mut end_markers = 0;
    timeout(Duration::from_secs(30), async {
        while let Some(msg) = stream.next().await {
            if matches!(
                msg.unwrap().payload,
                Some(checkpoint_data::Payload::EndMarker(_))
            ) {
                end_markers += 1;
            }
        }
    })
    .await
    .expect("server did not close stream after final EndMarker");

    assert_eq!(end_markers, 3, "expected EndMarker for checkpoints 0, 1, 2");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_event_filtering() {
    let (cluster, client) = setup_grpc_test(None, None).await;

    let sender_1 = cluster.get_address_0();
    let sender_2 = cluster.get_address_1();

    // Publish NFT and basics packages
    let nft_package_id = publish_example_package(&cluster, sender_1, NFT_PACKAGE).await;
    let basics_package_id = publish_example_package(&cluster, sender_1, BASICS_PACKAGE).await;

    // Define event filters for later use
    let sender_filter = grpc_filter::EventFilter::default().with_sender(
        grpc_filter::AddressFilter::default().with_address(
            grpc_types::Address::default().with_address(sender_1.as_bytes().to_vec()),
        ),
    );

    let nft_filter = grpc_filter::EventFilter::default().with_move_event_type(
        grpc_filter::MoveEventTypeFilter::default().with_struct_tag(format!(
            "{nft_package_id}::{NFT_MODULE}::{NFT_MINTED_EVENT}"
        )),
    );

    let any_filter = grpc_filter::EventFilter::default().with_any(
        grpc_filter::AnyEventFilter::default()
            .with_filters(vec![sender_filter.clone(), nft_filter.clone()]),
    );

    // Generate all events first before streaming
    // Generate 2 NFT events from sender_1
    for _i in 0..2 {
        let nft_tx = cluster
            .test_transaction_builder_with_sender(sender_1)
            .await
            .call_nft_create(nft_package_id)
            .build();
        let signed_tx = cluster.sign_transaction(&nft_tx);
        cluster.execute_transaction(signed_tx).await;
    }

    // Generate 1 NFT event from sender_2
    let nft_tx = cluster
        .test_transaction_builder_with_sender(sender_2)
        .await
        .call_nft_create(nft_package_id)
        .build();
    let signed_tx = cluster.sign_transaction(&nft_tx);
    cluster.execute_transaction(signed_tx).await;

    // Generate 1 TimeEvent using clock::access from basics package
    let clock_tx = cluster
        .test_transaction_builder_with_sender(sender_2)
        .await
        .move_call(
            basics_package_id,
            CLOCK_MODULE,
            CLOCK_ACCESS_FUNCTION,
            vec![CallArg::CLOCK_IMMUTABLE],
        )
        .build();
    let signed_tx = cluster.sign_transaction(&clock_tx);
    cluster.execute_transaction(signed_tx).await;

    // Wait for checkpoints to include all transactions
    tokio::time::sleep(Duration::from_millis(1500)).await;

    // get the latest checkpoint sequence number
    let latest_checkpoint_seq = client
        .get_checkpoint_latest(Some(""), None, None)
        .await
        .expect("Failed to get latest checkpoint")
        .body()
        .sequence_number();

    // Test 1: SenderFilter - should receive only events from sender_1
    let mut sender_stream = client
        .stream_checkpoints(
            Some(0),
            Some(latest_checkpoint_seq),
            Some("events"),
            None,
            Some(sender_filter),
        )
        .await
        .expect("Failed to create sender events stream");

    let mut sender_events = Vec::new();
    let result = timeout(Duration::from_secs(5), async {
        while let Some(checkpoint_result) = sender_stream.body_mut().next().await {
            match checkpoint_result {
                Ok(response) => {
                    let events = response.events();
                    for event in events {
                        // Verify BCS serialization integrity
                        assert!(event.bcs_contents.is_some(), "BCS data must be valid");
                        // Verify sender filter logic: only events from sender_1
                        assert!(
                            event
                                .sender
                                .as_ref()
                                .map(|s| s.address.as_ref() == sender_1.as_bytes())
                                .unwrap_or(false),
                            "SenderFilter should only match sender_1 events"
                        );

                        sender_events.push(event.clone());
                    }
                }
                Err(e) => panic!("SenderFilter client error: {e}"),
            }
        }
    })
    .await;

    assert!(result.is_ok(), "SenderFilter should receive events");
    assert_eq!(
        sender_events.len(),
        2,
        "SenderFilter should receive 2 events from sender_1"
    );

    // Test 2: MoveEventTypeFilter - should receive only NFT events
    let mut nft_stream = client
        .stream_checkpoints(
            Some(0),
            Some(latest_checkpoint_seq),
            Some("events"),
            None,
            Some(nft_filter),
        )
        .await
        .expect("Failed to create NFT events stream");

    let mut nft_events = Vec::new();
    let result = timeout(Duration::from_secs(5), async {
        while let Some(checkpoint_result) = nft_stream.body_mut().next().await {
            match checkpoint_result {
                Ok(response) => {
                    let events = response.events();
                    for event in events {
                        // Verify BCS serialization integrity
                        assert!(event.bcs_contents.is_some(), "BCS data must be valid");

                        // Verify NFT filter logic: only NFT events
                        assert_eq!(
                            event.package_id.as_ref().unwrap().object_id.as_ref(),
                            nft_package_id.as_bytes(),
                            "MoveEventTypeFilter should only match NFT package events"
                        );

                        nft_events.push(event.clone());
                    }
                }
                Err(e) => panic!("MoveEventTypeFilter client error: {e}"),
            }
        }
    })
    .await;

    assert!(result.is_ok(), "MoveEventTypeFilter should receive events");
    assert_eq!(
        nft_events.len(),
        3,
        "MoveEventTypeFilter should receive 3 NFT events from both senders"
    );

    // Test 3: AnyEventFilter - should receive sender_1 events, and all NFT events
    let mut any_stream = client
        .stream_checkpoints(
            Some(0),
            Some(latest_checkpoint_seq),
            Some("events"),
            None,
            Some(any_filter),
        )
        .await
        .expect("Failed to create all events stream");

    let mut any_events = Vec::new();
    let result = timeout(Duration::from_secs(5), async {
        while let Some(checkpoint_result) = any_stream.body_mut().next().await {
            match checkpoint_result {
                Ok(response) => {
                    let events = response.events();
                    for event in events {
                        // Verify BCS serialization integrity
                        assert!(event.bcs_contents.is_some());

                        // Verify AnyEventFilter logic: events from sender_1 and NFT events
                        let sender_matches = event
                            .sender
                            .as_ref()
                            .map(|s| s.address.as_ref() == sender_1.as_bytes())
                            .unwrap_or(false);
                        let nft_package_matches = event
                            .package_id
                            .as_ref()
                            .map(|p| p.object_id.as_ref() == nft_package_id.as_bytes())
                            .unwrap_or(false);
                        assert!(
                            sender_matches || nft_package_matches,
                            "AnyEventFilter should receive events from both events: {:?}",
                            event.package_id.as_ref().map(|p| &p.object_id)
                        );

                        any_events.push(event.clone());
                    }
                }
                Err(e) => panic!("AnyEventFilter client error: {e}"),
            }
        }
    })
    .await;

    assert!(result.is_ok(), "AnyEventFilter should receive events");

    // - AnyEventFilter: receives all 3 events from both filters
    //  (2 from sender_1 and 1 NFT from sender_2)
    assert_eq!(
        any_events.len(),
        3,
        "AnyEventFilter should receive all 3 events"
    );
}
