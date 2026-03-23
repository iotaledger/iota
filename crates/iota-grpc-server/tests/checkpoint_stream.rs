// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::HashSet,
    sync::{Arc, LazyLock, Mutex},
    time::Duration,
};

use iota_config::{local_ip_utils, node::GrpcApiConfig};
use iota_grpc_client::{CheckpointStreamItem, Client};
use iota_grpc_server::{GrpcReader, GrpcServerHandle, start_grpc_server};
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    base_types::{IotaAddress, ObjectID, random_object_ref},
    committee::EpochId,
    crypto::{AccountKeyPair, AuthorityStrongQuorumSignInfo, get_key_pair},
    effects::TestEffectsBuilder,
    full_checkpoint_content::{CheckpointData, CheckpointTransaction},
    messages_checkpoint::{
        CertifiedCheckpointSummary, CheckpointContents, CheckpointSequenceNumber,
        CheckpointSummary, VerifiedCheckpoint,
    },
    storage::{RestIndexes, RestStateReader, error::Result as StorageResult},
};
use tokio_stream::StreamExt;

struct MockRestStateReader {
    chain_identifier: iota_types::digests::ChainIdentifier,
    checkpoints: Arc<Mutex<HashSet<CheckpointSequenceNumber>>>,
    large_checkpoints: Arc<Mutex<HashSet<CheckpointSequenceNumber>>>,
}
impl MockRestStateReader {
    fn new_from_iter<I: Iterator<Item = u64>>(iter: I) -> Self {
        Self {
            chain_identifier: iota_types::digests::ChainIdentifier::default(),
            checkpoints: Arc::new(Mutex::new(iter.collect())),
            large_checkpoints: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// Mark a checkpoint sequence number as using large data
    fn mark_checkpoint_as_large(&self, seq: CheckpointSequenceNumber) {
        self.large_checkpoints.lock().unwrap().insert(seq);
    }

    /// Check if a checkpoint should use large data
    fn is_large_checkpoint(&self, seq: CheckpointSequenceNumber) -> bool {
        self.large_checkpoints.lock().unwrap().contains(&seq)
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

fn mock_checkpoint_data(sequence_number: u64) -> CheckpointData {
    let summary = mock_summary(sequence_number);
    CheckpointData {
        checkpoint_summary: summary,
        checkpoint_contents: MOCK_CHECKPOINT_CONTENTS.clone(),
        transactions: vec![],
    }
}

/// Create checkpoint data with a transaction from a specific sender.
fn mock_checkpoint_data_with_sender(
    sequence_number: u64,
    sender: IotaAddress,
    key: &AccountKeyPair,
) -> CheckpointData {
    let summary = mock_summary(sequence_number);
    let gas = random_object_ref();
    let transaction = TestTransactionBuilder::new(sender, gas, 1000)
        .transfer(random_object_ref(), sender)
        .build_and_sign(key);
    let effects = TestEffectsBuilder::new(transaction.data()).build();
    CheckpointData {
        checkpoint_summary: summary,
        checkpoint_contents: MOCK_CHECKPOINT_CONTENTS.clone(),
        transactions: vec![CheckpointTransaction {
            transaction,
            effects,
            events: None,
            input_objects: vec![],
            output_objects: vec![],
        }],
    }
}

fn mock_large_checkpoint_data(sequence_number: u64) -> CheckpointData {
    let summary = mock_summary(sequence_number);

    // Create many dummy transactions to exceed the message size limit when chunked
    // Each transaction will be roughly 1KB when serialized, so we need about 5000
    // transactions to exceed 4MB when serialized
    let num_transactions = 50000;
    let mut transactions = Vec::with_capacity(num_transactions);

    for _i in 0..num_transactions {
        let (sender, key): (_, AccountKeyPair) = get_key_pair();
        let gas = random_object_ref();
        let transaction = TestTransactionBuilder::new(sender, gas, 1000)
            .transfer(random_object_ref(), sender)
            .build_and_sign(&key);

        let effects = TestEffectsBuilder::new(transaction.data()).build();

        transactions.push(CheckpointTransaction {
            transaction,
            effects,
            events: None,
            input_objects: vec![],  // Empty for simplicity
            output_objects: vec![], // Empty for simplicity
        });
    }

    CheckpointData {
        checkpoint_summary: summary,
        checkpoint_contents: MOCK_CHECKPOINT_CONTENTS.clone(),
        transactions,
    }
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
        Ok(0)
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
        _digest: &iota_types::digests::TransactionDigest,
    ) -> iota_types::storage::error::Result<Option<iota_types::effects::TransactionEvents>> {
        unimplemented!()
    }

    fn get_events(
        &self,
        digest: &iota_types::digests::TransactionDigest,
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

    fn get_checkpoint_data(
        &self,
        checkpoint: VerifiedCheckpoint,
        checkpoint_contents: CheckpointContents,
    ) -> CheckpointData {
        let seq = checkpoint.sequence_number;

        // If this is a large checkpoint, return the large mock data
        if self.is_large_checkpoint(seq) {
            return mock_large_checkpoint_data(seq);
        }

        // Otherwise return the regular mock data
        CheckpointData {
            checkpoint_summary: checkpoint.into_inner(),
            checkpoint_contents,
            transactions: vec![], // Empty transactions for mock
        }
    }
}

impl RestStateReader for MockRestStateReader {
    fn get_lowest_available_checkpoint_objects(&self) -> StorageResult<CheckpointSequenceNumber> {
        Ok(0)
    }

    fn get_chain_identifier(&self) -> StorageResult<iota_types::digests::ChainIdentifier> {
        Ok(self.chain_identifier)
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

    fn get_struct_layout(
        &self,
        _: &move_core_types::language_storage::StructTag,
    ) -> iota_types::storage::error::Result<Option<move_core_types::annotated_value::MoveTypeLayout>>
    {
        Ok(None)
    }
}

/// Helper to set up test server with specific large checkpoints
async fn test_server_and_client_setup_with_large_checkpoints<
    I: Iterator<Item = u64>,
    L: Iterator<Item = u64>,
>(
    checkpoint_range: I,
    large_checkpoints: L,
    config_customizer: impl FnOnce(&mut GrpcApiConfig),
    client_max_message_size_bytes: Option<u32>,
) -> (
    GrpcServerHandle,
    Client,
    Arc<Mutex<HashSet<CheckpointSequenceNumber>>>,
) {
    let mock = Arc::new(MockRestStateReader::new_from_iter(checkpoint_range));

    // Mark specified checkpoints as large
    for seq in large_checkpoints {
        mock.mark_checkpoint_as_large(seq);
    }

    test_server_and_client_setup(
        std::iter::empty(),
        config_customizer,
        Some(mock),
        client_max_message_size_bytes,
    )
    .await
}

async fn test_server_and_client_setup<I: Iterator<Item = u64>>(
    checkpoint_range: I,
    config_customizer: impl FnOnce(&mut GrpcApiConfig),
    mock_state_reader: Option<Arc<MockRestStateReader>>,
    client_max_message_size_bytes: Option<u32>,
) -> (
    GrpcServerHandle,
    Client,
    Arc<Mutex<HashSet<CheckpointSequenceNumber>>>,
) {
    let mock = mock_state_reader.unwrap_or(Arc::new(MockRestStateReader::new_from_iter(
        checkpoint_range,
    )));
    let checkpoints = mock.checkpoints.clone();
    let cancellation_token = tokio_util::sync::CancellationToken::new();
    let grpc_reader = Arc::new(GrpcReader::from_rest_state_reader(
        mock,
        Some("test".to_string()),
    ));

    let localhost = local_ip_utils::localhost_for_testing();
    let grpc_port = local_ip_utils::get_available_port(&localhost);

    let mut config = GrpcApiConfig {
        address: format!("{localhost}:{grpc_port}").parse().unwrap(),
        ..GrpcApiConfig::default()
    };
    config_customizer(&mut config);

    let server_handle = start_grpc_server(
        grpc_reader,
        None, // No transaction executor for this test
        config,
        cancellation_token,
        iota_types::digests::ChainIdentifier::default(),
        None, // No metrics for this test
    )
    .await
    .expect("Failed to start gRPC server");

    let server_addr = server_handle.address();
    let mut client = Client::connect(&format!("http://{server_addr}"))
        .await
        .expect("Failed to connect to gRPC server");

    if let Some(max_size) = client_max_message_size_bytes {
        client = client.with_max_decoding_message_size(max_size as usize);
    }

    (server_handle, client, checkpoints)
}

// Helper function to spawn a background checkpoint sender for checkpoint data
fn spawn_checkpoint_sender(server_handle: &GrpcServerHandle, start_seq: u64) {
    let data_broadcaster = server_handle.checkpoint_data_broadcaster().clone();

    tokio::spawn(async move {
        let mut seq = start_seq;
        loop {
            let data = mock_checkpoint_data(seq);
            data_broadcaster.send_traced(&data);
            seq += 1;
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    });
}

#[tokio::test]
async fn test_start_sequence_number_only() {
    let (server_handle, client, _) = test_server_and_client_setup(0..=10, |_| {}, None, None).await;

    let range = (Some(5), None);

    let mut stream = client
        .stream_checkpoints(range.0, range.1, None, None, None)
        .await
        .unwrap();

    let mut results = Vec::new();

    // Start the checkpoint sender after we've subscribed to the stream
    spawn_checkpoint_sender(&server_handle, 11);

    tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(res) = stream.body_mut().next().await {
            match res {
                Ok(response) => {
                    let sequence_number = response.sequence_number();
                    // Only collect the expected range
                    if sequence_number > 30 {
                        break;
                    }
                    results.push(sequence_number);
                }
                Err(iota_grpc_client::Error::Grpc(status))
                    if status.code() == tonic::Code::NotFound =>
                {
                    break;
                }
                Err(e) => panic!("Unexpected error in stream: {e:?}"),
            }
        }
    })
    .await
    .expect("waiting for stream timed out");

    assert_eq!(results, (5..=30).collect::<Vec<_>>());

    // Clean up
    server_handle
        .shutdown()
        .await
        .expect("Failed to shutdown server");
}

#[tokio::test]
async fn test_start_and_future_end_sequence_number() {
    let (server_handle, client, _) = test_server_and_client_setup(0..=10, |_| {}, None, None).await;

    spawn_checkpoint_sender(&server_handle, 11);

    let range = (Some(3), Some(15));

    let mut stream = client
        .stream_checkpoints(range.0, range.1, None, None, None)
        .await
        .unwrap();

    let mut results = Vec::new();

    tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(res) = stream.body_mut().next().await {
            match res {
                Ok(response) => {
                    let sequence_number = response.sequence_number();
                    // Only collect the expected range
                    if sequence_number > 7 {
                        break;
                    }
                    results.push(sequence_number);
                }
                Err(iota_grpc_client::Error::Grpc(status))
                    if status.code() == tonic::Code::NotFound =>
                {
                    break;
                }
                Err(e) => panic!("Unexpected error in stream: {e:?}"),
            }
        }
    })
    .await
    .expect("waiting for stream timed out");

    assert_eq!(results, (3..=7).collect::<Vec<_>>());

    // Clean up
    server_handle
        .shutdown()
        .await
        .expect("Failed to shutdown server");
}

#[tokio::test]
async fn test_historical_end_sequence_number_only() {
    let (server_handle, client, _) = test_server_and_client_setup(0..=10, |_| {}, None, None).await;

    let range = (None, Some(4));

    let mut stream = client
        .stream_checkpoints(range.0, range.1, None, None, None)
        .await
        .unwrap();

    let mut results = Vec::new();

    tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(res) = stream.body_mut().next().await {
            match res {
                Ok(response) => {
                    let sequence_number = response.sequence_number();
                    results.push(sequence_number);
                }
                Err(iota_grpc_client::Error::Grpc(status))
                    if status.code() == tonic::Code::NotFound =>
                {
                    break;
                }
                Err(e) => panic!("Unexpected error in stream: {e:?}"),
            }
        }
    })
    .await
    .expect("waiting for stream timed out");

    assert_eq!(results, vec![4]);

    // Clean up
    server_handle
        .shutdown()
        .await
        .expect("Failed to shutdown server");
}

#[tokio::test]
async fn test_future_end_sequence_number_only_full() {
    let (server_handle, client, _) = test_server_and_client_setup(0..=10, |_| {}, None, None).await;
    spawn_checkpoint_sender(&server_handle, 11);

    let range = (None, Some(100));

    let mut stream = client
        .stream_checkpoints(range.0, range.1, None, None, None)
        .await
        .unwrap();

    let mut results = Vec::new();

    tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(res) = stream.body_mut().next().await {
            match res {
                Ok(response) => {
                    let sequence_number = response.sequence_number();
                    results.push(sequence_number);
                }
                Err(iota_grpc_client::Error::Grpc(status))
                    if status.code() == tonic::Code::NotFound =>
                {
                    break;
                }
                Err(e) => panic!("Unexpected error in stream: {e:?}"),
            }
        }
    })
    .await
    .expect("waiting for stream timed out");

    assert_eq!(results, vec![100]);

    // Clean up
    server_handle
        .shutdown()
        .await
        .expect("Failed to shutdown server");
}

#[tokio::test]
async fn test_both_indices_omitted() {
    let (server_handle, client, _) = test_server_and_client_setup(0..=10, |_| {}, None, None).await;

    // Subscribe to the stream after buffer is pre-filled (0..=10)
    let range = (None, None);

    let mut stream = client
        .stream_checkpoints(range.0, range.1, None, None, None)
        .await
        .unwrap();

    // Now send new checkpoints (live) after subscribing
    spawn_checkpoint_sender(&server_handle, 11);

    let mut results = Vec::new();

    // Collect enough checkpoints to see both buffered and live ones
    tokio::time::timeout(Duration::from_secs(10), async {
        let mut count = 0;

        while let Some(res) = stream.body_mut().next().await {
            match res {
                Ok(response) => {
                    let sequence_number = response.sequence_number();
                    results.push(sequence_number);
                    count += 1;
                    if count >= 15 {
                        break;
                    }
                }
                Err(iota_grpc_client::Error::Grpc(status))
                    if status.code() == tonic::Code::NotFound =>
                {
                    break;
                }
                Err(e) => panic!("Unexpected error in stream: {e:?}"),
            }
        }
    })
    .await
    .expect("waiting for stream timed out");

    // The first 11 should be 0..=10 (buffered), then live ones (11, 12, ...)
    assert_eq!(&results[..], &(10..=24).collect::<Vec<_>>()[..]);

    // Clean up
    server_handle
        .shutdown()
        .await
        .expect("Failed to shutdown server");
}

#[tokio::test]
async fn test_historical_to_live_gap_fill() {
    // Simulate storage with checkpoints 0..=149 (missing 150)
    let (server_handle, client, _) =
        test_server_and_client_setup(0..=149, |_| {}, None, None).await;

    // Client requests from 0 (historical) - should get 0..=149 from storage, then
    // 150 from broadcast
    let range = (Some(0), None);

    let mut stream = client
        .stream_checkpoints(range.0, range.1, None, None, None)
        .await
        .unwrap();

    // Simulate broadcast of checkpoint 150 AFTER subscribing
    let data_150 = mock_checkpoint_data(150);
    server_handle
        .checkpoint_data_broadcaster()
        .send_traced(&data_150);

    let mut results = Vec::new();

    // Collect up to 151 checkpoints
    tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(res) = stream.body_mut().next().await {
            match res {
                Ok(response) => {
                    let sequence_number = response.sequence_number();
                    results.push(sequence_number);
                    if sequence_number == 150 {
                        break;
                    }
                }
                Err(iota_grpc_client::Error::Grpc(status))
                    if status.code() == tonic::Code::NotFound =>
                {
                    break;
                }
                Err(e) => panic!("Unexpected error in stream: {e:?}"),
            }
        }
    })
    .await
    .expect("waiting for stream timed out");

    // Assert we got all checkpoints 0..=150 (0..=149 from storage, 150 from
    // broadcast)
    assert_eq!(results, (0..=150u64).collect::<Vec<_>>());

    // Clean up
    server_handle
        .shutdown()
        .await
        .expect("Failed to shutdown server");
}

#[tokio::test(flavor = "current_thread")]
async fn test_gap_fill_with_slow_client() {
    // Pre-populate storage with checkpoints 0..=10 before spawning the producer
    let (server_handle, client, checkpoints) = test_server_and_client_setup(
        0..=10,
        |config| {
            config.broadcast_buffer_size = 5;
        },
        None,
        None,
    )
    .await;

    // Producer: generates checkpoints 11..=200, one every 100ms
    tokio::spawn({
        let data_broadcaster = server_handle.checkpoint_data_broadcaster().clone();
        let checkpoints = checkpoints.clone();
        async move {
            for i in 11..=200u64 {
                let data = mock_checkpoint_data(i);
                checkpoints.lock().unwrap().insert(i);
                data_broadcaster.send_traced(&data);
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    });

    // Client: slow consumer
    let range = (Some(0), None);

    let mut stream = client
        .stream_checkpoints(range.0, range.1, None, None, None)
        .await
        .unwrap();

    let mut results = Vec::new();

    tokio::time::timeout(Duration::from_secs(120), async {
        while let Some(res) = stream.body_mut().next().await {
            match res {
                Ok(response) => {
                    let sequence_number = response.sequence_number();
                    results.push(sequence_number);
                    tokio::time::sleep(Duration::from_millis(250)).await; // slow down the client
                    if sequence_number >= 20 {
                        break;
                    }
                }
                Err(iota_grpc_client::Error::Grpc(status))
                    if status.code() == tonic::Code::NotFound =>
                {
                    break;
                }
                Err(e) => panic!("Unexpected error in stream: {e:?}"),
            }
        }
    })
    .await
    .expect("waiting for stream timed out");

    // Assert we got all checkpoints 0..=20
    assert_eq!(results, (0..=20u64).collect::<Vec<_>>());

    // Clean up
    server_handle
        .shutdown()
        .await
        .expect("Failed to shutdown server");
}

#[tokio::test]
async fn test_chunked_checkpoint_streaming() {
    // Test chunking by using a naturally large checkpoint that exceeds 4MB
    let (server_handle, client, _checkpoints) =
        test_server_and_client_setup_with_large_checkpoints(
            0..=1,
            std::iter::once(0), // Mark checkpoint 0 as large
            |config| {
                config.max_message_size_bytes = 4 * 1024 * 1024; // 4 MB
            },
            Some(4 * 1024 * 1024),
        )
        .await;

    // Test individual checkpoint retrieval
    let individual_checkpoint = client
        .get_checkpoint_by_sequence_number(0, None, None, None)
        .await
        .expect("get_checkpoint should work");

    // Verify the checkpoint data is correct
    assert_eq!(individual_checkpoint.body().sequence_number(), 0);
    let summary = individual_checkpoint
        .body()
        .summary()
        .expect("should have summary")
        .summary()
        .expect("should have checkpoint summary");
    assert_eq!(summary.epoch, 0);

    // Test streaming checkpoints - this should also work with small chunks
    let mut stream = client
        .stream_checkpoints(Some(0), Some(0), None, None, None)
        .await
        .unwrap();

    let streamed_checkpoint = stream
        .body_mut()
        .next()
        .await
        .expect("stream should have data")
        .expect("stream should not error");

    // Verify the streamed checkpoint data matches
    assert_eq!(streamed_checkpoint.sequence_number(), 0);
    let streamed_summary = streamed_checkpoint
        .summary()
        .expect("should have summary")
        .summary()
        .expect("should have checkpoint summary");
    assert_eq!(streamed_summary.epoch, 0);

    // Clean up
    server_handle
        .shutdown()
        .await
        .expect("Failed to shutdown server");
}

#[tokio::test]
async fn test_filter_checkpoints_validation() {
    use iota_grpc_types::v0::filter;

    let (server_handle, client, _) = test_server_and_client_setup(0..=5, |_| {}, None, None).await;

    // filter_checkpoints=true with no filters should fail
    let result = client
        .stream_checkpoints_filtered(Some(0), Some(5), None, None, None, None)
        .await;
    assert!(result.is_err(), "expected error when no filters are set");

    // tx filter without transactions in read_mask should fail
    let (sender, _): (IotaAddress, AccountKeyPair) = get_key_pair();
    let sender_bytes = sender.to_inner();
    let tx_filter = filter::TransactionFilter::default().with_sender(
        filter::AddressFilter::default().with_address(
            iota_grpc_types::v0::types::Address::default().with_address(sender_bytes.to_vec()),
        ),
    );

    let result = client
        .stream_checkpoints_filtered(
            Some(0),
            Some(5),
            Some("checkpoint"),
            Some(tx_filter),
            None,
            None,
        )
        .await;
    assert!(
        result.is_err(),
        "expected error when tx filter is set but transactions not in read_mask"
    );

    server_handle
        .shutdown()
        .await
        .expect("Failed to shutdown server");
}

#[tokio::test]
async fn test_filter_checkpoints_streaming() {
    let (server_handle, client, _) = test_server_and_client_setup(0..=0, |_| {}, None, None).await;

    let (sender, key): (IotaAddress, AccountKeyPair) = get_key_pair();
    let sender_bytes = sender.to_inner();

    // Create a sender filter matching our known sender
    let make_tx_filter = || {
        iota_grpc_types::v0::filter::TransactionFilter::default().with_sender(
            iota_grpc_types::v0::filter::AddressFilter::default().with_address(
                iota_grpc_types::v0::types::Address::default().with_address(sender_bytes.to_vec()),
            ),
        )
    };

    // Scenario 1: matching txs are returned, non-matching are skipped
    let mut stream = client
        .stream_checkpoints_filtered(
            None,
            None,
            Some("checkpoint,transactions"),
            Some(make_tx_filter()),
            None,
            None,
        )
        .await
        .unwrap();

    // Broadcast checkpoint 1 with matching sender
    server_handle
        .checkpoint_data_broadcaster()
        .send_traced(&mock_checkpoint_data_with_sender(1, sender, &key));
    // Broadcast checkpoint 2 with no transactions (should be skipped)
    server_handle
        .checkpoint_data_broadcaster()
        .send_traced(&mock_checkpoint_data(2));
    // Broadcast checkpoint 3 with matching sender
    server_handle
        .checkpoint_data_broadcaster()
        .send_traced(&mock_checkpoint_data_with_sender(3, sender, &key));

    let mut results = Vec::new();
    tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(res) = stream.body_mut().next().await {
            match res {
                Ok(CheckpointStreamItem::Checkpoint(response)) => {
                    results.push(response.sequence_number());
                    if results.len() >= 2 {
                        break;
                    }
                }
                Ok(_) => {}
                Err(e) => panic!("Unexpected error: {e:?}"),
            }
        }
    })
    .await
    .expect("waiting for stream timed out");
    // Only checkpoints 1 and 3 should be received (checkpoint 2 was filtered out)
    assert_eq!(results, vec![1, 3]);
    drop(stream);

    // Scenario 2: non-matching checkpoints are skipped until a match
    let mut stream = client
        .stream_checkpoints_filtered(
            None,
            None,
            Some("checkpoint,transactions"),
            Some(make_tx_filter()),
            None,
            None,
        )
        .await
        .unwrap();

    // Broadcast checkpoints with no transactions (should all be skipped)
    for i in 1..=5 {
        server_handle
            .checkpoint_data_broadcaster()
            .send_traced(&mock_checkpoint_data(i));
    }
    // Broadcast checkpoint with a different sender (should be skipped)
    let (other_sender, other_key): (IotaAddress, AccountKeyPair) = get_key_pair();
    server_handle
        .checkpoint_data_broadcaster()
        .send_traced(&mock_checkpoint_data_with_sender(
            6,
            other_sender,
            &other_key,
        ));
    // Finally broadcast one with the matching sender
    server_handle
        .checkpoint_data_broadcaster()
        .send_traced(&mock_checkpoint_data_with_sender(7, sender, &key));

    let mut results = Vec::new();
    tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(res) = stream.body_mut().next().await {
            match res {
                Ok(CheckpointStreamItem::Checkpoint(response)) => {
                    results.push(response.sequence_number());
                    return; // Just collect the first match
                }
                Ok(_) => {}
                Err(e) => panic!("Unexpected error: {e:?}"),
            }
        }
    })
    .await
    .expect("waiting for stream timed out");
    // Only checkpoint 7 should be received (all others were filtered out)
    assert_eq!(results, vec![7]);

    server_handle
        .shutdown()
        .await
        .expect("Failed to shutdown server");
}
