"""Low-level primitives that introduce network disruptions."""

from __future__ import annotations

import logging
import subprocess

from . import docker_env

log = logging.getLogger(__name__)

_TC_DEV = "eth0"
_IPTABLES_CHAIN = "DOCKER-USER"
_RULE_COMMENT_PREFIX = "net-fuzz"


class DisruptionError(RuntimeError):
    """Raised when applying or reverting a disruption fails."""


def _run_host_command(args: list[str], *, check: bool = True) -> subprocess.CompletedProcess[str]:
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
    pid = _require_pid(src)
    _nsenter(pid, ["tc", "qdisc", "del", "dev", _TC_DEV, "root"], check=False)

    cmd = [
        "tc",
        "qdisc",
        "replace",
        "dev",
        _TC_DEV,
        "root",
        "netem",
        "delay",
        f"{delay_ms}ms",
    ]
    if jitter_ms:
        cmd.append(f"{jitter_ms}ms")
    if loss_pct > 0:
        cmd.extend(["loss", f"{loss_pct:.2f}%"])
    _nsenter(pid, cmd)
    log.info(
        "Applied latency: src=%s dst=%s delay=%sms jitter=%sms loss=%.2f%%",
        src,
        dst,
        delay_ms,
        jitter_ms,
        loss_pct,
    )


def _ensure_docker_user_chain() -> None:
    res = _run_host_command(["iptables", "-nL", _IPTABLES_CHAIN], check=False)
    if res.returncode != 0:
        _run_host_command(["iptables", "-N", _IPTABLES_CHAIN])
    res = _run_host_command(["iptables", "-C", "FORWARD", "-j", _IPTABLES_CHAIN], check=False)
    if res.returncode != 0:
        _run_host_command(["iptables", "-I", "FORWARD", "-j", _IPTABLES_CHAIN])


def _rule_comment(label: str) -> str:
    return f"{_RULE_COMMENT_PREFIX}:{label}"


def _add_drop_rule(src_ip: str, dst_ip: str, label: str) -> None:
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
    src_ip = docker_env.get_container_ip(src)
    dst_ip = docker_env.get_container_ip(dst)
    if not src_ip or not dst_ip:
        raise DisruptionError(f"Unable to resolve container IPs for {src!r} or {dst!r}")

    _ensure_docker_user_chain()
    _add_drop_rule(src_ip, dst_ip, f"{src}->{dst}")
    _add_drop_rule(dst_ip, src_ip, f"{dst}->{src}")
    log.info("Blocked connection between %s (%s) and %s (%s)", src, src_ip, dst, dst_ip)


def _delete_drop_rule(src_ip: str, dst_ip: str, label: str) -> None:
    spec = ["-s", src_ip, "-d", dst_ip, "-j", "DROP", "-m", "comment", "--comment", _rule_comment(label)]
    _run_host_command(["iptables", "-D", _IPTABLES_CHAIN, *spec], check=False)


def unblock_connection(src: str, dst: str) -> None:
    src_ip = docker_env.get_container_ip(src)
    dst_ip = docker_env.get_container_ip(dst)
    if not src_ip or not dst_ip:
        return
    _delete_drop_rule(src_ip, dst_ip, f"{src}->{dst}")
    _delete_drop_rule(dst_ip, src_ip, f"{dst}->{src}")
    log.info("Unblocked connection between %s and %s", src, dst)


def restart_node(name: str) -> None:
    docker_env.restart_container(name)
    log.info("Restarted node %s", name)


def kill_node(name: str) -> None:
    docker_env.stop_container(name, timeout=5)
    log.info("Stopped node %s", name)
