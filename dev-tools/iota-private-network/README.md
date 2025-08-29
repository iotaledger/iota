# IOTA Private Network

## Requirements

- [Docker Compose](https://docs.docker.com/engine/install/)
- [yq](https://github.com/mikefarah/yq)

## Steps

### 1. Build Docker Images

Run the following commands to build the necessary Docker images:

#### iota-node

```bash
../../docker/iota-node/build.sh -t iota-node --no-cache
```

#### iota-indexer

```bash
../../docker/iota-indexer/build.sh -t iota-indexer --no-cache
```

#### iota-tools

```bash
../../docker/iota-tools/build.sh -t iota-tools --no-cache
```

### 2. Bootstrap the Network

Generate the genesis files and validators’ configuration:

```bash
# By default, bootstrap 4 validators:
./bootstrap.sh

# To bootstrap 19 validators instead:
./bootstrap.sh -n 19
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

To bring up 4 validators, three full nodes (one with the backup feature enabled), one indexer, and one faucet:

```
./run.sh faucet backup indexer
```

To bring up 10 validators and faucet:

```bash
./run.sh -n 10 faucet
```

> **Note:** Out of the box, the validator network for any number between 4 and 19 is supported by the provided code.\
> If you wish to run a large number, <N>, of validators, you must manually update the corresponding YAML files:
>
> - `configs/genesis-<N>-template.yaml` for the genesis template
> - `docker-compose.yaml` (validator services and network IPs)
> - `prometheus/prometheus.yaml` (scrape targets)
> - **(Optional)** Adjust the stake distribution in the chosen `genesis-template-<N>.yaml` if you want different validator stakes.

### Optional: Selecting a Consensus Protocol

You can run the network with an optional consensus protocol flag. There are two options `starfish` and `mysticeti`.
If the flag is not provided, the default protocol is Mysticeti.

For example, to start a **Starfish** consensus protocol with 10 validators:

```bash
./run.sh -n 10 -p starfish
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

## Span Tracing with Tempo

To enable span tracing for the nodes, you need to modify the docker-compose.yaml file to include the necessary environment variables for each node.

for example, for fullnode-1, you would add the following environment variables:

```yaml
- OTLP_ENDPOINT=http://tempo:4317 # The endpoint of the Tempo instance
- OTEL_SERVICE_NAME=fullnode-1 # A unique name for the service, it could be later used to filter traces in Grafana
- TRACE_FILTER=warn # The trace filter level, you can adjust it based on your needs
```

The `TRACE_FILTER` variable follows the rules defined in the [tracing documentation](https://crates.io/crates/tracing-filter).

To trace the **checkpoint lifecycle** only, you can set the `TRACE_FILTER` to `TRACE_FILTER=[checkpoint_received_from_state_sync]=trace,[checkpoint_received_from_consensus]=trace`.

To trace the **transaction lifecycle** only, you can set the `TRACE_FILTER` to `TRACE_FILTER=[handle_consensus_output]=trace,[tx_orchestrator_execute_transaction_block]=trace,[json_rpc_api_execute_transaction_block]=trace`.
