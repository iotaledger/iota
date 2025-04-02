#!/bin/bash -e 


# INPUTS
NETWORK=${NETWORK-"testnet"}; VALID_NETWORKS=("testnet" "mainnet")
CLONE_DIR=${CLONE_DIR-"$(git rev-parse --show-toplevel || echo ".")/.cache/iota-clone"}
NODE_WORKDIR=${NODE_WORKDIR-"/opt/iota"}
CONFIG_DIR=${CONFIG_DIR-"$NODE_WORKDIR/config"}
BIN_DIR=${BIN_DIR-"$NODE_WORKDIR/bin"}

err() {
    printf "\e[31m[ERROR]: $1\e[0m\n"
    exit 1
}
G='\033[0;32m'
NC='\033[0m'

# Validate inputs
if [[ ! " ${VALID_NETWORKS[@]} " =~ " $NETWORK " ]]; then
  err "Invalid network selected: $NETWORK. Env var \$NETWORK must be one of: ${VALID_NETWORKS[*]}"
  exit 1
fi


# TODO after 6017 remove once yaml templates merged
HACK_ROOT="$(git rev-parse --show-toplevel || echo "$CLONE_DIR")"


if [  "$(uname -s)" != "Linux" ]; then 
    err "This script is for systemd so will only work on Linux."
    exit 1
fi
# Ensure rust is installed and up to date
if ! command -v cargo &> /dev/null; then
    err "Rust & cargo not installed or not found in \$PATH, install with:"
    err "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi



echo -e "This script will perform the following:"
echo -e " ${G}1.${NC} Check rust toolchain version"
echo -e " ${G}2.${NC} Install system packages (libraries & other dependencies)"
echo -e " ${G}3.${NC} Clone the iota repo"
echo -e " ${G}4.${NC} Build the iota-node binary"
echo -e " ${G}5.${NC} Create a user called iota, make it own directories for service binary, config, data"
echo -e " ${G}6.${NC} Create node config file, download genesis/migration blobs"
echo -e " ${G}6.${NC} Create systemd service unit file"
echo -e " ${G}7.${NC} Start the service\n"
read -p "Continue ? [y/N] " response
if [[ ! $response =~ ^[Yy]$ ]]; then
    echo "Install cancelled"
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
    if [ "$(git remote get-url origin)" != "https://github.com/iotaledger/iota.git" ]; then
        err "Cloned repo does not have correct origin, please delete then re-run this script."
        exit 1
    fi
fi
cd "$CLONE_DIR"
git checkout $NETWORK
git pull


# Check rustc version is above minimum (needs iota repo cloned before this step)
MIN_RUSTC_VERSION=$(grep 'channel' "$CLONE_DIR/rust-toolchain.toml" | awk -F '"' '{print $2}' )
rustc_version=$(rustc --version | sed -n 's/rustc \([0-9]\+\.[0-9]\+\).*/\1/p')
# checks that the min version is the smallest of both (by sorting)
if [[ $(echo -e "$rustc_version\n$MIN_RUSTC_VERSION" | sort -V | head -n1) == "$rustc_version" ]]; then 
    err "Rust compiler version is "$rustc_version". Needs at least version "$MIN_RUSTC_VERSION". Upgrade with:"
    err "rustup update" # build works on either stable or nightly
    exit 1
fi


# Build the binaries 
cargo build --release --bin iota-node


# Add a IOTA user, create directories for iota-node service
if id iota &>/dev/null;
    then echo "[INFO] IOTA user already exists"
    else echo "Creating IOTA user" && sudo useradd iota
fi
sudo mkdir -p "$BIN_DIR"
sudo mkdir -p "$CONFIG_DIR"
sudo mkdir -p "$NODE_WORKDIR/db"
sudo chown -R iota:iota "$NODE_WORKDIR"
sudo chown -R iota:iota "$BIN_DIR"
sudo chown -R iota:iota "$CONFIG_DIR"

# Create node config file
# TODO after 6017 remove override once yaml files per network merged
CONFIG_TEMPLATE=$( if [ -f "$CLONE_DIR/setups/fullnode/fullnode-template-$NETWORK.yaml" ]; 
    then cat "$CLONE_DIR/setups/fullnode/fullnode-template-$NETWORK.yaml"; 
    else cat "$HACK_ROOT/setups/fullnode/fullnode-template-$NETWORK.yaml";
fi )
echo "$CONFIG_TEMPLATE" \
    | sed "s|/opt/iota/config/genesis.blob|$CONFIG_DIR/genesis.blob|g" \
    | sed "s|/opt/iota/config/migration.blob|$CONFIG_DIR/migration.blob|g" \
    > "$CONFIG_DIR/fullnode.config.yaml"

# Download genesis/migration blobs for NETWORK
curl -fLJ https://dbfiles.$NETWORK.iota.cafe/genesis.blob -o "$CONFIG_DIR/genesis.blob"
if [ "$NETWORK" == "mainnet" ]; then
    curl -fLJ https://dbfiles.$NETWORK.iota.cafe/migration.blob -o "$CONFIG_DIR/migration.blob"
fi


# Move bin to $BIN_DIR
cp ./target/release/iota-node "$BIN_DIR/iota-node"

EXEC_START="\"$BIN_DIR/iota-node\" --config-path \"$CONFIG_DIR/fullnode.config.yaml\""

# TODO after 6017 remove use of HACK_ROOT local override (once file actually exists in $NETWORK branch/tag)
SERVICE_TEMPLATE=$( if [ -f "$CLONE_DIR/setups/fullnode/systemd/iota-node.service" ];
    then cat "$CLONE_DIR/setups/fullnode/systemd/iota-node.service"
    else cat "$HACK_ROOT/setups/fullnode/systemd/iota-node.service"
fi )
SERVICE_DEF=$(echo "$SERVICE_TEMPLATE" \
    | sed "s|/usr/local/bin/iota-node --config-path /opt/iota/config/validator.yaml|$EXEC_START|g" \
    )
echo "$SERVICE_DEF" > "/etc/systemd/system/iota-node.service"


# Reload systemd with this new service unit file
sudo systemctl daemon-reload
# Enable the new service with systemd
sudo systemctl enable iota-node.service
# Start the Validator
sudo systemctl start iota-node

# Check that the node is up and running
sudo systemctl status iota-node
# Follow logs with 
# journalctl -u iota-node -f




