# IOTA Proxy

IOTA Proxy is a middleware component that enhances IOTA node deployments by collecting and forwarding node metrics to a centralized monitoring system.

This README provides comprehensive guidance on setting up and using IOTA Proxy in Docker.

## Example Use Case For `iota-proxy`

This use case is an example of how to use iota-proxy in a docker setup that includes monitoring capabilities; you can use Docker Compose to deploy your full node along with iota-proxy, Mimir, and Grafana. This setup provides a complete solution for running and monitoring your full node.

### Prerequisites

- Docker and Docker Compose installed on your server
- SSL certificates for `iota-proxy` (`fullchain.pem` and `privkey.pem`)
- Generated network key for your full node
- Genesis and migration blobs for the network you're connecting to

### Directory Structure

Create a directory for your full node setup with the following structure:

```
full-node-setup/
├── docker-compose.yaml
├── fullnode-template.yaml
├── iota-proxy.yaml
├── mimir.yaml
├── genesis.blob
├── migration.blob
├── network.key
├── privkey.pem
├── fullchain.pem
└── data/
    └── mimir/
```

### Step 0: Build the iota-proxy Docker image

Go to the `docker/iota-proxy` directory and run the following command to build the iota-proxy image:

```bash
./build.sh
```

The image will be built and saved as `iotaledger/iota-proxy`.

### Step 1: Create Docker Compose Configuration

Create a `docker-compose.yaml` file with the following content:

```yaml
---
services:
  fullnode:
    # Use the appropriate image tag for your network (mainnet, testnet, devnet)
    image: iotaledger/iota-node:devnet
    ports:
      - "8080:8080"
      - "8084:8084/udp"
      - "9000:9000"
      - "9184:9184"
    volumes:
      - ./fullnode-template.yaml:/opt/iota/config/fullnode.yaml:ro
      - ./validator.yaml:/opt/iota/config/validator.yaml:ro
      - ./genesis.blob:/opt/iota/config/genesis.blob:ro
      - ./migration.blob:/opt/iota/config/migration.blob:ro
      - ./iotadb:/opt/iota/db:rw
      - ./network.key:/opt/iota/config/network.key:ro
    command: [
      "/usr/local/bin/iota-node",
      "--config-path",
      "/opt/iota/config/fullnode.yaml",
    ]
    depends_on:
      - iota-proxy
      - mimir

  mimir:
    image: grafana/mimir:latest
    container_name: mimir
    restart: unless-stopped
    ports:
      - "9009:9009"
    environment:
      - ENABLE_MULTITENANCY=false
    volumes:
      - ./mimir.yaml:/etc/mimir/mimir.yaml
      - ./data/mimir:/data
    command:
      - -config.file=/etc/mimir/mimir.yaml

  grafana:
    image: grafana/grafana:latest
    environment:
      - GF_AUTH_ANONYMOUS_ENABLED=true
      - GF_AUTH_ANONYMOUS_ORG_ROLE=Admin
      - GF_AUTH_DISABLE_LOGIN_FORM=true
      - GF_FEATURE_TOGGLES_ENABLE=traceqlEditor
    ports:
      - "3000:3000"

  iota-proxy:
    image: iotaledger/iota-proxy
    environment:
      - RUST_BACKTRACE=1
      - RUST_LOG=debug
    command:
      - "/usr/local/bin/iota-proxy"
      - "--config=/etc/iota-proxy.yaml"
    ports:
      - "8443:8443"
    volumes:
      - ./iota-proxy.yaml:/etc/iota-proxy.yaml
      - ./privkey.pem:/etc/privkey.pem:ro
      - ./fullchain.pem:/etc/fullchain.pem:ro
    depends_on:
      - mimir

volumes:
  iotadb:

networks:
  default:
    name: iota-network
```

### Step 2: Configure Full Node (fullnode-template.yaml)

Create a `fullnode-template.yaml` file with the following content, adjusting the values as needed for your setup:

```yaml
# Database path
db-path: /opt/iota/db

# Network configuration
network-address: /dns/0.0.0.0/tcp/8080/http
metrics-address: "0.0.0.0:9184"
json-rpc-address: "0.0.0.0:9000"
enable-event-processing: true

# Network key configuration
network-key-pair:
  path: /opt/iota/config/network.key

# P2P configuration
p2p-config:
  listen-address: "0.0.0.0:8084"
  external-address: /dns/your-node-hostname/udp/8084
  seed-peers:
    # For connecting to your validator
    - address: /dns/validator-hostname/udp/8084
      peer-id: <validator-peer-id>
    # For connecting to the network (example for devnet)
    - address: /dns/access-0.r.devnet.iota.cafe/udp/8084
      peer-id: 01589ac910a5993f80fbc34a6e0c8b2041ddc5526a951c838df3037e11ab0188

# Resource optimization
enable-index-processing: false

# Genesis configuration
genesis:
  genesis-file-location: /opt/iota/config/genesis.blob

# Migration configuration
migration-tx-data-path: /opt/iota/config/migration.blob

# Pruning configuration
authority-store-pruning-config:
  num-latest-epoch-dbs-to-retain: 3
  epoch-db-pruning-period-secs: 3600
  max-checkpoints-in-batch: 10
  max-transactions-in-batch: 1000
  num-epochs-to-retain: 0
  num-epochs-to-retain-for-checkpoints: 2
  periodic-compaction-threshold-days: 1

# Metrics configuration for iota-proxy
metrics:
  push-interval-seconds: 10
  push-url: https://your-node-hostname:8443/publish/metrics
```

### Step 3: Configure iota-proxy

Create an `iota-proxy.yaml` file with the following content:

```yaml
# Specify the network you're connecting to
network: devnet # Change to mainnet or testnet as needed
listen-address: 0.0.0.0:8443

# Mimir configuration for metrics storage
remote-write:
  url: http://mimir:9009/api/v1/push
  username: "" # Leave empty if no auth required
  password: "" # Leave empty if no auth required
  pool-max-idle-per-host: 8

# Metrics endpoint configuration
metrics:
  endpoint: /publish/metrics

# Static peers configuration
static-peers:
  pub-keys:
    - name: my-fullnode
      p2p-address: /dns/your-node-hostname/udp/8084
      peer-id: "<your-fullnode-peer-id>" # Use the peer ID from your network.key
# Add additional peers if needed

# Dynamic peers configuration (for committee information)
dynamic-peers:
  url: https://api.devnet.iota.cafe # Change to the appropriate API URL for your network
  interval: 30
  certificate-file: /etc/fullchain.pem
  private-key: /etc/privkey.pem

# Metrics addresses
metrics-address: 0.0.0.0:9184
histogram-address: 0.0.0.0:9185
```

### Step 4: Configure Mimir

Create a `mimir.yaml` file with the following content:

```yaml
server:
  http_listen_address: 0.0.0.0
  http_listen_port: 9009

# Disable multi-tenant auth
multitenancy_enabled: false

# ===== STORAGE CONFIGURATION OPTIONS =====
# IMPORTANT: Choose ONLY ONE storage option below.
# Either use the default local filesystem OR the S3 bucket option.
# If using S3, you MUST comment out the entire local filesystem blocks_storage section
# and uncomment the S3 configuration section.

# OPTION 1: LOCAL FILESYSTEM STORAGE (default)
# This configuration uses local filesystem storage for metrics data
# Comment out this entire section if using S3 storage instead
blocks_storage:
  backend: filesystem # Using local filesystem for storage
  bucket_store:
    sync_dir: /tmp/mimir/tsdb-sync # Directory for syncing TSDB blocks
  filesystem:
    dir: /tmp/mimir/data/tsdb # Main directory for storing TSDB data
  tsdb:
    dir: /tmp/mimir/tsdb # Directory for active TSDB data

# OPTION 2: S3 BUCKET STORAGE
# For production environments, S3 storage is recommended for better scalability and reliability
# To use S3 storage:
# 1. Comment out the ENTIRE local filesystem blocks_storage section above
# 2. Uncomment the following common and blocks_storage sections
# 3. Fill in your S3 credentials and settings

# common:
#   storage:
#     backend: s3  # Use S3-compatible storage
#     s3:
#       endpoint:  # Optional: URL of S3-compatible API (leave empty for AWS S3)
#       region: eu-west-1  # S3 region
#       secret_access_key:  # Your S3 secret key
#       access_key_id:  # Your S3 access key ID
#       bucket_name:  # Your S3 bucket name
#       insecure: false  # Set to true if using non-HTTPS endpoint
#
# blocks_storage:
#   storage_prefix: blocks  # Prefix for blocks in the S3 bucket
#   tsdb:
#     dir: /data/ingester  # Local directory for temporary TSDB data before upload

alertmanager_storage:
  storage_prefix: alertmanager # Prefix for alertmanager data

ruler_storage:
  storage_prefix: ruler # Prefix for ruler data

# Lower the replication factor for single-node
ingester:
  ring:
    replication_factor: 1 # Single replica for single-node deployments

# Limits configuration
limits:
  compactor_blocks_retention_period: 60d # Keep data for 60 days
  ingestion_rate: 250000 # Maximum samples per second
  ingestion_burst_size: 500000 # Maximum burst size for ingestion
  max_global_series_per_user: 1000000 # Maximum number of active series
```

> **Important Storage Configuration Note**:
>
> You must choose ONLY ONE storage option:
>
> 1. **Local filesystem** (default): Simpler setup, good for testing or small deployments.
>    - Keep the default blocks_storage section as is.
>    - Make sure the S3 configuration is commented out.
> 2. **S3 bucket**: Better for production, provides durability, scalability, and easier backup management.
>    - Comment out the ENTIRE local filesystem blocks_storage section.
>    - Uncomment the common.storage and blocks_storage sections for S3.
>    - Fill in all required S3 credentials (region, keys, bucket name).
>
> Mixing both configurations will cause errors. Ensure only one storage backend is configured.

### Step 5: Start the Full Node Stack

Once all configuration files are in place, start the full node stack with Docker Compose:

```bash
docker-compose up -d
```

This command will start all services in detached mode. You can check the status of the services with:

```bash
docker-compose ps
```

### Step 6: Grafana Dashboard

Once the services are running, you can access the Grafana dashboard at `https://your-node-hostname:3000` and check your metrics.
