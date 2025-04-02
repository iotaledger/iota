// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
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
    crypto::KeypairTraits,
    full_checkpoint_content::CheckpointData,
    gas::GasCostSummary,
    messages_checkpoint::{
        CertifiedCheckpointSummary, CheckpointContents, CheckpointSequenceNumber,
        CheckpointSummary, SignedCheckpointSummary,
    },
    utils::make_committee_key,
};
use prometheus::Registry;
use rand::{SeedableRng, prelude::StdRng};
use tempfile::NamedTempFile;
use tokio_util::sync::CancellationToken;

use crate::{
    DataIngestionMetrics, FileProgressStore, IndexerExecutor, IngestionError, IngestionResult,
    ReaderOptions, Reducer, Worker, WorkerPool, progress_store::ExecutorProgress,
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
    path: Option<PathBuf>,
    duration: Option<Duration>,
    token: CancellationToken,
) -> IngestionResult<ExecutorProgress> {
    let options = ReaderOptions {
        tick_interval_ms: 10,
        batch_size: 1,
        ..Default::default()
    };

    match duration {
        None => {
            indexer
                .run(path.unwrap_or_else(temp_dir), None, vec![], options)
                .await
        }
        Some(duration) => {
            let handle = tokio::task::spawn(indexer.run(
                path.unwrap_or_else(temp_dir),
                None,
                vec![],
                options,
            ));
            tokio::time::sleep(duration).await;
            token.cancel();
            handle.await.map_err(|err| IngestionError::Shutdown {
                component: "Indexer Executor".into(),
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
        std::fs::write(path.join(format!("{}.chk", checkpoint_number)), bytes).unwrap();
    }
    let result = run(
        bundle.executor,
        Some(path),
        Some(Duration::from_secs(1)),
        bundle.token,
    )
    .await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap().get("test"), Some(&20));
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
        std::fs::write(path.join(format!("{}.chk", checkpoint_number)), bytes).unwrap();
    }
    let result = run(
        bundle.executor,
        Some(path),
        Some(Duration::from_secs(1)),
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

    let path = temp_dir();
    for checkpoint_number in 0..20 {
        let bytes = mock_checkpoint_data_bytes(checkpoint_number);
        std::fs::write(path.join(format!("{}.chk", checkpoint_number)), bytes).unwrap();
    }
    let result = run(
        bundle.executor,
        Some(path),
        Some(Duration::from_secs(1)),
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

    let path = temp_dir();
    for checkpoint_number in 0..20 {
        let bytes = mock_checkpoint_data_bytes(checkpoint_number);
        std::fs::write(path.join(format!("{}.chk", checkpoint_number)), bytes).unwrap();
    }
    let result = run(
        bundle.executor,
        Some(path),
        Some(Duration::from_secs(1)),
        bundle.token,
    )
    .await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap().get("test"), Some(&0));
}

fn temp_dir() -> std::path::PathBuf {
    tempfile::tempdir()
        .expect("Failed to open temporary directory")
        .into_path()
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
    let mut rng = StdRng::from_seed(RNG_SEED);
    let (keys, committee) = make_committee_key(&mut rng);
    let contents = CheckpointContents::new_with_digests_only_for_tests(vec![]);
    let summary = CheckpointSummary::new(
        &ProtocolConfig::get_for_max_version_UNSAFE(),
        0,
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
        transactions: vec![],
    };
    Blob::encode(&checkpoint_data, BlobEncoding::Bcs)
        .unwrap()
        .to_bytes()
}
