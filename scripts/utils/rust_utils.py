# Copyright (c) 2024 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0
import re

# Parse the rust-toolchain.toml file to get the Rust version
def parse_rust_toolchain_version():
    try:
        content = None
        with open('rust-toolchain.toml', 'r') as file:
            content = file.read()

        # Regex to find the Rust version
        match = re.search(r'(?<=channel = ").*(?=")', content)
        if not match:
            raise Exception("Rust version not found in rust-toolchain.toml.")
        
        return match.group(0)

    except FileNotFoundError:
        raise FileNotFoundError("rust-toolchain.toml not found.")
