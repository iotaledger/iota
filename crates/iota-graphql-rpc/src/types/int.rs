// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::fmt::Display;

use crate::error::Error;

/// Convert an integer to `i32`, the value range of a GraphQL `Int`, returning
/// an internal error if the value does not fit.
///
/// This should be preferred over returning wider rust int types like u32, i64,
/// etc. as they are officially exposed in the GraphQL schema as i32 anyway, and
/// may result in data being truncated on client side.
pub(crate) fn try_into_int<T>(value: T) -> Result<i32, Error>
where
    T: TryInto<i32> + Display + Copy,
{
    value
        .try_into()
        .map_err(|_| Error::Internal(format!("value {value} does not fit in a GraphQL Int")))
}
