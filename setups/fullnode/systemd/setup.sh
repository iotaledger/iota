#!/bin/bash -e 

# INPUTS
NETWORK=${NETWORK-"testnet"}; VALID_NETWORKS=("testnet" "mainnet" "devnet")
CLONE_DIR=${CLONE_DIR-"$HOME/.cache/iota-clone"}
NODE_WORKDIR=${NODE_WORKDIR-"/opt/iota"}
CONFIG_DIR=${CONFIG_DIR-"$NODE_WORKDIR/config"}
BIN_DIR=${BIN_DIR-"$NODE_WORKDIR/bin"}


red() { printf "\e[31m$1\e[0m\n"; }
info() { printf "\e[32m[INFO]: $1\e[0m\n"; }
G='\033[0;32m'
NC='\033[0m'

CONFIG_FILE_PATH="$CONFIG_DIR/fullnode.yaml"

# Validate inputs
if [[ ! "${VALID_NETWORKS[@]}" =~ "$NETWORK" ]]; then
  red "[ERROR] Invalid network selected: $NETWORK. Env var \$NETWORK must be one of: ${VALID_NETWORKS[*]}"
  exit 1
fi

if [ -f "$CONFIG_FILE_PATH" ]; then 
    PREV_NETWORK=$(grep -oP '/dns/(?:[^/]+\.)*\K[^./]+(?=\.(?:iota\.cafe)/)' -m 1 "$CONFIG_FILE_PATH")
    if [ "$NETWORK" != "$PREV_NETWORK" ]; then 
        red "[ERROR] Found a previous config file at $CONFIG_FILE_PATH for a different network ($PREV_NETWORK)."
        red "Please:"
        red "1. Move / backup / delete the file at $CONFIG_FILE_PATH"
        red "2. Move / backup / delete the directory at'$NODE_WORKDIR/db' "
        red "3. Re-run this installer script (or just re-run it with the same network to keep using files above)"
        exit 1
    fi
fi

# Check dependencies
if [  "$(uname -s)" != "Linux" ]; then 
    red "[ERROR] This script is for systemd so will only work on Linux."
    exit 1
fi
# Ensure rust is installed and up to date
if ! command -v cargo &> /dev/null; then
    red "[ERROR] Rust & cargo not installed or not found in \$PATH, install with:"
    echo " \$ curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi


echo -e "This script will perform the following steps:"
echo -e " ${G}1.${NC} Check rust toolchain version"
echo -e " ${G}2.${NC} Install system packages (libraries & other dependencies)"
echo -e " ${G}3.${NC} Clone the iota repo (set to the branch for the $NETWORK network)"
echo -e " ${G}4.${NC} Build the iota-node binary"
echo -e " ${G}5.${NC} Create a user called iota, make it own directories for service binary, config and data"
echo -e " ${G}6.${NC} Create a node config file, download genesis/migration blobs"
echo -e " ${G}7.${NC} Create a systemd service unit file"
echo -e " ${G}8.${NC} Start the service\n"
read -p "Continue ? [y/N] " response
if [[ ! $response =~ ^[Yy]$ ]]; then
    red "[ERROR] Install cancelled"
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
if ! command -v diff &> /dev/null; then sudo apt-get install -y --no-install-recommends diffutils; fi

mkdir -p "$(dirname $CLONE_DIR)"
if [ ! -d "$CLONE_DIR" ]; then
    git clone https://github.com/iotaledger/iota.git "$CLONE_DIR"
else
    cd "$CLONE_DIR"
    if [ "$(git remote get-url origin)" != "https://github.com/iotaledger/iota.git" ]; then
        red "[ERROR] Cloned repo does not have correct origin, please delete then re-run this script."
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
if [ "$rustc_version" != "$MIN_RUSTC_VERSION" ] && [[ $(echo -e "$rustc_version\n$MIN_RUSTC_VERSION" | sort -V | head -n1) == "$rustc_version" ]]; then 
    red "[ERROR] Rust compiler version is "$rustc_version". Needs at least version "$MIN_RUSTC_VERSION". Upgrade with:"
    echo " \$ rustup update " # build works on either stable or nightly
    exit 1
fi


# Build the binaries 
cargo build --release --bin iota-node 


# Add a IOTA user, create directories for iota-node service
if id iota &>/dev/null;
    then info "IOTA user already exists"
    else info "Creating IOTA user" && sudo useradd iota
fi
sudo mkdir -p "$BIN_DIR"
sudo mkdir -p "$CONFIG_DIR"
sudo mkdir -p "$NODE_WORKDIR/db"
sudo chown -R iota:iota "$NODE_WORKDIR"
sudo chown -R iota:iota "$BIN_DIR"
sudo chown -R iota:iota "$CONFIG_DIR"

write_to_file() {
    CONTENTS="$1"; FILE_PATH="$2";
    if [ -f "$FILE_PATH" ]; then
        if diff -q <(echo -e "$CONTENTS") "$FILE_PATH" >/dev/null; then
            info "$FILE_PATH already exists and matches"
        else
            read -p "Config file $FILE_PATH already exists, but does not match. Overwrite ? [y/N]" answer
            if [[ ! $answer =~ ^[Yy]$ ]]; then
                red "Install cancelled"
                exit 1
            fi
            sudo mkdir -p "$(dirname "$FILE_PATH")"
            sudo echo -e "$CONTENTS" > "$FILE_PATH"
        fi
    else 
        sudo mkdir -p "$(dirname "$FILE_PATH")"
        sudo echo -e "$CONTENTS" > "$FILE_PATH"
    fi
}

# This is only temporary, to make the script work locally without waiting for the PR to be merged
# TODO after 6017 remove once yaml templates merged
HACK_ROOT="$(git rev-parse --show-toplevel || echo "$CLONE_DIR")"

# Create node config file
CONFIG_TEMPLATE=$( if [ -f "$CLONE_DIR/setups/fullnode/fullnode-template-$NETWORK.yaml" ]; 
    then cat "$CLONE_DIR/setups/fullnode/fullnode-template-$NETWORK.yaml"; 
    # TODO after 6017 remove override once yaml files per network merged
    # This hack is only temporary, to work around the problem that the template is not available until we merge this PR
    else cat "$HACK_ROOT/setups/fullnode/fullnode-template-$NETWORK.yaml";
fi )
CONFIG=$(echo "$CONFIG_TEMPLATE" \
    | sed "s|/opt/iota/config/genesis.blob|$CONFIG_DIR/genesis.blob|g" \
    | sed "s|/opt/iota/config/migration.blob|$CONFIG_DIR/migration.blob|g")
write_to_file "$CONFIG" "$CONFIG_FILE_PATH"

# Download genesis/migration blobs for NETWORK
curl -sfLJ https://dbfiles.$NETWORK.iota.cafe/genesis.blob -o "$CONFIG_DIR/genesis.blob"
if [ "$NETWORK" == "mainnet" ] || [ "$NETWORK" == "devnet" ]; then
    curl -sfLJ https://dbfiles.$NETWORK.iota.cafe/migration.blob -o "$CONFIG_DIR/migration.blob"
fi


# Move bin to $BIN_DIR
cp ./target/release/iota-node "$BIN_DIR/iota-node"

EXEC_START="\"$BIN_DIR/iota-node\" --config-path \"$CONFIG_DIR/fullnode.yaml\""

# TODO after 6017 remove use of HACK_ROOT local override (once file actually exists in $NETWORK branch/tag)
SERVICE_TEMPLATE=$( if [ -f "$CLONE_DIR/setups/fullnode/systemd/iota-node.service" ];
    then cat "$CLONE_DIR/setups/fullnode/systemd/iota-node.service"
    else cat "$HACK_ROOT/setups/fullnode/systemd/iota-node.service"
fi )
SERVICE_DEF=$(echo "$SERVICE_TEMPLATE" \
    | sed "s|/usr/local/bin/iota-node --config-path /opt/iota/config/validator.yaml|$EXEC_START|g" \
)
write_to_file "$SERVICE_DEF" "/etc/systemd/system/iota-node.service"


# Files might have been created / overwritten by root user
sudo chown -R iota:iota "$NODE_WORKDIR"
sudo chown -R iota:iota "$BIN_DIR"
sudo chown -R iota:iota "$CONFIG_DIR"
# Reload systemd with this new service unit file
sudo systemctl daemon-reload
# Enable the new service with systemd
sudo systemctl enable iota-node.service
# Start the Validator
sudo systemctl start iota-node

# Check that the node is up and running
sudo systemctl status iota-node
