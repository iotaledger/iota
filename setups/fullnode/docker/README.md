# Use Docker to Run an IOTA Full Node Locally

Use this guide to install and configure an IOTA Full Node locally using Docker for testing. The instructions apply to the following operating system and processor combinations:

- Linux/AMD64
- Darwin/AMD64
- Darwin/ARM64

## Prerequisites

Before you begin, ensure you have the following:

- [Docker](https://docs.docker.com/get-docker/) installed.
- [Docker Compose](https://docs.docker.com/compose/install/) installed.
- Download the full node [docker-compose.yaml](https://github.com/iotaledger/iota/blob/develop/setups/fullnode/docker/docker-compose.yaml) file.

## Configure the IOTA Full Node

Download the latest version of the IOTA Full Node configuration file [fullnode-template.yaml](https://github.com/iotaledger/iota/raw/develop/crates/iota-config/data/fullnode-template.yaml). You can use the following command to download the file:

```shell
wget https://github.com/iotaledger/iota/raw/develop/crates/iota-config/data/fullnode-template.yaml
```

### Add Peers

For `Testnet` or `Devnet` nodes, edit the `fullnode.yaml` file to include peer nodes for state synchronization. Append
the following to the end of the current configuration:

#### Devnet

```yaml
p2p-config:
    seed-peers:
    - address: /dns/access-0.r.devnet.iota.cafe/udp/8084
        peer-id: 01589ac910a5993f80fbc34a6e0c8b2041ddc5526a951c838df3037e11ab0188
    - address: /dns/access-1.r.devnet.iota.cafe/udp/8084
        peer-id: 32875c547ea3b44fa08a711646cedb70fa0c97959d236578131505da09723add
```

#### Testnet

```yaml
p2p-config:
    seed-peers:
    - address: /dns/access-0.r.testnet.iota.cafe/udp/8084
        peer-id: 46064108d0b689ed89d1f44153e532bb101ce8f8ca3a3d01ab991d4dea122cfc
    - address: /dns/access-1.r.testnet.iota.cafe/udp/8084
        peer-id: 8ffd25fa4e86c30c3f8da7092695e8a103462d7a213b815d77d6da7f0a2a52f5
```

### Download the IOTA Genesis Blob

The genesis blob defines the IOTA network configuration. Before starting the Full Node, download the latest
`genesis.blob` file to ensure compatibility with your version of IOTA. Keep in mind that the [IOTA Mainnet](https://wiki.iota.org/build/networks-endpoints/#iota) runs the
[Stardust Protocol](https://wiki.iota.org/learn/protocols/stardust/introduction/) at the moment, so only Testnet and Devnet blobs are available.

- [Testnet genesis blob](https://dbfiles.testnet.iota.cafe/genesis.blob):
  `curl -fLJO https://dbfiles.testnet.iota.cafe/genesis.blob`
- [Devnet genesis blob](https://dbfiles.devnet.iota.cafe/genesis.blob):
  `curl -fLJO https://dbfiles.devnet.iota.cafe/genesis.blob`
- [Devnet migration blob](https://dbfiles.devnet.iota.cafe/migration.blob):
  `curl -fLJO https://dbfiles.devnet.iota.cafe/migration.blob`

### Set Up Archival Fallback

This allows nodes that fall behind to catch up by downloading archive data rather than relying on synchronization. For more details about archives, see [IOTA Archives](https://docs.iota.org/operator/archives).

```yaml
state-archive-read-config:
  - object-store-config:
      object-store: "S3"
      aws-endpoint: "https://archive.<devnet|testnet|mainnet>.iota.cafe"
      aws-virtual-hosted-style-request: true
      object-store-connection-limit: 20
      no-sign-request: true
    concurrency: 5
    use-for-pruning-watermark: false
```

## Start Your IOTA Full Node

Run the following command to start the IOTA Full Node in Docker:

```shell
docker compose up
```

**Important:** These commands assume you are using Docker Compose V2. In Docker Compose V1, the `docker compose` command uses a dash (`docker-compose`). If you use Docker Compose V1, replace the space in each `docker compose` command with a dash (`docker-compose`). For more information, see [Docker Compose V2](https://docs.docker.com/compose/#compose-v2-and-the-new-docker-compose-command).

## Stop the Full Node

Run the following command to stop the Full Node when you finish using it:

```shell
docker compose stop
```

## Test the IOTA Full Node

After the Full Node starts, you can test the [JSON-RPC interfaces](https://docs.iota.org/iota-api-ref).

## Troubleshooting

If you encounter errors or your Full Node stops working, run the commands in the following section to resolve the issue.

### Start the Full Node in Detached Mode

First, try starting the Full Node in detached mode:

```shell
docker compose up -d
```

### Reset the Environment

If you continue to see issues, stop the Full Node (`docker compose stop`) and delete the Docker container and volume. Then run the following command to start a new instance of the Full Node using the same genesis blob:

```shell
docker compose down --volumes
```

## Monitoring

### View Activity on Your Local Full Node with IOTA Explorer

The IOTA Explorer supports connecting to any network as long as it has `https` enabled. To view activity on your local
Full Node, open the URL: [https://explorer.rebased.iota.org/](https://explorer.rebased.iota.org/), and select Custom
RPC URL from the network dropdown in the top right.

### View Resource Usage (CPU and Memory)

To view resource usage details for the Full Node running in Docker, run the following command:

```shell
docker stats
```

This command shows a live data stream of the Docker container's resource usage, such as CPU and memory. To view data for all containers, use the following command:

```shell
docker stats -a
```

### Inspect the State of a Running Full Node

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

### Investigate Local RPC Connectivity Issues

Update the `json-rpc-address` in the Full Node config to listen on all addresses:

```shell
sed -i 's/127.0.0.1/0.0.0.0/' fullnode-template.yaml
```

```shell
-json-rpc-address: "127.0.0.1:9000"
+json-rpc-address: "0.0.0.0:9000"
```
