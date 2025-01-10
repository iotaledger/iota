## Overview

This document focuses on running the IOTA Node software as a Validator.

<div className="hidden-text">

## Contents

- [Overview](#overview)
- [Contents](#contents)
- [Requirements](#requirements)
- [Deployment](#deployment)
- [Configuration](#configuration)
- [Connectivity](#connectivity)
- [Network Buffer](#network-buffer)
  - [Option 1: With /etc/sysctl.d/](#option-1-with-etcsysctld)
  - [Option 2: With sysctl command](#option-2-with-sysctl-command)
  - [Verification](#verification)
- [Storage](#storage)
- [Key Management](#key-management)
- [Monitoring](#monitoring)
  - [Metrics](#metrics)
  - [Logs](#logs)
- [Software Updates](#software-updates)
- [State Sync](#state-sync)
- [Chain Operations](#chain-operations)
  - [Updating On-chain Metadata](#updating-on-chain-metadata)
  - [Operation Cap](#operation-cap)
  - [Reporting/Un-reporting Validators](#reportingun-reporting-validators)
  - [Joining the Validator Set](#joining-the-validator-set)
  - [Leaving the Validator Set](#leaving-the-validator-set)
- [Private Security Fixes](#private-security-fixes)

</div>

## Requirements

To run an Iota Validator a machine with the following is required:

- CPU: 24 physical cores (or 48 virtual cores)
- Memory: 128 GB
- Storage: 4 TB NVME
- Network: 1 Gbps

## Deployment

IOTA Node can be deployed in a number of ways.

There are pre-built container images available in [Docker Hub](https://hub.docker.com/r/iotaledger/iota-node/tags).

And pre-built `linux/amd64` binaries available in [Github](https://github.com/iotaledger/iota/releases):

To build directly from source (select the branch you want to build from depending on the network you want to connect to):

```shell
git clone https://github.com/iotaledger/iota.git && cd iota
git checkout [devnet|testnet|mainnet]
cargo build --release --bin iota-node
```

Configuration and guides are available for the following deployment options:

- [Systemd](./systemd/)
- [Ansible](./ansible/)
- [Docker Compose](./docker/)

## Configuration

IOTA Node runs with a single configuration file provided as an argument, example:

`./iota-node --config-path /opt/iota/config/validator.yaml`.

Configuration templates are available here:

- [Validator](./config/validator.yaml)

## Connectivity

Iota Node uses the following ports by default:

| protocol/port | reachability     | purpose                           |
| ------------- | ---------------- | --------------------------------- |
| TCP/8080      | inbound          | protocol/transaction interface    |
| TCP/8081      | inbound/outbound | primary interface                 |
| UDP/8084      | inbound/outbound | peer to peer state sync interface |
| TCP/8443      | outbound         | metrics pushing                   |
| TCP/9184      | localhost        | metrics scraping                  |

To run a validator successfully it is critical that ports 8080-8084 are open as outlined above, including the specific
protocol (TCP/UDP).

## Network Buffer

From load testing IOTA validator networks, it has been determined that the default Linux network buffer sizes are too
small.
We recommend increasing them using one of the following two methods:

### Option 1: With /etc/sysctl.d/

These settings can be added to a new sysctl file specifically for the iota-node, or appended to an existing file.
Modifications made in this way will persist across system restarts.

```shell
# Create a new sysctl file for the iota-node
sudo nano /etc/sysctl.d/100-iota-node.conf

# Add these lines to the file, overwriting existing settings if necessary.
net.core.rmem_max = 104857600
net.core.wmem_max = 104857600
net.ipv4.tcp_rmem = 8192 262144 104857600
net.ipv4.tcp_wmem = 8192 262144 104857600

# Apply the settings immediately, before the next restart
sudo sysctl --system
```

### Option 2: With sysctl command

These modifications do not persist across system restarts. Therefore, the commands should be run each time the host
restarts.

```shell
sudo sysctl -w net.core.wmem_max=104857600
sudo sysctl -w net.core.rmem_max=104857600
sudo sysctl -w net.ipv4.tcp_rmem="8192 262144 104857600"
sudo sysctl -w net.ipv4.tcp_wmem="8192 262144 104857600"
```

### Verification

To verify that the system settings have indeed been updated, check the output of the following command:

```shell
sudo sysctl -a | egrep [rw]mem
```

## Storage

All Iota Node-related data is stored by default under `/opt/iota/db/`. This is controlled in the Iota Node configuration
file.

```shell
$ cat /opt/iota/config/validator.yaml | grep db-path
db-path: /opt/iota/db/authorities_db
  db-path: /opt/iota/db/consensus_db
```

Ensure that you have an appropriately sized disk mounted for the database to write to.

- To check the size of the local Iota Node databases:

```shell
du -sh /opt/iota/db/
du -sh /opt/iota/db/authorities_db
du -sh /opt/iota/db/consensus_db
```

- To delete the local Iota Node databases:

```shell
sudo systemctl stop iota-node
sudo rm -rf /opt/iota/db/authorities_db /opt/iota/db/consensus_db
```

## Key Management

The following keys are used by Iota Node:

| key           | scheme   | purpose                                                 |
| ------------- | -------- | ------------------------------------------------------- |
| authority.key | bls12381 | transactions, BLS key to create an aggregated signature |
| account.key   | ed25519  | controls assets for staking                             |
| network.key   | ed25519  | consensus primary, iota state sync                      |
| protocol.key  | ed25519  | key to sign mysticeti consensus blocks                  |

These are configured in the [Iota Node configuration file](#configuration).

## Monitoring

### Metrics

Iota Node exposes metrics via a local HTTP interface. These can be scraped for use in a central monitoring system as
well as viewed directly from the node.

- View all metrics:

```shell
curl -s http://localhost:9184/metrics
```

- Search for a particular metric:

```shell
curl http://localhost:9184/metrics | grep <METRIC>
```

Iota Node also pushes metrics to a central Iota metrics proxy.

### Logs

Logs are controlled using the `RUST_LOG` environment variable.

The `RUST_LOG_JSON=1` environment variable can optionally be set to enable logging in JSON structured format.

Depending on your deployment method, these will be configured in the following places:

- If using Ansible, [here](./ansible/roles/iota-node/files/iota-node.service)
- If using Systemd natively, [here](./systemd/iota-node.service)
- If using Docker Compose, [here](./docker/docker-compose.yaml)

To view and follow the Iota Node logs:

```shell
journalctl -u iota-node -f
```

To search for a particular match

```shell
journalctl -u iota-node -g <SEARCH_TERM>
```

- If using Docker Compose, look at the examples [here](./docker/#logs)

It is possible to change the logging configuration while a node is running using the admin interface.

To view the currently configured logging values:

```shell
curl localhost:1337/logging
```

To change the currently configured logging values:

```shell
curl localhost:1337/logging -d "info"
```

## Software Updates

When an update is required to the Iota Node software the following process can be used. Follow the relevant Systemd or
Docker Compose runbook depending on your deployment type. It is highly unlikely that you will want to restart with a
clean database.

- If using Systemd, [here](./systemd/#updates)
- If using Docker Compose, [here](./docker/#updates)

## State Sync

Checkpoints in Iota contain the permanent history of the network. They are comparable to blocks in other blockchains
with the biggest difference being that they are lagging instead of leading. All transactions are final and executed
prior to being included in a checkpoint.

These checkpoints are synchronized between validators and fullnodes via a dedicated peer to peer state sync interface.

Inter-validator state sync is always permitted, however, there are controls available to limit what fullnodes are
allowed to sync from a specific validator.

The default and recommended `max-concurrent-connections: 0` configuration does not affect inter-validator state sync,
but will restrict all fullnodes from syncing. The Iota Node [configuration](#configuration) can be modified to allow a
known fullnode to sync from a validator:

```shell
p2p-config:
  anemo-config:
    max-concurrent-connections: 0
  seed-peers:
    - address: <multiaddr>  # The p2p address of the fullnode
      peer-id: <peer-id>    # hex encoded network public key of the node
    - address: ...          # another permitted peer
      peer-id: ...
```

## Chain Operations

The following chain operations are executed using the `iota` CLI. This binary is built and provided as a release similar
to `iota-node` on [Github](https://github.com/iotaledger/iota/releases).

It is recommended and often required that the `iota` binary release/version matches that of the deployed network.

### Updating On-chain Metadata

You can leverage the [Validator Tool](../validator-tools) to perform the majority of the following tasks.

An active/pending validator can update its on-chain metadata by submitting a transaction. Some metadata changes take
effect immediately, including:

- name
- description
- image url
- project url

Other metadata (keys, addresses etc) only come into effect at the next epoch.

To update metadata, a validator makes a MoveCall transaction that interacts with the System Object. For example:

1. To update the name to `new_validator_name`, use the Iota Client CLI to call `iota_system::update_validator_name`:

```shell
iota client call --package 0x3 --module iota_system --function update_validator_name --args 0x5 \"new_validator_name\" --gas-budget 10000
```

2. To update the p2p address starting from next epoch to `/ip4/192.168.1.1`, use the Iota Client CLI to call
   `iota_system::update_validator_next_epoch_p2p_address`:

```shell
iota client call --package 0x3 --module iota_system --function update_validator_next_epoch_p2p_address --args 0x5 "[4, 192, 168, 1, 1]" --gas-budget 10000
```

<!-- Will be fixed by issue 1867. -->
<!-- See the full list of metadata `update_*` functions starting [from here](https://docs.iota.org/references/framework/iota-system/iota_system#function-update_validator_name). -->

### Operation Cap

To avoid touching account keys too often and allowing them to be stored offline, validators can delegate the operation
ability to another address. This address can then update the reference gas price and tallying rule on behalf of the
validator.

Upon creating a `Validator`, an `UnverifiedValidatorOperationCap` is created as well and transferred to the validator
address. The holder of this `Cap` object (short for "Capability") therefore could perform operational actions for this
validator. To authorize another address to conduct these operations, a validator transfers the object to another address
that they control. The transfer can be done by using the Iota Client CLI: `iota client transfer`.

To rotate the delegatee address or revoke the authorization, the current holder of `Cap` transfers it to another
address. In the event of compromised or lost keys, the validator could create a new `Cap` object to invalidate the
incumbent one. This is done by calling `iota_system::rotate_operation_cap`:

```shell
iota client call --package 0x3 --module iota_system --function rotate_operation_cap --args 0x5 --gas-budget 10000
```

By default the new `Cap` object is transferred to the validator address, which then could be transferred to the new
delegatee address. At this point, the old `Cap` becomes invalidated and no longer represents eligibility.

To get the current valid `Cap` object's ID of a validator, use the Iota Client CLI `iota client objects` command after
setting the holder as the active address.

<!-- Will be fixed by issue 1867. -->
<!-- Or go to the [explorer](https://explorer.rebased.iota.org/object/0x0000000000000000000000000000000000000005) and look for `operation_cap_id` of that validator in the `validators` module. -->

### Reporting/Un-reporting Validators

To report a validator or undo an existing report, the sender needs to hold a valid [`UnverifiedValidatorOperationCap`](#operation-cap). The sender could be the validator itself, or a trusted delegatee. To
do so, call `iota_system::report_validator/undo_report_validator`:

```shell
iota client call --package 0x3 --module iota_system --function report_validator/undo_report_validator --args 0x5 {cap_object_id} {reportee_address} --gas-budget 10000
```

Once a validator is reported by `2f + 1` other validators by voting power, their staking rewards will be slashed.

### Joining the Validator Set

In order for an Iota address to join the validator set, they need to first sign up as a validator candidate by calling
`iota_system::request_add_validator_candidate` with their metadata and initial configs:

```shell
iota client call --package 0x3 --module iota_system --function request_add_validator_candidate --args 0x5 {authority_pubkey_bytes} {network_pubkey_bytes} {protocol_pubkey_bytes} {proof_of_possession} {name} {description} {image_url} {project_url} {net_address}
{p2p_address} {primary_address} {gas_price} {commission_rate} --gas-budget 10000
```

After an address becomes a validator candidate, any address (including the candidate address itself) can start staking
with the candidate's staking pool. Once a candidate's staking pool has accumulated at least
`iota_system::MIN_VALIDATOR_JOINING_STAKE` amount of stake, the candidate can call `iota_system::request_add_validator`
to officially add themselves to the next epoch's active validator set:

```shell
iota client call --package 0x3 --module iota_system --function request_add_validator --args 0x5 --gas-budget 10000000
```

### Leaving the Validator Set

To leave the validator set starting from the next epoch, the sender needs to be an active validator in the current epoch
and should call `iota_system::request_remove_validator`:

```shell
iota client call --package 0x3 --module iota_system --function request_remove_validator --args 0x5 --gas-budget 10000
```

After the validator is removed at the next epoch change, the staking pool will become inactive and stakes can only be
withdrawn from an inactive pool.

## Private Security Fixes

There may be instances where urgent security fixes need to be rolled out before publicly announcing it's presence (
issues affecting liveness, invariants such as IOTA supply, governance, etc.). In order to not be actively exploited the
IOTA Foundation will release signed security binaries incorporating such fixes with a delay in publishing the source
code until a large % of our validators have patched the vulnerability.

This release process will be different and we expect to announce the directory for such binaries out of band.

<!-- Will be fixed by issue 1867. -->
<!-- Our public key to verify these binaries would be stored [here](https://<TODO_SECURITY_FIXES_URL>/iota_security_release.pem) -->

You can download all the necessary signed binaries and docker artifacts incorporating the security fixes by using
the [download_private.sh](https://github.com/iotaledger/iota/blob/develop/nre/download_private.sh)

Usage
`./download_private.sh <directory-name>`

You can also download and verify specific binaries that may not be included by the above script using
the [download_and_verify_private_binary.sh](https://github.com/iotaledger/iota/blob/develop/nre/download_and_verify_private_binary.sh)
script.

Usage:
`./download_and_verify_private_binary.sh <directory-name> <binary-name>`
