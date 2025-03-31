#!/bin/bash -x

# INPUTS
NETWORK=${NETWORK-"testnet"}; VALID_NETWORKS=("testnet" "devnet")
CLONE_DIR=${CLONE_DIR-"$(git rev-parse --show-toplevel || echo ".")/.cache/iota-clone"}
NODE_CONFIG_DIR=${NODE_CONFIG_DIR-"/opt/iota/config"}


# TODO after 6017 remove once yaml templates merged
HACK_ROOT="$(git rev-parse --show-toplevel || echo "$CLONE_DIR")"


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

if [[ ! " ${VALID_NETWORKS[@]} " =~ " $NETWORK " ]]; then
  echo "Invalid network selected: $NETWORK. Must be one of: ${VALID_NETWORKS[*]}"
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

mkdir -p "$(dirname $CLONE_DIR)"
if [ ! -d "$CLONE_DIR" ]; then
    git clone https://github.com/iotaledger/iota.git "$CLONE_DIR"
else
    cd "$CLONE_DIR"
    if [ "$(git remote get-url origin)" -ne "https://github.com/iotaledger/iota.git" ]; then
        echo "Cloned repo does not have correct origin, please delete and re-clone"
        exit 1
    fi
fi
cd "$CLONE_DIR"
git checkout $NETWORK
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


# Build the binaries 
cargo build --release --bin iota-node

# Copy fullnode config
mkdir -p "$NODE_CONFIG_DIR"
# TODO after 6017 remove override once yaml files per network merged
TEMPLATE=$( if [ -f "$CLONE_DIR/setups/fullnode/fullnode-template-$NETWORK.yaml" ]; 
    then echo "$CLONE_DIR/setups/fullnode/fullnode-template-$NETWORK.yaml"; 
    else echo "$HACK_ROOT/setups/fullnode/fullnode-template-$NETWORK.yaml";
fi )

# TODO in 6017 replace paths in yaml then pipe to file
cat "$TEMPLATE" \
    | sed "s|/opt/iota/config/genesis.blob|$NODE_CONFIG_DIR/genesis.blob|g" \
    | sed "s|/opt/iota/config/migration.blob|$NODE_CONFIG_DIR/migration.blob|g" \
    > "$NODE_CONFIG_DIR/fullnode.config.yaml"

# Download genesis/migration blob for NETWORK
curl -fLJ https://dbfiles.$NETWORK.iota.cafe/genesis.blob -o "$NODE_CONFIG_DIR/genesis.blob"
if [ "$NETWORK" == "devnet" ]; then
    curl -fLJ https://dbfiles.$NETWORK.iota.cafe/migration.blob -o "$NODE_CONFIG_DIR/migration.blob"
fi


# Finally run the node
"$CLONE_DIR/target/release/iota-node" --config-path fullnode.yaml


# TODO in 6017 add systemd service (config, start, enable)




