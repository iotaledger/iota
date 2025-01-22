#!/bin/bash

# Copyright (c) 2024 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

set -e

TEMP_EXPORT_DIR="$(pwd)/configs/temp"
VALIDATOR_CONFIGS_DIR="$(pwd)/configs/validators"
GENESIS_DIR="$(pwd)/configs/genesis"
OVERLAY_PATH="$(pwd)/configs/validator-common.yaml"
GENESIS_TEMPLATE="$(pwd)/configs/genesis-template.yaml"
PRIVATE_DATA_DIR="$(pwd)/data"

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
  mkdir "$TEMP_EXPORT_DIR"

  docker run --rm \
    -v "$(pwd):/iota" \
    -w /iota \
    iota-tools \
    /usr/local/bin/iota genesis --from-config "/iota/configs/genesis-template.yaml" --working-dir "/iota/configs/temp" -f

  for file in "$TEMP_EXPORT_DIR"/validator*.yaml; do
    if [ -f "$file" ]; then
      yq eval-all '
        select(fileIndex == 1).validator as $overlay |
        select(fileIndex == 0) |
        .network-address = $overlay.network-address |
        .metrics-address = $overlay.metrics-address |
        .json-rpc-address = $overlay.json-rpc-address |
        .admin-interface-port = $overlay.admin-interface-port |
        .genesis.genesis-file-location = $overlay.genesis.genesis-file-location |
        .db-path = $overlay.db-path |
        .consensus-config.db-path = $overlay.consensus-config.db-path |
        .expensive-safety-check-config = $overlay.expensive-safety-check-config |
        .epoch_duration_ms = $overlay.epoch_duration_ms
      ' "$file" "$OVERLAY_PATH" >"${file}.tmp" && mv "${file}.tmp" "$file"
    fi
  done

  for file in "$TEMP_EXPORT_DIR"/validator*; do
    if [ -e "$file" ]; then
      mv "$file" "$VALIDATOR_CONFIGS_DIR/"
    fi
  done

  mv "$TEMP_EXPORT_DIR/genesis.blob" "$GENESIS_DIR/"

  rm -rf "$TEMP_EXPORT_DIR"
}

create_folder_for_postgres() {
  mkdir -p ./data/primary ./data/replica
  chown -R 999:999 ./data/primary ./data/replica
  chmod 0755 ./data/primary ./data/replica
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

  for image in iota-tools iota-node iota-indexer; do
    check_docker_image_exist "$image"
  done

  generate_genesis_files
  create_folder_for_postgres

  echo "Done"
}

main
