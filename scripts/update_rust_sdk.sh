#!/bin/bash
# Copyright (c) 2026 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

# Pins the iota-rust-sdk crates to either a crates.io release or a git commit
# of https://github.com/iotaledger/iota-rust-sdk in every Cargo.toml that
# depends on them, lists the SDK repository in deny.toml only while a commit
# is pinned, and refreshes the affected Cargo.lock files without compiling.
#
# Without an argument, every SDK crate the repository depends on is pinned to
# its own most recently published crates.io release, pre-releases included, so
# crates whose latest release lags behind the others keep working. The release
# dates come from the crates.io API; yanked releases are skipped.
#
# An argument is detected: a semver version (for example 1.0.0-rc.2) pins that
# crates.io release of every crate, a hex commit hash (7 to 40 characters) pins
# that commit. Every other attribute of a dependency line (package,
# default-features, features, optional, ...) is kept as is.
#
# Usage: ./scripts/update_rust_sdk.sh [version|commit]
# Examples:
#   ./scripts/update_rust_sdk.sh
#   ./scripts/update_rust_sdk.sh 1.0.0-rc.2
#   ./scripts/update_rust_sdk.sh e85d63ce8048a08e3a74949f0548299f13c58e49

set -euo pipefail

SDK_GIT_URL="https://github.com/iotaledger/iota-rust-sdk.git"
CRATES_IO_API="https://crates.io/api/v1/crates"
# crates.io rejects requests without a User-Agent header.
CRATES_IO_USER_AGENT="iota/scripts/update_rust_sdk.sh"

# Every crate published from iota-rust-sdk is named iota-sdk-<name>. A dependency
# line is rewritten when its key, or the crate it renames with `package`, matches
# and the line pins a version or a git source. The umbrella `iota-sdk` crate is
# left out on purpose: `crates/iota-sdk` is the node's own crate of that name.
SDK_CRATE_PATTERN='iota-sdk-[a-z0-9-]+'

SOURCE=${1:-}

VERSION_PATTERN='^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$'
COMMIT_PATTERN='^[0-9a-f]{7,40}$'

if [ -z "$SOURCE" ]; then
    SOURCE_KIND=latest
elif [[ $SOURCE =~ $VERSION_PATTERN ]]; then
    SOURCE_KIND=version
elif [[ $SOURCE =~ $COMMIT_PATTERN ]]; then
    SOURCE_KIND=rev
else
    echo "Usage: $0 [version|commit]"
    echo "Examples:"
    echo "  $0"
    echo "  $0 1.0.0-rc.2"
    echo "  $0 e85d63ce8048a08e3a74949f0548299f13c58e49"
    exit 1
fi

cd "$(git rev-parse --show-toplevel)"

# Prints the Perl script that parses the SDK dependency lines of the Cargo.toml
# files given as arguments.
#
# With SDK_MODE=list, prints the name of every SDK crate found to stdout.
# Otherwise rewrites the lines: a commit (SDK_SOURCE_KIND=rev, SDK_SOURCE) or
# the per-crate versions of SDK_VERSIONS (one `crate=version` per line). Prints
# each change to stderr and the path of each modified file to stdout.
sdk_manifests_script() {
    cat <<'PERL'
use strict;
use warnings;

my $mode    = $ENV{SDK_MODE} // 'rewrite';
my $kind    = $ENV{SDK_SOURCE_KIND} // '';
my $source  = $ENV{SDK_SOURCE} // '';
my $git_url = $ENV{SDK_GIT_URL};
my $crates  = $ENV{SDK_CRATE_PATTERN};
my %version = map { split /=/, $_, 2 } grep { length } split /\n/, $ENV{SDK_VERSIONS} // '';

# Splits a dependency line into its key and its attributes in source order, as
# [name, raw value]. Returns nothing for lines that are not dependencies.
sub parse_dependency {
    my ($file, $line) = @_;
    my ($key, $rhs) = $line =~ /^([A-Za-z0-9_-]+)\s*=\s*(.*?)\s*$/ or return;

    return ($key, [['version', $rhs]]) if $rhs =~ /^"[^"]*"$/;
    my ($body) = $rhs =~ /^\{\s*(.*?)\s*\}$/ or return;

    my @attrs;
    while ($body =~ /\G\s*([A-Za-z0-9_-]+)\s*=\s*("(?:[^"\\]|\\.)*"|\[[^\]]*\]|[^,]*?)\s*(?:,|$)/gc) {
        push @attrs, [$1, $2];
    }
    if ((pos($body) // 0) != length $body) {
        warn "$file: cannot parse dependency line, left as is:\n  $line"
            if $key =~ /^(?:$crates)$/ || $body =~ /package\s*=\s*"(?:$crates)"/;
        return;
    }
    return ($key, \@attrs);
}

for my $file (@ARGV) {
    open my $in, '<', $file or die "$file: $!";
    my @lines = <$in>;
    close $in;

    my $changed = 0;
    for (@lines) {
        my ($key, $attrs) = parse_dependency($file, $_) or next;
        my %attr = map { $_->[0] => $_->[1] } @$attrs;
        my ($crate) = ($attr{package} // qq{"$key"}) =~ /^"($crates)"$/ or next;
        my $old = $attr{rev} // $attr{tag} // $attr{branch} // $attr{version} // next;

        if ($mode eq 'list') {
            print "$crate\n";
            next;
        }

        my @new;
        if ($kind eq 'rev') {
            @new = (['git', qq{"$git_url"}], ['rev', qq{"$source"}]);
        } elsif (defined $version{$crate}) {
            @new = (['version', qq{"$version{$crate}"}]);
        } else {
            warn "$file [$key]: no version resolved for $crate, left as is\n";
            next;
        }
        my @all = (@new, grep { $_->[0] !~ /^(?:version|git|rev|tag|branch)$/ } @$attrs);
        my $new_rhs = @all == 1 && $all[0][0] eq 'version'
            ? $all[0][1]
            : '{ ' . join(', ', map { "$_->[0] = $_->[1]" } @all) . ' }';
        my $new_line = "$key = $new_rhs\n";
        next if $new_line eq $_;

        print STDERR "  $file [$key]: $old -> $new[-1][1]\n";
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
}

# Runs the manifest parser over every Cargo.toml that mentions an SDK crate.
sdk_manifests() {
    git grep -lz -E "$SDK_CRATE_PATTERN" -- '*Cargo.toml' \
        | SDK_GIT_URL="$SDK_GIT_URL" SDK_CRATE_PATTERN="$SDK_CRATE_PATTERN" \
            SDK_SOURCE_KIND="$SOURCE_KIND" SDK_SOURCE="$SOURCE" SDK_VERSIONS="${SDK_VERSIONS:-}" "$@" \
            xargs -0 perl -e "$(sdk_manifests_script)"
}

# Prints the most recently published, non-yanked crates.io release of a crate.
latest_release() {
    curl -fsS -H "User-Agent: $CRATES_IO_USER_AGENT" "$CRATES_IO_API/$1/versions" \
        | perl -MJSON::PP -0777 -ne '
            my @published = grep { !$_->{yanked} } @{ decode_json($_)->{versions} };
            my ($latest) = sort { $b->{created_at} cmp $a->{created_at} } @published;
            die "no published release found\n" unless $latest;
            print "$latest->{num}\n";
        '
}

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

SDK_VERSIONS=""
case $SOURCE_KIND in
latest)
    echo "Resolving the latest crates.io release of each SDK crate..."
    for crate in $(sdk_manifests env SDK_MODE=list | sort -u); do
        release=$(latest_release "$crate") || {
            echo "Could not resolve the latest release of $crate." >&2
            exit 1
        }
        echo "  $crate: $release"
        SDK_VERSIONS+="$crate=$release"$'\n'
    done
    echo "Pinning the SDK crates to their latest releases"
    ;;
version)
    for crate in $(sdk_manifests env SDK_MODE=list | sort -u); do
        SDK_VERSIONS+="$crate=$SOURCE"$'\n'
    done
    echo "Pinning the SDK crates to version $SOURCE"
    ;;
rev)
    echo "Pinning the SDK crates to commit $SOURCE"
    ;;
esac

echo "Updating Cargo.toml files..."
changed=$(sdk_manifests)

update_deny

if [ -z "$changed" ]; then
    echo "Every Cargo.toml is already up to date."
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
