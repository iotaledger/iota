// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use async_trait::async_trait;
use bytes::Bytes;
use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::{
    Address, ObjectDigest, ObjectId, ObjectReference, RandomnessStateUpdate, Transaction,
    TransactionEffects, TransactionKind, Version,
    checkpoint::{CheckpointContents, CheckpointSummary},
    gas::GasCostSummary,
};
use iota_storage::{
    FileCompression, StorageFormat,
    blob::{Blob, BlobEncoding},
};
use iota_types::{
    committee::EpochId,
    crypto::KeypairTraits,
    effects::TransactionEffectsExtForTesting,
    full_checkpoint_content::{CheckpointData, CheckpointTransaction},
    messages_checkpoint::{
        CertifiedCheckpointSummary, CheckpointContentsExt, CheckpointSequenceNumber,
        CheckpointSummaryExt, SignedCheckpointSummary,
    },
    transaction::{TransactionAPI, TransactionEnvelope},
    utils::make_committee_key,
};
use prometheus_filtered::Registry;
use rand::{SeedableRng, prelude::StdRng};
use tempfile::NamedTempFile;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    time::timeout,
};
use tokio_util::sync::CancellationToken;

use crate::{
    DataIngestionMetrics, FileProgressStore, IndexerExecutor, IngestionError, IngestionLimit,
    IngestionResult, ProgressStore, ReaderOptions, Reducer, ShutdownAction, Worker, WorkerPool,
    history::{
        CHECKPOINT_FILE_MAGIC,
        manifest::{Manifest, create_file_metadata_from_bytes, finalize_manifest},
    },
    progress_store::ExecutorProgress,
    reader::v2::{CheckpointReaderConfig, RemoteUrl},
};

async fn add_worker_pool<W: Worker + 'static>(
    indexer: &mut IndexerExecutor<FileProgressStore>,
    worker: W,
    concurrency: usize,
) -> IngestionResult<()> {
    let worker_pool = WorkerPool::new(worker, "test".to_string(), concurrency, Default::default());
    indexer.register(worker_pool).await?;
    Ok(())
}

async fn run(
    indexer: IndexerExecutor<FileProgressStore>,
    path: impl Into<Option<PathBuf>>,
    duration: impl Into<Option<Duration>>,
    token: CancellationToken,
) -> IngestionResult<ExecutorProgress> {
    let reader_options = ReaderOptions {
        tick_interval_ms: 10,
        batch_size: 1,
        ..Default::default()
    };

    let indexer_executor_fut = indexer.run_with_config(CheckpointReaderConfig {
        reader_options,
        ingestion_path: path.into(),
        remote_store_url: None,
    });

    if let Some(duration) = duration.into() {
        tokio::task::spawn({
            let token = token.clone();
            async move {
                tokio::time::sleep(duration).await;
                token.cancel();
            }
        });
    };

    indexer_executor_fut.await
}

struct ExecutorBundle {
    executor: IndexerExecutor<FileProgressStore>,
    _progress_file: NamedTempFile,
    token: CancellationToken,
}

#[derive(Clone)]
struct TestWorker;

#[async_trait]
impl Worker for TestWorker {
    type Message = ();
    type Error = IngestionError;

    async fn process_checkpoint(
        &self,
        _checkpoint: Arc<CheckpointData>,
    ) -> Result<Self::Message, Self::Error> {
        Ok(())
    }
}

/// This worker implementation always returns an error when processing a
/// checkpoint.
///
/// Useful for testing graceful shutdown logic.
#[derive(Clone)]
struct FaultyWorker;

#[async_trait]
impl Worker for FaultyWorker {
    type Message = ();
    type Error = IngestionError;

    async fn process_checkpoint(
        &self,
        _checkpoint: Arc<CheckpointData>,
    ) -> Result<Self::Message, Self::Error> {
        Err(IngestionError::CheckpointProcessing(
            "unable to process checkpoint".into(),
        ))
    }
}

/// A Reducer implementation that commits messages in fixed-size batches.
///
/// This reducer maintains a count of committed batches and enforces a fixed
/// batch size before triggering commits. It's primarily used for testing the
/// worker pool and reducer functionality.
struct FixedBatchSizeReducer {
    commit_count: Arc<AtomicU64>,
    batch_size: usize,
}

impl FixedBatchSizeReducer {
    fn new(batch_size: usize) -> Self {
        Self {
            commit_count: Arc::new(AtomicU64::new(0)),
            batch_size,
        }
    }
}

#[async_trait]
impl Reducer<TestWorker> for FixedBatchSizeReducer {
    async fn commit(&self, _batch: &[()]) -> Result<(), IngestionError> {
        self.commit_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    fn should_close_batch(&self, batch: &[()], _next_item: Option<&()>) -> bool {
        batch.len() >= self.batch_size
    }
}

/// This reducer implementation always returns an error when committing a batch.
///
/// Useful for testing graceful shutdown logic.
struct FaultyReducer {
    batch_size: usize,
}

impl FaultyReducer {
    fn new(batch_size: usize) -> Self {
        Self { batch_size }
    }
}

#[async_trait]
impl Reducer<TestWorker> for FaultyReducer {
    async fn commit(&self, _batch: &[()]) -> Result<(), IngestionError> {
        Err(IngestionError::Reducer("unable to commit data".into()))
    }

    fn should_close_batch(&self, batch: &[()], _next_item: Option<&()>) -> bool {
        batch.len() >= self.batch_size
    }
}

#[tokio::test]
async fn empty_pools() {
    let bundle = create_executor_bundle().await;
    let result = run(bundle.executor, None, None, bundle.token).await;
    assert!(matches!(result, Err(IngestionError::EmptyWorkerPool)));
}

#[tokio::test]
async fn basic_flow() {
    let mut bundle = create_executor_bundle().await;
    add_worker_pool(&mut bundle.executor, TestWorker, 5)
        .await
        .unwrap();
    let tmp_dir = iota_common::tempdir();
    for checkpoint_number in 0..20 {
        let bytes = mock_checkpoint_data_bytes(checkpoint_number);
        std::fs::write(
            tmp_dir.path().join(format!("{checkpoint_number}.chk")),
            bytes,
        )
        .unwrap();
    }
    let result = run(
        bundle.executor,
        tmp_dir.path().to_path_buf(),
        Duration::from_secs(3),
        bundle.token,
    )
    .await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap().get("test"), Some(&20));
}

// Tests the graceful shutdown behavior when a checkpoint upper limit is
// provided.
//
// This test verifies that:
// 1. The framework process checkpoints not exceeding the upper limit.
// 2. The Executor handles the upper limit correctly by not sending any more
//    checkpoints to workers.
// 3. The graceful shutdown is triggered by the Executor when the Worker reports
//    the processed checkpoint matching the upper limit one, making sure to not
//    trigger the shutdown prematurely.
#[tokio::test]
async fn basic_flow_with_checkpoint_upper_limit() {
    let mut bundle = create_executor_bundle().await;
    add_worker_pool(&mut bundle.executor, TestWorker, 5)
        .await
        .unwrap();
    let tmp_dir = iota_common::tempdir();
    // range not inclusive actual chk files generated 0.chk .. 24.chk
    for checkpoint_number in 0..25 {
        let bytes = mock_checkpoint_data_bytes(checkpoint_number);
        std::fs::write(
            tmp_dir.path().join(format!("{checkpoint_number}.chk")),
            bytes,
        )
        .unwrap();
    }
    // process until we reach the checkpoint sequence number 19. Subsequent
    // checkpoints should be skipped.
    bundle
        .executor
        .with_ingestion_limit(IngestionLimit::MaxCheckpoint(19));

    let result = run(
        bundle.executor,
        tmp_dir.path().to_path_buf(),
        None,
        bundle.token,
    )
    .await;
    assert!(result.is_ok());
    // expect watermark == processed_last_checkpoint + 1 == 20.
    assert_eq!(result.unwrap().get("test"), Some(&20));
}

// Tests the graceful shutdown behavior when a checkpoint upper limit is
// provided through a custom callback.
//
// This test verifies that:
// 1. The framework process checkpoints not exceeding the upper limit.
// 2. The Executor handles the upper limit correctly by not sending any more
//    checkpoints to workers.
// 3. The graceful shutdown is triggered by the Executor when the Worker reports
//    the processed checkpoint matching the upper limit one, making sure to not
//    trigger the shutdown prematurely.
#[tokio::test]
async fn basic_flow_with_custom_callback_checkpoint_limit() {
    let mut bundle = create_executor_bundle().await;
    add_worker_pool(&mut bundle.executor, TestWorker, 5)
        .await
        .unwrap();
    let tmp_dir = iota_common::tempdir();
    // range not inclusive actual chk files generated 0.chk .. 24.chk
    for checkpoint_number in 0..25 {
        let bytes = mock_checkpoint_data_bytes(checkpoint_number);
        std::fs::write(
            tmp_dir.path().join(format!("{checkpoint_number}.chk")),
            bytes,
        )
        .unwrap();
    }

    // process until we reach the checkpoint sequence number 19 (inclusive).
    // Subsequent checkpoints should be skipped.
    bundle.executor.shutdown_when(|chk| {
        if chk.checkpoint_summary.sequence_number == 19 {
            return ShutdownAction::IncludeAndShutdown;
        }
        ShutdownAction::Continue
    });

    let result = run(
        bundle.executor,
        tmp_dir.path().to_path_buf(),
        None,
        bundle.token,
    )
    .await;
    assert!(result.is_ok());
    // expect watermark == processed_last_checkpoint + 1 == 20.
    assert_eq!(result.unwrap().get("test"), Some(&20));
}

// Tests the graceful shutdown behavior when an epoch upper limit is
// provided.
//
// This test verifies that:
// 1. The framework process checkpoints not exceeding the epoch upper limit.
// 2. The Executor handles the upper limit correctly by not sending any more
//    checkpoints to workers.
// 3. The graceful shutdown is triggered by the Executor when the Worker reports
//    the processed checkpoint matching the upper limit one, making sure to not
//    trigger the shutdown prematurely.
#[tokio::test]
async fn basic_flow_with_epoch_upper_limit() {
    let mut bundle = create_executor_bundle().await;
    add_worker_pool(&mut bundle.executor, TestWorker, 5)
        .await
        .unwrap();
    let tmp_dir = iota_common::tempdir();
    // range not inclusive actual chk files generated 0.chk .. 14.chk
    for checkpoint_number in 0..15 {
        let bytes = mock_checkpoint_data_bytes(checkpoint_number);
        std::fs::write(
            tmp_dir.path().join(format!("{checkpoint_number}.chk")),
            bytes,
        )
        .unwrap();
    }
    // create a single checkpoint with a new epoch to simulate epoch change
    // this checkpoint should not be processed
    let bytes = mock_checkpoint_data_bytes_with_opt(15, 1, vec![]);
    std::fs::write(tmp_dir.path().join("15.chk"), bytes).unwrap();

    // process until we reach the epoch upper limit 0, so it should process up to
    // checkpoint file 14.chk (inclusive). Subsequent checkpoints (15.chk) should be
    // skipped.
    bundle
        .executor
        .with_ingestion_limit(IngestionLimit::EndOfEpoch(0));

    let result = run(
        bundle.executor,
        tmp_dir.path().to_path_buf(),
        None,
        bundle.token,
    )
    .await;
    assert!(result.is_ok());
    // expect watermark == processed_last_checkpoint + 1 == 15.
    assert_eq!(result.unwrap().get("test"), Some(&15));
}

// Tests the graceful shutdown behavior when an epoch upper limit is
// provided through a custom callback.
//
// This test verifies that:
// 1. The framework process checkpoints not exceeding the epoch upper limit.
// 2. The Executor handles the upper limit correctly by not sending any more
//    checkpoints to workers.
// 3. The graceful shutdown is triggered by the Executor when the Worker reports
//    the processed checkpoint matching the upper limit one, making sure to not
//    trigger the shutdown prematurely.
#[tokio::test]
async fn basic_flow_with_custom_callback_epoch_limit() {
    let mut bundle = create_executor_bundle().await;
    add_worker_pool(&mut bundle.executor, TestWorker, 5)
        .await
        .unwrap();
    let tmp_dir = iota_common::tempdir();
    // range not inclusive actual chk files generated 0.chk .. 14.chk
    for checkpoint_number in 0..15 {
        let bytes = mock_checkpoint_data_bytes(checkpoint_number);
        std::fs::write(
            tmp_dir.path().join(format!("{checkpoint_number}.chk")),
            bytes,
        )
        .unwrap();
    }
    // create a single checkpoint with a new epoch to simulate epoch change
    // this checkpoint should not be processed
    let bytes = mock_checkpoint_data_bytes_with_opt(15, 1, vec![]);
    std::fs::write(tmp_dir.path().join("15.chk"), bytes).unwrap();

    // process until we reach the epoch upper limit 0, so it should process up to
    // checkpoint file 14.chk (inclusive). Subsequent checkpoints (15.chk) should be
    // skipped.
    bundle.executor.shutdown_when(|chk| {
        if chk.checkpoint_summary.epoch > 0 {
            return ShutdownAction::ExcludeAndShutdown;
        }
        ShutdownAction::Continue
    });

    let result = run(
        bundle.executor,
        tmp_dir.path().to_path_buf(),
        None,
        bundle.token,
    )
    .await;
    assert!(result.is_ok());
    // expect watermark == processed_last_checkpoint + 1 == 15.
    assert_eq!(result.unwrap().get("test"), Some(&15));
}

// Test: graceful shutdown via a custom callback.
//
// Scenario:
// A transaction with a known digest is embedded only in checkpoint 10. The
// callback `shutdown_when` inspects each processed checkpoint and returns
// `ShutdownAction::IncludeAndShutdown` enum variant if it contains the target
// transaction digest. Once the condition is met, the Executor will stop sending
// new checkpoints and will wait for all previously sent checkpoints to be
// processed by workers before initiating graceful shutdown process. 11.chk is
// skipped and becomes the upper limit.
//
// This test verifies that:
// 1. The framework only processes checkpoints with sequence numbers strictly
//    less than the one containing the matching transaction digest (0.chk =>
//    10.chk).
// 2. Upon hitting the shutdown condition, the Executor stops dispatching
//    further checkpoints (11.chk and later are not sent to workers).
// 3. Graceful shutdown is triggered exactly when the matching digest would be
//    encountered, never prematurely.
#[tokio::test]
async fn basic_flow_with_custom_callback() {
    let mut bundle = create_executor_bundle().await;
    add_worker_pool(&mut bundle.executor, TestWorker, 5)
        .await
        .unwrap();
    let tmp_dir = iota_common::tempdir();

    let tx = Transaction::new(
        TransactionKind::RandomnessStateUpdate(RandomnessStateUpdate {
            epoch: 0,
            randomness_round: 0.into(),
            random_bytes: vec![],
            randomness_obj_initial_shared_version: Version::default(),
        }),
        Address::random(),
        ObjectReference::new(ObjectId::ZERO, Version::default(), ObjectDigest::MIN),
        0,
        0,
    );

    let transaction = TransactionEnvelope::from_data(tx, vec![]);
    let effects = TransactionEffects::new_empty_v1_for_testing(*transaction.digest());
    let ch_tx = CheckpointTransaction {
        transaction,
        effects,
        events: None,
        input_objects: vec![],
        output_objects: vec![],
    };

    let tx_digest = *ch_tx.transaction.digest();

    // range not inclusive actual chk files generated 0.chk .. 14.chk
    for checkpoint_number in 0..15 {
        if checkpoint_number == 10 {
            let bytes =
                mock_checkpoint_data_bytes_with_opt(checkpoint_number, 0, vec![ch_tx.clone()]);
            std::fs::write(
                tmp_dir.path().join(format!("{checkpoint_number}.chk")),
                bytes,
            )
            .unwrap();
        } else {
            let bytes = mock_checkpoint_data_bytes(checkpoint_number);
            std::fs::write(
                tmp_dir.path().join(format!("{checkpoint_number}.chk")),
                bytes,
            )
            .unwrap();
        }
    }

    // process until we reach the checkpoint number 10 the one that holds the
    // transaction digest.
    bundle.executor.shutdown_when(move |chk| {
        if chk
            .transactions
            .iter()
            .any(|tx| *tx.transaction.digest() == tx_digest)
        {
            return ShutdownAction::IncludeAndShutdown;
        }
        ShutdownAction::Continue
    });

    let result = run(
        bundle.executor,
        tmp_dir.path().to_path_buf(),
        None,
        bundle.token,
    )
    .await;
    assert!(result.is_ok());
    // expect watermark == processed_last_checkpoint + 1 == 11.
    assert_eq!(result.unwrap().get("test"), Some(&11));
}

// Tests the graceful shutdown behavior when workers encounter persistent
// failures.
//
// This test verifies that:
// 1. When Worker::process_checkpoint implementation continuously fails.
// 2. The exponential backoff retry mechanism would normally create an loop
//    until the successful value is returned.
// 3. The graceful shutdown logic successfully breaks these retry loops upon
//    cancellation.
// 4. All workers exit cleanly without processing any checkpoints.
//
// The test uses `FaultyWorker` which always fails, simulating a worst-case
// scenario where all workers are unable to process checkpoints.
#[tokio::test]
async fn graceful_shutdown_faulty_worker() {
    let mut bundle = create_executor_bundle().await;
    // all worker pool's workers will not be able to process any checkpoint
    add_worker_pool(&mut bundle.executor, FaultyWorker, 5)
        .await
        .unwrap();
    let tmp_dir = iota_common::tempdir();
    for checkpoint_number in 0..20 {
        let bytes = mock_checkpoint_data_bytes(checkpoint_number);
        std::fs::write(
            tmp_dir.path().join(format!("{checkpoint_number}.chk")),
            bytes,
        )
        .unwrap();
    }
    let result = run(
        bundle.executor,
        tmp_dir.path().to_path_buf(),
        Duration::from_secs(1),
        bundle.token,
    )
    .await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap().get("test"), Some(&0));
}

/// Tests the integration of WorkerPool with a FixedBatchSizeReducer.
///
/// This test verifies reducer processing logic:
/// - Creates 20 mock checkpoints.
/// - Configures reducer with fixed batch size of 5.
/// - Expects minimum 4 batch commits (20/5 = 4).
/// - ExecutorProgress should show 20 processed checkpoints.
#[tokio::test]
async fn worker_pool_with_reducer() {
    // create a reducer with max batch of 5
    let reducer = FixedBatchSizeReducer::new(5);
    let commit_count = reducer.commit_count.clone();
    let mut bundle = create_executor_bundle().await;
    // Create worker pool with reducer
    let pool = WorkerPool::new_with_reducer(
        TestWorker,
        "test".to_string(),
        5,
        Default::default(),
        reducer,
    );
    bundle.executor.register(pool).await.unwrap();

    let tmp_dir = iota_common::tempdir();
    for checkpoint_number in 0..20 {
        let bytes = mock_checkpoint_data_bytes(checkpoint_number);
        std::fs::write(
            tmp_dir.path().join(format!("{checkpoint_number}.chk")),
            bytes,
        )
        .unwrap();
    }
    let result = run(
        bundle.executor,
        tmp_dir.path().to_path_buf(),
        Duration::from_secs(3),
        bundle.token,
    )
    .await;
    // 4 commits (batches of 5 checkpoints)
    assert_eq!(commit_count.load(Ordering::SeqCst), 4);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().get("test"), Some(&20));
}

// Tests the graceful shutdown behavior when reducer encounter persistent
// failures.
//
// This test verifies that:
// 1. When Reducer::commit implementation continuously fails.
// 2. The exponential backoff retry mechanism would normally create a loop until
//    the successful value is returned.
// 3. The graceful shutdown logic successfully breaks these retry loops upon
//    cancellation.
// 4. The Reducer exit cleanly without committing any batch.
//
// The test uses `FaultyReducer` which always fails, simulating a worst-case
// scenario where all WorkerPools are unable to send progress data to
// IndexerExecutor.
#[tokio::test]
async fn graceful_shutdown_faulty_reducer() {
    // create a reducer with max batch of 5
    let reducer = FaultyReducer::new(5);
    let mut bundle = create_executor_bundle().await;
    // Create worker pool with reducer
    let pool = WorkerPool::new_with_reducer(
        TestWorker,
        "test".to_string(),
        5,
        Default::default(),
        reducer,
    );
    bundle.executor.register(pool).await.unwrap();

    let tmp_dir = iota_common::tempdir();
    for checkpoint_number in 0..20 {
        let bytes = mock_checkpoint_data_bytes(checkpoint_number);
        std::fs::write(
            tmp_dir.path().join(format!("{checkpoint_number}.chk")),
            bytes,
        )
        .unwrap();
    }
    let result = run(
        bundle.executor,
        tmp_dir.path().to_path_buf(),
        Duration::from_secs(1),
        bundle.token,
    )
    .await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap().get("test"), Some(&0));
}

/// Tests the atomicity of FileProgressStore's save operation by simulating a
/// crash/interruption.
///
/// This test attempts to save a new value with a very short timeout, simulating
/// a crash before the save completes. It verifies that if the save is
/// interrupted, the original value remains unchanged, demonstrating that
/// FileProgressStore does not leave the file in a partial or corrupted state
/// even if the save is not completed.
#[tokio::test]
async fn file_progress_store_save_timeout_simulates_crash() {
    // Setup: create a FileProgressStore with initial data
    let progress_file = NamedTempFile::new().unwrap();
    let path = progress_file.path().to_path_buf();
    let mut store = FileProgressStore::new(path.clone()).await.unwrap();

    // Save an initial value
    store.save("task1".to_string(), 42).await.unwrap();

    // Confirm the value is present
    let value = store.load("task1".to_string()).await.unwrap();
    assert_eq!(value, 42);

    // Attempt to save a new value, but with a very short timeout to simulate a
    // crash/interruption
    let result = timeout(
        Duration::from_nanos(5),
        store.save("task1".to_string(), 100),
    )
    .await;

    // The operation should time out (simulate crash)
    assert!(result.is_err(), "save did not time out as expected");

    // The value should still be the old value, as the save was interrupted
    let value = store.load("task1".to_string()).await.unwrap();
    assert_eq!(
        value, 42,
        "value should remain unchanged after interrupted save"
    );
}

/// Tests the basic save and load functionality of FileProgressStore.
///
/// This test saves an initial value, verifies it, then saves a new value and
/// verifies the update. It demonstrates that FileProgressStore correctly
/// persists and retrieves checkpoint data.
#[tokio::test]
async fn file_progress_store() {
    // Setup: create a FileProgressStore with initial data
    let progress_file = NamedTempFile::new().unwrap();
    let path = progress_file.path().to_path_buf();
    let mut store = FileProgressStore::new(path.clone()).await.unwrap();

    // Save an initial value
    store.save("task1".to_string(), 42).await.unwrap();

    // Confirm the value is present
    let value = store.load("task1".to_string()).await.unwrap();
    assert_eq!(value, 42);

    // Save a new value
    store.save("task1".to_string(), 100).await.unwrap();

    // Confirm the value is updated
    let value = store.load("task1".to_string()).await.unwrap();
    assert_eq!(value, 100);
}

async fn create_executor_bundle() -> ExecutorBundle {
    let progress_file = NamedTempFile::new().unwrap();
    let path = progress_file.path().to_path_buf();
    std::fs::write(path.clone(), "{}").unwrap();
    let progress_store = FileProgressStore::new(path).await.unwrap();
    let token = CancellationToken::new();
    let child_token = token.child_token();
    let executor = IndexerExecutor::new(
        progress_store,
        1,
        DataIngestionMetrics::new(&Registry::new()),
        child_token,
    );
    ExecutorBundle {
        executor,
        _progress_file: progress_file,
        token,
    }
}

const RNG_SEED: [u8; 32] = [
    21, 23, 199, 200, 234, 250, 252, 178, 94, 15, 202, 178, 62, 186, 88, 137, 233, 192, 130, 157,
    179, 179, 65, 9, 31, 249, 221, 123, 225, 112, 199, 247,
];

fn mock_checkpoint_data_bytes(seq_number: CheckpointSequenceNumber) -> Vec<u8> {
    mock_checkpoint_data_bytes_with_opt(seq_number, 0, vec![])
}

fn mock_checkpoint_data_bytes_with_opt(
    seq_number: CheckpointSequenceNumber,
    epoch: EpochId,
    transactions: Vec<CheckpointTransaction>,
) -> Vec<u8> {
    Blob::encode(
        &mock_checkpoint_data(seq_number, epoch, transactions),
        BlobEncoding::Bcs,
    )
    .unwrap()
    .to_bytes()
}

fn mock_checkpoint_data(
    seq_number: CheckpointSequenceNumber,
    epoch: EpochId,
    transactions: Vec<CheckpointTransaction>,
) -> CheckpointData {
    let mut rng = StdRng::from_seed(RNG_SEED);
    let (keys, committee) = make_committee_key(&mut rng);
    let contents = CheckpointContents::new_with_digests_only_for_tests(vec![]);
    let summary = CheckpointSummary::new_with_protocol_config(
        &ProtocolConfig::get_for_max_version_UNSAFE(),
        epoch,
        seq_number,
        0,
        &contents,
        None,
        GasCostSummary::default(),
        None,
        0,
        Vec::new(),
    );

    let sign_infos: Vec<_> = keys
        .iter()
        .map(|k| {
            let name = k.public().into();
            SignedCheckpointSummary::sign(committee.epoch, &summary, k, name)
        })
        .collect();

    CheckpointData {
        checkpoint_summary: CertifiedCheckpointSummary::new(summary, sign_infos, &committee)
            .unwrap(),
        checkpoint_contents: contents,
        transactions,
    }
}

/// Serves a fixed set of files over HTTP and counts the requests received for
/// each of them.
///
/// Returns the base URL of the server and the request counter.
async fn spawn_counting_file_server(
    files: HashMap<String, Vec<u8>>,
    token: CancellationToken,
) -> (String, Arc<Mutex<HashMap<String, usize>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let requests = Arc::new(Mutex::new(HashMap::new()));
    let files = Arc::new(files);

    tokio::spawn({
        let requests = requests.clone();
        async move {
            loop {
                let socket = tokio::select! {
                    _ = token.cancelled() => break,
                    accepted = listener.accept() => match accepted {
                        Ok((socket, _)) => socket,
                        Err(_) => break,
                    },
                };
                tokio::spawn(serve_connection(socket, files.clone(), requests.clone()));
            }
        }
    });

    (url, requests)
}

/// Answers every request on `socket` until the peer closes the connection.
async fn serve_connection(
    mut socket: TcpStream,
    files: Arc<HashMap<String, Vec<u8>>>,
    requests: Arc<Mutex<HashMap<String, usize>>>,
) {
    let mut pending = Vec::new();
    let mut chunk = [0u8; 1024];
    loop {
        // Requests are all GETs, so the head is the whole request.
        let head_len = loop {
            if let Some(offset) = pending.windows(4).position(|window| window == b"\r\n\r\n") {
                break offset + 4;
            }
            match socket.read(&mut chunk).await {
                Ok(0) | Err(_) => return,
                Ok(read) => pending.extend_from_slice(&chunk[..read]),
            }
        };
        let head = String::from_utf8_lossy(&pending[..head_len]).into_owned();
        pending.drain(..head_len);

        let path = head
            .split_whitespace()
            .nth(1)
            .unwrap_or_default()
            .trim_start_matches('/')
            .to_owned();
        *requests.lock().unwrap().entry(path.clone()).or_default() += 1;

        let response = match files.get(&path) {
            Some(body) => {
                let mut response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nETag: \"{path}\"\r\nLast-Modified: \
                     Wed, 21 Oct 2015 07:28:00 +0000\r\n\r\n",
                    body.len()
                )
                .into_bytes();
                response.extend_from_slice(body);
                response
            }
            None => b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n".to_vec(),
        };
        if socket.write_all(&response).await.is_err() {
            return;
        }
    }
}

/// Builds a historical store holding `checkpoint_count` checkpoints in a
/// single file, as the file name to contents map a file server expects.
fn mock_historical_store(checkpoint_count: CheckpointSequenceNumber) -> HashMap<String, Vec<u8>> {
    let mut file = Vec::new();
    file.extend_from_slice(&CHECKPOINT_FILE_MAGIC.to_be_bytes());
    file.push(StorageFormat::Blob as u8);
    file.push(FileCompression::None as u8);
    for sequence_number in 0..checkpoint_count {
        Blob::encode(
            &mock_checkpoint_data(sequence_number, 0, vec![]),
            BlobEncoding::Bcs,
        )
        .unwrap()
        .write(&mut file)
        .unwrap();
    }

    let file_metadata =
        create_file_metadata_from_bytes(Bytes::from(file.clone()), 0..checkpoint_count).unwrap();
    let mut manifest = Manifest::new(0);
    manifest.update(checkpoint_count, file_metadata);
    let manifest_bytes = finalize_manifest(manifest).unwrap();

    HashMap::from([
        ("0.chk".to_owned(), file),
        ("MANIFEST".to_owned(), manifest_bytes.to_vec()),
    ])
}

/// A worker that never finishes processing a checkpoint, so that the executor
/// makes no progress and the reader's capacity is never released.
#[derive(Clone)]
struct StalledWorker(CancellationToken);

#[async_trait]
impl Worker for StalledWorker {
    type Message = ();
    type Error = IngestionError;

    async fn process_checkpoint(
        &self,
        _checkpoint: Arc<CheckpointData>,
    ) -> Result<Self::Message, Self::Error> {
        self.0.cancelled().await;
        Ok(())
    }
}

/// Once the reader has as many checkpoints in progress as it is allowed to
/// hold, it must stop fetching from the historical store instead of
/// downloading files it can only throw away again.
#[tokio::test]
async fn historical_read_stops_fetching_at_capacity() {
    let files = mock_historical_store(4);
    let server_token = CancellationToken::new();
    let (url, requests) = spawn_counting_file_server(files, server_token.clone()).await;

    let bundle = create_executor_bundle().await;
    let mut executor = bundle.executor;
    add_worker_pool(&mut executor, StalledWorker(bundle.token.clone()), 1)
        .await
        .unwrap();

    let executor_fut = executor.run_with_config(CheckpointReaderConfig {
        reader_options: ReaderOptions {
            tick_interval_ms: 10,
            batch_size: 1,
            // Room for a single checkpoint, so the reader is at capacity as
            // soon as it has handed over the first one.
            data_limit: 1,
            ..Default::default()
        },
        ingestion_path: None,
        remote_store_url: Some(RemoteUrl::HybridHistoricalStore {
            historical_url: url,
            live_url: None,
        }),
    });
    let executor_handle = tokio::spawn(executor_fut);

    // Long enough for a reader that keeps fetching to tick many times over.
    tokio::time::sleep(Duration::from_secs(1)).await;
    bundle.token.cancel();
    let _ = executor_handle.await;
    server_token.cancel();

    let checkpoint_file_requests = requests.lock().unwrap().get("0.chk").copied().unwrap_or(0);
    assert_eq!(
        checkpoint_file_requests, 1,
        "the reader downloaded the same checkpoint file {checkpoint_file_requests} times while \
         being at capacity"
    );
}
