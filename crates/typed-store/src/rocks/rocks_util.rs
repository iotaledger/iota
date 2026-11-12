// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use crate::{
    rocks::options::ReadWriteOptions,
    util::{big_endian_add_one, is_max},
};

/// Read options for an iterator over `[lower_bound, upper_bound)`, both
/// already serialized. A reverse iterator gets only the lower bound, since
/// its initial seek to the upper bound enforces that one.
///
/// With a prefix extractor installed, the iterator uses the prefix bloom
/// filters only when the scan provably stays within one prefix, and seeks in
/// total order otherwise.
pub(crate) fn iterator_read_options(
    opts: &ReadWriteOptions,
    lower_bound: Option<Vec<u8>>,
    upper_bound: Option<Vec<u8>>,
    reverse: bool,
) -> rocksdb::ReadOptions {
    let mut read_options = opts.readopts();
    if let Some(prefix_len) = opts.prefix_len {
        let within_prefix = scan_stays_within_prefix(
            prefix_len,
            lower_bound.as_deref(),
            upper_bound.as_deref(),
            reverse,
        );
        read_options.set_total_order_seek(!within_prefix);
    }
    let upper_bound = if reverse { None } else { upper_bound };
    apply_range_bounds(read_options, lower_bound, upper_bound)
}

/// Whether every key in `[lower_bound, upper_bound)` shares its first
/// `prefix_len` bytes with the key the scan seeks to. An upper bound that is
/// exactly the next prefix ends a forward scan at the end of the prefix, so
/// it qualifies there, but not for a reverse scan, which seeks to it.
pub(crate) fn scan_stays_within_prefix(
    prefix_len: usize,
    lower_bound: Option<&[u8]>,
    upper_bound: Option<&[u8]>,
    reverse: bool,
) -> bool {
    let (Some(lower), Some(upper)) = (lower_bound, upper_bound) else {
        return false;
    };
    if lower.len() < prefix_len {
        return false;
    }
    let prefix = &lower[..prefix_len];
    if upper.len() >= prefix_len && &upper[..prefix_len] == prefix {
        return true;
    }
    if reverse || is_max(prefix) {
        return false;
    }
    let mut next_prefix = prefix.to_vec();
    big_endian_add_one(&mut next_prefix);
    upper == next_prefix.as_slice()
}

pub(crate) fn apply_range_bounds(
    mut read_options: rocksdb::ReadOptions,
    lower_bound: Option<Vec<u8>>,
    upper_bound: Option<Vec<u8>>,
) -> rocksdb::ReadOptions {
    if let Some(lower_bound) = lower_bound {
        read_options.set_iterate_lower_bound(lower_bound);
    }
    if let Some(upper_bound) = upper_bound {
        read_options.set_iterate_upper_bound(upper_bound);
    }
    read_options
}
