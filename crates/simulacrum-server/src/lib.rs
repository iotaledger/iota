// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Simulacrum Server Library
//!
//! This crate provides both gRPC and REST API servers for IOTA Simulacrum.

pub mod faucet;
pub mod grpc_server;
pub mod rest_api;

pub use grpc_server::start_simulacrum_grpc_server;
pub use rest_api::{AppState, create_router};
pub use simulacrum::{Simulacrum, store::SimulatorStore};
