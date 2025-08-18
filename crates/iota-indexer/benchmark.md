# Performance Report – Indexing and Backfill Performance of the Indexer

This report evaluates the historical backfill and indexing performance of the [iota-indexer](benchmark) application
using synthetic checkpoints generated using [iota-synthetic-ingestion](../iota-synthetic-ingestion).
The objectives of these benchmarks were to measure:

- checkpoints per second (CPS) on download, indexing, and commit pipelines
- transactions per second (TPS) on commit pipelines
- latency for indexing and database commits
- backfill rate (entries per second) and time to complete the backfill
- identify ingestion pipeline stages that constrain performance

Benchmarks are split into two parts:

1. Indexing performance
2. Historical backfill performance

## Test Environment

The benchmarks were conducted on a server with the following specifications:

### Hardware

| Component    | Spec                                                              |
| ------------ | ----------------------------------------------------------------- |
| Server Model | Netcup Root Server RS 8000 G11 (virtualized, dedicated resources) |
| CPU          | AMD EPYC™ 9634, 12 dedicated cores                                |
| RAM          | 32 GB DDR5 ECC                                                    |
| Storage      | 1 TB NVMe SSD                                                     |

### Software

| Component    | Spec                                                                                                                              |
| ------------ | --------------------------------------------------------------------------------------------------------------------------------- |
| OS           | Ubuntu 22.04.5 amd64 (minimal)                                                                                                    |
| Rust         | rustc 1.87.0 with required packages for the [iota](https://github.com/iotaledger/iota) repo                                       |
| Docker       | Docker Engine 28.3.3                                                                                                              |
| Postgres     | Docker container from [pg-services-local](../../dev-tools/pg-services-local)                                                      |
| iota-indexer | Compiled binary (release), version: https://github.com/iotaledger/iota/pull/8112/commits/060ec525391639598b4833f416398972abf994ac |
| Prometheus   | Docker container from [iota-indexer-monitoring](../../dev-tools/iota-indexer-monitoring)                                          |

All benchmarks started from a cold database (empty with only migrations applied) and the indexer database reset before
each run.
Logging was restricted to minimal output to reduce console overhead.
The default Prometheus scraping interval was changed from `15` seconds to `5` seconds to capture more detailed metrics
while avoiding overwhelming the Indexer.

## Dataset & Methodology

Datasets were generated using the [iota-synthetic-ingestion](../iota-synthetic-ingestion) application, which simulates
transaction execution and produces serialized checkpoint files consumable by applications
integrating [iota-data-ingestion](../iota-data-ingestion-core).

Each dataset contains a fixed number of checkpoints and transactions per checkpoint (see configurations below). Dataset
sizes were defined to provide a consistent ingestion load over an extended period while ensuring benchmark
repeatability.
All datasets where limited to 100,000 generated checkpoints.
Larger datasets were considered, but (1) it appeared that the runtimes are sufficient to capture required metrics and (

2. time to generate larger datasets was an issue, as the generation process is resource intensive as outlined below.

It should be noted, the generated datasets are designed for controlled and consistent load, and do not reflect the
activity of a production-like network. Currently, on mainnet, we see around 4–5 checkpoints per second. Most checkpoints
contain 4–6 transactions, typically including system transactions e.g. `ConsensusCommitPrologueV1`
or `RandomnessStateUpdate`. User transactions (e.g. transfer of coins as part of `ProgrammableTransaction`s) appear
regularly (in fewer quantity) but not in every checkpoint, resulting in variations in transaction counts between
consecutive checkpoints.

The datasets were structured as follows:

- `0.chk`: genesis checkpoint
- `1.chk`: checkpoint for the gas request transaction (funds subsequent benchmark transactions)
- `2.chk`+: checkpoint containing gas transfer transactions to reflect consistent user load

### Configurations

| Dataset | Transactions per Checkpoint | Total Checkpoints | Notes                                         | Data Generation Time |
| ------- | --------------------------- | ----------------- | --------------------------------------------- | -------------------- |
| 1       | 5                           | 100 k             | Baseline, TXs per checkpoint close to mainnet | 806.79s, ~22min      |
| 2       | 25                          | 100 k             | Mid-load, begin stressing indexing and commit | 4057.11s, ~1.12h     |
| 3       | 100                         | 100 k             | High-load, stress indexing and commit         | 15940.90s, ~4.42h    |

All configurations already include the genesis checkpoint and the checkpoint for the gas request transaction.
Checkpoint 0 provides the genesis, while checkpoint 1 contains a single transaction to request gas for the subsequent
checkpoints. Therefore, differences in transaction counts for the first two checkpoints are expected.

## Indexing Benchmark Metrics

The indexer can be divided into several major stages, each responsible for a specific part of the ingestion pipeline.
These stages include:

- Download stage: receives checkpoints from the `iota-data-ingestion-core` layer.
- Indexing stage: processes the received checkpoints and prepares them for database insertion.
- Commit stage: writes the processed checkpoints and transactions to the database.

All major stages have been analyzed and outlined below.
Metrics are derived from the existing Prometheus `IndexerMetrics` and were queried using `PromQL` from the deployed
Prometheus server.
Queries use `[5m]` range vectors to smooth out short term fluctuations while still reacting quickly to changes.
For each stage, we tracked throughput and/or latency to understand how the pipeline behaves under load.

### Download stage

Represents the average number of checkpoints per
second [arriving](https://github.com/iotaledger/iota/blob/develop/crates/iota-indexer/src/handlers/checkpoint_handler.rs#L96-L99)
from the `iota-data-ingestion-core` layer, calculated over the last 5 minutes.

```promql
rate(indexer_max_downloaded_checkpoint_sequence_number[5m])  # checkpoints/sec
```

### Indexing stage

Measures how many checkpoints per second are processed in
the [indexing stage](https://github.com/iotaledger/iota/blob/develop/crates/iota-indexer/src/handlers/checkpoint_handler.rs#L238).
This stage is responsible for processing the incoming checkpoint data and preparing it for database insertion.

```promql
rate(indexer_max_indexed_checkpoint_sequence_number[5m])  # checkpoints/sec
```

Track the 95th-percentile processing time (latency) for major indexing operations over the last 5 minutes.

Indexing objects latency:

```promql
histogram_quantile(0.95, sum(rate(indexer_indexing_objects_latency_bucket[5m])) by (le)) * 1000 # milliseconds
```

Indexing object changes latency:

```promql
histogram_quantile(0.95, sum(rate(indexer_indexing_tx_object_changes_latency_bucket[5m])) by (le)) * 1000 # milliseconds
```

Indexing packages latency:

```promql
histogram_quantile(0.95, sum(rate(indexer_indexing_packages_latency_bucket[5m])) by (le)) * 1000 # milliseconds
```

### Checkpoint Commit Queue Depth

Measures the number of checkpoints that have been indexed but are still waiting to be committed.
A value close to 0 means the commit stage is keeping up, a stable non-zero value would suggest the pipeline is full but
is draining at the same rate as it fills.
A increasing value would indicate the commit stage is falling behind and backlog is building up.

```promql
avg_over_time(
  clamp_min(
    indexer_max_indexed_checkpoint_sequence_number
  - indexer_max_committed_checkpoint_sequence_number,
  0
  )[5m:]
) # checkpoints in queue
```

### Commit stage

Measures checkpoints per second committed to the database over the last 5 minutes.

```promql
rate(indexer_total_tx_checkpoint_committed[5m])  # checkpoints/sec
```

Measures transactions per second committed to the database over the last 5 minutes.

```promql
rate(indexer_total_transaction_committed[5m])  # transactions/sec
```

95th-percentile time to commit a batch of checkpoints to the database, calculated over the last 5 minutes.

```promql
histogram_quantile(0.95, sum(rate(indexer_checkpoint_db_commit_latency_bucket[5m])) by (le)) * 1000 # milliseconds
```

### End to end lag

Number of checkpoints the indexer is behind the fullnode head at commit time.
Calculated as the difference between the latest fullnode checkpoint sequence number and the maximum committed checkpoint
sequence number, averaged over the last 5 minutes.

```promql
avg_over_time(
  clamp_min(
    indexer_latest_fullnode_checkpoint_sequence_number
  - indexer_max_committed_checkpoint_sequence_number,
  0
  )[5m:]
) # checkpoints behind
```

## Indexing Benchmark Results

The following table summarizes the benchmark results for each dataset configuration.
Two measurement windows are used:

- W1 early (T+6m, 5m avg), measures (T+1m to T+5m) after warm-up (~T+30s) so the averaging window contains only post
  warm-up performance
- W2 sustained (T+14m, 5m avg), measures mid to near the end of the run so the averaging window (T+9m to T+14m) covers
  the
  steady long-term rate

### Throughput

| Dataset | Download CPS (W1) | Index CPS (W1) | Commit CPS (W1) | Commit TPS (W1) | Download CPS (W2) | Index CPS (W2) | Commit CPS (W2) | Commit TPS (W2) |
| ------- | ----------------- | -------------- | --------------- | --------------- | ----------------- | -------------- | --------------- | --------------- |
| 1       | 96.11             | 96.11          | 97.50           | 487.54          | 36.38             | 36.38          | 36.35           | 181.77          |
| 2       | 113.16            | 113.16         | 113.22          | 2830            | 74.51             | 74.51          | 74.57           | 1864.42         |
| 3       | 28.90             | 28.90          | 28.81           | 2881.35         | 21.01             | 21.01          | 21.01           | 2101            |

### Latency (W1)

| Dataset | Indexing Objects p95 (ms) | Indexing Object Changes p95 (ms) | Indexing Packages p95 (ms) | Commit p95 (ms) |
| ------- | ------------------------- | -------------------------------- | -------------------------- | --------------- |
| 1       | 0.95                      | 0.95                             | 0.95                       | 56.38           |
| 2       | 0.95                      | 0.95                             | 0.95                       | 1541            |
| 3       | 0.98                      | 0.95                             | 0.95                       | 4925            |

### Checkpoints Commit Queue Depth (W1)

| Dataset | Avg Queue Depth (checkpoints) |
| ------- | ----------------------------- |
| 1       | 0.2                           |
| 2       | 376.8                         |
| 3       | 397.2                         |

### End to end lag (W1)

| Dataset | Avg Lag (checkpoints) |
| ------- | --------------------- |
| 1       | 0.2                   |
| 2       | 380.4                 |
| 3       | 397.2                 |

### Findings

1. Throughput drops significantly over time, across all datasets: the sustained throughput (W2) gets notably lower than
   the early warm-up window (W1).
2. Indexing and commit stages are balanced, Index CPS ≈ Commit CPS in all runs. Commit queue depth remains near zero for
   dataset 1 but grows for dataset 2 and dataset 3, showing that higher offered load begins to impact commit capacity.
3. Indexing latencies (objects, object changes, packages) remain sub-millisecond. Commit p95 jumps from ~56 ms dataset 1
   to 1.54 s in dataset 2 and ~4.93 s in dataset 3, indicating database write throughput is a bottleneck under
   heavier load.
4. End to end lag is negligible for dataset 1 (~0.2 checkpoints). For dataset 2 and dataset 3, lag matches queue
   depth (~380–397 checkpoints), and hints again that the backlog is at the commit stage.

## Historical Backfill Benchmark Metrics

To evaluate the indexer’s historical backfill performance, be same datasets were used as for the indexing benchmarks.
The measurements focus on backfill rate (entries per second) and total time to complete the backfill.

In order to benchmark backfilling, a dedicated backfill job was required.
Since the synthetic datasets simulate gas transactions, it appeared reasonable to backfill the existing `tx_recipients`
table, which indexes the recipients of the gas transfers.

The `TxRecipientsBackfill` job was defined as follows:

```rust
// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use diesel::RunQueryDsl;
use iota_types::{
    base_types::IotaAddress, full_checkpoint_content::CheckpointData, object::Owner,
    transaction::TransactionDataAPI,
};
use itertools::Itertools;

use crate::{
    backfill::ingestion::IngestionBackfill,
    db::{ConnectionPool, get_pool_connection},
    errors::IndexerError,
    models::tx_indices::StoredTxRecipients,
    schema::tx_recipients,
};

pub(crate) struct TxRecipientsBackfill;

#[async_trait::async_trait]
impl IngestionBackfill for TxRecipientsBackfill {
    type ProcessedType = StoredTxRecipients;

    fn process_checkpoint(
        checkpoint: Arc<CheckpointData>,
    ) -> Result<Vec<Self::ProcessedType>, IndexerError> {
        let checkpoint_summary = &checkpoint.checkpoint_summary;
        let checkpoint_contents = &checkpoint.checkpoint_contents;
        let transactions = &checkpoint.transactions;
        let checkpoint_seq = checkpoint_summary.sequence_number;

        if checkpoint_contents.size() != transactions.len() {
            return Err(IndexerError::FullNodeReading(format!(
                "Checkpoint content size mismatch at checkpoint {checkpoint_seq}: expected {}, found {}",
                checkpoint_contents.size(),
                transactions.len()
            )));
        }

        let tx_seq_numbers = checkpoint_contents
            .enumerate_transactions(checkpoint_summary)
            .map(|(seq, digest)| (digest.transaction, seq));

        let mut results = Vec::new();

        for (tx, (expected_digest, tx_sequence_number)) in transactions.iter().zip(tx_seq_numbers) {
            let actual_digest = tx.transaction.digest();

            if expected_digest != *actual_digest {
                return Err(IndexerError::FullNodeReading(format!(
                    "Digest mismatch at checkpoint {checkpoint_seq}: expected {expected_digest}, found {actual_digest}",
                )));
            };

            let sender = tx.transaction.transaction_data().sender();

            results.extend(
                tx.effects
                    .all_changed_objects()
                    .into_iter()
                    .filter_map(|(_object_ref, owner, _write_kind)| match owner {
                        Owner::AddressOwner(address) => Some(address),
                        _ => None,
                    })
                    .unique()
                    .map(|address: IotaAddress| StoredTxRecipients {
                        tx_sequence_number: tx_sequence_number as i64,
                        recipient: address.to_vec(),
                        sender: sender.to_vec(),
                    })
                    .collect::<Vec<_>>(),
            );
        }

        Ok(results)
    }

    async fn persist_chunk(
        pool: ConnectionPool,
        processed_data: Vec<Self::ProcessedType>,
    ) -> Result<(), IndexerError> {
        let mut conn = get_pool_connection(&pool)?;

        diesel::insert_into(tx_recipients::table)
            .values(processed_data)
            .on_conflict_do_nothing()
            .execute(&mut conn)?;

        Ok(())
    }
}
```

The job processes incoming checkpoints and extracts the `tx_recipients` entries from the checkpoint data.
It then persists the entries to the `tx_recipients` table in the database.

## Backfilling benchmkark results:

| Dataset | Backfill Rate (entries/sec) | Time to Complete Backfill (seconds) |
| ------- | --------------------------- | ----------------------------------- |
| 1       | 384.65                      | 259.97                              |
| 2       | 276.99                      | 361.01                              |
| 3       | 185.47                      | 539.14                              |

### Findings

1. Backfill rate decreases when dataset transaction size increases.
2. Total backfill time grows accordingly with dataset size.
3. Higher load datasets contain more transactions and therefore there are more `tx_recipients` entries to write. This
   correlates with the slower backfill rates.
4. The processing work done per checkpoint in the backfill job is the same across all datasets, which would further
   suggest that the slowdown correlates with more rows to insert.

## Conclusion

At lower load the ingestion pipelines appear to be balanced. As tx density rises the db commit path becomes the
bottleneck. Indexing stays sub-ms, while commit latency climbs (56ms -> 1.5s -> 4.9s) and queue depth grows.
The W1 to W2 drop (mid-run) doesn't appear to be caused by the indexing logic, but rather by the database commit path.
The backfill benchmarks align, the more tx per checkpoint the more rows to write. rate falls with more
transactions and total time goes up, even with constant processing work for each checkpoint.
Therefore, we should focus on optimizing the database commit path to handle higher transaction loads.

Another observation when creating the different datasets: the time to generate synthetic data increases significantly
with
the number of transactions per checkpoint, which should also be optimized to allow for larger datasets to be generated.
