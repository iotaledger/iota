# Checkpoint Remote Store

This Docker container enables storing checkpoint blobs to a remote storage service (e.g., AWS S3). The service requires AWS credentials for proper execution.

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

A default configuration file is provided at `config/config.yaml`. This configuration can be customized based on your needs and is mounted into the container via Docker Compose.

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
docker compose up -d
```

### 4. Stop the Service

Stop and remove the container and associated resources:

```shell
docker compose down
```

## Local development

### Prerequisites

Before starting the service, you need to set up the required AWS components. The following examples use [localstack](https://github.com/localstack/localstack), but can be adapted for production AWS environments.

### 1. Create S3 Bucket

```bash
aws --profile localstack s3 mb s3://checkpoints
```

### 2. Verify Resources

Verify that the resources were created correctly:

```bash
aws --profile localstack s3 ls
```

## Troubleshooting

- Ensure all AWS credentials are properly set in the `.env` file
- Verify that the S3 bucket and DynamoDB table exist before starting the service
- Check container logs if the service fails to start:
  ```bash
  docker compose logs
  ```
