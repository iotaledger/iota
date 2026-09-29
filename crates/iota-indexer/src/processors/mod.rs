// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::time::Duration;

/// How long a processor waits before checking the database again when the
/// data it needs is not there yet.
pub(crate) const POLL_INTERVAL: Duration = Duration::from_secs(1);

pub mod address_metrics_processor;
pub mod move_call_metrics_processor;
pub mod network_metrics_processor;
pub mod processor_orchestrator;
