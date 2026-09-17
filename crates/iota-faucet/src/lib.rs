// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#![cfg_attr(nightly_lint, feature(non_exhaustive_omitted_patterns_lint))]
#![cfg_attr(nightly_lint, warn(non_exhaustive_omitted_patterns))]

mod errors;
mod faucet;
mod metrics;
mod requests;
mod responses;
mod server;

pub mod metrics_layer;
pub use errors::FaucetError;
pub use faucet::*;
pub use metrics_layer::*;
pub use requests::*;
pub use responses::*;
pub use server::{create_wallet_context, start_faucet};
