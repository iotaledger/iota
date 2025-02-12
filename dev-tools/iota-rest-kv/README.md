# Kv Store Rest Api

This docker-compose configuration allows running the `iota-rest-kv` service. The service requires AWS credentials for proper execution.

## Configuration

The application requires a `yaml` file for its configuration.

A default configuration file is provided at `config/config.yaml`. This configuration can be customized based on your needs and is mounted into the container via Docker Compose.

```yaml
# Remote KV Store REST API config
#
# The Rest Api address
rest-api-address: "0.0.0.0:3555"

# remote Object Store config for more info:
# - https://docs.iota.org/operator/archives#set-up-archival-fallback
#
# The object store config should point to the same object store the
# `KvStoreWorker` from the `iota-data-ingestion` crate uses to write data
object-store-config:
  object-store: "S3"
  aws-endpoint: "http://0.0.0.0:4566"
  bucket: "iota-storage-bucket"
  aws-access-key-id: "test"
  aws-secret-access-key: "test"
  aws-allow-http: true
  object-store-connection-limit: 20

# DynamoDb config
#
# The DynamoDb config should point to the same tables the
# `KvStoreWorker` from the `iota-data-ingestion` crate uses to write data
dynamo-db-config:
  aws-access-key-id: "test"
  aws-secret-access-key: "test"
  aws-region: "us-east-1"
  aws-endpoint: "http://0.0.0.0:4566"
  # DynamoDB table name
  table-name: "iota-storage"
```

## Usage

#### 1. Build the required image

```shell
pushd <iota project directory>/docker/iota-rest-kv && ./build.sh && popd
```

#### 2. CD into the iota-rest-kv directory

```shell
cd <iota project directory>/dev-tools/iota-rest-kv
```

### 3. Start the Service

Run the container in detached mode:

```shell
docker compose up -d
```

> [!NOTE]
> Double check the rest api server port in the `config.yaml` and the exposed port on the `docker-compose.yaml` file, they should match.

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
aws --profile localstack s3 mb s3://iota-storage-bucket
```

### 2. Set up DynamoDB

Create the DynamoDB table:

```bash
aws --profile localstack \
dynamodb create-table \
--table-name iota-storage \
--attribute-definitions \
    AttributeName=digest,AttributeType=B \
    AttributeName=type,AttributeType=S \
--key-schema \
    AttributeName=digest,KeyType=HASH \
    AttributeName=type,KeyType=RANGE \
--provisioned-throughput ReadCapacityUnits=5,WriteCapacityUnits=5
```

### 3. Verify Resources

Verify that the resources were created correctly:

```bash
aws --profile localstack s3 ls
aws --profile localstack dynamodb list-tables
```

## Troubleshooting

- Ensure all AWS credentials are properly set in the `config.yaml` file
- Verify that the S3 bucket and DynamoDB table exist before starting the service
- Check container logs if the service fails to start:
  ```bash
  docker compose logs
  ```
