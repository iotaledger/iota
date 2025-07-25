// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::HashSet,
    sync::{Arc, LazyLock, Mutex},
    time::Duration,
};

use iota_grpc_api::{
    CheckpointGrpcService,
    checkpoint::{CheckpointStreamRequest, checkpoint_service_server::CheckpointService},
};
use iota_grpc_types::{
    CertifiedCheckpointSummary as GrpcCertifiedCheckpointSummary,
    CheckpointData as GrpcCheckpointData,
};
use iota_types::{
    base_types::ObjectID,
    committee::EpochId,
    crypto::AuthorityStrongQuorumSignInfo,
    full_checkpoint_content::CheckpointData,
    messages_checkpoint::{
        CertifiedCheckpointSummary, CheckpointContents, CheckpointSequenceNumber,
        CheckpointSummary, VerifiedCheckpoint,
    },
    storage::{RestIndexes, RestStateReader, error::Result as StorageResult},
};
use tokio::sync::broadcast;
use tokio_stream::StreamExt;
use tonic::Request;

struct MockRestStateReader {
    checkpoints: Arc<Mutex<HashSet<CheckpointSequenceNumber>>>,
}
impl MockRestStateReader {
    fn new_from_iter<I: Iterator<Item = u64>>(iter: I) -> Self {
        Self {
            checkpoints: Arc::new(Mutex::new(iter.collect())),
        }
    }
}

static MOCK_CHECKPOINT_CONTENTS: LazyLock<CheckpointContents> =
    LazyLock::new(|| CheckpointContents::new_with_digests_only_for_tests(vec![]));

fn mock_checkpoint_summary(sequence_number: u64) -> CheckpointSummary {
    CheckpointSummary {
        epoch: 0,
        sequence_number,
        network_total_transactions: 0,
        content_digest: *MOCK_CHECKPOINT_CONTENTS.digest(),
        previous_digest: None,
        epoch_rolling_gas_cost_summary: Default::default(),
        timestamp_ms: 0,
        checkpoint_commitments: vec![],
        end_of_epoch_data: None,
        version_specific_data: vec![],
    }
}

fn mock_summary(sequence_number: u64) -> CertifiedCheckpointSummary {
    let summary = mock_checkpoint_summary(sequence_number);
    let sig = AuthorityStrongQuorumSignInfo {
        epoch: 0,
        signature: Default::default(),
        signers_map: Default::default(),
    };
    CertifiedCheckpointSummary::new_from_data_and_sig(summary, sig)
}

fn mock_summary_data(sequence_number: u64) -> (CertifiedCheckpointSummary, CheckpointData) {
    let summary = mock_summary(sequence_number);
    let data = CheckpointData {
        checkpoint_summary: summary.clone(),
        checkpoint_contents: MOCK_CHECKPOINT_CONTENTS.clone(),
        transactions: vec![],
    };
    (summary, data)
}

// Minimal empty trait impls to satisfy RestStateReader supertraits
impl iota_types::storage::ObjectStore for MockRestStateReader {
    fn try_get_object(
        &self,
        _id: &ObjectID,
    ) -> iota_types::storage::error::Result<Option<iota_types::object::Object>> {
        unimplemented!()
    }

    fn try_get_object_by_key(
        &self,
        _id: &ObjectID,
        _version: iota_types::base_types::SequenceNumber,
    ) -> iota_types::storage::error::Result<Option<iota_types::object::Object>> {
        unimplemented!()
    }

    fn get_object(&self, id: &ObjectID) -> Option<iota_types::object::Object> {
        self.try_get_object(id).expect("storage access failed")
    }

    fn get_object_by_key(
        &self,
        id: &ObjectID,
        version: iota_types::base_types::SequenceNumber,
    ) -> Option<iota_types::object::Object> {
        self.try_get_object_by_key(id, version)
            .expect("storage access failed")
    }
}
impl iota_types::storage::ReadStore for MockRestStateReader {
    fn try_get_committee(
        &self,
        _epoch: EpochId,
    ) -> iota_types::storage::error::Result<Option<std::sync::Arc<iota_types::committee::Committee>>>
    {
        unimplemented!()
    }

    fn get_committee(
        &self,
        epoch: EpochId,
    ) -> Option<std::sync::Arc<iota_types::committee::Committee>> {
        self.try_get_committee(epoch)
            .expect("storage access failed")
    }

    fn try_get_latest_checkpoint(&self) -> iota_types::storage::error::Result<VerifiedCheckpoint> {
        // Return the checkpoint with the highest sequence number
        let guard = self.checkpoints.lock().unwrap();
        if let Some(max_seq) = guard.iter().max().cloned() {
            Ok(VerifiedCheckpoint::new_unchecked(mock_summary(max_seq)))
        } else {
            // Use the missing error constructor
            Err(iota_types::storage::error::Error::missing(
                "No checkpoints available",
            ))
        }
    }

    fn get_latest_checkpoint(&self) -> VerifiedCheckpoint {
        self.try_get_latest_checkpoint()
            .expect("storage access failed")
    }

    fn try_get_highest_verified_checkpoint(
        &self,
    ) -> iota_types::storage::error::Result<VerifiedCheckpoint> {
        unimplemented!()
    }

    fn get_highest_verified_checkpoint(&self) -> VerifiedCheckpoint {
        self.try_get_highest_verified_checkpoint()
            .expect("storage access failed")
    }

    fn try_get_highest_synced_checkpoint(
        &self,
    ) -> iota_types::storage::error::Result<VerifiedCheckpoint> {
        let guard = self.checkpoints.lock().unwrap();
        if let Some(max_seq) = guard.iter().max().cloned() {
            Ok(VerifiedCheckpoint::new_unchecked(mock_summary(max_seq)))
        } else {
            Err(iota_types::storage::error::Error::custom(
                "No checkpoints available",
            ))
        }
    }

    fn get_highest_synced_checkpoint(&self) -> VerifiedCheckpoint {
        self.try_get_highest_synced_checkpoint()
            .expect("storage access failed")
    }

    fn try_get_lowest_available_checkpoint(&self) -> iota_types::storage::error::Result<u64> {
        unimplemented!()
    }

    fn get_lowest_available_checkpoint(&self) -> u64 {
        self.try_get_lowest_available_checkpoint()
            .expect("storage access failed")
    }

    fn try_get_checkpoint_by_digest(
        &self,
        _digest: &iota_types::messages_checkpoint::CheckpointDigest,
    ) -> iota_types::storage::error::Result<Option<VerifiedCheckpoint>> {
        unimplemented!()
    }

    fn get_checkpoint_by_digest(
        &self,
        digest: &iota_types::messages_checkpoint::CheckpointDigest,
    ) -> Option<VerifiedCheckpoint> {
        self.try_get_checkpoint_by_digest(digest)
            .expect("storage access failed")
    }

    fn try_get_checkpoint_by_sequence_number(
        &self,
        seq: CheckpointSequenceNumber,
    ) -> iota_types::storage::error::Result<Option<VerifiedCheckpoint>> {
        let guard = self.checkpoints.lock().unwrap();
        if seq == u64::MAX {
            // Return the highest checkpoint
            if let Some(max_seq) = guard.iter().max().cloned() {
                return Ok(Some(VerifiedCheckpoint::new_unchecked(mock_summary(
                    max_seq,
                ))));
            } else {
                return Ok(None);
            }
        }
        Ok(guard
            .get(&seq)
            .map(|_| VerifiedCheckpoint::new_unchecked(mock_summary(seq))))
    }

    fn get_checkpoint_by_sequence_number(
        &self,
        seq: CheckpointSequenceNumber,
    ) -> Option<VerifiedCheckpoint> {
        self.try_get_checkpoint_by_sequence_number(seq)
            .expect("storage access failed")
    }

    fn try_get_checkpoint_contents_by_digest(
        &self,
        _digest: &iota_types::messages_checkpoint::CheckpointContentsDigest,
    ) -> iota_types::storage::error::Result<Option<CheckpointContents>> {
        unimplemented!()
    }

    fn get_checkpoint_contents_by_digest(
        &self,
        digest: &iota_types::messages_checkpoint::CheckpointContentsDigest,
    ) -> Option<CheckpointContents> {
        self.try_get_checkpoint_contents_by_digest(digest)
            .expect("storage access failed")
    }

    fn try_get_checkpoint_contents_by_sequence_number(
        &self,
        seq: CheckpointSequenceNumber,
    ) -> iota_types::storage::error::Result<Option<CheckpointContents>> {
        let guard = self.checkpoints.lock().unwrap();
        Ok(guard.get(&seq).map(|_| MOCK_CHECKPOINT_CONTENTS.clone()))
    }

    fn get_checkpoint_contents_by_sequence_number(
        &self,
        seq: CheckpointSequenceNumber,
    ) -> Option<CheckpointContents> {
        self.try_get_checkpoint_contents_by_sequence_number(seq)
            .expect("storage access failed")
    }

    fn try_get_transaction(
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

    fn get_transaction(
        &self,
        digest: &iota_types::digests::TransactionDigest,
    ) -> Option<
        std::sync::Arc<
            iota_types::message_envelope::VerifiedEnvelope<
                iota_types::transaction::SenderSignedData,
                iota_types::crypto::EmptySignInfo,
            >,
        >,
    > {
        self.try_get_transaction(digest)
            .expect("storage access failed")
    }

    fn try_get_transaction_effects(
        &self,
        _digest: &iota_types::digests::TransactionDigest,
    ) -> iota_types::storage::error::Result<Option<iota_types::effects::TransactionEffects>> {
        unimplemented!()
    }

    fn get_transaction_effects(
        &self,
        digest: &iota_types::digests::TransactionDigest,
    ) -> Option<iota_types::effects::TransactionEffects> {
        self.try_get_transaction_effects(digest)
            .expect("storage access failed")
    }

    fn try_get_events(
        &self,
        _digest: &iota_types::digests::TransactionEventsDigest,
    ) -> iota_types::storage::error::Result<Option<iota_types::effects::TransactionEvents>> {
        unimplemented!()
    }

    fn get_events(
        &self,
        digest: &iota_types::digests::TransactionEventsDigest,
    ) -> Option<iota_types::effects::TransactionEvents> {
        self.try_get_events(digest).expect("storage access failed")
    }

    fn try_get_full_checkpoint_contents_by_sequence_number(
        &self,
        _seq: CheckpointSequenceNumber,
    ) -> iota_types::storage::error::Result<
        Option<iota_types::messages_checkpoint::FullCheckpointContents>,
    > {
        unimplemented!()
    }

    fn get_full_checkpoint_contents_by_sequence_number(
        &self,
        seq: CheckpointSequenceNumber,
    ) -> Option<iota_types::messages_checkpoint::FullCheckpointContents> {
        self.try_get_full_checkpoint_contents_by_sequence_number(seq)
            .expect("storage access failed")
    }

    fn try_get_full_checkpoint_contents(
        &self,
        _digest: &iota_types::messages_checkpoint::CheckpointContentsDigest,
    ) -> iota_types::storage::error::Result<
        Option<iota_types::messages_checkpoint::FullCheckpointContents>,
    > {
        unimplemented!()
    }

    fn get_full_checkpoint_contents(
        &self,
        digest: &iota_types::messages_checkpoint::CheckpointContentsDigest,
    ) -> Option<iota_types::messages_checkpoint::FullCheckpointContents> {
        self.try_get_full_checkpoint_contents(digest)
            .expect("storage access failed")
    }
}

impl RestStateReader for MockRestStateReader {
    fn get_lowest_available_checkpoint_objects(&self) -> StorageResult<CheckpointSequenceNumber> {
        Ok(0)
    }

    fn get_chain_identifier(&self) -> StorageResult<iota_types::digests::ChainIdentifier> {
        unimplemented!()
    }

    fn get_epoch_last_checkpoint(
        &self,
        _: EpochId,
    ) -> StorageResult<Option<iota_types::messages_checkpoint::VerifiedCheckpoint>> {
        unimplemented!()
    }

    fn indexes(&self) -> Option<&dyn RestIndexes> {
        None
    }
}

async fn test_service() -> CheckpointGrpcService {
    let mock = Arc::new(MockRestStateReader::new_from_iter(0..=10));
    let config = iota_grpc_api::Config::default();
    let (grpc_checkpoint_summary_tx, _) =
        tokio::sync::broadcast::channel(config.checkpoint_broadcast_buffer_size);
    let (grpc_checkpoint_data_tx, _) =
        tokio::sync::broadcast::channel(config.checkpoint_broadcast_buffer_size);
    CheckpointGrpcService::new(mock, grpc_checkpoint_summary_tx, grpc_checkpoint_data_tx)
}

// Helper function to spawn a background checkpoint sender for summaries and
// data
fn spawn_checkpoint_sender(
    summary_tx: tokio::sync::broadcast::Sender<Arc<GrpcCertifiedCheckpointSummary>>,
    data_tx: tokio::sync::broadcast::Sender<Arc<GrpcCheckpointData>>,
    start_seq: u64,
) {
    tokio::spawn(async move {
        let mut seq = start_seq;
        loop {
            let (summary, data) = mock_summary_data(seq);
            let _ = summary_tx.send(Arc::new(GrpcCertifiedCheckpointSummary::from(summary)));
            let _ = data_tx.send(Arc::new(GrpcCheckpointData::from(data)));
            seq += 1;
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    });
}

#[tokio::test]
async fn test_start_sequence_number_only() {
    let svc = test_service().await;

    spawn_checkpoint_sender(
        svc.checkpoint_summary_tx.clone(),
        svc.checkpoint_data_tx.clone(),
        11,
    );
    let req = CheckpointStreamRequest {
        start_sequence_number: Some(5),
        end_sequence_number: None,
        full: true,
    };
    let mut stream = svc
        .stream_checkpoints(Request::new(req))
        .await
        .unwrap()
        .into_inner();
    let mut result = Vec::new();

    tokio::time::timeout(Duration::from_secs(120), async {
        while let Some(res) = stream.next().await {
            match res {
                Ok(cp) => {
                    // Only collect the expected range
                    if cp.sequence_number > 30 {
                        break;
                    }
                    result.push(cp.sequence_number)
                }
                Err(status) if status.code() == tonic::Code::NotFound => break,
                Err(e) => panic!("Unexpected error: {e:?}"),
            }
        }
    })
    .await
    .expect("waiting for checkpoints timed out");

    assert_eq!(result, (5..=30).collect::<Vec<_>>());
}

#[tokio::test]
async fn test_start_and_future_end_sequence_number() {
    let svc = test_service().await;

    spawn_checkpoint_sender(
        svc.checkpoint_summary_tx.clone(),
        svc.checkpoint_data_tx.clone(),
        11,
    );
    let req = CheckpointStreamRequest {
        start_sequence_number: Some(3),
        end_sequence_number: Some(15),
        full: false,
    };
    let mut stream = svc
        .stream_checkpoints(Request::new(req))
        .await
        .unwrap()
        .into_inner();
    let mut result = Vec::new();

    tokio::time::timeout(Duration::from_secs(120), async {
        while let Some(res) = stream.next().await {
            match res {
                Ok(cp) => {
                    // Only collect the expected range
                    if cp.sequence_number > 7 {
                        break;
                    }
                    result.push(cp.sequence_number)
                }
                Err(status) if status.code() == tonic::Code::NotFound => break,
                Err(e) => panic!("Unexpected error: {e:?}"),
            }
        }
    })
    .await
    .expect("waiting for checkpoints timed out");

    assert_eq!(result, (3..=7).collect::<Vec<_>>());
}

#[tokio::test]
async fn test_historical_end_sequence_number_only() {
    let svc = test_service().await;

    let req = CheckpointStreamRequest {
        start_sequence_number: None,
        end_sequence_number: Some(4),
        full: false,
    };
    let mut stream = svc
        .stream_checkpoints(Request::new(req))
        .await
        .unwrap()
        .into_inner();
    let mut result = Vec::new();

    tokio::time::timeout(Duration::from_secs(120), async {
        while let Some(res) = stream.next().await {
            match res {
                Ok(cp) => result.push(cp.sequence_number),
                Err(status) if status.code() == tonic::Code::NotFound => break,
                Err(e) => panic!("Unexpected error: {e:?}"),
            }
        }
    })
    .await
    .expect("waiting for checkpoints timed out");

    assert_eq!(result, vec![4]);
}

#[tokio::test]
async fn test_future_end_sequence_number_only_full() {
    let svc = test_service().await;
    spawn_checkpoint_sender(
        svc.checkpoint_summary_tx.clone(),
        svc.checkpoint_data_tx.clone(),
        11,
    );

    let req = CheckpointStreamRequest {
        start_sequence_number: None,
        end_sequence_number: Some(100),
        full: true,
    };
    let mut stream = svc
        .stream_checkpoints(Request::new(req))
        .await
        .unwrap()
        .into_inner();
    let mut result = Vec::new();

    tokio::time::timeout(Duration::from_secs(120), async {
        if let Some(res) = stream.next().await {
            match res {
                Ok(cp) => {
                    // For this test, we just need to verify we got checkpoint 100
                    // The BCS data deserialization is handled automatically by the client API
                    // but this test is working with the raw service, so we check the metadata
                    assert_eq!(cp.sequence_number, 100);
                    result.push(cp.sequence_number);
                }
                Err(status) if status.code() == tonic::Code::NotFound => {
                    panic!("Stream ended unexpectedly before receiving enough checkpoints")
                }
                Err(e) => panic!("Unexpected error: {e:?}"),
            }
        }
    })
    .await
    .expect("waiting for checkpoint data timed out");

    assert_eq!(result, vec![100]);
}

#[tokio::test]
async fn test_both_indices_omitted() {
    let svc = test_service().await;
    let req = CheckpointStreamRequest {
        start_sequence_number: None,
        end_sequence_number: None,
        full: false,
    };

    // Subscribe to the stream after buffer is pre-filled (0..=10)
    let mut stream = svc
        .stream_checkpoints(Request::new(req))
        .await
        .unwrap()
        .into_inner();
    let mut result = Vec::new();

    // Now send new checkpoints (live) after subscribing
    spawn_checkpoint_sender(
        svc.checkpoint_summary_tx.clone(),
        svc.checkpoint_data_tx.clone(),
        11,
    );

    // Collect enough checkpoints to see both buffered and live ones
    tokio::time::timeout(Duration::from_secs(120), async {
        for _ in 0..15 {
            if let Some(res) = stream.next().await {
                match res {
                    Ok(cp) => result.push(cp.sequence_number),
                    Err(status) if status.code() == tonic::Code::NotFound => break,
                    Err(e) => panic!("Unexpected error: {e:?}"),
                }
            }
        }
    })
    .await
    .expect("waiting for checkpoints timed out");

    // The first 11 should be 0..=10 (buffered), then live ones (11, 12, ...)
    assert_eq!(&result[..], &(10..=24).collect::<Vec<_>>()[..]);
}

#[tokio::test]
async fn test_historical_to_live_gap_fill() {
    // Simulate storage with checkpoints 0..=150
    let mock = Arc::new(MockRestStateReader::new_from_iter(0..=150));
    let (grpc_checkpoint_summary_tx, _) = broadcast::channel(10);
    let (grpc_checkpoint_data_tx, _) = broadcast::channel(10);
    let svc = CheckpointGrpcService::new(
        mock.clone(),
        grpc_checkpoint_summary_tx.clone(),
        grpc_checkpoint_data_tx.clone(),
    );

    // Simulate broadcast channel at 150
    let (summary_150, data_150) = mock_summary_data(150);
    let _ = grpc_checkpoint_summary_tx
        .send(Arc::new(GrpcCertifiedCheckpointSummary::from(summary_150)));
    let _ = grpc_checkpoint_data_tx.send(Arc::new(GrpcCheckpointData::from(data_150)));

    // Client requests from 0 (historical)
    let req = CheckpointStreamRequest {
        start_sequence_number: Some(0),
        end_sequence_number: None,
        full: true,
    };
    let mut stream = svc
        .stream_checkpoints(Request::new(req))
        .await
        .unwrap()
        .into_inner();
    let mut received = Vec::new();
    // Collect up to 151 checkpoints

    tokio::time::timeout(Duration::from_secs(120), async {
        while let Some(Ok(cp)) = stream.next().await {
            // For this test, we just need the sequence numbers
            received.push(cp.sequence_number);
            if cp.sequence_number == 150 {
                break;
            }
        }
    })
    .await
    .expect("waiting for checkpoint data timed out");

    // Assert we got all checkpoints 0..=150
    assert_eq!(received, (0..=150u64).collect::<Vec<_>>());
}

#[tokio::test(flavor = "current_thread")]
async fn test_gap_fill_with_slow_client() {
    // Pre-populate storage with checkpoints 0..=10 before spawning the producer and
    let mock = Arc::new(MockRestStateReader::new_from_iter(0..=10));
    let checkpoints = mock.checkpoints.clone();
    let (grpc_checkpoint_summary_tx, _) = broadcast::channel(5);
    let (grpc_checkpoint_data_tx, _) = broadcast::channel(5);
    let svc = CheckpointGrpcService::new(
        mock,
        grpc_checkpoint_summary_tx.clone(),
        grpc_checkpoint_data_tx.clone(),
    );

    // Producer: generates checkpoints 0..=200, one every 100ms
    tokio::spawn({
        let grpc_checkpoint_summary_tx = grpc_checkpoint_summary_tx.clone();
        let grpc_checkpoint_data_tx = grpc_checkpoint_data_tx.clone();
        async move {
            for i in 11..=200u64 {
                let (summary, data) = mock_summary_data(i);
                checkpoints.lock().unwrap().insert(i);
                let _ = grpc_checkpoint_summary_tx
                    .send(Arc::new(GrpcCertifiedCheckpointSummary::from(summary)));
                let _ = grpc_checkpoint_data_tx.send(Arc::new(GrpcCheckpointData::from(data)));
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    });

    // Client: slow consumer
    let req = CheckpointStreamRequest {
        start_sequence_number: Some(0),
        end_sequence_number: None,
        full: true,
    };
    let mut stream = svc
        .stream_checkpoints(Request::new(req))
        .await
        .unwrap()
        .into_inner();
    let mut received = Vec::new();

    tokio::time::timeout(Duration::from_secs(120), async {
        while let Some(Ok(cp)) = stream.next().await {
            // For this test, we just need the sequence numbers
            received.push(cp.sequence_number);
            tokio::time::sleep(Duration::from_millis(500)).await; // slow down the client
            if cp.sequence_number == 20 {
                break;
            }
        }
    })
    .await
    .expect("waiting for checkpoint data timed out");

    assert_eq!(received, (0..=20u64).collect::<Vec<_>>());
}
