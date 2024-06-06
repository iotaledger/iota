// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub const TEST_DIR: &str = "tests";
use iota_transactional_test_runner::run_test;

datatest_stable::harness!(run_test, TEST_DIR, r".*\.(mvir|move)$");
