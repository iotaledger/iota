#!/bin/bash
# Copyright (c) 2026 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

# Pins the iota-rust-sdk crates to either a crates.io release or a git commit
# of https://github.com/iotaledger/iota-rust-sdk in every Cargo.toml that
# depends on them, lists the SDK repository in deny.toml only while a commit
# is pinned, and refreshes the affected Cargo.lock files without compiling.
#
# The argument is detected: a semver version (for example 1.0.0-rc.2) pins the
# crates.io release, a hex commit hash (7 to 40 characters) pins that commit.
# Every other attribute of a dependency line (package, default-features,
# features, optional, ...) is kept as is.
#
# Usage: ./scripts/update_rust_sdk.sh <version|commit>
# Examples:
#   ./scripts/update_rust_sdk.sh 1.0.0-rc.2
#   ./scripts/update_rust_sdk.sh e85d63ce8048a08e3a74949f0548299f13c58e49

set -euo pipefail

SDK_GIT_URL="https://github.com/iotaledger/iota-rust-sdk.git"

# Every crate published from iota-rust-sdk is named iota-sdk-<name>. A dependency
# line is rewritten when its key, or the crate it renames with `package`, matches
# and the line pins a version or a git source. The umbrella `iota-sdk` crate is
# left out on purpose: `crates/iota-sdk` is the node's own crate of that name.
SDK_CRATE_PATTERN='iota-sdk-[a-z0-9-]+'

SOURCE=${1:-}

VERSION_PATTERN='^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$'
COMMIT_PATTERN='^[0-9a-f]{7,40}$'

if [[ $SOURCE =~ $VERSION_PATTERN ]]; then
    SOURCE_KIND=version
elif [[ $SOURCE =~ $COMMIT_PATTERN ]]; then
    SOURCE_KIND=rev
else
    echo "Usage: $0 <version|commit>"
    echo "Examples:"
    echo "  $0 1.0.0-rc.2"
    echo "  $0 e85d63ce8048a08e3a74949f0548299f13c58e49"
    exit 1
fi

cd "$(git rev-parse --show-toplevel)"

# Rewrites the SDK dependency lines of the Cargo.toml files given as arguments.
# Prints each change to stderr and the path of each modified file to stdout.
REWRITE_MANIFESTS=$(cat <<'PERL'
use strict;
use warnings;

my $kind    = $ENV{SDK_SOURCE_KIND};
my $source  = $ENV{SDK_SOURCE};
my $git_url = $ENV{SDK_GIT_URL};
my $crates  = $ENV{SDK_CRATE_PATTERN};

for my $file (@ARGV) {
    open my $in, '<', $file or die "$file: $!";
    my @lines = <$in>;
    close $in;

    my $changed = 0;
    for (@lines) {
        my ($key, $rhs) = /^([A-Za-z0-9_-]+)\s*=\s*(.*?)\s*$/ or next;

        # Attributes of the dependency in source order, as [name, raw value].
        my @attrs;
        if ($rhs =~ /^"[^"]*"$/) {
            @attrs = (['version', $rhs]);
        } elsif ($rhs =~ /^\{\s*(.*?)\s*\}$/) {
            my $body = $1;
            while ($body =~ /\G\s*([A-Za-z0-9_-]+)\s*=\s*("(?:[^"\\]|\\.)*"|\[[^\]]*\]|[^,]*?)\s*(?:,|$)/gc) {
                push @attrs, [$1, $2];
            }
            if ((pos($body) // 0) != length $body) {
                warn "$file: cannot parse dependency line, left as is:\n  $_"
                    if $key =~ /^(?:$crates)$/ || $body =~ /package\s*=\s*"(?:$crates)"/;
                next;
            }
        } else {
            next;
        }

        my %attr  = map { $_->[0] => $_->[1] } @attrs;
        my $crate = $attr{package} // qq{"$key"};
        next unless $crate =~ /^"(?:$crates)"$/;
        my $old = $attr{rev} // $attr{tag} // $attr{branch} // $attr{version} // next;

        my @new = $kind eq 'rev'
            ? (['git', qq{"$git_url"}], ['rev', qq{"$source"}])
            : (['version', qq{"$source"}]);
        my @all = (@new, grep { $_->[0] !~ /^(?:version|git|rev|tag|branch)$/ } @attrs);
        my $new_rhs = @all == 1 && $all[0][0] eq 'version'
            ? $all[0][1]
            : '{ ' . join(', ', map { "$_->[0] = $_->[1]" } @all) . ' }';
        my $new_line = "$key = $new_rhs\n";
        next if $new_line eq $_;

        print STDERR "  $file [$key]: $old -> \"$source\"\n";
        $_ = $new_line;
        $changed = 1;
    }

    next unless $changed;
    open my $out, '>', $file or die "$file: $!";
    print $out @lines;
    close $out;
    print "$file\n";
}
PERL
)

# cargo-deny only accepts git sources listed in `allow-git` and warns about an
# entry no dependency uses, so the SDK repository is listed exactly while a
# commit is pinned.
update_deny() {
    local entry="  \"$SDK_GIT_URL\","
    if [ "$SOURCE_KIND" = rev ]; then
        if ! grep -qxF "$entry" deny.toml; then
            echo "  deny.toml: adding $SDK_GIT_URL to allow-git"
            SDK_DENY_ENTRY="$entry" perl -pi -e 's/^allow-git = \[$/$&\n$ENV{SDK_DENY_ENTRY}/' deny.toml
        fi
    elif grep -qxF "$entry" deny.toml; then
        echo "  deny.toml: removing $SDK_GIT_URL from allow-git"
        SDK_DENY_ENTRY="$entry" perl -ni -e 'print unless $_ eq "$ENV{SDK_DENY_ENTRY}\n"' deny.toml
    fi
}

echo "Pinning the SDK crates to $SOURCE_KIND $SOURCE"
echo "Updating Cargo.toml files..."

changed=$(git grep -lz -E "$SDK_CRATE_PATTERN" -- '*Cargo.toml' \
    | SDK_SOURCE_KIND="$SOURCE_KIND" SDK_SOURCE="$SOURCE" SDK_GIT_URL="$SDK_GIT_URL" SDK_CRATE_PATTERN="$SDK_CRATE_PATTERN" \
        xargs -0 perl -e "$REWRITE_MANIFESTS")

update_deny

if [ -z "$changed" ]; then
    echo "Every Cargo.toml already pins $SOURCE_KIND $SOURCE."
    exit 0
fi

echo "Updating Cargo.lock files..."
echo "$changed" | while read -r manifest; do
    cargo locate-project --workspace --message-format plain --manifest-path "$manifest"
done | sort -u | while read -r root_manifest; do
    echo "  $(dirname "$root_manifest")/Cargo.lock"
    cargo fetch --manifest-path "$root_manifest"
done

echo "All updates complete."
