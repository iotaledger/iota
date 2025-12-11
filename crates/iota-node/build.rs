// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

fn main() {
    // Declare the custom cfg to avoid warnings
    println!("cargo::rustc-check-cfg=cfg(nightly)");

    // Detect if we're using nightly Rust
    let meta = rustc_version_runtime::version_meta();
    if meta.channel == rustc_version_runtime::Channel::Nightly {
        println!("cargo:rustc-cfg=nightly");
    }
}
