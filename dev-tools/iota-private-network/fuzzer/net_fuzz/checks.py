"""Verification helpers for network disruptions."""

from __future__ import annotations

import logging
import re
import subprocess

from . import docker_env

log = logging.getLogger(__name__)

_TC_DEV = "eth0"
_IPTABLES_CHAIN = "DOCKER-USER"
_DELAY_RE = re.compile(r"delay\s+([0-9.]+)ms")
_LOSS_RE = re.compile(r"loss\s+([0-9.]+)%")
_RULE_COMMENT_PREFIX = "net-fuzz"


def _run_host_command(args: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(args, capture_output=True, text=True)


def _read_tc_qdisc(name: str) -> str | None:
    """Read ``tc qdisc`` output for the container via ``nsenter``.

    Mirrors the mechanism used in :mod:`disruptions`, which applies
    netem rules from the host by entering the container's network
    namespace instead of relying on ``tc`` being installed inside the
    container image.
    """

    pid = docker_env.get_container_pid(name)
    if not pid:
        log.warning("Container %s has no PID; cannot read tc qdisc", name)
        return None

    result = subprocess.run(
        ["nsenter", "-t", str(pid), "-n", "tc", "qdisc", "show", "dev", _TC_DEV],
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        log.warning("Failed to read tc qdisc for %s: %s", name, result.stderr.strip())
        return None
    return result.stdout


def check_latency(src: str, dst: str, expected_min_ms: int, expected_max_ms: int) -> bool:
    output = _read_tc_qdisc(src)
    if output is None:
        return False

    match = _DELAY_RE.search(output)
    if not match:
        log.debug("No delay configured on %s", src)
        return expected_min_ms == 0

    delay = float(match.group(1))
    if expected_min_ms <= delay <= expected_max_ms:
        log.debug("Latency check passed for %s->%s (%.2fms)", src, dst, delay)
        return True
    log.debug(
        "Latency check failed for %s->%s: %.2fms not in [%s, %s]",
        src,
        dst,
        delay,
        expected_min_ms,
        expected_max_ms,
    )
    return False


def check_loss(src: str, expected_min_pct: float, expected_max_pct: float) -> bool:
    """Verify that ``tc netem`` loss on ``src`` is within the expected range.

    Loss is configured per-node via :func:`disruptions.add_latency`'s
    ``loss_pct`` parameter and exposed in the ``tc qdisc`` output as a
    ``loss X%`` token.  This helper parses that value and checks that it
    lies between ``expected_min_pct`` and ``expected_max_pct``.
    """

    output = _read_tc_qdisc(src)
    if output is None:
        return False

    match = _LOSS_RE.search(output)
    if not match:
        loss = 0.0
    else:
        loss = float(match.group(1))

    if expected_min_pct <= loss <= expected_max_pct:
        log.debug("Loss check passed for %s (%.2f%%)", src, loss)
        return True

    log.debug(
        "Loss check failed for %s: %.2f%% not in [%.2f, %.2f]",
        src,
        loss,
        expected_min_pct,
        expected_max_pct,
    )
    return False


def _rule_comment(label: str) -> str:
    """Return the comment string used when installing DROP rules."""

    return f"{_RULE_COMMENT_PREFIX}:{label}"


def _iptables_has_drop(src_name: str, dst_name: str, src_ip: str, dst_ip: str) -> bool:
    """Check iptables for a DROP rule that matches src/dst and our comment.

    We mirror the full rule specification used by :mod:`disruptions` when
    adding rules, including the comment match, so that ``iptables -C``
    performs an exact lookup.
    """

    label = f"{src_name}->{dst_name}"
    spec = [
        "-s",
        src_ip,
        "-d",
        dst_ip,
        "-j",
        "DROP",
        "-m",
        "comment",
        "--comment",
        _rule_comment(label),
    ]
    cmd = ["iptables", "-C", _IPTABLES_CHAIN, *spec]
    result = subprocess.run(cmd, capture_output=True, text=True)
    return result.returncode == 0


def check_blocked(src: str, dst: str) -> bool:
    src_ip = docker_env.get_container_ip(src)
    dst_ip = docker_env.get_container_ip(dst)
    if not src_ip or not dst_ip:
        return False
    forward = _iptables_has_drop(src, dst, src_ip, dst_ip)
    backward = _iptables_has_drop(dst, src, dst_ip, src_ip)
    log.debug(
        "Checked block %s(%s) <-> %s(%s): forward=%s backward=%s",
        src,
        src_ip,
        dst,
        dst_ip,
        forward,
        backward,
    )
    return forward and backward


def check_unblocked(src: str, dst: str) -> bool:
    src_ip = docker_env.get_container_ip(src)
    dst_ip = docker_env.get_container_ip(dst)
    if not src_ip or not dst_ip:
        return True
    return not (
        _iptables_has_drop(src, dst, src_ip, dst_ip)
        or _iptables_has_drop(dst, src, dst_ip, src_ip)
    )


def check_node_down(name: str) -> bool:
    running = docker_env.is_container_running(name)
    log.debug("Node %s running=%s", name, running)
    return not running


def check_node_up(name: str) -> bool:
    running = docker_env.is_container_running(name)
    log.debug("Node %s running=%s", name, running)
    return running
