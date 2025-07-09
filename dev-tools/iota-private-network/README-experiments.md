# Network Disruption & Fuzz Testing

This collection of Bash scripts enables automated network perturbation experiments on an IOTA private validator network. Use these scripts to simulate various failure scenarios and measure system resilience.

## Prerequisites

- **Docker** (v20.10+)
- **gaiadocker/iproute2** image (for `tc netem` commands)
- **nicolaka/netshoot** image (for `iptables` testing)
- Scripts must be run on a host with root or equivalent privileges to manage Docker and network namespaces.

## Scripts Overview

### 1. `network-disruption-experiments.sh`

Apply controlled packet loss to a single validator (`validator-1`) with increasing percentages (20%, 40%, 60%, 80%, 100%), each for 60s, followed by 60s recovery.

**Usage:**

```bash
chmod +x network-disruption-experiments.sh
./network-disruption-experiments.sh
```

### 2. `network-disruption-experiments-all-validators.sh`

Same packet loss experiment, but executed on **all four validators** in parallel.

**Usage:**

```bash
chmod +x network-disruption-experiments-all-validators.sh
./network-disruption-experiments-all-validators.sh
```

### 3. `network-filtering-experiments.sh`

Simulate selective peer isolation using `iptables`. Runs three phases:

1. `validator-1` isolated from `validator-3` & `validator-4`
2. `validator-2` isolated from `validator-3`
3. `validator-4` isolated from `validator-2` & `validator-3`

Each phase lasts 60s with automatic cleanup.

**Usage:**

```bash
chmod +x network-filtering-experiments.sh
./network-filtering-experiments.sh
```

### 4. `network-fuzz-test.sh`

A 24-hour “fuzz” test that randomly applies:

- Container **pause/unpause**
- Container **restart**
- **Packet loss** (random percent)
- **iptables** block/unblock to random peers

Results are logged with timestamps. At the end of 24h, all rules are cleaned up.

**Usage:**

```bash
chmod +x network-fuzz-test.sh
./network-fuzz-test.sh
```

## How It Works

- **Packet Loss**: uses `tc netem` via `gaiadocker/iproute2` in the target container’s network namespace.
- **IPTables Filtering**: uses `nicolaka/netshoot` to run `iptables` commands against specific peer IPs.
- **Pause/Restart**: uses `docker pause` / `unpause` and `docker restart`.
- **Fuzz Testing**: randomizes timing and action types for long-duration robustness testing.

---

Ensure you have all Docker images pulled before running any script:

```bash
docker pull gaiadocker/iproute2
docker pull nicolaka/netshoot
```
