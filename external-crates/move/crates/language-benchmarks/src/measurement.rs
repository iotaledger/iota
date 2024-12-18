// Copyright (c) The Diem Core Contributors
// Copyright (c) The Move Contributors
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use criterion::Criterion;

use crate::posix_time::PosixTime;

pub fn cpu_time_measurement() -> Criterion<PosixTime> {
    Criterion::default().with_measurement(PosixTime::UserAndSystemTime)
}

pub fn wall_time_measurement() -> Criterion {
    Criterion::default()
}
