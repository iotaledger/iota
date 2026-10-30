// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod dev_inspect;
mod dry_run;
mod executed_transaction;
mod gas_cost_summary;
mod ptb_preview;
mod status;
mod summary;

pub use dry_run::DryRunDisplay;

pub struct Pretty<'a, T>(pub &'a T);
