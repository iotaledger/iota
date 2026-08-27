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
- `KeyValueStoreReader`, `KeyValueStoreWriter`:
  Traits for reading and writing key-value pairs to a persistent store.

## Protocol Buffers

Before building this crate, you must have the `protoc` Protocol Buffers compiler installed. This is required by the `build.rs` script to generate Rust code from the `.proto` files.

- **Linux (apt or apt-get):**
  ```sh
  sudo apt install -y protobuf-compiler
  ```
- **macOS (Homebrew):**
  ```sh
  brew install protobuf
  ```

If you encounter build errors related to missing generated files, ensure that `protoc` is installed and available in your `PATH`.

## Setup

### Remote Development

To instantiate a `BigTableClient` for communicating with a remote Google Cloud Bigtable instance, you need the following:

- **Instance ID:**
  This can be found in the Google Cloud Console (e.g., `my-instance-id`).

- **Credentials:**
  Following the application default credentials [guidelines](https://cloud.google.com/docs/authentication/application-default-credentials) we will use the `GOOGLE_APPLICATION_CREDENTIALS` environment variable to authenticate with the Google Cloud API.

  Download your service account credentials JSON file from the Google Cloud Console. Store this file securely on your local machine.
  The client will use this file via the `GOOGLE_APPLICATION_CREDENTIALS` environment variable.
  Example:
  ```sh
  export GOOGLE_APPLICATION_CREDENTIALS=/path/to/my-credentials.json
  ```

**Note:**
Never commit your credentials file to version control. Always keep it secure and private.

- Run the following script to configure the remote instance (replace `<instance_id>` and `<project_id>` accordingly).
  The project ID can be found in your credentials JSON file:
  ```sh
  ./init.sh <instance_id> <project_id>
  ```

### Local development

- install `gcloud` CLI tool: https://cloud.google.com/sdk/docs/install

- install the `cbt` CLI tool and the Bigtable emulator

```sh
gcloud components install cbt bigtable
```

- start the emulator

```sh
gcloud beta emulators bigtable start
```

- set `BIGTABLE_EMULATOR_HOST` environment variable

```sh
$(gcloud beta emulators bigtable env-init)
```

- Run `./init.sh` to configure the emulator

### Testing

The integration tests need the same tools as [local development](#local-development): `gcloud` (used to locate the SDK root), the Bigtable emulator binary (`cbtemulator`, resolved under the SDK root since it is not on `PATH`), and `cbt` (used to create the tables and column family).

Unlike the local development flow, the tests are self-contained: each test spawns its own emulator on a random free port, creates the required tables, and kills the emulator when done. There is no need to start the emulator manually, set `BIGTABLE_EMULATOR_HOST`, or run `./init.sh`.

```shell
cargo test --features integration_tests
```

nextest is also available for running tests:

```shell
cargo nextest run --features integration_tests
```
