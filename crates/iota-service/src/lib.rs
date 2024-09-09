// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod health;
pub mod logging;
pub mod metrics;
pub mod server_timing;
mod service;

pub use service::{get_iota_service, serve};

pub const DEFAULT_PORT: u16 = 2024;

#[macro_export]
macro_rules! package_name {
    () => {
        env!("CARGO_PKG_NAME")
    };
}

#[macro_export]
macro_rules! package_version {
    () => {
        env!("CARGO_PKG_VERSION")
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_name_works() {
        assert_eq!(package_name!(), "iota-service",);
    }

    #[test]
    fn package_version_works() {
        assert_eq!(package_version!(), "0.0.1",);
    }
}
