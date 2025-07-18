# IOTA Synthetic Ingestion

Synthetic ingestion data generator for benchmarking and testing database ingestion performance.

Provides functionality to generate synthetic checkpoint data consisting of transactions for benchmarking or testing purposes. It simulates transaction execution and produces serialized checkpoint files, which can later be loaded into memory or ingested by a database.

## Usage

Use the CLI to generate synthetic checkpoint data:

```sh
cargo run -- --ingestion-dir ./path/to/checkpoints \
           --starting-checkpoint 0 \
           --num-checkpoints 1000 \
           --checkpoint-size 100
```

## Command Line Options

`--ingestion-dir`: Directory to store checkpoint files.

`--starting-checkpoint`: Starting checkpoint sequence number (`default: 0`).

`--num-checkpoints`: Total checkpoints to generate (`default: 2000`).

`--checkpoint-size`: Number of transactions per checkpoint (`default: 200`).

## Testing

You can run the included tests using the following command:

```sh
cargo test
```
