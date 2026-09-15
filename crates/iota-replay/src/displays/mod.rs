// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::fmt::{Display, Formatter};

mod gas_status_displays;
pub mod transaction_displays;

pub struct Pretty<'a, T>(pub &'a T);

/// Writes `items` to `f`, joined by `separator`.
///
/// When `delimiters` is given the output is wrapped in them. An empty iterator
/// writes nothing at all, delimiters included.
pub(crate) fn write_sep<T: Display>(
    f: &mut Formatter<'_>,
    items: impl IntoIterator<Item = T>,
    delimiters: Option<(&str, &str)>,
    separator: &str,
) -> std::fmt::Result {
    let mut xs = items.into_iter();
    let Some(x) = xs.next() else {
        return Ok(());
    };
    if let Some((left, _)) = delimiters {
        write!(f, "{left}")?;
    }
    write!(f, "{x}")?;
    for x in xs {
        write!(f, "{separator}{x}")?;
    }
    if let Some((_, right)) = delimiters {
        write!(f, "{right}")?;
    }
    Ok(())
}
