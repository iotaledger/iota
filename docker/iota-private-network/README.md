# IOTA Private Network

## Requirements

- [Docker Compose](https://docs.docker.com/engine/install/)
- [yq](https://github.com/mikefarah/yq)

## Steps

### 1. Build Docker Images

Run the following commands to build the necessary Docker images:

#### iota-node

```bash
../iota-node/build.sh -t iota-node --no-cache
```

#### iota-indexer

```bash
../iota-indexer/build.sh -t iota-indexer --no-cache
```

#### iota-tools

```bash
../iota-tools/build.sh -t iota-tools --no-cache
```

### 2. Bootstrap the Network

Generate the genesis files and validators’ configuration:

```bash
./bootstrap.sh
```

### 3. Start the Network

The script supports different modes, which can be used individually or in combination. Regardless of the mode chosen, the validators will always be active.

- faucet: Brings up one fullnode, and faucet.
- backup: Brings up one fullnode with backup features enabled. This includes generating database snapshots, formal snapshots, and enabling archive mode. If you do not want to enable archive mode, comment out the configuration in `configs/fullnode/backup.yaml`.
- indexer: Brings up one fullnode, one indexer, and a PostgreSQL database.
- indexer-cluster: Brings up two fullnodes, two indexers, and a PostgreSQL cluster with a primary and replica database. indexer-1 uses the primary PostgreSQL, while indexer-2 uses the replica.
- all: Brings up all services.

#### Example

To bring up everything:

```bash
./run.sh all
```

To bring up 4 validators, three full nodes (one with the backup feature enabled), one indexer, and one faucet, use the following command:

```
./run.sh faucet backup indexer
```

### Ports

- fullnode-1:
  - JSON-RPC: http://127.0.0.1:9000
  - Metrics: http://127.0.0.1:9184

- fullnode-2:
  - JSON-RPC: http://127.0.0.1:9001
  - Metrics: http://127.0.0.1:9185

- fullnode-3:
  - JSON-RPC: http://127.0.0.1:9002
  - Metrics: http://127.0.0.1:9186

- fullnode-4:
  - JSON-RPC: http://127.0.0.1:9003
  - Metrics: http://127.0.0.1:9187

- faucet-1:
  - JSON-RPC: http://127.0.0.1:5003
  - Metrics: http://127.0.0.1:9188

- indexer-1:
  - JSON-RPC: http://127.0.0.1:9004
  - Metrics: http://127.0.0.1:9181

- indexer-2:
  - JSON-RPC: http://127.0.0.1:9005
  - Metrics: http://127.0.0.1:9182

- postgres_primary:
  - PostgreSQL: http://127.0.0.1:5432

- postgres_replica:
  - PostgreSQL: http://127.0.0.1:5433
