// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    fs,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use async_trait::async_trait;
use iota_protocol_config::ProtocolConfig;
use iota_storage::blob::{Blob, BlobEncoding};
use iota_types::{
    base_types::{IotaAddress, ObjectID, SequenceNumber},
    committee::EpochId,
    crypto::{KeypairTraits, RandomnessRound},
    digests::ObjectDigest,
    effects::TransactionEffects,
    full_checkpoint_content::{CheckpointData, CheckpointTransaction},
    gas::GasCostSummary,
    messages_checkpoint::{
        CertifiedCheckpointSummary, CheckpointContents, CheckpointSequenceNumber,
        CheckpointSummary, SignedCheckpointSummary,
    },
    transaction::{RandomnessStateUpdate, Transaction, TransactionData, TransactionKind},
    utils::make_committee_key,
};
use prometheus::Registry;
use rand::{SeedableRng, prelude::StdRng};
use tempfile::NamedTempFile;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

use crate::{
    DataIngestionMetrics, FileProgressStore, IndexerExecutor, IngestionError, IngestionLimit,
    IngestionResult, ProgressStore, ReaderOptions, Reducer, ShutdownAction, Worker, WorkerPool,
    progress_store::ExecutorProgress,
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

    match duration.into() {
        None => {
            indexer
                .run(
                    path.into().unwrap_or_else(temp_dir),
                    None,
                    vec![],
                    reader_options,
                )
                .await
        }
        Some(duration) => {
            let handle = tokio::task::spawn(indexer.run(
                path.into().unwrap_or_else(temp_dir),
                None,
                vec![],
                reader_options,
            ));
            tokio::time::sleep(duration).await;
            token.cancel();
            handle.await.map_err(|err| IngestionError::Shutdown {
                component: "indexer executor".into(),
                msg: err.to_string(),
            })?
        }
    }
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
    let path = temp_dir();
    for checkpoint_number in 0..20 {
        let bytes = mock_checkpoint_data_bytes(checkpoint_number);
        std::fs::write(path.join(format!("{checkpoint_number}.chk")), bytes).unwrap();
    }
    let result = run(bundle.executor, path, Duration::from_secs(3), bundle.token).await;
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
    let path = temp_dir();
    // range not inclusive actual chk files generated 0.chk .. 24.chk
    for checkpoint_number in 0..25 {
        let bytes = mock_checkpoint_data_bytes(checkpoint_number);
        std::fs::write(path.join(format!("{checkpoint_number}.chk")), bytes).unwrap();
    }
    // process until we reach the checkpoint sequence number 19. Subsequent
    // checkpoints should be skipped.
    bundle
        .executor
        .with_ingestion_limit(IngestionLimit::MaxCheckpoint(19));

    let result = run(bundle.executor, path.clone(), None, bundle.token).await;
    assert!(result.is_ok());
    // expect watermark == processed_last_checkpoint + 1 == 20.
    assert_eq!(result.unwrap().get("test"), Some(&20));
    // remove leftover checkpoint files.
    fs::remove_dir_all(path).unwrap();
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
    let path = temp_dir();
    // range not inclusive actual chk files generated 0.chk .. 24.chk
    for checkpoint_number in 0..25 {
        let bytes = mock_checkpoint_data_bytes(checkpoint_number);
        std::fs::write(path.join(format!("{checkpoint_number}.chk")), bytes).unwrap();
    }

    // process until we reach the checkpoint sequence number 19 (inclusive).
    // Subsequent checkpoints should be skipped.
    bundle.executor.shutdown_when(|chk| {
        if chk.checkpoint_summary.sequence_number == 19 {
            return ShutdownAction::IncludeAndShutdown;
        }
        ShutdownAction::Continue
    });

    let result = run(bundle.executor, path.clone(), None, bundle.token).await;
    assert!(result.is_ok());
    // expect watermark == processed_last_checkpoint + 1 == 20.
    assert_eq!(result.unwrap().get("test"), Some(&20));
    // remove leftover checkpoint files.
    fs::remove_dir_all(path).unwrap();
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
    let path = temp_dir();
    // range not inclusive actual chk files generated 0.chk .. 14.chk
    for checkpoint_number in 0..15 {
        let bytes = mock_checkpoint_data_bytes(checkpoint_number);
        std::fs::write(path.join(format!("{checkpoint_number}.chk")), bytes).unwrap();
    }
    // create a single checkpoint with a new epoch to simulate epoch change
    // this checkpoint should not be processed
    let bytes = mock_checkpoint_data_bytes_with_opt(15, 1, vec![]);
    std::fs::write(path.join("15.chk"), bytes).unwrap();

    // process until we reach the epoch upper limit 0, so it should process up to
    // checkpoint file 14.chk (inclusive). Subsequent checkpoints (15.chk) should be
    // skipped.
    bundle
        .executor
        .with_ingestion_limit(IngestionLimit::EndOfEpoch(0));

    let result = run(bundle.executor, path.clone(), None, bundle.token).await;
    assert!(result.is_ok());
    // expect watermark == processed_last_checkpoint + 1 == 15.
    assert_eq!(result.unwrap().get("test"), Some(&15));
    // remove leftover checkpoint files.
    fs::remove_dir_all(path).unwrap();
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
    let path = temp_dir();
    // range not inclusive actual chk files generated 0.chk .. 14.chk
    for checkpoint_number in 0..15 {
        let bytes = mock_checkpoint_data_bytes(checkpoint_number);
        std::fs::write(path.join(format!("{checkpoint_number}.chk")), bytes).unwrap();
    }
    // create a single checkpoint with a new epoch to simulate epoch change
    // this checkpoint should not be processed
    let bytes = mock_checkpoint_data_bytes_with_opt(15, 1, vec![]);
    std::fs::write(path.join("15.chk"), bytes).unwrap();

    // process until we reach the epoch upper limit 0, so it should process up to
    // checkpoint file 14.chk (inclusive). Subsequent checkpoints (15.chk) should be
    // skipped.
    bundle.executor.shutdown_when(|chk| {
        if chk.checkpoint_summary.epoch > 0 {
            return ShutdownAction::ExcludeAndShutdown;
        }
        ShutdownAction::Continue
    });

    let result = run(bundle.executor, path.clone(), None, bundle.token).await;
    assert!(result.is_ok());
    // expect watermark == processed_last_checkpoint + 1 == 15.
    assert_eq!(result.unwrap().get("test"), Some(&15));
    // remove leftover checkpoint files.
    fs::remove_dir_all(path).unwrap();
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
    let path = temp_dir();

    let tx_data = TransactionData::new(
        TransactionKind::RandomnessStateUpdate(RandomnessStateUpdate {
            epoch: 0,
            randomness_round: RandomnessRound::new(0),
            random_bytes: vec![],
            randomness_obj_initial_shared_version: SequenceNumber::new(),
        }),
        IotaAddress::random_for_testing_only(),
        (ObjectID::ZERO, SequenceNumber::default(), ObjectDigest::MIN),
        0,
        0,
    );

    let ch_tx = CheckpointTransaction {
        transaction: Transaction::from_data(tx_data, vec![]),
        effects: TransactionEffects::default(),
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
            std::fs::write(path.join(format!("{checkpoint_number}.chk")), bytes).unwrap();
        } else {
            let bytes = mock_checkpoint_data_bytes(checkpoint_number);
            std::fs::write(path.join(format!("{checkpoint_number}.chk")), bytes).unwrap();
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

    let result = run(bundle.executor, path.clone(), None, bundle.token).await;
    assert!(result.is_ok());
    // expect watermark == processed_last_checkpoint + 1 == 11.
    assert_eq!(result.unwrap().get("test"), Some(&11));
    // remove leftover checkpoint files.
    fs::remove_dir_all(path).unwrap();
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
    let path = temp_dir();
    for checkpoint_number in 0..20 {
        let bytes = mock_checkpoint_data_bytes(checkpoint_number);
        std::fs::write(path.join(format!("{checkpoint_number}.chk")), bytes).unwrap();
    }
    let result = run(bundle.executor, path, Duration::from_secs(1), bundle.token).await;
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

    let path = temp_dir();
    for checkpoint_number in 0..20 {
        let bytes = mock_checkpoint_data_bytes(checkpoint_number);
        std::fs::write(path.join(format!("{checkpoint_number}.chk")), bytes).unwrap();
    }
    let result = run(bundle.executor, path, Duration::from_secs(3), bundle.token).await;
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

    let path = temp_dir();
    for checkpoint_number in 0..20 {
        let bytes = mock_checkpoint_data_bytes(checkpoint_number);
        std::fs::write(path.join(format!("{checkpoint_number}.chk")), bytes).unwrap();
    }
    let result = run(bundle.executor, path, Duration::from_secs(1), bundle.token).await;
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

fn temp_dir() -> std::path::PathBuf {
    tempfile::tempdir()
        .expect("failed to open temporary directory")
        .keep()
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
    let mut rng = StdRng::from_seed(RNG_SEED);
    let (keys, committee) = make_committee_key(&mut rng);
    let contents = CheckpointContents::new_with_digests_only_for_tests(vec![]);
    let summary = CheckpointSummary::new(
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

    let checkpoint_data = CheckpointData {
        checkpoint_summary: CertifiedCheckpointSummary::new(summary, sign_infos, &committee)
            .unwrap(),
        checkpoint_contents: contents,
        transactions,
    };
    Blob::encode(&checkpoint_data, BlobEncoding::Bcs)
        .unwrap()
        .to_bytes()
}
