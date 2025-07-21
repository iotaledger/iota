# Network Disruption & Fuzz Testing Suite

This suite of Bash scripts automates network perturbation experiments against an IOTA private validator network. Use them to simulate failures and measure system resilience.

## Prerequisites

- **Docker** (v20.10+)
- **gaiadocker/iproute2** image (for `tc netem` commands)
- **nicolaka/netshoot** image (for `iptables` testing)
- Scripts must be run on a host with root or equivalent privileges to manage Docker and network namespaces.

## Scripts Overview

### 1. `experiments.sh`

Simulate incremental validator downtime in three phases, with delays increasing from 10s to 90s:

1. **Phase 1:** Stop `validator-1` for a delay that starts at 10s and increases by 30s each iteration up to 90s; restart and wait the same delay.

2. **Phase 2:** Pause `validator-1` for the same series of delays, then unpause and wait.

3. **Phase 3:** Disconnect `validator-1` for each delay, then reconnect, iterating delays from 10s to 90s.

**Usage:**

```bash
./experiments.sh
```

### 2. `network-disruption-experiments.sh`

Apply controlled packet loss to **validator-1**, stepping through 20%, 40%, 60%, 80%, and 100% loss. Each loss phase runs for 60s, followed by 60s recovery.

**Usage:**

```bash
./network-disruption-experiments.sh
```

### 3. `network-disruption-experiments-all-validators.sh`

Run the same packet loss sequence concurrently on all validators.

**Usage:**

```bash
./network-disruption-experiments-all-validators.sh
```

### 4. `network-filtering-experiments.sh`

Simulate directed connection partitions in three phases via `iptables` (default duration: 60s per phase):

1. **Phase 1:** isolate `validator-1` from `validator-2`

2. **Phase 2:** isolate `validator-1` from `validator-2` and isolate `validator-1` from `validator-3`

3. **Phase 3 (Mixed):**
   - isolate `validator-1` from `validator-2`
   - isolate `validator-1` from `validator-3`
   - isolate `validator-3` from `validator-4`
   - isolate `validator-4` from `validator-1`

Each phase lasts for the configured `duration` (default 60s) and is automatically restored afterward.

**Usage:**

```bash
./network-filtering-experiments.sh
```

### 5. `network-filtering-experiments-19.sh`

Same filtering experiment against a 19-validator network.

### 6. `network-fuzz-test.sh`

Execute a 24h randomized chaos test:

- Default: 4 validators; specify up to 19 with `-n`.

- Randomly selects validators to:
  - Stop & restart
  - Apply packet loss (20–100%)
  - Block/unblock peer traffic via `iptables`

**Usage:**

```bash
./network-fuzz-test.sh      # default (4 validators)
./network-fuzz-test.sh -n 19  # run with 19 validators
```

Results are logged to **fuzz.log** with timestamps for analysis.

---

Ensure you have all Docker images pulled before running any script:

```bash
docker pull gaiadocker/iproute2
docker pull nicolaka/netshoot
```
