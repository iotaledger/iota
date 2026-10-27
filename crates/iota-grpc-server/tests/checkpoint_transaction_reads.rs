// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! `ReadStore::stream_checkpoint_transactions` fetches the transactions of a
//! checkpoint one digest at a time, not with one multi-get for the whole
//! checkpoint, so that a client that does not read cannot make the server
//! hold all transactions of a checkpoint.
//!
//! `MockGrpcStateReader` (see `tests/common/mod.rs`) overrides
//! `stream_checkpoint_transactions` directly for the convenience of the other
//! integration tests in this crate, which bypasses the default trait method
//! that this test checks. So this test uses its own minimal `ReadStore` /
//! `GrpcStateReader` implementation that does not override it, exercising
//! the real default implementation through the gRPC server.
mod common;

use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use iota_grpc_types::{
    field::FieldMaskUtil,
    v1::ledger_service::{GetCheckpointRequest, ledger_service_client::LedgerServiceClient},
};
use iota_node_storage::{GrpcIndexes, GrpcStateReader};
use iota_sdk_types::{
    CheckpointContents, CheckpointContentsDigest, CheckpointDigest, ObjectId, StructTag,
    TransactionDigest, TransactionEffects, TransactionEvents, Version,
};
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    base_types::random_object_ref,
    committee::{Committee, EpochId},
    crypto::{AccountPrivateKey, get_key_pair},
    digests::ChainIdentifier,
    effects::{TransactionEffectsExt as _, TransactionEffectsExtForTesting as _},
    messages_checkpoint::{
        CertifiedCheckpointSummary, CheckpointContentsExt as _, CheckpointSequenceNumber,
        VerifiedCheckpoint,
    },
    object::Object,
    storage::{EpochInfoV2, ObjectStore, ReadStore, error::Result as StorageResult},
    transaction::VerifiedTransaction,
};

/// Far more than one, so a single multi-get for the whole checkpoint is
/// easy to tell apart from one-digest-at-a-time reads.
const NUM_TRANSACTIONS: usize = 200;

/// The largest multi-get a one-digest-at-a-time read can ever issue.
const EXPECTED_MULTI_GET_SIZE: usize = 1;

/// A minimal `ReadStore` + `GrpcStateReader` backing a single checkpoint,
/// which records the largest slice length passed to
/// `try_multi_get_transactions`.
struct RecordingReader {
    seq: CheckpointSequenceNumber,
    summary: CertifiedCheckpointSummary,
    contents: CheckpointContents,
    transactions: HashMap<TransactionDigest, Arc<VerifiedTransaction>>,
    effects: HashMap<TransactionDigest, TransactionEffects>,
    max_multi_get_len: Arc<AtomicUsize>,
}

impl ObjectStore for RecordingReader {
    fn try_get_object(&self, _object_id: &ObjectId) -> StorageResult<Option<Object>> {
        // Unused: the test transactions carry no object changes.
        Ok(None)
    }

    fn try_get_object_by_key(
        &self,
        _object_id: &ObjectId,
        _version: Version,
    ) -> StorageResult<Option<Object>> {
        Ok(None)
    }
}

impl ReadStore for RecordingReader {
    fn try_get_committee(&self, _epoch: EpochId) -> StorageResult<Option<Arc<Committee>>> {
        Ok(None)
    }

    fn try_get_latest_checkpoint(&self) -> StorageResult<VerifiedCheckpoint> {
        Ok(VerifiedCheckpoint::new_unchecked(self.summary.clone()))
    }

    fn try_get_highest_verified_checkpoint(&self) -> StorageResult<VerifiedCheckpoint> {
        self.try_get_latest_checkpoint()
    }

    fn try_get_highest_synced_checkpoint(&self) -> StorageResult<VerifiedCheckpoint> {
        self.try_get_latest_checkpoint()
    }

    fn try_get_lowest_available_checkpoint(&self) -> StorageResult<CheckpointSequenceNumber> {
        Ok(0)
    }

    fn try_get_checkpoint_by_digest(
        &self,
        _digest: &CheckpointDigest,
    ) -> StorageResult<Option<VerifiedCheckpoint>> {
        Ok(None)
    }

    fn try_get_checkpoint_by_sequence_number(
        &self,
        seq: CheckpointSequenceNumber,
    ) -> StorageResult<Option<VerifiedCheckpoint>> {
        if seq == self.seq {
            Ok(Some(VerifiedCheckpoint::new_unchecked(
                self.summary.clone(),
            )))
        } else {
            Ok(None)
        }
    }

    fn try_get_checkpoint_contents_by_digest(
        &self,
        _digest: &CheckpointContentsDigest,
    ) -> StorageResult<Option<CheckpointContents>> {
        Ok(None)
    }

    fn try_get_checkpoint_contents_by_sequence_number(
        &self,
        seq: CheckpointSequenceNumber,
    ) -> StorageResult<Option<CheckpointContents>> {
        if seq == self.seq {
            Ok(Some(self.contents.clone()))
        } else {
            Ok(None)
        }
    }

    fn try_get_transaction(
        &self,
        digest: &TransactionDigest,
    ) -> StorageResult<Option<Arc<VerifiedTransaction>>> {
        Ok(self.transactions.get(digest).cloned())
    }

    fn try_multi_get_transactions(
        &self,
        tx_digests: &[TransactionDigest],
    ) -> StorageResult<Vec<Option<Arc<VerifiedTransaction>>>> {
        self.max_multi_get_len
            .fetch_max(tx_digests.len(), Ordering::SeqCst);
        tx_digests
            .iter()
            .map(|digest| self.try_get_transaction(digest))
            .collect()
    }

    fn try_get_transaction_effects(
        &self,
        digest: &TransactionDigest,
    ) -> StorageResult<Option<TransactionEffects>> {
        Ok(self.effects.get(digest).cloned())
    }

    fn try_get_events(
        &self,
        _digest: &TransactionDigest,
    ) -> StorageResult<Option<TransactionEvents>> {
        Ok(None)
    }

    fn try_get_full_checkpoint_contents_by_sequence_number(
        &self,
        _seq: CheckpointSequenceNumber,
    ) -> StorageResult<Option<iota_types::messages_checkpoint::FullCheckpointContents>> {
        unimplemented!("not used by this test")
    }

    fn try_get_full_checkpoint_contents(
        &self,
        _digest: &CheckpointContentsDigest,
    ) -> StorageResult<Option<iota_types::messages_checkpoint::FullCheckpointContents>> {
        unimplemented!("not used by this test")
    }
}

impl GrpcStateReader for RecordingReader {
    fn get_lowest_available_checkpoint_objects(&self) -> StorageResult<CheckpointSequenceNumber> {
        Ok(0)
    }

    fn get_chain_identifier(&self) -> StorageResult<ChainIdentifier> {
        Ok(ChainIdentifier::default())
    }

    fn get_epoch_last_checkpoint(
        &self,
        _epoch: EpochId,
    ) -> StorageResult<Option<VerifiedCheckpoint>> {
        Ok(None)
    }

    fn get_epoch_info(&self, _epoch: EpochId) -> StorageResult<Option<EpochInfoV2>> {
        Ok(None)
    }

    fn grpc_indexes(&self) -> Option<&dyn GrpcIndexes> {
        None
    }

    fn get_struct_layout(
        &self,
        _type_tag: &StructTag,
    ) -> StorageResult<Option<move_core_types::annotated_value::MoveTypeLayout>> {
        Ok(None)
    }
}

/// Build a signed transaction and matching empty-effects (no object changes,
/// no events) so the checkpoint pipeline doesn't need any object data.
fn build_transaction_and_effects() -> (
    TransactionDigest,
    Arc<VerifiedTransaction>,
    TransactionEffects,
) {
    let (sender, key): (_, AccountPrivateKey) = get_key_pair();
    let gas = random_object_ref();
    let tx = TestTransactionBuilder::new(sender, gas, 1000)
        .transfer(random_object_ref(), sender)
        .build_and_sign(&key);
    let digest = *tx.digest();
    let effects = TransactionEffects::new_empty_v1_for_testing(digest);
    (
        digest,
        Arc::new(VerifiedTransaction::new_unchecked(tx)),
        effects,
    )
}

#[tokio::test]
async fn stream_checkpoint_transactions_reads_one_at_a_time() {
    let mut transactions = HashMap::new();
    let mut effects = HashMap::new();
    let mut digests = Vec::with_capacity(NUM_TRANSACTIONS);

    for _ in 0..NUM_TRANSACTIONS {
        let (digest, tx, fx) = build_transaction_and_effects();
        digests.push(fx.execution_digests());
        transactions.insert(digest, tx);
        effects.insert(digest, fx);
    }

    let contents = CheckpointContents::new_with_digests_only_for_tests(digests);
    let summary = common::mock_summary(0, &contents);
    let max_multi_get_len = Arc::new(AtomicUsize::new(0));

    let reader: Arc<dyn GrpcStateReader> = Arc::new(RecordingReader {
        seq: 0,
        summary,
        contents,
        transactions,
        effects,
        max_multi_get_len: max_multi_get_len.clone(),
    });

    let (server_handle, _) = common::start_test_server(reader, |_| {}).await;
    let addr = server_handle.address();

    let channel = tonic::transport::Channel::from_shared(format!("http://{addr}"))
        .unwrap()
        .connect()
        .await
        .expect("connect");
    let mut client = LedgerServiceClient::new(channel);

    let req = GetCheckpointRequest::default()
        .with_sequence_number(0)
        .with_read_mask(prost_types::FieldMask::from_str(
            "checkpoint.summary,transactions",
        ));

    let mut stream = client
        .get_checkpoint(req)
        .await
        .expect("get_checkpoint should succeed")
        .into_inner();
    while stream
        .message()
        .await
        .expect("stream should not error")
        .is_some()
    {}

    server_handle.shutdown().await.expect("shutdown");

    let observed_max = max_multi_get_len.load(Ordering::SeqCst);
    assert_eq!(
        observed_max, EXPECTED_MULTI_GET_SIZE,
        "largest single multi-get for {NUM_TRANSACTIONS} transactions was {observed_max}, \
         expected {EXPECTED_MULTI_GET_SIZE} (checkpoint transactions must be fetched one digest \
         at a time, not all at once)"
    );
}
