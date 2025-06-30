This crate provides a key-value store implementation using Google Cloud Bigtable, designed for use with the IOTA data ingestion pipeline.

## Features

- **Read and Write Operations:**
  Implements traits for reading and writing objects, transactions, and checkpoints to a persistent store (Google Bigtable).
- **Checkpoint Progress Tracking:**
  Supports storing and retrieving ingestion progress (watermarks) for robust, resumable data pipelines.
- **Batch Operations:**
  Efficiently handles batch reads and writes for objects, transactions, and checkpoints.
- **Metrics:**
  Integrates with Prometheus to provide detailed metrics on key-value operations.
- **Local and Remote Modes:**
  Can connect to a local Bigtable emulator for development, or to a remote Google Cloud Bigtable instance for production.

## Main Components

- `BigTableClient`:
  High-level client for interacting with Bigtable, handling authentication, table naming, and metrics.
- `KvWorker`:
  Worker implementation that processes checkpoints and persists their data as key-value pairs in Bigtable.
- `BigTableProgressStore`:
  Manages persistent progress information (watermarks) in Bigtable for ingestion jobs.


## Setup

### Local development
- install `gcloud` CLI tool: https://cloud.google.com/sdk/docs/install

- install the `cbt` CLI tool

```sh
gcloud components install cbt
```

- start the emulator

```sh
gcloud beta emulators bigtable start
```

- set `BIGTABLE_EMULATOR_HOST` environment variable

```sh
$(gcloud beta emulators bigtable env-init)
```

- Run `./src/bigtable/init.sh` to configure the emulator
