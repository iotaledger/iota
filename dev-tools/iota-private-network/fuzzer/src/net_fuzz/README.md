# net_fuzz core documentation

`net_fuzz` is a Python package that powers network fuzzing for the IOTA private
network. It provides small, composable primitives (Docker, iptables, tc/netem)
and higher-level scenarios built on top of them.

For environment setup and end-to-end usage examples, see
`dev-tools/iota-private-network/fuzzer/README.md`. The package uses a `pyproject.toml` / `src/` layout, so all sources live under
`dev-tools/iota-private-network/fuzzer/src/net_fuzz`.

## Module map

- `docker_env`: Docker SDK wrapper for listing containers, IP/PID lookup, and
  start/stop/exec helpers. This is the only place that should talk to Docker
  directly.
- `disruptions`: low-level network mutations (block/unblock, latency/loss,
  restart/kill). Uses `iptables` and `tc` via `nsenter`.
- `checks`: best-effort verification helpers for disruptions.
- `scenarios`: composable, higher-level orchestration. Returns `ScenarioResult`
  for logging and reporting.
- `spammer`: manages the `stress-benchmark` container from `iota-tools`.
- `metrics`: minimal Prometheus parsing utilities used by stress scripts
  (`*_stress`. Some helpers are placeholders.
- `cli` / `__main__`: entry point for `python -m net_fuzz`.
- `verify_disruptions`: manual smoke test for the low-level primitives.
- Long-running experiments live under `net_fuzz.experiments`:
  `block_stress`, `mirage_stress`, `non_triangle_stress`, `sync_stress`,
  `adaptive_fuzz`.

## Privilege boundary

- Docker operations require access to the Docker daemon, but no `sudo` inside
  Python.
- `iptables`, `tc`, and `nsenter` require root. Use `sudo -E "$PYTHON"` when
  running scenarios that call `disruptions` or `checks`.
- Always clean up: call `disruptions.reset_network` at the start and in a
  `finally` block to remove iptables rules and clear `tc` qdiscs.

## Environment assumptions

- Containers are named `validator-1..N`, `fullnode-1`, `faucet-1`.
- The spammer uses Docker network `iota-private-network_iota-network`.
- Host has `iptables`, `tc`, and `nsenter` available.

## Composition patterns

Use primitives from `disruptions` + `checks`, then wrap them in a clean-up
boundary:

```python
from net_fuzz import checks, disruptions

num_validators = 4
src, dst = "validator-1", "validator-2"

try:
    disruptions.reset_network(num_validators)
    disruptions.block_connection(src, dst)
    if not checks.check_blocked(src, dst):
        raise RuntimeError("block did not apply")
finally:
    disruptions.reset_network(num_validators)
```

When building new scenarios:

- keep subprocess/host calls (iptables, tc, docker CLI, nsenter) confined to
  `disruptions` and `docker_env`, so privileged boundaries stay obvious
- log key events and return a structured `ScenarioResult`
- use a deterministic `Random(seed)` for reproducibility
- restore the network on exit

## Entry points

- `python -m net_fuzz run-scenario --name latency --src validator-1 --dst validator-2 --delay-ms 100`
- `python -m net_fuzz run-scenario --name fuzz --num-validators 19 --duration 600`
- `python -m net_fuzz.verify_disruptions --num-validators 4`
- `python -m net_fuzz.spammer --tps 100 --duration 600`
- `python -m net_fuzz.experiments.block_stress` (long-running stress script)
