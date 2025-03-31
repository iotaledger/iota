#!/bin/bash -x

NETWORK=${NETWORK-"testnet"}
CLONE_DEST=${CLONE_DEST-"./.cache/iota"}


if [  "$(uname -s)" -ne "Linux" ]; then 
    echo "This script is for systemd so will only work on Linux."
    exit 1
fi
# make sure rust is installed in path and up to date
# TODO in 6017 check if building binary nes nightly or stable is enough
if ! command -v cargo &> /dev/null; then
    echo "Rust not installed or not found in \$PATH, install with:"
    echo "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

mkdir -p "$(dirname $CLONE_DEST)"
if [ ! -d "$CLONE_DEST" ]; then
    git clone https://github.com/iotaledger/iota.git "$CLONE_DEST"
else
    cd "$CLONE_DEST"
    if [ "$(git remote get-url origin)" -ne "https://github.com/iotaledger/iota.git" ]; then
        echo "Cloned repo does not have correct origin, please delete and re-clone"
        exit 1
    fi
fi
cd "$CLONE_DEST"
git checkout testnet
git pull

# TODO in 6017 extract min version from rust-toolchain.toml
MIN_RUSTC_VERSION=1.85
rustc_version=$(rustc --version | sed -n 's/rustc \([0-9]\+\.[0-9]\+\).*/\1/p')
# checks that the min version is the smallest of both (by sorting)
if [[ $(echo -e "$rustc_version\n$MIN_RUSTC_VERSION" | sort -V | head -n1) == "$rustc_version" ]]; then 
    echo "Rust compiler version is "$rustc_version". Needs at least version "$MIN_RUSTC_VERSION". Upgrade with:"
    echo "rustup update" # TODO in 6017 nightly ????
    exit 1
fi


# Install system packages (libraries & other dependencies)
sudo apt-get update \
&& sudo apt-get install -y --no-install-recommends \
    tzdata \
    libprotobuf-dev \
    ca-certificates \
    build-essential \
    libssl-dev \
    libclang-dev \
    libpq-dev \
    pkg-config \
    openssl \
    protobuf-compiler \
    git \
    clang \
    cmake

REPO_ROOT="$(git rev-parse --show-toplevel || echo "$CLONE_DEST")"
# Install the binaries 
# TODO in 6017 build with --release
cargo build --bin iota-node

