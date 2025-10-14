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
