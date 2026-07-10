# Sync benchmark

Measures how long a fullnode takes to sync to a target checkpoint, to compare
the sync performance of two node builds (e.g. two branches) against the same
network. Each run starts from a wiped database inside an `iota-node` docker
image and uses the `iota-tool measure-sync-time` subcommand bundled in that
image: it starts the node, polls the executed checkpoint height via the gRPC
API, and writes a timing result to `./data/results/<label>.json`.

## Setup

1. Build one image per branch with [`docker/iota-node`](../../docker/iota-node)
   and tag them, e.g.:

   ```sh
   git checkout branch-a
   docker build -f docker/iota-node/Dockerfile -t iota-node:branch-a .
   git checkout branch-b
   docker build -f docker/iota-node/Dockerfile -t iota-node:branch-b .
   ```

2. Place `genesis.blob` and `fullnode.yaml` for the network to sync against
   under `./data/config/` (same layout as the
   [fullnode docker setup](../../setups/fullnode/docker)). The config must
   enable the gRPC API and use paths inside the container:

   ```yaml
   db-path: "/opt/iota/db"
   genesis:
     genesis-file-location: "/opt/iota/config/genesis.blob"
   enable-grpc-api: true
   grpc-api-config:
     address: "127.0.0.1:50051"
   ```

## Run

One run per image, with the same target checkpoint (pick one that the network
has already passed, so it is fixed across runs):

```sh
IOTA_NODE_IMAGE=iota-node:branch-a RUN_LABEL=branch-a TARGET_CHECKPOINT=5000000 \
  docker compose run --rm --service-ports sync-benchmark
IOTA_NODE_IMAGE=iota-node:branch-b RUN_LABEL=branch-b TARGET_CHECKPOINT=5000000 \
  docker compose run --rm --service-ports sync-benchmark
```

Each run wipes `./data/db` first, so runs are independent of each other.
`STALL_TIMEOUT` (default `30m`) aborts a run when the checkpoint height stops
advancing. `--service-ports` publishes the metrics port 9184 so progress can
also be watched in Grafana/Prometheus.

## Results

Compare the JSON results, e.g.:

```sh
diff <(jq . data/results/branch-a.json) <(jq . data/results/branch-b.json)
```

The node's own log (including the one-time
`state sync caught up to checkpoint ...` message, if the build contains it)
is written to `./data/results/<label>-node.log`.
