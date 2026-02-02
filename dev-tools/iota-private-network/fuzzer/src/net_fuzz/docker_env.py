"""Helpers that encapsulate all Docker interactions used by the fuzz tests."""

from __future__ import annotations

import subprocess
import logging
from collections.abc import Iterable
from dataclasses import dataclass

import docker
from docker import errors as docker_errors
from docker.models.containers import Container

log = logging.getLogger(__name__)


class DockerEnvError(RuntimeError):
    """Raised when communicating with Docker fails."""


@dataclass(slots=True)
class ContainerInfo:
    """Simple container description returned by enumeration helpers."""

    name: str
    status: str
    container_id: str


_client: docker.DockerClient | None = None


def _get_client() -> docker.DockerClient:
    """Return a cached Docker client so repeated calls stay lightweight."""

    global _client
    if _client is None:
        try:
            _client = docker.from_env()
        except docker_errors.DockerException as exc:  # pragma: no cover
            raise DockerEnvError("Failed to connect to the Docker daemon") from exc
    return _client


def _get_container(name: str) -> Container:
    try:
        return _get_client().containers.get(name)
    except docker_errors.DockerException as exc:  # pragma: no cover
        raise DockerEnvError(f"Container {name!r} not found") from exc


def _list_containers_by_prefix(prefix: str) -> list[ContainerInfo]:
    try:
        containers = _get_client().containers.list(all=True)
    except docker_errors.DockerException as exc:  # pragma: no cover
        raise DockerEnvError("Unable to list containers") from exc

    matches: list[ContainerInfo] = []
    for container in containers:
        name = getattr(container, "name", "") or ""
        if name.startswith(prefix):
            matches.append(ContainerInfo(name=name, status=container.status, container_id=container.id))
    matches.sort(key=lambda info: info.name)
    return matches


def list_validator_containers(prefix: str = "validator-") -> list[ContainerInfo]:
    """Return validators with names matching the prefix."""
    return _list_containers_by_prefix(prefix)


def list_fullnode_containers(prefix: str = "fullnode-") -> list[ContainerInfo]:
    """Return fullnodes with names matching the prefix."""
    return _list_containers_by_prefix(prefix)


def list_faucet_containers(prefix: str = "faucet-") -> list[ContainerInfo]:
    """Return faucets with names matching the prefix."""
    return _list_containers_by_prefix(prefix)


def get_container_ip(name: str, network: str | None = None) -> str | None:
    """Return the container IP address, optionally scoped to a Docker network."""
    container = _get_container(name)
    networks = container.attrs.get("NetworkSettings", {}).get("Networks", {})
    if network:
        entry = networks.get(network)
        if entry:
            return entry.get("IPAddress") or entry.get("IPAMConfig", {}).get("IPv4Address")
        return None
    for entry in networks.values():
        ip = entry.get("IPAddress") or entry.get("IPAMConfig", {}).get("IPv4Address")
        if ip:
            return ip
    return None


def get_container_pid(name: str) -> int | None:
    container = _get_container(name)
    try:
        container.reload()
    except docker_errors.DockerException:  # pragma: no cover - best effort
        pass
    pid = container.attrs.get("State", {}).get("Pid")
    if pid:
        try:
            return int(pid)
        except (TypeError, ValueError):
            pass
    # Fallback: use docker inspect to query PID
    result = subprocess.run(
        ["docker", "inspect", "-f", "{{.State.Pid}}", name],
        capture_output=True,
        text=True,
    )
    if result.returncode == 0:
        pid_str = result.stdout.strip()
        if pid_str.isdigit():
            val = int(pid_str)
            return val if val > 0 else None
    return None


def _normalize_cmd(cmd: Iterable[str] | str) -> Iterable[str] | str:
    if isinstance(cmd, str):
        return cmd
    return list(cmd)


def run_in_container(name: str, cmd: Iterable[str] | str, *, check: bool = True) -> str:
    try:
        container = _get_client().containers.get(name)
    except docker_errors.DockerException as exc:  # pragma: no cover
        raise DockerEnvError(f"Container {name!r} not found") from exc

    try:
        exec_result = container.exec_run(_normalize_cmd(cmd), stdout=True, stderr=True)
    except docker_errors.DockerException as exc:  # pragma: no cover
        raise DockerEnvError(f"Failed to execute {cmd!r} inside {name}") from exc

    output = exec_result.output.decode("utf-8", errors="replace")
    if check and exec_result.exit_code != 0:
        raise DockerEnvError(
            f"Command {cmd!r} inside {name!r} failed with exit code {exec_result.exit_code}: {output.strip()}"
        )
    return output


def get_container_logs(name: str, *, tail: int | None = None) -> str:
    """Return combined stdout/stderr logs for a container."""
    container = _get_container(name)
    kwargs = {"stdout": True, "stderr": True}
    if tail is not None:
        kwargs["tail"] = tail
    try:
        output = container.logs(**kwargs)
    except docker_errors.DockerException as exc:  # pragma: no cover
        raise DockerEnvError(f"Failed to read logs from {name!r}") from exc
    return output.decode("utf-8", errors="replace")


def restart_container(name: str, *, timeout: int = 10) -> None:
    container = _get_container(name)
    try:
        container.restart(timeout=timeout)
    except docker_errors.DockerException as exc:  # pragma: no cover
        raise DockerEnvError(f"Failed to restart container {name!r}") from exc


def stop_container(name: str, *, timeout: int = 10) -> None:
    container = _get_container(name)
    try:
        container.stop(timeout=timeout)
    except docker_errors.APIError as exc:  # pragma: no cover
        raise DockerEnvError(f"Failed to stop container {name!r}") from exc


def start_container(name: str) -> None:
    container = _get_container(name)
    try:
        container.start()
    except docker_errors.APIError as exc:  # pragma: no cover
        raise DockerEnvError(f"Failed to start container {name!r}") from exc


def get_container_status(name: str) -> str | None:
    try:
        container = _get_container(name)
    except DockerEnvError:
        return None
    return container.status


def is_container_running(name: str) -> bool:
    return get_container_status(name) == "running"
