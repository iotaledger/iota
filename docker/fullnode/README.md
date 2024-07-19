# Use Docker to Run a Iota Full node Locally

Follow the steps in this Readme to install and configure a Iota Full node for testing locally using Docker. The instructions were validated on the following operating system/processor combinations:

- Linux/AMD64
- Darwin/AMD64
- Darwin/ARM64

## Prerequisites

- [Install Docker](https://docs.docker.com/get-docker/)
- [Install Docker Compose](https://docs.docker.com/compose/install/)
- Download the Full node [docker-compose.yaml](https://github.com/iotaledger/iota/blob/main/docker/fullnode/docker-compose.yaml) file.

## Configure Iota Full node

Download the latest version of the Iota Full node configuration file [fullnode-template.yaml](https://github.com/iotaledger/iota/raw/main/crates/iota-config/data/fullnode-template.yaml). Use the following command to download the file:

```shell
wget https://github.com/iotaledger/iota/raw/main/crates/iota-config/data/fullnode-template.yaml
```

### Download the Iota genesis blob

The genesis blob contains the information that defined the Iota network configuration. Before you can start the Full node, you need to download the most recent file to ensure compatibility with the version of Iota you use. Use the following command to download the [genesis.blob](https://github.com/iotaledger/iota-genesis/raw/main/devnet/genesis.blob) from the `devnet` branch of the Iota repository:

`wget https://github.com/iotaledger/iota-genesis/raw/main/devnet/genesis.blob`

## Start your Iota Full node

Run the following command to start the Iota fullnode in Docker:

```shell
docker compose up
```

**Important:** The commands in this document assume you use Docker Compose V2. The `docker compose` command uses a dash (`docker-compose`) in Docker Compose V1. If you use Docker Compose V1, replace the space in each `docker compose` command with a dash (`docker-compose`). For more information, see [Docker Compose V2](https://docs.docker.com/compose/#compose-v2-and-the-new-docker-compose-command).

## Test the Iota Full node

After the Full node starts you can test the JSON-RPC interfaces.

## View activity on your local Full node with Iota Explorer

Iota Explorer supports connecting to a local network. To view activity on your local Full node, open the URL: [https://explorer.iota.io/?network=local](https://explorer.iota.io/?network=local).

You can also change the network that Iota Explorer connects to by select it in the Iota Explorer interface.

### Stop the Full node

Run the following command to stop the Full node when you finish using it:

```shell
docker compose stop
```

## Troubleshooting

If you encounter errors or your Full node stops working, run the commands in the following section to resolve the issue.

### Start the Full node in detached mode

First, try starting the Full node in detached mode:

```shell
docker compose up -d
```

### Reset the environment

If you continue to see issues, stop the Full node (`docker compose stop`) and delete the Docker container and volume. Then run the following command to start a new instance of the Full node using the same genesis blob.

```shell
docker compose down --volumes
```

### Stats (CPU/MEM USAGE %)

To view usage details for the Full node running in Docker, run the following command:

```shell
docker stats
```

This command shows a live data stream of the Docker container resource usage, such as CPU and memory. To view data for all containers, use the following command:

```shell
docker stats -a
```

### Inspect the state of a running Full node

Get the running container ID:

```shell
docker ps
```

Connect to a bash shell inside the container:

```shell
docker exec -it $CONTAINER_ID /bin/bash
```

Inspect the database:

```shell
ls -la iotadb/
```

### Investigate local RPC connectivity issues

Update the `json-rpc-address` in the Full node config to listen on all addresses:

```shell
sed -i 's/127.0.0.1/0.0.0.0/' fullnode-template.yaml
```

```shell
-json-rpc-address: "127.0.0.1:9000"
+json-rpc-address: "0.0.0.0:9000"
```

### Install wget and curl

Download each package. For example, on macOS use [homebrew](https://brew.sh/):

`brew install wget curl`

### Learn more about Iota

- https://docs.iota.io/learn

### Learn more about building and running a Full node from source code

- https://docs.iota.io/build/fullnode

### Learn more about Docker Compose

- https://docs.docker.com/compose/gettingstarted/
