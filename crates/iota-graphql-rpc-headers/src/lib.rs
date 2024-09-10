// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use axum::http::HeaderName;

pub static VERSION_HEADER: HeaderName = HeaderName::from_static("x-iota-rpc-version");
pub static LIMITS_HEADER: HeaderName = HeaderName::from_static("x-iota-rpc-show-usage");
