# IOTA Data Ingestion Services

Contains services for ingesting and storing IOTA blockchain data in different formats to support various use cases. The services utilize Docker containers to store checkpoint data to remote storage services (e.g. AWS S3).

## Available Services

There are three separate services, each in its own directory:

1. **Live Checkpoint Storage** (`live/`) - Stores raw checkpoint blobs to object storage for immediate access.
2. **Historical Checkpoint Storage** (`historical/`) - Compresses and batches checkpoint data for efficient long-term storage to object storage.
3. **KV Store** (`kv-store/`) - Processes checkpoint data and stores it in a structured format in DynamoDB and S3. This service is primarily used to provide historical data for Archival IOTA Nodes, which can query specific transactions, events, and effects efficiently through a REST API.

## Configuration

### Environment Variables

The Docker Compose service accepts an `.env` file containing the following AWS credentials:

```text
AWS_ACCESS_KEY_ID=test
AWS_SECRET_ACCESS_KEY=test
AWS_DEFAULT_REGION=us-east-1
AWS_ENDPOINT_URL=http://localstack:4566  # Optional: Used for local testing with localstack
```

### Configuration File

Each service has its own configuration file:

- **Live Service**: `live/config.yaml`
- **Historical Service**: `historical/config.yaml`
- **KV Store Service**: `kv-store/config.yaml`

These configurations can be customized based on your needs and are mounted into their respective containers via Docker Compose.

An example of `config.yaml` for the Blob worker:

```yaml
# IndexerExecutor config
#
path: "./test-checkpoints"
# IOTA Node Rest API URL
remote-store-url: "http://localhost:9000/api/v1"

# Path to the progress store JSON file.
#
# The ingestion pipeline uses this file to persist its progress,
# ensuring state is preserved across restarts.
#
progress-store-path: "/iota/output/ingestion_progress.json"

# Workers Configs
#
tasks:
  # Task unique name
  - name: "local-blob-storage"
    # Number of workers will process the checkpoints in parallel
    concurrency: 1
    # Task type
    blob:
      # remote Object Store config for more info:
      # - https://docs.iota.org/operator/archives#set-up-archival-fallback
      #
      object-store-config:
        object-store: "S3"
        aws-endpoint: "http://localhost:4566"
        bucket: "checkpoints"
        aws-access-key-id: "test"
        aws-secret-access-key: "test"
        aws-allow-http: true
        object-store-connection-limit: 20
      # Checkpoint upload chunk size (in MB) that determines the upload strategy:
      #
      # If checkpoint size < checkpoint_chunk_size_mb:
      #   - Uploads checkpoint using single PUT operation
      #   - Optimal for smaller checkpoints
      #
      # If checkpoint size >= checkpoint_chunk_size_mb:
      #   - Divides checkpoint into chunks of this size
      #   - Uploads chunks as multipart
      #   - Storage service concatenates parts on completion
      #
      # Example with 50MB chunk size:
      #   200MB checkpoint:
      #   - Splits into 4 parts (50MB each)
      #   - Multipart upload of each part
      #   - Parts merged on remote storage
      #
      #   40MB checkpoint:
      #   - Single PUT upload
      #   - No chunking needed
      #
      # Minimum allowed chunk size is 5MB
      #
      checkpoint-chunk-size-mb: 100
      node-rest-api-url: "http://localhost:9000/api/v1"
```

## Usage

#### 1. Build the required image

```shell
pushd <iota project directory>/docker/iota-data-ingestion && ./build.sh && popd
```

#### 2. CD into the iota-data-ingestion directory

```shell
cd <iota project directory>/dev-tools/iota-data-ingestion
```

### 3. Start the Service

Run the container in detached mode:

```shell
pushd historical && docker compose up -d && popd
pushd live && docker compose up -d && popd
pushd kv-store && docker compose up -d && popd
```

### 4. Stop the Service

Stop and remove the container and associated resources:

```shell
pushd historical && docker compose down && popd
pushd live && docker compose down && popd
pushd kv-store && docker compose down && popd
```

## Local development

### Prerequisites

Before starting the service, you need to set up the required AWS components. The following examples use [localstack](https://github.com/localstack/localstack), but can be adapted for production AWS environments.

### 1. Create S3 Bucket

```bash
# For live and historical services
aws --profile localstack s3 mb s3://checkpoints

# For KV Store service
aws --profile localstack s3 mb s3://iota-storage-bucket
```

### 2. Verify Resources

Verify that the resources were created correctly:

```bash
aws --profile localstack s3 ls
```

### 3. Create DynamoDB Table (for KV Store)

```bash
aws --profile localstack dynamodb create-table \
    --table-name iota-storage \
    --attribute-definitions AttributeName=pk,AttributeType=S AttributeName=sk,AttributeType=S \
    --key-schema AttributeName=pk,KeyType=HASH AttributeName=sk,KeyType=RANGE \
    --billing-mode PAY_PER_REQUEST
```

## Troubleshooting

- Ensure all AWS credentials are properly set in the `.env` file
- Verify that the S3 bucket and DynamoDB table exist before starting the service
- Check container logs if the service fails to start:
  ```bash
  docker compose logs
  ```
