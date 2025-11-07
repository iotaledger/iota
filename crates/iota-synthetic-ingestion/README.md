# IOTA Synthetic Ingestion

Synthetic ingestion data generator for benchmarking and testing database ingestion performance.

Provides functionality to generate synthetic checkpoint data consisting of transactions for benchmarking or testing purposes. It simulates transaction execution and produces serialized checkpoint files, which can later be loaded into memory or ingested by a database.

## Usage

Use the CLI to generate synthetic checkpoint data:

```sh
cargo run --bin iota-synthetic-ingestion -- --ingestion-dir ./path/to/checkpoints \
           --starting-checkpoint 0 \
           --num-checkpoints 1000 \
           --checkpoint-size 100
```

## Command Line Options

`--ingestion-dir`: Directory to write the ingestion data to.

`--starting-checkpoint`: Starting checkpoint sequence number for workload generation (`default: 0`). Useful for benchmarking or testing against a non-empty database.

`--num-checkpoints`: Number of checkpoints to generate (`default: 2000`). Creates an additional checkpoint for genesis and gas provisioning if workflow generation starts from checkpoint sequence number `0`.

`--checkpoint-size`: Number of transactions in a checkpoint (`default: 200`).

## Testing

You can run the included tests using the following command:

```sh
cargo test
```
