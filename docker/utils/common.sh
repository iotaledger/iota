#!/bin/bash
# Copyright (c) 2024 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

function print_step {
    echo -e "\e[32m$1\e[0m"
}

function print_error {
    echo -e "\e[31m$1\e[0m"
}

function check_error {
    if [ $? -ne 0 ]; then
        print_error "$1"
        exit 1
    fi
}
