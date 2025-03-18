# IOTA Validator Setup Guide - Testnet

This guide walks you through the essential steps to set up an IOTA validator node.

## Required Network Ports

Ensure the following ports are open in your firewall:

| Port     | Reachability     | Purpose                           |
| -------- | ---------------- | --------------------------------- |
| TCP/8080 | inbound          | protocol/transaction interface    |
| TCP/8081 | inbound/outbound | primary interface                 |
| UDP/8084 | inbound/outbound | peer to peer state sync interface |
| TCP/8443 | outbound         | metrics pushing                   |
| TCP/9184 | localhost        | metrics scraping                  |

> **Note**: Maybe you already noticed that port 8081 is using TCP, which conflicts with docs.iota.org and validator.info as well. This is a known bug, but the node is actually communicating via TCP.

## Environment Variables (Optional)

These environment variables are pre-configured for the testnet. You typically don't need to modify them unless you're using custom endpoints.

```bash
export IOTA_API_ENDPOINT="https://api.testnet.iota.cafe"
export IOTA_FAUCET_ENDPOINT="https://faucet.testnet.iota.cafe/v1/gas"
export IOTA_TOOLS_DOCKER_IMAGE="iotaledger/iota-tools:testnet"
```

## Setup Steps

### 1. Download Configuration Template

Download the validator configuration file template:

```bash
curl -L -o validator.yaml https://github.com/iotaledger/iota/raw/refs/heads/develop/setups/validator/systemd/validator.yaml
```

### 2. Configure P2P Settings

Configure the P2P settings in your `validator.yaml` file by following these steps:

1. Update the external address:

```yaml
p2p-config:
  external-address: /dns/<YOUR-DOMAIN>/udp/8084
```

> **Note**: Replace `<YOUR-DOMAIN>` with your validator's public domain name. This address must be accessible from the internet.

2. Add the seed peers configuration:

```yaml
p2p-config:
  listen-address: "0.0.0.0:8084"
  external-address: /dns/<YOUR-DOMAIN>/udp/8084
  anemo-config:
    max-concurrent-connections: 0
  seed-peers:
    - address: /dns/access-0.r.testnet.iota.cafe/udp/8084
      peer-id: 46064108d0b689ed89d1f44153e532bb101ce8f8ca3a3d01ab991d4dea122cfc
    - address: /dns/access-1.r.testnet.iota.cafe/udp/8084
      peer-id: 8ffd25fa4e86c30c3f8da7092695e8a103462d7a213b815d77d6da7f0a2a52f5
```

### 3. Configure Metrics Pushing Target

Configure the metrics settings in your `validator.yaml` file by following these steps:

Update the `push-url`:

```yaml
metrics:
  push-interval-seconds: 60
  push-url: https://metrics-proxy.testnet.iota.cafe:8443/publish/metrics
```

### 4. Download Genesis Block

```bash
curl -fLJO https://dbfiles.testnet.iota.cafe/genesis.blob
```

> **Note**: The URL is for the IOTA Testnet only.

### 5. Make validator.info and Generate Validator Keys

Generate the necessary key pairs for your validator, the key pairs will be stored in `key-pairs` folder.

```bash
./generate_validator_info.sh
```

After running `./generate_validator_info.sh`, you'll receive output similar to this:

```
Validator Address: 0xa8769934bf4fa35eb8fa8313beeb1756258e165dcd265239536ac396c26fa676
Script Version: 5d47a55
```

Copy this validator information and save it for later use in step 9, where you'll need to provide it to the IOTA Foundation when requesting delegation.

> **Important**: Back up your generated keys securely. Loss of these keys could result in loss of access to your validator.

### 6. Compile the Node

Run the following command to compile the `iota-node`.

```bash
cargo build --release --bin iota-node
```

### 7. Start Services

At this point, your IOTA full node is ready to connect to the IOTA network.

Open a terminal or console to the `iota` directory and run the following command to start your node:

```bash
./target/release/iota-node --config-path validator.yaml
```

### 8. Register as a Validator Candidate

We will obtain some tokens from the faucet for gas fees.

```bash
./become_candidate.sh
```

### 9. Request Delegation from IOTA Foundation

Contact the IOTA Foundation with your validator information obtained in Step 5.

### 10. Join the Committee

Before joining the committee, ensure:

- Your node is fully synced with the network
- IOTA Foundation has already delegated the staking tokens for your validator

Once your node is ready, submit your request to join the committee:

```bash
./join_committee.sh
```

### 11. Monitor Validator Status

```bash
docker run --rm -v ./iota_config:/root/.iota/iota_config iotaledger/iota-tools:testnet /bin/sh -c "/usr/local/bin/iota validator display-metadata" | grep status
```

You should see your node's status is `pending` now, it will become active and join the committee starting from the next epoch.

```
<YOUR-VALIDATOR_ADDRESS>'s validator status: Pending
```
