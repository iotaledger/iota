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


def _run_host_command(args: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(args, capture_output=True, text=True)


def check_latency(src: str, dst: str, expected_min_ms: int, expected_max_ms: int) -> bool:
    try:
        output = docker_env.run_in_container(src, ["tc", "qdisc", "show", "dev", _TC_DEV])
    except docker_env.DockerEnvError as exc:
        log.warning("Failed to read tc qdisc for %s: %s", src, exc)
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


def _iptables_has_drop(src_ip: str, dst_ip: str) -> bool:
    spec = ["sudo", "iptables", "-C", _IPTABLES_CHAIN, "-s", src_ip, "-d", dst_ip, "-j", "DROP"]
    result = _run_host_command(spec)
    return result.returncode == 0


def check_blocked(src: str, dst: str) -> bool:
    src_ip = docker_env.get_container_ip(src)
    dst_ip = docker_env.get_container_ip(dst)
    if not src_ip or not dst_ip:
        return False
    forward = _iptables_has_drop(src_ip, dst_ip)
    backward = _iptables_has_drop(dst_ip, src_ip)
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
    return not (_iptables_has_drop(src_ip, dst_ip) or _iptables_has_drop(dst_ip, src_ip))


def check_node_down(name: str) -> bool:
    running = docker_env.is_container_running(name)
    log.debug("Node %s running=%s", name, running)
    return not running


def check_node_up(name: str) -> bool:
    running = docker_env.is_container_running(name)
    log.debug("Node %s running=%s", name, running)
    return running
