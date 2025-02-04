// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{path::PathBuf, time::Duration};

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
    ReaderOptions, Worker, WorkerPool, progress_store::ExecutorProgress,
};

async fn add_worker_pool<W: Worker + 'static>(
    indexer: &mut IndexerExecutor<FileProgressStore>,
    worker: W,
    concurrency: usize,
) -> IngestionResult<()> {
    let worker_pool = WorkerPool::new(worker, "test".to_string(), concurrency);
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
    type Error = IngestionError;

    async fn process_checkpoint(&self, _checkpoint: CheckpointData) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[tokio::test]
async fn empty_pools() {
    let bundle = create_executor_bundle();
    let result = run(bundle.executor, None, None, bundle.token).await;
    assert!(matches!(result, Err(IngestionError::EmptyWorkerPool)));
}

#[tokio::test]
async fn basic_flow() {
    let mut bundle = create_executor_bundle();
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

fn temp_dir() -> std::path::PathBuf {
    tempfile::tempdir()
        .expect("Failed to open temporary directory")
        .into_path()
}

fn create_executor_bundle() -> ExecutorBundle {
    let progress_file = NamedTempFile::new().unwrap();
    let path = progress_file.path().to_path_buf();
    std::fs::write(path.clone(), "{}").unwrap();
    let progress_store = FileProgressStore::new(path);
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
