#!/usr/bin/env python3
# Copyright (c) Mysten Labs, Inc.
# Modifications Copyright (c) 2024 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

import argparse
from collections import defaultdict
import json
import os
import re
import subprocess
import sys
from typing import NamedTuple
import urllib.request

GH_TOKEN = os.environ.get("GH_TOKEN")

RE_NUM = re.compile("[0-9_]+")

RE_PR = re.compile(
    r"^.*\(#(\d+)\)$",
    re.MULTILINE,
)

RE_HEADING = re.compile(
    r"#+ Release notes(.*)",
    re.DOTALL | re.IGNORECASE,
)

RE_CHECK = re.compile(
    r"^\s*-\s*\[.\]",
    re.MULTILINE,
)

RE_NOTE = re.compile(
    r"^\s*-\s*\[( |x)?\]\s*([^:]+):",
    re.MULTILINE | re.IGNORECASE,
)

RE_ATTENTION = re.compile(
    r"#+\s*Attention(.*)",
    re.DOTALL | re.IGNORECASE,
)

RE_ATTENTION_NOTE = re.compile(
    r"^\s*-\s*\[( |x)?\]\s*(.+)$",
    re.MULTILINE | re.IGNORECASE,
)

# Only commits that affect changes in these directories will be
# considered when generating release notes.
INTERESTING_DIRECTORIES = [
    "crates",
    "consensus",
    "docker",
    "docs",
    "external-crates",
    "iota-execution",
    "kiosk",
    "nre",
    "sdk",
]

# Start release notes with these sections, if they contain relevant
# information (helps us keep a consistent order for impact areas we
# know about).
NOTE_ORDER = [
    "Protocol",
    "Nodes (Validators and Full nodes)",
    "Indexer",
    "JSON-RPC",
    "GraphQL",
    "CLI",
    "Rust SDK",
    "gRPC",
    "REST API",
    "Internal gRPC API",
]

ATTENTION_MESSAGES = {
    "Protocol Types Changed": "Users of iota-data-ingestion-core need to update their application.",
}

ATTENTION_ICON = "⚠️"


class Note(NamedTuple):
    checked: bool
    note: str


def parse_args():
    """Parse command line arguments."""

    parser = argparse.ArgumentParser(
        description=(
            "Extract release notes from git commits. Check help for the "
            "`generate` and `check` subcommands for more information."
        ),
    )

    sub_parser = parser.add_subparsers(dest="command")

    generate_p = sub_parser.add_parser(
        "generate",
        description="Generate release notes from git commits.",
    )

    generate_p.add_argument(
        "from",
        help="The commit to start from (exclusive)",
    )

    generate_p.add_argument(
        "to",
        nargs="?",
        default="HEAD",
        help="The commit to end at (inclusive), defaults to HEAD.",
    )

    check_p = sub_parser.add_parser(
        "check",
        description=(
            "Check if the release notes section of a given commit is complete, "
            "i.e. that every impacted component has a non-empty note."
        ),
    )

    check_p.add_argument(
        "commit",
        nargs="?",
        default="HEAD",
        help="The commit to check, defaults to HEAD.",
    )

    check_p = sub_parser.add_parser(
        "check-pr",
        description=(
            "Check if the release notes section of a given commit is complete, "
            "i.e. that every impacted component has a non-empty note."
        ),
    )

    check_p.add_argument(
        "pr-number",
        help="The number of the PR to check.",
    )

    return vars(parser.parse_args())


def git(*args):
    """Run a git command and return the output as a string."""
    return subprocess.check_output(["git"] + list(args)).decode().strip()


def extract_notes_from_commit(commit):
    # we'll need to go one level deeper to find the PR number
    url = f"https://api.github.com/repos/iotaledger/iota/commits/{commit}/pulls"
    headers = {
        "Accept": "application/vnd.github.v3+json",
    }
    if GH_TOKEN is not None:
        headers["Authorization"] = f"token {GH_TOKEN}"
    req = urllib.request.Request(url, headers=headers)
    with urllib.request.urlopen(req) as response:
        data = json.load(response)
        if len(data) == 0:
            return None, ""
        pr_number = data[0]["number"]
        pr_notes = data[0]["body"] if data[0]["body"] else ""
        return pr_number, pr_notes


def extract_notes_from_pr(pr_number):
    url = f"https://api.github.com/repos/iotaledger/iota/pulls/{pr_number}"
    headers = {
        "Accept": "application/vnd.github.v3+json",
    }
    if GH_TOKEN is not None:
        headers["Authorization"] = f"token {GH_TOKEN}"
    req = urllib.request.Request(url, headers=headers)
    with urllib.request.urlopen(req) as response:
        data = json.load(response)
        pr_notes = data["body"] if data["body"] else ""
        return pr_notes


def extract_attention(notes):
    """Extract checked attention items from a PR description."""
    if not notes:
        return []

    match = RE_ATTENTION.search(notes)
    if not match:
        return []

    section = match.group(1)
    next_heading = re.search(r"^\s*#+\s", section, re.MULTILINE)
    if next_heading:
        section = section[: next_heading.start()]

    items = []
    for m in RE_ATTENTION_NOTE.finditer(section):
        checked = m.group(1)
        label = m.group(2).strip()
        if checked and checked.lower() == "x":
            items.append(label)

    return items


def extract_notes(commit_or_pr, seen, is_pr):
    """Get release notes from a commit message or a PR description.

    Finds the 'Release notes' section in the message, and
    extracts the notes for each impacted area (area that has been
    ticked).

    Returns a tuple of the PR number and a dictionary of impacted
    areas mapped to their release note. Each release note indicates
    whether it has a note and whether it was checked (ticked).

    """
    if is_pr:
        pr = commit_or_pr
        notes = extract_notes_from_pr(pr)
    else:
        # Try to get the PR number from the commit message or fallback to the
        # one returned from the Github API
        match = RE_PR.match(git("show", "-s", "--format=%B", commit_or_pr))
        if match:
            pr = match.group(1)
            notes = extract_notes_from_pr(pr)
        else:
            pr, notes = extract_notes_from_commit(commit_or_pr)

    result = {}
    attention = extract_attention(notes)

    # Otherwise, find the release notes section from the squashed commit message
    match = RE_HEADING.search(notes)
    if not match:
        return pr, [], attention
    notes = match.group(1)

    if pr in seen:
        # a PR can be in multiple commits if it's from a rebase,
        # so we only want to process it once
        return pr, [], []

    start = 0
    while True:
        # Find the next possible release note
        match = RE_NOTE.search(notes, start)
        if not match:
            break

        checked = match.group(1)
        impacted = match.group(2)
        begin = match.end()

        # Find the end of the note, or the end of the commit
        match = RE_CHECK.search(notes, begin)
        end = match.start() if match else len(notes)

        result[impacted] = Note(
            checked=checked in "xX",
            note=notes[begin:end].strip(),
        )
        start = end

    return pr, result.items(), attention


def extract_protocol_version(commit):
    """Find the max protocol version at this commit.

    Assumes that it is being called from the root of the iota repository."""
    for line in git(
        "show", f"{commit}:crates/iota-protocol-config/src/lib.rs"
    ).splitlines():
        if "const MAX_PROTOCOL_VERSION" not in line:
            continue

        _, _, assign = line.partition("=")
        if not assign:
            continue

        match = RE_NUM.search(assign)
        if not match:
            continue

        return match[0]


def print_changelog(pr, log):
    if pr:
        print(f"https://github.com/iotaledger/iota/pull/{pr}: ", end="")
    print(log)


def do_check(commit_or_pr, is_pr):
    """Check if the release notes section of a given commit is complete.

    This means that every impacted component has a non-empty note,
    every note is attached to a checked checkbox, and every impact
    area is known.

    """

    _, notes, _ = extract_notes(commit_or_pr, set(), is_pr)

    issues = []
    any_checked = False
    for impacted, note in notes:
        any_checked |= note.checked

        if impacted not in NOTE_ORDER:
            issues.append(f" - Found unfamiliar impact area '{impacted}'.")

        if note.checked and not note.note:
            issues.append(f" - '{impacted}' is checked but has no release note.")

        if not note.checked and note.note:
            issues.append(
                f" - '{impacted}' has a release note but is not checked: {note.note}"
            )

    if not any_checked and len(notes) > 0:
        issues.append(f" - No checked items in release notes")

    if not issues:
        return

    print(f"Found issues with release notes in {commit_or_pr}:")
    for issue in issues:
        print(issue)
    sys.exit(1)


def do_generate(from_, to):
    """Generate release notes from git commits.

    This will extract the release notes from all commits between
    `from_` (exclusive) and `to` (inclusive), and print out a markdown
    snippet with a heading for each impact area that has a note,
    followed by a list of its relevant changelog.

    Only looks for commits affecting INTERESTING_DIRECTORIES.

    Additionally injects the current protocol version into the
    "Protocol" changelog.

    """
    results = defaultdict(list)
    attention_results = defaultdict(list)
    attention_seen = defaultdict(set)

    root = git("rev-parse", "--show-toplevel")
    os.chdir(root)

    protocol_version_from = extract_protocol_version(from_) or "XX"
    protocol_version_to = extract_protocol_version(to) or "XX"

    commits = git(
        "log",
        "--pretty=format:%H",
        f"{from_}..{to}",
        "--",
        *INTERESTING_DIRECTORIES,
    ).strip()

    if not commits:
        return

    seen_prs = set()
    for commit in commits.split("\n"):
        pr, notes, attention = extract_notes(commit, seen_prs, False)
        seen_prs.add(pr)
        for impacted, note in notes:
            if note.checked:
                results[impacted].append((pr, note.note))
        for label in attention:
            if pr in attention_seen[label]:
                continue
            attention_seen[label].add(pr)
            attention_results[label].append(pr)

    # Print the impact areas we know about first
    for impacted in NOTE_ORDER:
        notes = results.pop(impacted, None)
        if not notes and impacted != "Protocol":
            continue

        print(f"## {impacted}")

        if impacted == "Protocol":
            if protocol_version_from == protocol_version_to:
                print(f"\n#### This release does not introduce a new protocol version (current version: `{protocol_version_to}`)")
            else:
                print(f"\n#### This release introduces protocol version `{protocol_version_to}`")
        print()

        if notes:
            for pr, note in reversed(notes):
                print_changelog(pr, note)
                print()

    # Print any remaining impact areas
    for impacted, notes in results.items():
        print(f"## {impacted}\n")
        for pr, note in reversed(notes):
            print_changelog(pr, note)
            print()

    if attention_results:
        print(f"## {ATTENTION_ICON} Attention {ATTENTION_ICON}\n")
        for label in sorted(attention_results):
            message = ATTENTION_MESSAGES.get(label, label)
            for pr in reversed(attention_results[label]):
                print(message)
                print()


args = parse_args()
if args["command"] == "generate":
    do_generate(args["from"], args["to"])
elif args["command"] == "check":
    do_check(args["commit"], False)
elif args["command"] == "check-pr":
    do_check(args["pr-number"], True)
