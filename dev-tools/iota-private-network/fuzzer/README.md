# fuzzer – local usage guide

This directory contains the `net_fuzz` Python package, which provides
small, composable primitives to disrupt and inspect the IOTA private
network.

The goal of this document is to show how to:

- create a local Python environment for `fuzzer`
- apply disruptions (restart/kill, block connections, latency/loss)
- verify that each disruption actually took effect

> Note: the Python code never runs `sudo` internally. Anything
> that touches `iptables` or `tc/nsenter` must be executed with
> sufficient privileges from the outermost layer.

## Environment setup

From the repository root for the private network (make sure the validator
network is running, e.g. `./run.sh -n 10 -p mysticeti`):

```bash
cd ~/iota/dev-tools/iota-private-network

# create and activate a local virtualenv
python3 -m venv .venv
source .venv/bin/activate

# install the net_fuzz package + dependencies
pip install --upgrade pip
pip install -e fuzzer
```

For commands that need root (iptables, tc, nsenter), it is useful to
capture the venv Python path:

```bash
PYTHON=$(python -c 'import sys; print(sys.executable)')
```

If you prefer not to install the package inside the virtualenv, export
`PYTHONPATH=dev-tools/iota-private-network/fuzzer/src` so Python can import
`net_fuzz` straight from `src/net_fuzz`.

You can then run privileged scripts as:

```bash
sudo -E "$PYTHON" - <<'PY'
...
PY
```

## Node liveness: restart / kill and checks

These operations only talk to Docker and do not require root if your
user can access the Docker daemon.

Restart or stop a validator:

```bash
python - <<'PY'
from net_fuzz import disruptions, checks

name = "validator-1"

print("Before:", "up?", checks.check_node_up(name), "down?", checks.check_node_down(name))

disruptions.kill_node(name)       # stop the container (like `docker stop`)
print("After kill:", "up?", checks.check_node_up(name), "down?", checks.check_node_down(name))

disruptions.restart_node(name)    # restart the container
print("After restart:", "up?", checks.check_node_up(name), "down?", checks.check_node_down(name))
PY
```

## Blocking and unblocking connections

Blocking is implemented via host-level `iptables` rules in the
`DOCKER-USER` chain and requires root. The helpers install symmetric
DROP rules with comments of the form
`net-fuzz:validator-1->validator-2`.

Apply and verify a block:

```bash
sudo -E "$PYTHON" - <<'PY'
from net_fuzz import disruptions, checks

src, dst = "validator-1", "validator-2"

print("Initially blocked?", checks.check_blocked(src, dst))

disruptions.block_connection(src, dst)
print("After block:", "blocked?", checks.check_blocked(src, dst), "unblocked?", checks.check_unblocked(src, dst))

disruptions.unblock_connection(src, dst)
print("After unblock:", "blocked?", checks.check_blocked(src, dst), "unblocked?", checks.check_unblocked(src, dst))
PY
```

You can cross-check the rules manually:

```bash
sudo iptables -L DOCKER-USER -n --line-numbers | grep net-fuzz
```

## Latency and packet loss via tc/netem

Latency and loss are applied from the host by entering the container's
network namespace with `nsenter` and configuring `tc netem` on `eth0`.
Both application and verification require root.

Apply latency and loss:

```bash
sudo -E "$PYTHON" - <<'PY'
from net_fuzz import disruptions

src, dst = "validator-1", "validator-2"

disruptions.add_latency(src, dst, delay_ms=100, jitter_ms=10, loss_pct=2.0)
PY
```

Verify delay and loss:

```bash
sudo -E "$PYTHON" - <<'PY'
from net_fuzz import checks

src, dst = "validator-1", "validator-2"

print("Latency OK?", checks.check_latency(src, dst, expected_min_ms=95, expected_max_ms=105))
print("Loss OK?",    checks.check_loss(src, expected_min_pct=1.0, expected_max_pct=3.0))
PY
```

You can manually inspect the qdisc as well:

```bash
sudo nsenter -t "$(docker inspect -f '{{.State.Pid}}' validator-1)" -n \
  tc qdisc show dev eth0
```

## Running scenarios via the CLI

The `net_fuzz.scenarios` module exposes higher-level compositions. A
minimal example is the latency scenario:

```bash
sudo -E "$PYTHON" -m net_fuzz run-scenario \
  --name latency \
  --src validator-1 \
  --dst validator-2 \
  --delay-ms 100
```

This applies latency once and the CLI process exits immediately—there is no
long-running worker—but the `tc` state persists until you reset the network.
To clean up, either restart the validators (e.g. run
`sudo -E "$PYTHON" -m net_fuzz.verify_disruptions`, which resets at the end) or
call `disruptions.reset_network` manually, for example:

```bash
sudo -E "$PYTHON" - <<'PY'
from net_fuzz import disruptions
disruptions.reset_network(num_validators=10)
PY
```

More complex fuzz scenarios can be added later without changing how the
environment is set up.

## Long-running fuzz scenario

The `fuzz_scenario` combines node churn, network partitions and
latency/loss into a single long-running experiment. It:

- keeps strictly fewer than 1/3 of validators down at any time
- uses exponential time-to-stop and time-to-recover per node
- applies global latency/loss updates at exponential intervals
- blocks/unblocks every validator pair via its own exponential process
- resets the network to a clean state at the start and the end

Example invocation from the private-network root:

```bash
source .venv/bin/activate
PYTHON=$(python -c 'import sys; print(sys.executable)')

sudo -E "$PYTHON" -m net_fuzz run-scenario \
  --name fuzz \
  --num-validators 10 \
  --duration 6000 \
  --seed 42 \
  --mean-down 120 \
  --mean-up 600 \
  --max-latency-ms 200 \
  --max-loss-pct 5.0 \
  --latency-interval 120 \
  --block-interval 150 \
  --spammer-tps 100
```

At the beginning and in a `finally` block at the end the scenario
calls `disruptions.reset_network`, which:

- restarts all `validator-1 .. validator-N` containers
- clears any `tc` qdisc on their primary interfaces
- removes all `net-fuzz:` rules from the `DOCKER-USER` chain

If `--spammer-tps` is greater than zero, the scenario will also:

- stop any existing `stress-benchmark` container
- start a background `stress` spammer via the `iotaledger/iota-tools`
  image with `--target-qps` set to the given TPS
- ensure the spammer container is stopped again during scenario cleanup

You can also run the spammer directly:

```bash
source .venv/bin/activate
PYTHON=$(python -c 'import sys; print(sys.executable)')

sudo -E "$PYTHON" -m net_fuzz.spammer --tps 100 --duration 600
```

This will:

- ensure `fullnode-1` and `faucet-1` are running (starting them via
  `docker compose up -d` if necessary)
- start a detached `stress-benchmark` container on the
  `iota-private-network_iota-network` Docker network
- if `--duration` is set, stop the container after the given number of
  seconds.

## Longer experiments

Long-running stress experiments live under `net_fuzz.experiments`. See
`dev-tools/iota-private-network/fuzzer/src/net_fuzz/experiments/README.md` for
details. Protocol comparison runners (`run_*.py`) also live in
`dev-tools/iota-private-network/fuzzer/src/net_fuzz/experiments/` and automate
running the same scenario across Mysticeti and Starfish.

## Logging

`net_fuzz` uses the standard `logging` module with a basic
configuration (timestamp + level + logger name). By default logs go to
stderr, so you can redirect them to a file when running experiments:

```bash
sudo -E "$PYTHON" -m net_fuzz run-scenario ... 2>&1 | tee experiments/logs/net-fuzz-$(date +%Y%m%d-%H%M%S).log
```

This keeps the Python logs alongside the existing bash-based experiment
logs under `experiments/logs/`.
