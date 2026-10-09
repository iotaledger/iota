// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Integration tests for the metrics of the gRPC server. Each test runs a real
//! server and a real tonic client.

#[path = "../common/mod.rs"]
mod common;
mod harness;
mod requested_max;
mod response_item_sizes;
mod response_message_sizes;
