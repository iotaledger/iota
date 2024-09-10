#!/bin/bash

# Copyright (c) 2024 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

if [[ "$OSTYPE" != "darwin"* && "$EUID" -ne 0 ]]; then
  echo "Please run as root or with sudo"
  exit
fi

docker compose down --remove-orphans
rm -rf data