#!/usr/bin/env python3
# Copyright (c) 2026 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

"""Fails when `MIN_SDK_VERSION` has to be raised.

The node tells gRPC clients the lowest `iota-sdk-grpc-client` version it
supports, `MIN_SDK_VERSION` in crates/iota-grpc-server/src/constants.rs. A
client at that version must be able to decode every BCS value the node sends,
so the value has to be raised when the `iota-sdk-types` the node is built
against changes its BCS schema, the `bcs-schema.abnf` file of that crate.

Two copies of that file are compared:

- the one of the `iota-sdk-types` release in Cargo.lock;
- the one of the oldest `iota-sdk-types` release that the published client at
  `MIN_SDK_VERSION` accepts, taken from its dependency requirement.

The check fails when they differ, and also when `MIN_SDK_VERSION` names a
client version that is not on crates.io.

Only a crates.io release of `iota-sdk-types` is checked. While Cargo.lock pins
it to a commit of iota-rust-sdk or to a local path, there may be no client
release to name yet, so the check passes; it applies again once a release is
pinned.

Usage: ./scripts/check_min_sdk_version.py
"""

import difflib
import io
import re
import sys
import tarfile
import time
import tomllib
import urllib.error
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LOCKFILE = ROOT / "Cargo.lock"
CONSTANTS = ROOT / "crates/iota-grpc-server/src/constants.rs"

CLIENT_CRATE = "iota-sdk-grpc-client"
TYPES_CRATE = "iota-sdk-types"
SCHEMA = "bcs-schema.abnf"

CRATES_IO = "https://static.crates.io/crates"
# How Cargo.lock names crates.io as the source of a package.
CRATES_IO_SOURCE = "registry+https://github.com/rust-lang/crates.io-index"
USER_AGENT = "iota/scripts/check_min_sdk_version.py"

MIN_SDK_VERSION = re.compile(r'pub const MIN_SDK_VERSION: &str = "([^"]+)";')
# A requirement with a lower bound, such as `1.0.0-rc.2`, `^1.2` or `>=1`.
REQUIREMENT = re.compile(
    r"\s*(?:\^|=|~|>=)?\s*(\d+)(?:\.(\d+))?(?:\.(\d+))?(-[0-9A-Za-z.-]+)?\s*"
)
MAX_DIFF_LINES = 80


def fail(message: str):
    sys.exit(f"error: {message}")


def fetch(url: str) -> bytes | None:
    """The content at `url`, or `None` when nothing is published there."""
    request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    for attempt in range(3):
        try:
            with urllib.request.urlopen(request, timeout=30) as response:
                return response.read()
        except urllib.error.HTTPError as error:
            # static.crates.io answers 403 for a file that does not exist.
            if error.code in (403, 404):
                return None
            last_error = error
        except urllib.error.URLError as error:
            last_error = error
        time.sleep(2 * (attempt + 1))
    fail(f"could not download {url}: {last_error}")


def crate_file(crate: str, version: str, path: str) -> str | None:
    """A file of a crates.io release, or `None` when the release does not
    exist."""
    archive = fetch(f"{CRATES_IO}/{crate}/{crate}-{version}.crate")
    if archive is None:
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


def oldest_types_version(client_version: str) -> str | None:
    """The oldest `iota-sdk-types` version the published client accepts, or
    `None` when that client version is not on crates.io."""
    manifest = crate_file(CLIENT_CRATE, client_version, "Cargo.toml")
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
                f"{CLIENT_CRATE} {client_version} requires {TYPES_CRATE} "
                f"`{requirement}`, which has no oldest version this check can read"
            )
        major, minor, patch, pre_release = match.groups()
        return f"{major}.{minor or 0}.{patch or 0}{pre_release or ''}"
    fail(f"{CLIENT_CRATE} {client_version} does not depend on {TYPES_CRATE}")


def locked_client_version(lockfile: Path) -> str | None:
    """The crates.io version of the client in Cargo.lock, if it has one."""
    for package in tomllib.loads(lockfile.read_text())["package"]:
        if package["name"] == CLIENT_CRATE and package.get("source") == CRATES_IO_SOURCE:
            return package["version"]
    return None


def advice(lockfile: Path, node_schema: str) -> str:
    """What to set `MIN_SDK_VERSION` to so that `node_schema` is covered."""
    no_release = (
        f"No {CLIENT_CRATE} release found that covers it. Release the client "
        "from iota-rust-sdk first, then set MIN_SDK_VERSION to that release."
    )
    locked = locked_client_version(lockfile)
    if locked is None:
        return no_release
    types_version = oldest_types_version(locked)
    if types_version is None or released_schema(types_version) != node_schema:
        return no_release
    return (
        f"{CLIENT_CRATE} {locked} from Cargo.lock covers it. Set "
        f"MIN_SDK_VERSION to {locked}, or to an older release that this "
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


def main():
    match = MIN_SDK_VERSION.search(CONSTANTS.read_text())
    if match is None:
        fail(f"MIN_SDK_VERSION not found in {CONSTANTS.relative_to(ROOT)}")
    minimum = match.group(1)

    node = node_schemas(LOCKFILE)
    if not node:
        return

    types_version = oldest_types_version(minimum)
    if types_version is None:
        fail(
            f"MIN_SDK_VERSION is {minimum}, but {CLIENT_CRATE} {minimum} is "
            "not on crates.io"
        )
    minimum_name = f"{TYPES_CRATE} {types_version}"
    minimum_schema = released_schema(types_version)

    for node_name, node_schema in node:
        if node_schema == minimum_schema:
            print(
                f"MIN_SDK_VERSION {minimum}: the BCS schema of its "
                f"{minimum_name} matches the node's {node_name} — OK"
            )
            continue
        print(schema_diff(minimum_schema, node_schema, minimum_name, node_name))
        fail(
            f"MIN_SDK_VERSION has to be raised in {CONSTANTS.relative_to(ROOT)}.\n"
            f"It is {minimum}, and {CLIENT_CRATE} {minimum} accepts "
            f"{minimum_name}, whose BCS schema differs from the node's "
            f"{node_name} (diff above). Such a client cannot decode everything "
            "this node sends.\n" + advice(LOCKFILE, node_schema)
        )


if __name__ == "__main__":
    main()
