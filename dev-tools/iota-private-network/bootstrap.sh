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
GENESIS_TEMPLATE="$PRIVNET_DIR/configs/genesis-template.yaml"
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

  docker run --rm \
    -v "$PRIVNET_DIR:/iota" \
    -v "$TEMP_EXPORT_DIR:/iota/configs/temp" \
    -w /iota \
    iotaledger/iota-tools \
    /usr/local/bin/iota genesis --from-config "/iota/configs/genesis-template.yaml" --working-dir "/iota/configs/temp" -f

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

  for config_path in "$GENESIS_TEMPLATE" "$OVERLAY_PATH"; do
    check_configs_exist "$config_path"
  done

  for image in "iotaledger/iota-tools" "iotaledger/iota-node" "iotaledger/iota-indexer"; do
    check_docker_image_exist "$image"
  done

  generate_genesis_files
  create_folder_for_postgres

  echo "Done"
}

main
