# Run Local Network & Mimic Artificial Latency & Add Fuzz Disruptions Suite

This suite of Bash scripts automates network perturbation experiments against an IOTA private validator network. Use them to mimic latencies like in a geodistributed network, simulate failures and measure system resilience.

## Prerequisites

- **Linux** host
- **Docker** (v20.10+)
- **gaiadocker/iproute2** image (for `tc netem` commands)
- **nicolaka/netshoot** image (for `iptables` testing)
- Scripts must be run on a host with root or equivalent privileges to manage Docker and network namespaces.

```bash
docker pull gaiadocker/iproute2
docker pull nicolaka/netshoot
```

## Script

`run-all.sh` automates the full workflow:

1. Optionally rebuilds the `iota-node` and `iota-tools` Docker images.
2. Bootstraps the validator network.
3. Runs the private network.
4. Runs grafana (available at `http:://localhost:3030/dashboards`)
5. Applies network latencies and controlled disruptions (packet loss, connection blocking, validator restarts).
6. Periodically collects logs and saves them with timestamps.

Supports the following flags:

- `-n <NUM>`: number of validators (default: `4`; any number between `4` and `19` is supported)
- `-p <protocol>`: consensus protocol (default: `mysticeti`; another option: `starfish`)
- `-b <true|false>`: rebuild Docker images before running (default: `true`)
- `-g <true|false>`: enable geodistributed large network latencies (default: `false`)
- `-s <SEED>`: seed for pseudorandom disruptions (default: `42`)
- `-x <PERCENT_BLOCK>`: percent of validator pairs to block connections (default: `0`)
- `-l <PERCENT_NETEM>`: percent of validators to apply packet loss (default: `0`)
- `-r <PERCENT_RESTART>`: percent of validators to restart periodically (default: `0`)
- `-t <RUN_DURATION>`: total experiment duration in seconds (default: `3600`)
- `-m`: optional flag to output network metric statistics (packets and bytes).
- `-S <true|false>`: enable the transaction spammer (default: `false`)
- `-T <TPS>`: transactions per second used by the spammer (default: `100`)
- `-Z <TRX_SIZE>`: number of shared objects per transaction for the spammer (default: `10`)
- `-C <spammer_type>`: type of spammer to use (default: `stress`; another option: `iota-spammer`)

The script should be run from inside the `iota/dev-tools/iota-private-network/experiments/` directory.

**Usage:**

```bash
# Run default 4-validator Mysticeti network with small latencies without any additional disruptions
./run-all.sh

# Run 10-validator Starfish network with large geodistributed latencies for one hour without rebuilding images
./run-all.sh -n 10 -p starfish -g true -b false

# Run 19-validator Starfish network with geodistributed latencies, 10% blocked connections, 5% chances for packet loss, 10% for restarts and running for 2 hours
./run-all.sh -n 19 -p starfish -g true -x 10 -l 5 -r 10 -t 7200
```

---

## Optional Transaction Spammer

The experiment suite can optionally include a transaction spammer to generate load on the validator network during the run.
It supports two types of spammer tools, by default the stress test from the iota benchmark, and optionally the `iota-spammer` from a private repository.

### With default spammer enabled:

```bash
./run-all.sh -n 4 -p mysticeti -S true -T 500
```

This will load the default spammer with a TPS of 500.

### Required Setup for optional Spammer

To enable the optional spammer set `-S true` and '-C iota-spammer' you must clone the following **private** repository:

```
https://github.com/iotaledger/iota-spammer
```

Place it at the following relative path from `run-all.sh`, or update the path in the script accordingly:

```
../../../iota-spammer
```

The optional spammer allows a special transaction type, called `sizable`, and can be used as follows:

```bash
./run-all.sh -n 4 -p mysticeti -S true -T 100 -Z 10KiB
```

This will launch the spammer from the external repository with the configured transaction rate, TPS=100, and size, 10KiB.
