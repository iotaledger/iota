// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#![allow(unused_imports)]
#![allow(unused_variables)]
use std::{path::Path, sync::Arc};
use iota_transactional_test_runner::{
    run_test_impl,
    test_adapter::{IotaTestAdapter, PRE_COMPILED},
};

datatest_stable::harness!(
    run_test,
    "tests",
    if cfg!(feature = "staging") {
        r"\.move$"
    } else {
        r"stable/.*\.move$"
    }
);

#[cfg_attr(not(msim), tokio::main)]
#[cfg_attr(msim, msim::main)]
async fn run_test(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    telemetry_subscribers::init_for_testing();
    if !cfg!(msim) {
        run_test_impl::<IotaTestAdapter>(path, Some(Arc::new(PRE_COMPILED.clone()))).await?;
    }
    Ok(())
}
