// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[cfg(all(msim, test))]
mod node;

#[cfg(all(msim, test))]
#[path = "tests/simtests.rs"]
mod simtests;
