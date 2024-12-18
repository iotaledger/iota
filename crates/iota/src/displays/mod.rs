// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod dev_inspect;
mod dry_run_tx_block;
mod gas_cost_summary;
mod ptb_preview;
mod status;
mod summary;

pub struct Pretty<'a, T>(pub &'a T);
