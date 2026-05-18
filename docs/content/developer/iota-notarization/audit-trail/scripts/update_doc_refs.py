#!/usr/bin/env python3
"""
Update #L<start>-L<end> line references in how-to MDX files to match the
current state of the notarization example files.

The script diffs an OLD git ref (the baseline the docs were last updated against)
against a NEW git ref (the current target state) inside the notarization
repository, then remaps every line-range anchor in the how-to MDX files
accordingly.

Usage
-----
    python3 update_doc_refs.py \\
        --notarization-repo /path/to/notarization \\
        --old-ref 34190c6 \\
        --new-ref origin/feat/audit-trails-dev

Configuration can also be supplied via environment variables (CLI flags take
precedence):

    NOTARIZATION_REPO   Path to the local notarization clone
    DOCS_DIR            Path to the how-tos folder (default: auto-detected)
    OLD_REF             Old git ref (commit/tag/branch) — the docs' current baseline
    NEW_REF             New git ref (commit/tag/branch) — the target state

Run with --dry-run to preview changes without writing any files.
"""

import argparse
import difflib
import os
import re
import subprocess
import sys
from collections import defaultdict
from pathlib import Path

# Branch names can contain slashes (e.g. feat/audit-trails-dev), so we cannot
# use [^/]+ to capture the branch. Instead we anchor on the known root folders
# of the example files (examples/ or bindings/) to split branch from file path.
URL_PATTERN = re.compile(
    r"(https://github\.com/iotaledger/notarization/tree/.+?"
    r"/((?:examples|bindings)/[^#\s]+)#L(\d+)-L(\d+))"
)


# ---------------------------------------------------------------------------
# Git helpers
# ---------------------------------------------------------------------------

def git_show_file(repo: Path, ref: str, filepath: str) -> list[str] | None:
    """Return the lines of *filepath* at *ref*, or None on failure."""
    result = subprocess.run(
        ["git", "show", f"{ref}:{filepath}"],
        capture_output=True, text=True, cwd=repo,
    )
    if result.returncode != 0:
        return None
    return result.stdout.splitlines()


# ---------------------------------------------------------------------------
# Line-number mapping
# ---------------------------------------------------------------------------

def compute_line_mapping(old_lines: list[str], new_lines: list[str]) -> dict[int, int]:
    """
    Build a mapping from old 1-based line numbers to new 1-based line numbers
    using difflib SequenceMatcher.
    """
    matcher = difflib.SequenceMatcher(None, old_lines, new_lines, autojunk=False)
    mapping: dict[int, int] = {}

    for tag, i1, i2, j1, j2 in matcher.get_opcodes():
        if tag == "equal":
            for offset in range(i2 - i1):
                mapping[i1 + offset + 1] = j1 + offset + 1
        elif tag == "replace":
            old_count, new_count = i2 - i1, j2 - j1
            min_count = min(old_count, new_count)
            for offset in range(min_count):
                mapping[i1 + offset + 1] = j1 + offset + 1
            last_new = j1 + new_count if new_count > 0 else j1 + 1
            for offset in range(min_count, old_count):
                mapping[i1 + offset + 1] = last_new
        elif tag == "delete":
            prev_new = mapping.get(i1, j1) if i1 > 0 else j1
            for offset in range(i2 - i1):
                mapping[i1 + offset + 1] = prev_new
        # "insert": new lines with no old counterpart — nothing to map from

    return mapping


def map_range(
    mapping: dict[int, int],
    old_start: int,
    old_end: int,
    new_lines_count: int,
) -> tuple[int, int]:
    """Translate an old line range to a new line range."""
    new_start = mapping.get(old_start)
    new_end = mapping.get(old_end)

    if new_start is None or new_end is None:
        for offset in range(1, 10):
            if new_start is None and (old_start - offset) in mapping:
                new_start = mapping[old_start - offset]
            if new_start is None and (old_start + offset) in mapping:
                new_start = mapping[old_start + offset]
            if new_end is None and (old_end + offset) in mapping:
                new_end = mapping[old_end + offset]
            if new_end is None and (old_end - offset) in mapping:
                new_end = mapping[old_end - offset]
            if new_start is not None and new_end is not None:
                break

    new_start = new_start or old_start
    new_end = new_end or old_end

    new_start = max(1, min(new_start, new_lines_count))
    new_end = max(1, min(new_end, new_lines_count))
    return new_start, new_end


# ---------------------------------------------------------------------------
# Doc parsing
# ---------------------------------------------------------------------------

def get_all_references(docs_dir: Path):
    """Yield (mdx_path, line_num, full_url, repo_path, old_start, old_end)."""
    for mdx_file in sorted(docs_dir.rglob("*.mdx")):
        if "CLAUDE.md" in mdx_file.name:
            continue
        with open(mdx_file) as f:
            for lineno, line in enumerate(f, 1):
                for match in URL_PATTERN.finditer(line):
                    yield (
                        mdx_file,
                        lineno,
                        match.group(1),   # full URL
                        match.group(2),   # repo-relative path
                        int(match.group(3)),  # old start
                        int(match.group(4)),  # old end
                    )


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main() -> None:
    parser = argparse.ArgumentParser(
        description="Remap #L<start>-L<end> anchors in how-to MDX files after "
                    "notarization example code changes.",
    )
    parser.add_argument(
        "--notarization-repo",
        default=os.environ.get("NOTARIZATION_REPO"),
        metavar="PATH",
        help="Path to the local notarization git clone "
             "(env: NOTARIZATION_REPO)",
    )
    parser.add_argument(
        "--docs-dir",
        default=os.environ.get(
            "DOCS_DIR",
            str(Path(__file__).resolve().parent.parent / "how-tos"),
        ),
        metavar="PATH",
        help="Path to the how-tos directory "
             "(env: DOCS_DIR, default: sibling how-tos/ folder)",
    )
    parser.add_argument(
        "--old-ref",
        default=os.environ.get("OLD_REF"),
        metavar="REF",
        help="Git ref the docs currently reference (env: OLD_REF)",
    )
    parser.add_argument(
        "--new-ref",
        default=os.environ.get("NEW_REF"),
        metavar="REF",
        help="Git ref to update the docs to (env: NEW_REF)",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Print changes without writing any files",
    )
    args = parser.parse_args()

    missing = [name for name, val in [
        ("--notarization-repo / NOTARIZATION_REPO", args.notarization_repo),
        ("--old-ref / OLD_REF", args.old_ref),
        ("--new-ref / NEW_REF", args.new_ref),
    ] if not val]
    if missing:
        parser.error("required argument(s) not provided:\n  " + "\n  ".join(missing))

    repo = Path(args.notarization_repo)
    docs_dir = Path(args.docs_dir)

    if not repo.is_dir():
        sys.exit(f"notarization repo not found: {repo}")
    if not docs_dir.is_dir():
        sys.exit(f"docs directory not found: {docs_dir}")

    print(f"Notarization repo : {repo}")
    print(f"Docs directory    : {docs_dir}")
    print(f"Old ref           : {args.old_ref}")
    print(f"New ref           : {args.new_ref}")
    print(f"Dry run           : {args.dry_run}")
    print()

    refs = list(get_all_references(docs_dir))
    source_files = {r[3] for r in refs}
    print(f"Found {len(refs)} references across {len(source_files)} source files\n")

    # Build line mappings for every referenced source file
    file_mappings: dict[str, tuple[dict[int, int], int]] = {}
    file_new_content: dict[str, list[str]] = {}

    for filepath in sorted(source_files):
        old_lines = git_show_file(repo, args.old_ref, filepath)
        new_lines = git_show_file(repo, args.new_ref, filepath)

        if old_lines is None:
            print(f"WARNING: cannot retrieve {filepath} at {args.old_ref!r} — skipping")
            continue
        if new_lines is None:
            print(f"WARNING: cannot retrieve {filepath} at {args.new_ref!r} — skipping")
            continue

        file_mappings[filepath] = (compute_line_mapping(old_lines, new_lines), len(new_lines))
        file_new_content[filepath] = new_lines
        print(f"  {filepath}: {len(old_lines)} → {len(new_lines)} lines")

    print()

    # Compute replacements
    changes = []
    skipped = 0
    unchanged = 0
    for mdx_file, lineno, full_url, repo_path, old_start, old_end in refs:
        if repo_path not in file_mappings:
            skipped += 1
            continue
        mapping, new_count = file_mappings[repo_path]
        new_start, new_end = map_range(mapping, old_start, old_end, new_count)

        if new_start != old_start or new_end != old_end:
            new_url = full_url.replace(
                f"#L{old_start}-L{old_end}", f"#L{new_start}-L{new_end}"
            )
            changes.append({
                "mdx_file": mdx_file,
                "repo_path": repo_path,
                "old_start": old_start, "old_end": old_end,
                "new_start": new_start, "new_end": new_end,
                "old_url": full_url, "new_url": new_url,
            })
            print(
                f"CHANGE  {mdx_file.name}:{lineno}  "
                f"{Path(repo_path).name}#L{old_start}-L{old_end} "
                f"→ #L{new_start}-L{new_end}"
            )
        else:
            unchanged += 1
            print(
                f"  OK    {mdx_file.name}:{lineno}  "
                f"{Path(repo_path).name}#L{old_start}-L{old_end}"
            )

    print(f"\nChanges needed : {len(changes)}")
    print(f"Already current: {unchanged}")
    if skipped:
        print(f"Skipped (source file unavailable at one ref): {skipped}")

    if not changes:
        print("Nothing to update.")
        return

    if args.dry_run:
        print("\n[dry-run] No files written.")
        return

    # Apply
    by_file: dict[Path, dict[str, str]] = defaultdict(dict)
    for ch in changes:
        by_file[ch["mdx_file"]][ch["old_url"]] = ch["new_url"]

    print("\nApplying changes...")
    for mdx_file, url_map in by_file.items():
        text = mdx_file.read_text()
        for old_url, new_url in url_map.items():
            text = text.replace(old_url, new_url)
        mdx_file.write_text(text)
        print(f"  Updated {mdx_file.name} ({len(url_map)} replacement(s))")

    print("\nDone.")


if __name__ == "__main__":
    main()
