"""Verification helpers for network disruptions."""

from __future__ import annotations

import logging
import re
import subprocess

from . import docker_env

log = logging.getLogger(__name__)

_TC_DEV = "eth0"
_IPTABLES_CHAIN = "DOCKER-USER"
_DELAY_RE = re.compile(r"delay\s+([0-9.]+)\s*(ms|s|us)", re.IGNORECASE)
_LOSS_RE = re.compile(r"loss\s+([0-9.]+)%")
_RULE_COMMENT_PREFIX = "net-fuzz"
_VALIDATOR_RE = re.compile(r"^validator-(\d+)$")


def _ip_to_hex(ip: str) -> str | None:
    """Return the lowercase hex form of an IPv4 address (no separators)."""
    try:
        octets = [int(part) for part in ip.split(".")]
    except ValueError:
        return None
    if len(octets) != 4 or any(part < 0 or part > 255 for part in octets):
        return None
    return "".join(f"{part:02x}" for part in octets)


def _classid_for_dst(dst: str, dst_ip: str) -> str:
    """Return the classid used by disruptions.add_latency for a destination."""
    match = _VALIDATOR_RE.match(dst)
    if match:
        idx = int(match.group(1))
    else:
        try:
            idx = int(dst_ip.split(".")[-1])
        except (IndexError, ValueError):
            idx = 1
    return f"1:{100 + idx}"


def _parse_delay_ms(text: str) -> float | None:
    """Parse a tc netem delay string into milliseconds."""
    match = _DELAY_RE.search(text)
    if not match:
        return None
    value = float(match.group(1))
    unit = match.group(2).lower()
    if unit == "s":
        return value * 1000.0
    if unit == "us":
        return value / 1000.0
    return value


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


def check_latency(
    src: str,
    dst: str,
    expected_min_ms: int,
    expected_max_ms: int,
    *,
    emit_latency: bool = False,
) -> bool:
    """Check that latency from ``src`` to ``dst`` lies in the expected range.

    For per-destination shaping, we look for the ``tc filter`` matching
    ``dst``'s IP and then read the delay from the corresponding qdisc.
    If no such filter/qdisc is found, we fall back to the legacy
    node-wide netem check.
    """

    src_pid = docker_env.get_container_pid(src)
    if not src_pid:
        log.warning("Container %s has no PID; cannot check latency", src)
        return False

    dst_ip = docker_env.get_container_ip(dst)
    if not dst_ip:
        log.warning("Container %s has no IP; cannot check latency to %s", dst, src)
        return False
    dst_ip_hex = _ip_to_hex(dst_ip)

    classid = _classid_for_dst(dst, dst_ip)
    delay = None
    qdisc_out = subprocess.run(
        ["nsenter", "-t", str(src_pid), "-n", "tc", "qdisc", "show", "dev", _TC_DEV],
        capture_output=True,
        text=True,
    )
    if qdisc_out.returncode == 0:
        for line in qdisc_out.stdout.splitlines():
            if f"parent {classid} " in line:
                parsed = _parse_delay_ms(line)
                if parsed is not None:
                    delay = parsed
                    break

    # Fall back to filter lookup if classid is missing.
    if delay is None:
        filt = subprocess.run(
            ["nsenter", "-t", str(src_pid), "-n", "tc", "filter", "show", "dev", _TC_DEV, "parent", "1:"],
            capture_output=True,
            text=True,
        )
        flowid = None
        if filt.returncode == 0:
            for line in filt.stdout.splitlines():
                line_lower = line.lower()
                ip_match = dst_ip in line_lower
                if dst_ip_hex:
                    ip_match = ip_match or dst_ip_hex in line_lower
                if ip_match and "flowid" in line_lower:
                    parts = line.split()
                    for i, token in enumerate(parts):
                        if token == "flowid" and i + 1 < len(parts):
                            flowid = parts[i + 1]
                            break
                    if flowid:
                        break
        if flowid:
            for line in qdisc_out.stdout.splitlines():
                if f"parent {flowid} " in line:
                    parsed = _parse_delay_ms(line)
                    if parsed is not None:
                        delay = parsed
                        break

    # Fallback: legacy node-wide netem
    if delay is None:
        output = _read_tc_qdisc(src)
        if output is None:
            return False
        parsed = _parse_delay_ms(output)
        if parsed is None:
            log.debug("No delay configured on %s", src)
            return expected_min_ms == 0
        delay = parsed

    if expected_min_ms <= delay <= expected_max_ms:
        log.debug("Latency check passed for %s->%s (%.2fms)", src, dst, delay)
        return True
    if emit_latency:
        log.warning(
            "Latency mismatch for %s->%s: measured=%.2fms expected=[%s, %s]",
            src,
            dst,
            delay,
            expected_min_ms,
            expected_max_ms,
        )
    else:
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
    """Return True if bidirectional DROP rules exist for src<->dst."""
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
    """Return True if no bidirectional DROP rules exist for src<->dst."""
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
