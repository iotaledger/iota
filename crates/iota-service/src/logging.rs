// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use telemetry_subscribers::TelemetryConfig;

pub fn init() -> telemetry_subscribers::TelemetryGuards {
    let (guard, _handle) = TelemetryConfig::new().with_env().init();
    guard
}
