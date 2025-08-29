#!/bin/bash

# Copyright (c) 2024 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

set -e

ROOT=$(git rev-parse --show-toplevel || realpath "$(dirname "$0")/../..")
PRIVNET_DIR="$(realpath "$(dirname "$0")" || echo "$ROOT/dev-tools/iota-private-network")"

TEMP_EXPORT_DIR="${TEMP_EXPORT_DIR-"$PRIVNET_DIR/configs/temp"}"
VALIDATOR_CONFIGS_DIR="$PRIVNET_DIR/configs/validators"
GENESIS_DIR="$PRIVNET_DIR/configs/genesis"
OVERLAY_PATH="$PRIVNET_DIR/configs/validator-common.yaml"
# Parse `-n` for number of validators (default 4), and `-p` for protocol
NUM_VALIDATORS=4
PROTOCOL=""
while getopts "n:p:" opt; do
  case "$opt" in
    n) NUM_VALIDATORS="$OPTARG" ;;
    p) PROTOCOL="$OPTARG" ;;
    *) echo "Usage: $0 [-n num_validators] [-p protocol]"; exit 1 ;;
  esac
done

generate_genesis_template_if_missing() {
    mkdir -p "$PRIVNET_DIR/configs"
    GENESIS_TEMPLATE="$PRIVNET_DIR/configs/genesis-template-${NUM_VALIDATORS}.yaml"

    if [[ -f "$GENESIS_TEMPLATE" ]]; then
        echo "Genesis template already exists: $GENESIS_TEMPLATE"
    else
        echo "Generating genesis template for $NUM_VALIDATORS validators..."
        cat > "$GENESIS_TEMPLATE" <<EOF
accounts:
  - address: "0xd59d79516a4ed5b6825e80826c075a12bdd2759aaeb901df2f427f5f880c8f60"
    gas_amounts:
      - 750000000000000000
      - 750000000000000000
  - address: "0x160ef6ce4f395208a12119c5011bf8d8ceb760e3159307c819bd0197d154d384"
    gas_amounts:
      - 20000000000000000
      - 20000000000000000
      - 20000000000000000
      - 20000000000000000
      - 20000000000000000
  - address: "0x7cc6ff19b379d305b8363d9549269e388b8c1515772253ed4c868ee80b149ca0"
    gas_amounts:
      - 750000000000000000
parameters:
  allow_insertion_of_extra_objects: false
  epoch_duration_ms: 1200000
validator_config_info:
EOF

        for i in $(seq 1 $NUM_VALIDATORS); do
            cat >> "$GENESIS_TEMPLATE" <<EOF
  - commission_rate: 0
    gas_price: 1000
    name: validator-${i}
    primary_address: /dns/validator-${i}/udp/8081
    network_address: /dns/validator-${i}/tcp/8080/http
    p2p_address: /dns/validator-${i}/udp/8084
    stake: 20000000000000000
EOF
        done

        cat >> "$GENESIS_TEMPLATE" <<EOF
migration_sources: []
EOF
        echo "Genesis template generated: $GENESIS_TEMPLATE"
    fi
}


shift $((OPTIND-1))
set_env_var() {
  # Usage: set_env_var KEY VALUE FILE
  local key="$1" value="$2" file="$3"
  mkdir -p "$(dirname "$file")"
  if [ -f "$file" ]; then
    if grep -q "^${key}=" "$file"; then
      # portable in-place replace without sed -i
      tmpfile="$(mktemp)"
      awk -v k="$key" -v v="$value" 'BEGIN{changed=0} {if ($0 ~ "^"k"=") {print k"="v; changed=1} else {print}} END{if (!changed) print k"="v}' "$file" > "$tmpfile" && mv "$tmpfile" "$file"
    else
      echo "${key}=${value}" >> "$file"
    fi
  else
    echo "${key}=${value}" > "$file"
  fi
}
unset_env_var() {
  # Usage: unset_env_var KEY FILE
  local key="$1" file="$2"
  [ -f "$file" ] || return 0
  tmpfile="$(mktemp)"
  awk -v k="$key" 'BEGIN{removed=0} $0 !~ "^"k"=" {print} $0 ~ "^"k"=" {removed=1} END{}' "$file" > "$tmpfile" && mv "$tmpfile" "$file"
}

# Select the matching genesis template
GENESIS_TEMPLATE="$PRIVNET_DIR/configs/genesis-template-${NUM_VALIDATORS}.yaml"

PRIVATE_DATA_DIR="$PRIVNET_DIR/data"

check_docker_image_exist() {
  if ! docker image inspect "$1" >/dev/null 2>&1; then
    echo "Error: Docker image $1 not found."
    exit 1
  fi
}

check_configs_exist() {
  if [ ! -f "$1" ]; then
    echo "Error: $(basename "$1") not found at "$1""
    exit 1
  fi
}

generate_genesis_files() {
  mkdir -p "$TEMP_EXPORT_DIR"

  # Generate genesis using the selected template
  TMP_GENESIS_TEMPLATE="$(basename "$GENESIS_TEMPLATE")"
  docker run --rm \
    -v "$PRIVNET_DIR:/iota" \
    -v "$TEMP_EXPORT_DIR:/iota/configs/temp" \
    -w /iota \
    iotaledger/iota-tools \
    /usr/local/bin/iota genesis \
      --from-config "/iota/configs/$TMP_GENESIS_TEMPLATE" \
      --working-dir "/iota/configs/temp" -f

  for file in "$TEMP_EXPORT_DIR"/validator*.yaml; do
    if [ -f "$file" ]; then
      yq eval-all '
        select(fileIndex == 1).validator as $overlay |
        select(fileIndex == 0) |
        .network-address = $overlay.network-address |
        .metrics-address = $overlay.metrics-address |
        .json-rpc-address = $overlay.json-rpc-address |
        .admin-interface-address = $overlay.admin-interface-address |
        .genesis.genesis-file-location = $overlay.genesis.genesis-file-location |
        .db-path = $overlay.db-path |
        .consensus-config.db-path = $overlay.consensus-config.db-path |
        .expensive-safety-check-config = $overlay.expensive-safety-check-config |
        .epoch_duration_ms = $overlay.epoch_duration_ms
      ' "$file" "$OVERLAY_PATH" >"${file}.tmp" && mv "${file}.tmp" "$file"
    fi
  done

  # copy generated validator configs
  for src_validator_config_filepath in "$TEMP_EXPORT_DIR"/validator*; do
    src_filename=$(basename -- "$src_validator_config_filepath")
    dest_filepath="$VALIDATOR_CONFIGS_DIR/$src_filename"

    # delete if directory (happens if docker-compose was started without the file being present)
    if [ -d "$dest_filepath" ] && [ -n "$dest_filepath" ] && (echo "$dest_filepath" | grep -q "configs/validators/validator-"); then
      rm -rf "$dest_filepath"
    fi
    if [ -e "$src_validator_config_filepath" ]; then
      mv "$src_validator_config_filepath" "$VALIDATOR_CONFIGS_DIR/"
    fi
  done

  genesis_dest_filepath="$GENESIS_DIR/genesis.blob"
  echo "$genesis_dest_filepath"
  # delete if directory (happens if docker-compose was started without the file being present)
  if [ -d "$genesis_dest_filepath" ] && [ -n "$genesis_dest_filepath" ] && (echo "$genesis_dest_filepath" | grep -q "iota-private-network/configs/genesis/genesis.blob"); then
    rm -rf "$genesis_dest_filepath"
  fi
  mv "$TEMP_EXPORT_DIR/genesis.blob" "$GENESIS_DIR/"

  rm -rf "$TEMP_EXPORT_DIR"
}

create_folder_for_postgres() {
  mkdir -p "$PRIVATE_DATA_DIR/primary" "$PRIVATE_DATA_DIR/replica"
  if [ "$(uname -s)" == "Linux" ]; then
    chown -R 999:999 "$PRIVATE_DATA_DIR/primary" "$PRIVATE_DATA_DIR/replica"
  fi
  chmod 0755 "$PRIVATE_DATA_DIR/primary" "$PRIVATE_DATA_DIR/replica"
}

main() {
  if [[ "$OSTYPE" != "darwin"* && "$EUID" -ne 0 ]]; then
      echo "Please run as root or with sudo"
      exit 1
    fi
    [ -d "$TEMP_EXPORT_DIR" ] && rm -rf "$TEMP_EXPORT_DIR"

    [ -d "$PRIVATE_DATA_DIR" ] && ./cleanup.sh

    # Generate genesis template if missing
    generate_genesis_template_if_missing

    # Only check overlay file existence
    check_configs_exist "$OVERLAY_PATH"

    for image in "iotaledger/iota-tools" "iotaledger/iota-node" "iotaledger/iota-indexer"; do
      check_docker_image_exist "$image"
    done

    generate_genesis_files
    create_folder_for_postgres

  # Manage CONSENSUS_PROTOCOL in .env: set if provided, otherwise remove it
  ENV_FILE="$PRIVNET_DIR/.env"
  if [ -n "$PROTOCOL" ]; then
    set_env_var CONSENSUS_PROTOCOL "$PROTOCOL" "$ENV_FILE"
    echo "Set CONSENSUS_PROTOCOL=$PROTOCOL in $ENV_FILE"
  else
    unset_env_var CONSENSUS_PROTOCOL "$ENV_FILE"
    echo "Cleared CONSENSUS_PROTOCOL from $ENV_FILE (no -p provided)"
  fi

  echo "Done"
}

main
