"""Low-level primitives that introduce network disruptions."""

from __future__ import annotations

import logging
import re
import subprocess

from . import docker_env

log = logging.getLogger(__name__)

_TC_DEV = "eth0"
_IPTABLES_CHAIN = "DOCKER-USER"
_RULE_COMMENT_PREFIX = "net-fuzz"
_VALIDATOR_RE = re.compile(r"^validator-(\d+)$")


class DisruptionError(RuntimeError):
    """Raised when applying or reverting a disruption fails."""


def _run_host_command(args: list[str], *, check: bool = True) -> subprocess.CompletedProcess[str]:
    """Run a command on the host system."""
    log.debug("Host command: %s", " ".join(args))
    result = subprocess.run(args, capture_output=True, text=True)
    if check and result.returncode != 0:
        raise DisruptionError(
            f"Command {' '.join(args)} failed (code={result.returncode}): {result.stderr.strip()}"
        )
    return result


def _nsenter(pid: int, args: list[str], *, check: bool = True) -> subprocess.CompletedProcess[str]:
    """Run a command inside the container's network namespace."""

    return _run_host_command(["nsenter", "-t", str(pid), "-n", *args], check=check)


def _require_pid(name: str) -> int:
    """Get the PID of the container, raising if not running."""
    pid = docker_env.get_container_pid(name)
    if not pid:
        raise DisruptionError(f"Container {name!r} is not running (no PID)")
    return pid


def add_latency(
    src: str,
    dst: str,
    delay_ms: int,
    jitter_ms: int = 0,
    loss_pct: float = 0.0,
) -> None:
    """Apply latency and optional loss from ``src`` to ``dst``.

    Implements per-destination shaping by:

    - ensuring a classful HTB root on the source interface,
    - attaching a dedicated class + netem qdisc for the destination, and
    - adding a ``u32`` filter that matches the destination IP and sends
      traffic into that class.

    Subsequent calls for other destinations reuse the same root and add
    additional classes/filters.
    """

    pid = _require_pid(src)
    dst_ip = docker_env.get_container_ip(dst)
    if not dst_ip:
        raise DisruptionError(f"Unable to resolve container IP for {dst!r}")

    # Ensure a classful root qdisc exists once per node
    show = _nsenter(pid, ["tc", "qdisc", "show", "dev", _TC_DEV], check=False)
    if "htb 1:" not in (show.stdout or ""):
        _nsenter(pid, ["tc", "qdisc", "del", "dev", _TC_DEV, "root"], check=False)
        _nsenter(
            pid,
            ["tc", "qdisc", "add", "dev", _TC_DEV, "root", "handle", "1:", "htb", "default", "1"],
            check=False,
        )
        _nsenter(
            pid,
            [
                "tc",
                "class",
                "add",
                "dev",
                _TC_DEV,
                "parent",
                "1:",
                "classid",
                "1:1",
                "htb",
                "rate",
                "1000mbit",
                "ceil",
                "1000mbit",
            ],
            check=False,
        )

    # Derive a stable classid from dst name or IP
    m = _VALIDATOR_RE.match(dst)
    if m:
        idx = int(m.group(1))
    else:
        # Fallback: use last octet of the IP for non-validator containers
        try:
            idx = int(dst_ip.split(".")[-1])
        except ValueError:
            idx = 1
    classid = f"1:{100 + idx}"

    # Create/update dedicated class and netem qdisc for this destination
    _nsenter(
        pid,
        [
            "tc",
            "class",
            "replace",
            "dev",
            _TC_DEV,
            "parent",
            "1:",
            "classid",
            classid,
            "htb",
            "rate",
            "1000mbit",
            "ceil",
            "1000mbit",
        ],
        check=False,
    )

    cmd = [
        "tc",
        "qdisc",
        "replace",
        "dev",
        _TC_DEV,
        "parent",
        classid,
        "netem",
        "delay",
        f"{delay_ms}ms",
    ]
    if jitter_ms:
        cmd.append(f"{jitter_ms}ms")
    if loss_pct > 0:
        cmd.extend(["loss", f"{loss_pct:.2f}%"])
    _nsenter(pid, cmd, check=False)

    # Check existing filters to avoid duplicates. We must ensure that the
    # IP and classid appear on the *same* filter line; otherwise we might
    # incorrectly assume the filter exists.
    filters = _nsenter(pid, ["tc", "filter", "show", "dev", _TC_DEV, "parent", "1:"], check=False)
    filters_out = filters.stdout or ""
    existing = False
    for line in filters_out.splitlines():
        if dst_ip in line and f"flowid {classid}" in line:
            existing = True
            break

    if existing:
        log.info("Filter for %s -> %s already exists, skipping filter add", src, dst)
    else:
        # Attach a filter that routes traffic to dst_ip into the class
        _nsenter(
            pid,
            [
                "tc",
                "filter",
                "add",
                "dev",
                _TC_DEV,
                "parent",
                "1:",
                "protocol",
                "ip",
                "u32",
                "match",
                "ip",
                "dst",
                f"{dst_ip}/32",
                "flowid",
                classid,
            ],
            check=False,
        )
        log.info("Added filter for %s -> %s: %s -> class %s", src, dst, dst_ip, classid)

    log.info(
        "Applied latency: src=%s dst=%s delay=%sms jitter=%sms loss=%.2f%%",
        src,
        dst,
        delay_ms,
        jitter_ms,
        loss_pct,
    )


def _ensure_docker_user_chain() -> None:
    """Ensure that the DOCKER-USER chain exists and is hooked into FORWARD."""
    res = _run_host_command(["iptables", "-nL", _IPTABLES_CHAIN], check=False)
    if res.returncode != 0:
        _run_host_command(["iptables", "-N", _IPTABLES_CHAIN])
    res = _run_host_command(["iptables", "-C", "FORWARD", "-j", _IPTABLES_CHAIN], check=False)
    if res.returncode != 0:
        _run_host_command(["iptables", "-I", "FORWARD", "-j", _IPTABLES_CHAIN])


def _rule_comment(label: str) -> str:
    """Return the iptables comment used to tag net-fuzz rules."""
    return f"{_RULE_COMMENT_PREFIX}:{label}"


def _add_drop_rule(src_ip: str, dst_ip: str, label: str) -> None:
    """Add an iptables DROP rule from src_ip to dst_ip with a comment label."""
    spec = ["-s", src_ip, "-d", dst_ip, "-j", "DROP"]
    res = _run_host_command(["iptables", "-C", _IPTABLES_CHAIN, *spec], check=False)
    if res.returncode == 0:
        return
    _run_host_command(
        [
            "iptables",
            "-A",
            _IPTABLES_CHAIN,
            *spec,
            "-m",
            "comment",
            "--comment",
            _rule_comment(label),
        ]
    )


def block_connection(src: str, dst: str) -> None:
    """Block all network traffic between ``src`` and ``dst`` containers."""
    src_ip = docker_env.get_container_ip(src)
    dst_ip = docker_env.get_container_ip(dst)
    if not src_ip or not dst_ip:
        raise DisruptionError(f"Unable to resolve container IPs for {src!r} or {dst!r}")

    _ensure_docker_user_chain()
    _add_drop_rule(src_ip, dst_ip, f"{src}->{dst}")
    _add_drop_rule(dst_ip, src_ip, f"{dst}->{src}")
    log.info("Blocked connection between %s (%s) and %s (%s)", src, src_ip, dst, dst_ip)


def _delete_drop_rule(src_ip: str, dst_ip: str, label: str) -> None:
    """Delete an iptables DROP rule from src_ip to dst_ip with a comment label."""
    spec = ["-s", src_ip, "-d", dst_ip, "-j", "DROP", "-m", "comment", "--comment", _rule_comment(label)]
    _run_host_command(["iptables", "-D", _IPTABLES_CHAIN, *spec], check=False)


def unblock_connection(src: str, dst: str) -> None:
    """Unblock all network traffic between ``src`` and ``dst`` containers."""
    src_ip = docker_env.get_container_ip(src)
    dst_ip = docker_env.get_container_ip(dst)
    if not src_ip or not dst_ip:
        return
    _delete_drop_rule(src_ip, dst_ip, f"{src}->{dst}")
    _delete_drop_rule(dst_ip, src_ip, f"{dst}->{src}")
    log.info("Unblocked connection between %s and %s", src, dst)


def restart_node(name: str) -> None:
    """Restart a container by name."""
    docker_env.restart_container(name)
    log.info("Restarted node %s", name)


def kill_node(name: str) -> None:
    """Stop a container by name."""
    docker_env.stop_container(name, timeout=5)
    log.info("Stopped node %s", name)


def _clear_node_qdisc(name: str) -> None:
    """Remove any existing qdisc on the node's primary interface."""

    try:
        pid = docker_env.get_container_pid(name)
    except docker_env.DockerEnvError:
        return
    if not pid:
        return
    _nsenter(pid, ["tc", "qdisc", "del", "dev", _TC_DEV, "root"], check=False)


def clear_all_net_fuzz_rules() -> None:
    """Delete all iptables rules installed by this module in DOCKER-USER.

    Rules are identified via the ``net-fuzz:...`` comment prefix and
    removed bottom-up to keep line numbers valid.
    """

    res = _run_host_command(
        ["iptables", "-L", _IPTABLES_CHAIN, "-n", "--line-numbers"],
        check=False,
    )
    if res.returncode != 0:
        return

    to_delete: list[int] = []
    for line in res.stdout.splitlines():
        if _RULE_COMMENT_PREFIX not in line:
            continue
        parts = line.split()
        if not parts:
            continue
        num_str = parts[0]
        if num_str.isdigit():
            to_delete.append(int(num_str))

    for num in sorted(to_delete, reverse=True):
        _run_host_command(["iptables", "-D", _IPTABLES_CHAIN, str(num)], check=False)


def reset_network(num_validators: int) -> None:
    """Best-effort reset of validators and network shaping state.

    - Restart all ``validator-1 .. validator-N`` containers.
    - Clear any tc/netem configuration on their primary interface.
    - Remove all DOCKER-USER rules installed by :mod:`disruptions`.
    """

    names = [f"validator-{i}" for i in range(1, num_validators + 1)]

    for name in names:
        try:
            docker_env.restart_container(name)
        except docker_env.DockerEnvError:
            log.warning("Failed to restart %s during reset", name)

    for name in names:
        _clear_node_qdisc(name)

    clear_all_net_fuzz_rules()
