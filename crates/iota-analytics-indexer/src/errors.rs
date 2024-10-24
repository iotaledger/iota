// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AnalyticsIndexerError {
    #[error("Generic error: `{0}`")]
    Generic(String),
    #[error("Failed to retrieve the current directory.")]
    CurrentDir,
}
