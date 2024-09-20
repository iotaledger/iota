// Copyright 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use thiserror::Error;

use super::Query;

#[derive(Debug, Error)]
#[error("no value matching {0}")]
pub struct QueryError(pub Query);
