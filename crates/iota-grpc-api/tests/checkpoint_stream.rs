// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashMap, sync::Arc};

use iota_grpc_api::{
    CheckpointGrpcService,
    checkpoint::{StreamRequest, checkpoint_service_server::CheckpointService},
};
use iota_types::{
    base_types::{IotaAddress, ObjectID},
    committee::EpochId,
    crypto::AuthorityStrongQuorumSignInfo,
    messages_checkpoint::{
        CertifiedCheckpointSummary, CheckpointContents, CheckpointSequenceNumber, CheckpointSummary,
    },
    storage::{AccountOwnedObjectInfo, CoinInfo, RestStateReader, error::Result as StorageResult},
};
use tokio_stream::StreamExt;
use tonic::Request;

struct MockRestStateReader {
    checkpoints: Arc<
        std::sync::Mutex<
            HashMap<CheckpointSequenceNumber, (CertifiedCheckpointSummary, CheckpointContents)>,
        >,
    >,
}

// Minimal empty trait impls to satisfy RestStateReader supertraits
impl iota_types::storage::ObjectStore for MockRestStateReader {
    fn get_object(
        &self,
        _id: &ObjectID,
    ) -> iota_types::storage::error::Result<Option<iota_types::object::Object>> {
        unimplemented!()
    }
    fn get_object_by_key(
        &self,
        _id: &ObjectID,
        _version: iota_types::base_types::SequenceNumber,
    ) -> iota_types::storage::error::Result<Option<iota_types::object::Object>> {
        unimplemented!()
    }
}
impl iota_types::storage::ReadStore for MockRestStateReader {
    fn get_committee(
        &self,
        _epoch: EpochId,
    ) -> iota_types::storage::error::Result<Option<std::sync::Arc<iota_types::committee::Committee>>>
    {
        unimplemented!()
    }
    fn get_latest_checkpoint(
        &self,
    ) -> iota_types::storage::error::Result<
        iota_types::message_envelope::VerifiedEnvelope<
            iota_types::messages_checkpoint::CheckpointSummary,
            iota_types::crypto::AuthorityQuorumSignInfo<true>,
        >,
    > {
        // Return the checkpoint with the highest sequence number
        let guard = self.checkpoints.lock().unwrap();
        if let Some(max_seq) = guard.keys().max().cloned() {
            let (summary, _) = guard.get(&max_seq).unwrap();
            Ok(iota_types::message_envelope::VerifiedEnvelope::new_unchecked(summary.clone()))
        } else {
            // Use the missing error constructor
            Err(iota_types::storage::error::Error::missing(
                "No checkpoints available",
            ))
        }
    }
    fn get_highest_verified_checkpoint(
        &self,
    ) -> iota_types::storage::error::Result<
        iota_types::message_envelope::VerifiedEnvelope<
            iota_types::messages_checkpoint::CheckpointSummary,
            iota_types::crypto::AuthorityQuorumSignInfo<true>,
        >,
    > {
        unimplemented!()
    }
    fn get_highest_synced_checkpoint(
        &self,
    ) -> iota_types::storage::error::Result<
        iota_types::message_envelope::VerifiedEnvelope<
            iota_types::messages_checkpoint::CheckpointSummary,
            iota_types::crypto::AuthorityQuorumSignInfo<true>,
        >,
    > {
        unimplemented!()
    }
    fn get_lowest_available_checkpoint(&self) -> iota_types::storage::error::Result<u64> {
        unimplemented!()
    }
    fn get_checkpoint_by_digest(
        &self,
        _digest: &iota_types::messages_checkpoint::CheckpointDigest,
    ) -> iota_types::storage::error::Result<
        Option<
            iota_types::message_envelope::VerifiedEnvelope<
                iota_types::messages_checkpoint::CheckpointSummary,
                iota_types::crypto::AuthorityQuorumSignInfo<true>,
            >,
        >,
    > {
        unimplemented!()
    }
    fn get_checkpoint_by_sequence_number(
        &self,
        seq: CheckpointSequenceNumber,
    ) -> iota_types::storage::error::Result<
        Option<
            iota_types::message_envelope::VerifiedEnvelope<
                iota_types::messages_checkpoint::CheckpointSummary,
                iota_types::crypto::AuthorityQuorumSignInfo<true>,
            >,
        >,
    > {
        println!(
            "Mock get_checkpoint_by_sequence_number called for seq: {}",
            seq
        );
        let guard = self.checkpoints.lock().unwrap();
        if seq == u64::MAX {
            // Return the highest checkpoint
            if let Some(max_seq) = guard.keys().max().cloned() {
                println!("[READER] Returning highest checkpoint {}", max_seq);
                return Ok(guard.get(&max_seq).map(|(c, _)| {
                    iota_types::message_envelope::VerifiedEnvelope::new_unchecked(c.clone())
                }));
            } else {
                return Ok(None);
            }
        }
        let found = guard.get(&seq).map(|(c, _)| {
            println!("[READER] Returning checkpoint {}", seq);
            iota_types::message_envelope::VerifiedEnvelope::new_unchecked(c.clone())
        });
        Ok(found)
    }
    fn get_checkpoint_contents_by_digest(
        &self,
        _digest: &iota_types::messages_checkpoint::CheckpointContentsDigest,
    ) -> iota_types::storage::error::Result<Option<CheckpointContents>> {
        unimplemented!()
    }
    fn get_checkpoint_contents_by_sequence_number(
        &self,
        seq: CheckpointSequenceNumber,
    ) -> iota_types::storage::error::Result<Option<CheckpointContents>> {
        let guard = self.checkpoints.lock().unwrap();
        Ok(guard.get(&seq).map(|(_, contents)| contents.clone()))
    }
    fn get_transaction(
        &self,
        _digest: &iota_types::digests::TransactionDigest,
    ) -> iota_types::storage::error::Result<
        Option<
            std::sync::Arc<
                iota_types::message_envelope::VerifiedEnvelope<
                    iota_types::transaction::SenderSignedData,
                    iota_types::crypto::EmptySignInfo,
                >,
            >,
        >,
    > {
        unimplemented!()
    }
    fn get_transaction_effects(
        &self,
        _digest: &iota_types::digests::TransactionDigest,
    ) -> iota_types::storage::error::Result<Option<iota_types::effects::TransactionEffects>> {
        unimplemented!()
    }
    fn get_events(
        &self,
        _digest: &iota_types::digests::TransactionEventsDigest,
    ) -> iota_types::storage::error::Result<Option<iota_types::effects::TransactionEvents>> {
        unimplemented!()
    }
    fn get_full_checkpoint_contents_by_sequence_number(
        &self,
        _seq: CheckpointSequenceNumber,
    ) -> iota_types::storage::error::Result<
        Option<iota_types::messages_checkpoint::FullCheckpointContents>,
    > {
        unimplemented!()
    }
    fn get_full_checkpoint_contents(
        &self,
        _digest: &iota_types::messages_checkpoint::CheckpointContentsDigest,
    ) -> iota_types::storage::error::Result<
        Option<iota_types::messages_checkpoint::FullCheckpointContents>,
    > {
        unimplemented!()
    }
}

impl RestStateReader for MockRestStateReader {
    fn get_transaction_checkpoint(
        &self,
        _: &iota_types::digests::TransactionDigest,
    ) -> StorageResult<Option<CheckpointSequenceNumber>> {
        unimplemented!()
    }
    fn get_lowest_available_checkpoint_objects(&self) -> StorageResult<CheckpointSequenceNumber> {
        Ok(0)
    }
    fn get_chain_identifier(&self) -> StorageResult<iota_types::digests::ChainIdentifier> {
        unimplemented!()
    }
    fn account_owned_objects_info_iter(
        &self,
        _: IotaAddress,
        _: Option<ObjectID>,
    ) -> StorageResult<Box<dyn Iterator<Item = AccountOwnedObjectInfo> + '_>> {
        unimplemented!()
    }
    fn dynamic_field_iter(
        &self,
        _: ObjectID,
        _: Option<ObjectID>,
    ) -> StorageResult<
        Box<
            dyn Iterator<
                    Item = (
                        iota_types::storage::DynamicFieldKey,
                        iota_types::storage::DynamicFieldIndexInfo,
                    ),
                > + '_,
        >,
    > {
        unimplemented!()
    }
    fn get_coin_info(
        &self,
        _: &move_core_types::language_storage::StructTag,
    ) -> StorageResult<Option<CoinInfo>> {
        unimplemented!()
    }
    fn get_epoch_last_checkpoint(
        &self,
        _: EpochId,
    ) -> StorageResult<Option<iota_types::messages_checkpoint::VerifiedCheckpoint>> {
        unimplemented!()
    }
}

async fn test_service() -> CheckpointGrpcService {
    let checkpoints = (0..=10)
        .map(|i| {
            let contents = CheckpointContents::new_with_digests_only_for_tests(vec![]);
            let summary = CheckpointSummary {
                epoch: 0,
                sequence_number: i,
                network_total_transactions: 0,
                content_digest: *contents.digest(),
                previous_digest: None,
                epoch_rolling_gas_cost_summary: Default::default(),
                timestamp_ms: 0,
                checkpoint_commitments: vec![],
                end_of_epoch_data: None,
                version_specific_data: vec![],
            };
            let sig = AuthorityStrongQuorumSignInfo {
                epoch: 0,
                signature: Default::default(),
                signers_map: Default::default(),
            };
            let cert = CertifiedCheckpointSummary::new_from_data_and_sig(summary, sig);
            (i, (cert, contents))
        })
        .collect::<HashMap<_, _>>();
    let mock = Arc::new(MockRestStateReader {
        checkpoints: Arc::new(std::sync::Mutex::new(checkpoints.clone())),
    });
    let (grpc_checkpoint_summary_tx, _) = tokio::sync::broadcast::channel(100);
    let (grpc_checkpoint_data_tx, _) = tokio::sync::broadcast::channel(100);
    CheckpointGrpcService::new(mock, grpc_checkpoint_summary_tx, grpc_checkpoint_data_tx)
}

// Helper function to spawn a background checkpoint sender for summaries and
// data
fn spawn_checkpoint_sender(
    summary_tx: tokio::sync::broadcast::Sender<Arc<CertifiedCheckpointSummary>>,
    data_tx: tokio::sync::broadcast::Sender<
        Arc<iota_types::full_checkpoint_content::CheckpointData>,
    >,
    start_seq: u64,
) {
    tokio::spawn(async move {
        let mut seq = start_seq;
        loop {
            let contents = CheckpointContents::new_with_digests_only_for_tests(vec![]);
            let summary = CheckpointSummary {
                epoch: 0,
                sequence_number: seq,
                network_total_transactions: 0,
                content_digest: *contents.digest(),
                previous_digest: None,
                epoch_rolling_gas_cost_summary: Default::default(),
                timestamp_ms: 0,
                checkpoint_commitments: vec![],
                end_of_epoch_data: None,
                version_specific_data: vec![],
            };
            let sig = AuthorityStrongQuorumSignInfo {
                epoch: 0,
                signature: Default::default(),
                signers_map: Default::default(),
            };
            let cert = CertifiedCheckpointSummary::new_from_data_and_sig(summary, sig);
            let data = iota_types::full_checkpoint_content::CheckpointData {
                checkpoint_summary: cert.clone(),
                checkpoint_contents: contents.clone().into(),
                transactions: vec![],
            };
            let _ = summary_tx.send(Arc::new(cert));
            let _ = data_tx.send(Arc::new(data));
            seq += 1;
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    });
}

#[tokio::test]
async fn test_start_index_only() {
    let svc = test_service().await;

    spawn_checkpoint_sender(
        svc.grpc_checkpoint_summary_tx.clone(),
        svc.grpc_checkpoint_data_tx.clone(),
        11,
    );
    let req = StreamRequest {
        start_index: Some(5),
        end_index: None,
        full: Some(true),
    };
    let mut stream = svc
        .stream_checkpoints(Request::new(req))
        .await
        .unwrap()
        .into_inner();
    println!("Collecting results for test_start_index_only");
    let mut result = Vec::new();
    while let Some(res) = stream.next().await {
        match res {
            Ok(cp) => {
                // Only collect the expected range
                if cp.index > 30 {
                    break;
                }
                result.push(cp.index)
            }
            Err(status) if status.code() == tonic::Code::NotFound => break,
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }
    println!("Result: {:?}", result);
    assert_eq!(result, (5..=30).collect::<Vec<_>>());
}

#[tokio::test]
async fn test_start_and_end_index() {
    let svc = test_service().await;

    spawn_checkpoint_sender(
        svc.grpc_checkpoint_summary_tx.clone(),
        svc.grpc_checkpoint_data_tx.clone(),
        100,
    );
    let req = StreamRequest {
        start_index: Some(3),
        end_index: Some(7),
        full: Some(false),
    };
    let mut stream = svc
        .stream_checkpoints(Request::new(req))
        .await
        .unwrap()
        .into_inner();
    println!("Collecting results for test_start_and_end_index");
    let mut result = Vec::new();
    while let Some(res) = stream.next().await {
        match res {
            Ok(cp) => {
                // Only collect the expected range
                if cp.index > 7 {
                    break;
                }
                result.push(cp.index)
            }
            Err(status) if status.code() == tonic::Code::NotFound => break,
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }
    println!("Result: {:?}", result);
    assert_eq!(result, (3..=7).collect::<Vec<_>>());
}

#[tokio::test]
async fn test_end_index_only() {
    let svc = test_service().await;

    spawn_checkpoint_sender(
        svc.grpc_checkpoint_summary_tx.clone(),
        svc.grpc_checkpoint_data_tx.clone(),
        100,
    );
    let req = StreamRequest {
        start_index: None,
        end_index: Some(4),
        full: Some(false),
    };
    let mut stream = svc
        .stream_checkpoints(Request::new(req))
        .await
        .unwrap()
        .into_inner();
    println!("Collecting results for test_end_index_only");
    let mut result = Vec::new();
    while let Some(res) = stream.next().await {
        match res {
            Ok(cp) => result.push(cp.index),
            Err(status) if status.code() == tonic::Code::NotFound => break,
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }
    println!("Result: {:?}", result);
    assert_eq!(result, vec![4]);
}

#[tokio::test]
async fn test_future_end_index_only_full() {
    let svc = test_service().await;
    spawn_checkpoint_sender(
        svc.grpc_checkpoint_summary_tx.clone(),
        svc.grpc_checkpoint_data_tx.clone(),
        11,
    );

    let req = StreamRequest {
        start_index: None,
        end_index: Some(100),
        full: Some(true),
    };
    let mut stream = svc
        .stream_checkpoints(Request::new(req))
        .await
        .unwrap()
        .into_inner();
    println!("Collecting results for test_end_index_only_full");
    let mut result = Vec::new();
    while let Some(res) = stream.next().await {
        match res {
            Ok(cp) => {
                let checkpoint_data: iota_types::full_checkpoint_content::CheckpointData =
                    bcs::from_bytes(&cp.data).expect("bcs decode");
                result.push(checkpoint_data.checkpoint_summary.sequence_number);
                break;
            }
            Err(status) if status.code() == tonic::Code::NotFound => break,
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }
    println!("Result: {:?}", result);
    assert_eq!(result, vec![100]);
}

#[tokio::test]
async fn test_both_indices_omitted() {
    let svc = test_service().await;
    let req = StreamRequest {
        start_index: None,
        end_index: None,
        full: Some(false),
    };

    // Subscribe to the stream after buffer is pre-filled (0..=10)
    let mut stream = svc
        .stream_checkpoints(Request::new(req))
        .await
        .unwrap()
        .into_inner();
    println!("Collecting results for test_both_indices_omitted");
    let mut result = Vec::new();

    // Now send new checkpoints (live) after subscribing
    spawn_checkpoint_sender(
        svc.grpc_checkpoint_summary_tx.clone(),
        svc.grpc_checkpoint_data_tx.clone(),
        11,
    );

    // Collect enough checkpoints to see both buffered and live ones
    for _ in 0..15 {
        if let Some(res) = stream.next().await {
            match res {
                Ok(cp) => result.push(cp.index),
                Err(status) if status.code() == tonic::Code::NotFound => break,
                Err(e) => panic!("Unexpected error: {:?}", e),
            }
        }
    }
    println!("Result: {:?}", result);
    // The first 11 should be 0..=10 (buffered), then live ones (11, 12, ...)
    assert_eq!(&result[..], &(10..=24).collect::<Vec<_>>()[..]);
}

#[tokio::test]
async fn test_historical_to_live_gap_fill() {
    use std::sync::Arc;

    use iota_types::{
        full_checkpoint_content::CheckpointData, messages_checkpoint::CertifiedCheckpointSummary,
    };
    use tokio::sync::broadcast;

    // Simulate storage with checkpoints 0..=150
    let mut checkpoints = std::collections::HashMap::new();
    for i in 0..=150u64 {
        let contents = CheckpointContents::new_with_digests_only_for_tests(vec![]);
        let summary = CheckpointSummary {
            epoch: 0,
            sequence_number: i,
            network_total_transactions: 0,
            content_digest: *contents.digest(),
            previous_digest: None,
            epoch_rolling_gas_cost_summary: Default::default(),
            timestamp_ms: 0,
            checkpoint_commitments: vec![],
            end_of_epoch_data: None,
            version_specific_data: vec![],
        };
        let sig = AuthorityStrongQuorumSignInfo {
            epoch: 0,
            signature: Default::default(),
            signers_map: Default::default(),
        };
        let cert = CertifiedCheckpointSummary::new_from_data_and_sig(summary, sig);
        checkpoints.insert(i, (cert, contents));
    }
    let mock = Arc::new(MockRestStateReader {
        checkpoints: Arc::new(std::sync::Mutex::new(checkpoints.clone())),
    });
    let (grpc_checkpoint_summary_tx, _) = broadcast::channel(10);
    let (grpc_checkpoint_data_tx, _) = broadcast::channel(10);
    let svc = CheckpointGrpcService::new(
        mock.clone(),
        grpc_checkpoint_summary_tx.clone(),
        grpc_checkpoint_data_tx.clone(),
    );

    // Simulate broadcast channel at 150
    let (summary_150, contents_150) = checkpoints.get(&150).unwrap();
    let data_150 = CheckpointData {
        checkpoint_summary: summary_150.clone(),
        checkpoint_contents: contents_150.clone().into(),
        transactions: vec![],
    };
    let _ = grpc_checkpoint_summary_tx.send(Arc::new(summary_150.clone()));
    let _ = grpc_checkpoint_data_tx.send(Arc::new(data_150));

    // Client requests from 0 (historical)
    let req = StreamRequest {
        start_index: Some(0),
        end_index: None,
        full: Some(true),
    };
    let mut stream = svc
        .stream_checkpoints(Request::new(req))
        .await
        .unwrap()
        .into_inner();
    let mut received = Vec::new();
    // Collect up to 151 checkpoints
    while let Some(Ok(cp)) = stream.next().await {
        let checkpoint_data: CheckpointData = bcs::from_bytes(&cp.data).expect("bcs decode");
        received.push(checkpoint_data.checkpoint_summary.sequence_number);
        if checkpoint_data.checkpoint_summary.sequence_number == 150 {
            break;
        }
    }
    // Assert we got all checkpoints 0..=150
    assert_eq!(received, (0..=150u64).collect::<Vec<_>>());
}

#[tokio::test(flavor = "current_thread")]
async fn test_gap_fill_with_slow_client() {
    use std::{sync::Arc, time::Duration};

    use iota_types::{
        crypto::AuthorityStrongQuorumSignInfo,
        full_checkpoint_content::CheckpointData,
        messages_checkpoint::{CertifiedCheckpointSummary, CheckpointContents, CheckpointSummary},
    };
    use tokio::sync::broadcast;

    let checkpoints = Arc::new(std::sync::Mutex::new(HashMap::new()));
    let mock = Arc::new(MockRestStateReader {
        checkpoints: checkpoints.clone(),
    });
    let (grpc_checkpoint_summary_tx, _) = broadcast::channel(5);
    let (grpc_checkpoint_data_tx, _) = broadcast::channel(5);
    let svc = CheckpointGrpcService::new(
        mock.clone(),
        grpc_checkpoint_summary_tx.clone(),
        grpc_checkpoint_data_tx.clone(),
    );

    // Pre-populate storage with checkpoints 0..=10 before spawning the producer and
    // client
    for i in 0..=10u64 {
        let contents = CheckpointContents::new_with_digests_only_for_tests(vec![]);
        let summary = CheckpointSummary {
            epoch: 0,
            sequence_number: i,
            network_total_transactions: 0,
            content_digest: *contents.digest(),
            previous_digest: None,
            epoch_rolling_gas_cost_summary: Default::default(),
            timestamp_ms: 0,
            checkpoint_commitments: vec![],
            end_of_epoch_data: None,
            version_specific_data: vec![],
        };
        let sig = AuthorityStrongQuorumSignInfo {
            epoch: 0,
            signature: Default::default(),
            signers_map: Default::default(),
        };
        let cert = CertifiedCheckpointSummary::new_from_data_and_sig(summary, sig);
        checkpoints
            .lock()
            .unwrap()
            .insert(i, (cert.clone(), contents.clone()));
    }

    // Producer: generates checkpoints 0..=20, one every 100ms
    tokio::spawn({
        let checkpoints = checkpoints.clone();
        let grpc_checkpoint_summary_tx = grpc_checkpoint_summary_tx.clone();
        let grpc_checkpoint_data_tx = grpc_checkpoint_data_tx.clone();
        async move {
            for i in 11..=200u64 {
                let contents = CheckpointContents::new_with_digests_only_for_tests(vec![]);
                let summary = CheckpointSummary {
                    epoch: 0,
                    sequence_number: i,
                    network_total_transactions: 0,
                    content_digest: *contents.digest(),
                    previous_digest: None,
                    epoch_rolling_gas_cost_summary: Default::default(),
                    timestamp_ms: 0,
                    checkpoint_commitments: vec![],
                    end_of_epoch_data: None,
                    version_specific_data: vec![],
                };
                let sig = AuthorityStrongQuorumSignInfo {
                    epoch: 0,
                    signature: Default::default(),
                    signers_map: Default::default(),
                };
                let cert = CertifiedCheckpointSummary::new_from_data_and_sig(summary, sig);
                checkpoints
                    .lock()
                    .unwrap()
                    .insert(i, (cert.clone(), contents.clone()));
                println!("[gRPC] Producer inserted checkpoint {}", i);
                let data = CheckpointData {
                    checkpoint_summary: cert.clone(),
                    checkpoint_contents: contents.clone().into(),
                    transactions: vec![],
                };
                let _ = grpc_checkpoint_summary_tx.send(Arc::new(cert.clone()));
                let _ = grpc_checkpoint_data_tx.send(Arc::new(data));
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    });

    // Client: slow consumer
    let req = StreamRequest {
        start_index: Some(0),
        end_index: None,
        full: Some(true),
    };
    let mut stream = svc
        .stream_checkpoints(Request::new(req))
        .await
        .unwrap()
        .into_inner();
    let mut received = Vec::new();
    while let Some(Ok(cp)) = stream.next().await {
        let checkpoint_data: CheckpointData = bcs::from_bytes(&cp.data).expect("bcs decode");
        received.push(checkpoint_data.checkpoint_summary.sequence_number);
        tokio::time::sleep(Duration::from_millis(500)).await; // slow down the client
        println!(
            "[gRPC] Client gets Checkpoint {:?}",
            checkpoint_data.checkpoint_summary.sequence_number
        );
        if checkpoint_data.checkpoint_summary.sequence_number == 20 {
            break;
        }
    }
    assert_eq!(received, (0..=20u64).collect::<Vec<_>>());
}
