#!/usr/bin/env python3
# Copyright (c) 2026 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

"""Fails when a minimum SDK client version of the node has to be raised.

The gRPC and GraphQL services each tell their clients the lowest version of
the SDK client crate they support:

- `MIN_IOTA_SDK_GRPC_CLIENT_VERSION` in
  crates/iota-grpc-server/src/constants.rs, for `iota-sdk-grpc-client`;
- `MIN_IOTA_SDK_GRAPHQL_CLIENT_VERSION` in
  crates/iota-graphql-rpc/src/server/version.rs, for `iota-sdk-graphql-client`.

A client at that version must be able to decode every BCS value the node
sends, so the value has to be raised when the `iota-sdk-types` the node is
built against changes its BCS schema, the `bcs-schema.abnf` file of that
crate.

For each constant, two copies of that file are compared:

- the one of the `iota-sdk-types` release in Cargo.lock;
- the one of the oldest `iota-sdk-types` release that the published client at
  the version of the constant accepts, taken from its dependency requirement.

The check fails when they differ, and also when a constant names a client
version that is not on crates.io.

Only a crates.io release of `iota-sdk-types` is checked. While Cargo.lock pins
it to a commit of iota-rust-sdk or to a local path, there may be no client
release to name yet, so the check passes; it applies again once a release is
pinned.

Usage: ./scripts/check_min_iota_sdk_client_version.py
"""

import difflib
import functools
import io
import json
import re
import sys
import tarfile
import time
import tomllib
import urllib.error
import urllib.request
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LOCKFILE = ROOT / "Cargo.lock"


@dataclass(frozen=True)
class Minimum:
    """A constant holding the lowest version of an SDK client crate that a
    service of the node supports."""

    constant: str
    # Relative to the repository root.
    file: str
    client_crate: str


MINIMUMS = [
    Minimum(
        "MIN_IOTA_SDK_GRPC_CLIENT_VERSION",
        "crates/iota-grpc-server/src/constants.rs",
        "iota-sdk-grpc-client",
    ),
    Minimum(
        "MIN_IOTA_SDK_GRAPHQL_CLIENT_VERSION",
        "crates/iota-graphql-rpc/src/server/version.rs",
        "iota-sdk-graphql-client",
    ),
]

TYPES_CRATE = "iota-sdk-types"
SCHEMA = "bcs-schema.abnf"

CRATES_IO = "https://static.crates.io/crates"
CRATES_IO_INDEX = "https://index.crates.io"
# How Cargo.lock names crates.io as the source of a package.
CRATES_IO_SOURCE = "registry+https://github.com/rust-lang/crates.io-index"
USER_AGENT = "iota/scripts/check_min_iota_sdk_client_version.py"

# A requirement with a lower bound, such as `1.0.0-rc.2`, `^1.2` or `>=1`.
REQUIREMENT = re.compile(
    r"\s*(?:\^|=|~|>=)?\s*(\d+)(?:\.(\d+))?(?:\.(\d+))?(-[0-9A-Za-z.-]+)?\s*"
)
MAX_DIFF_LINES = 80


def fail(message: str):
    sys.exit(f"error: {message}")


def fetch(url: str, missing: tuple[int, ...] = (404,)) -> bytes | None:
    """The content at `url`, or `None` when the server answers with one of the
    `missing` statuses."""
    request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    for attempt in range(3):
        try:
            with urllib.request.urlopen(request, timeout=30) as response:
                return response.read()
        except urllib.error.HTTPError as error:
            if error.code in missing:
                return None
            last_error = error
        except urllib.error.URLError as error:
            last_error = error
        time.sleep(2 * (attempt + 1))
    fail(f"could not download {url}: {last_error}")


@functools.cache
def published_versions(crate: str) -> set[str]:
    """The versions of `crate` listed in the crates.io index."""
    name = crate.lower()
    if len(name) <= 2:
        directory = str(len(name))
    elif len(name) == 3:
        directory = f"3/{name[0]}"
    else:
        directory = f"{name[:2]}/{name[2:4]}"
    index = fetch(f"{CRATES_IO_INDEX}/{directory}/{name}")
    if index is None:
        return set()
    return {json.loads(line)["vers"] for line in index.decode().splitlines() if line}


@functools.cache
def crate_file(crate: str, version: str, path: str) -> str | None:
    """A file of a crates.io release, or `None` when the release does not
    exist."""
    url = f"{CRATES_IO}/{crate}/{crate}-{version}.crate"
    # static.crates.io has answered 403 as well as 404 for a file that does
    # not exist, so the index decides whether a refused download is a missing
    # release.
    archive = fetch(url, missing=(403, 404))
    if archive is None:
        if version in published_versions(crate):
            fail(
                f"{crate} {version} is on crates.io, but {url} could not be "
                "downloaded"
            )
        return None
    with tarfile.open(fileobj=io.BytesIO(archive), mode="r:gz") as tar:
        try:
            member = tar.extractfile(f"{crate}-{version}/{path}")
        except KeyError:
            member = None
        if member is None:
            fail(f"{crate} {version} on crates.io does not contain {path}")
        return member.read().decode()


def released_schema(version: str) -> str:
    schema = crate_file(TYPES_CRATE, version, SCHEMA)
    if schema is None:
        fail(f"{TYPES_CRATE} {version} is not on crates.io")
    return schema


def node_schemas(lockfile: Path) -> list[tuple[str, str]]:
    """A description and the BCS schema of every crates.io release of
    `iota-sdk-types` the node resolves to. A package from any other source is
    reported and left out."""
    packages = [
        package
        for package in tomllib.loads(lockfile.read_text())["package"]
        if package["name"] == TYPES_CRATE
    ]
    if not packages:
        fail(f"{TYPES_CRATE} is not in {lockfile.name}")
    schemas = []
    for package in packages:
        version, source = package["version"], package.get("source")
        if source == CRATES_IO_SOURCE:
            schemas.append((f"{TYPES_CRATE} {version}", released_schema(version)))
        else:
            print(
                f"{TYPES_CRATE} {version} comes from {source or 'a local path'}, "
                "not from crates.io — not checked"
            )
    return schemas


def oldest_types_version(client_crate: str, client_version: str) -> str | None:
    """The oldest `iota-sdk-types` version the published client accepts, or
    `None` when that client version is not on crates.io."""
    manifest = crate_file(client_crate, client_version, "Cargo.toml")
    if manifest is None:
        return None
    dependencies = tomllib.loads(manifest).get("dependencies", {})
    for key, dependency in dependencies.items():
        if isinstance(dependency, str):
            dependency = {"version": dependency}
        if dependency.get("package", key) != TYPES_CRATE:
            continue
        requirement = dependency.get("version", "")
        match = REQUIREMENT.fullmatch(requirement)
        if match is None:
            fail(
                f"{client_crate} {client_version} requires {TYPES_CRATE} "
                f"`{requirement}`, which has no oldest version this check can read"
            )
        major, minor, patch, pre_release = match.groups()
        return f"{major}.{minor or 0}.{patch or 0}{pre_release or ''}"
    fail(f"{client_crate} {client_version} does not depend on {TYPES_CRATE}")


def locked_client_version(lockfile: Path, client_crate: str) -> str | None:
    """The crates.io version of the client in Cargo.lock, if it has one."""
    for package in tomllib.loads(lockfile.read_text())["package"]:
        if package["name"] == client_crate and package.get("source") == CRATES_IO_SOURCE:
            return package["version"]
    return None


def advice(lockfile: Path, minimum: Minimum, node_schema: str) -> str:
    """What to set the constant to so that `node_schema` is covered."""
    client_crate = minimum.client_crate
    no_release = (
        f"No {client_crate} release found that covers it. Release the client "
        f"from iota-rust-sdk first, then set {minimum.constant} to that release."
    )
    locked = locked_client_version(lockfile, client_crate)
    if locked is None:
        return no_release
    types_version = oldest_types_version(client_crate, locked)
    if types_version is None or released_schema(types_version) != node_schema:
        return no_release
    return (
        f"{client_crate} {locked} from Cargo.lock covers it. Set "
        f"{minimum.constant} to {locked}, or to an older release that this "
        "check also accepts."
    )


def schema_diff(old: str, new: str, old_name: str, new_name: str) -> str:
    lines = list(
        difflib.unified_diff(
            old.splitlines(), new.splitlines(), old_name, new_name, lineterm=""
        )
    )
    if len(lines) > MAX_DIFF_LINES:
        hidden = len(lines) - MAX_DIFF_LINES
        lines = lines[:MAX_DIFF_LINES] + [f"... {hidden} more lines"]
    return "\n".join(lines)


def check(minimum: Minimum, node: list[tuple[str, str]]) -> list[str]:
    """The reasons the constant of `minimum` has to change, given the BCS
    schemas of the node."""
    constant, client_crate = minimum.constant, minimum.client_crate
    pattern = rf'const {re.escape(constant)}: &str = "([^"]+)";'
    match = re.search(pattern, (ROOT / minimum.file).read_text())
    if match is None:
        fail(f"{constant} not found in {minimum.file}")
    version = match.group(1)

    types_version = oldest_types_version(client_crate, version)
    if types_version is None:
        return [
            f"{constant} in {minimum.file} is {version}, but {client_crate} "
            f"{version} is not on crates.io"
        ]
    types_name = f"{TYPES_CRATE} {types_version}"
    types_schema = released_schema(types_version)

    errors = []
    for node_name, node_schema in node:
        if node_schema == types_schema:
            print(
                f"{constant} {version}: the BCS schema of its {types_name} "
                f"matches the node's {node_name} — OK"
            )
            continue
        errors.append(
            f"{constant} has to be raised in {minimum.file}.\n"
            f"It is {version}, and {client_crate} {version} accepts "
            f"{types_name}, whose BCS schema differs from the node's "
            f"{node_name}:\n"
            f"{schema_diff(types_schema, node_schema, types_name, node_name)}\n"
            "Such a client cannot decode everything this node sends.\n"
            + advice(LOCKFILE, minimum, node_schema)
        )
    return errors


def main():
    node = node_schemas(LOCKFILE)
    if not node:
        return
    errors = [error for minimum in MINIMUMS for error in check(minimum, node)]
    if errors:
        fail("\n\nerror: ".join(errors))


if __name__ == "__main__":
    main()
