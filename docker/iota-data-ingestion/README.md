# Checkpoint Remote Store

This Docker image enables storing checkpoint blobs to a remote storage service (e.g., AWS S3). The service requires AWS credentials for proper execution.

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
remote_store_url: "http://localhost:9000/api/v1"

# DynamoDbProgressStore config
#
progress_store:
  aws_access_key_id: "test"
  aws_secret_access_key: "test"
  aws_region: "us-east-1"
  # DynamoDB table name
  table_name: "checkpoint-progress"

# Wrokers Configs
#
tasks:
  # Task unique name
  - name: "local-blob-storage"
    # Number of workers will process the checkpoints in parallel
    concurrency: 1
    # Task type
    blob:
      # S3 bucket where checkpoints will be stored
      url: "s3://checkpoints"
      # AWS S3 client config options
      remote_store_options:
        - ["access_key_id", "test"]
        - ["secret_access_key", "test"]
        - ["region", "us-east-1"]
        # Only needed if using Localstack for local development purposes, it's preferred to be removed
        - ["endpoint_url", "http://localhost:4566"]
```

## Usage

### Build the Docker Image

Build a fresh Docker image without using cached layers:

```bash
docker compose build --no-cache
```

### Start the Service

Run the container in detached mode:

```bash
docker compose up -d
```

### Stop the Service

Stop and remove the container and associated resources:

```bash
docker compose down
```

## Local development

### Prerequisites

Before starting the service, you need to set up the required AWS components. The following examples use [localstack](https://github.com/localstack/localstack), but can be adapted for production AWS environments.

### 1. Create S3 Bucket

```bash
aws --profile localstack s3 mb s3://checkpoints
```

### 2. Set up DynamoDB

Create the DynamoDB table:

```bash
aws --profile localstack dynamodb create-table \
    --table-name checkpoint-progress \
    --attribute-definitions AttributeName=task_name,AttributeType=S \
    --key-schema AttributeName=task_name,KeyType=HASH \
    --provisioned-throughput ReadCapacityUnits=5,WriteCapacityUnits=5
```

Initialize the required record:

```bash
aws --profile localstack dynamodb put-item \
    --table-name checkpoint-progress \
    --item '{
        "task_name": {"S": "local-blob-storage"},
        "nstate": {"N": "0"}
    }'
```

### 3. Verify Resources

Verify that the resources were created correctly:

```bash
aws --profile localstack s3 ls
aws --profile localstack dynamodb list-tables
```

## Troubleshooting

- Ensure all AWS credentials are properly set in the `.env` file
- Verify that the S3 bucket and DynamoDB table exist before starting the service
- Check container logs if the service fails to start:
  ```bash
  docker compose logs
  ```
