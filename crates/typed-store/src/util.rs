// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::ops::{Bound, RangeBounds};

use bincode::Options;
use serde::Serialize;

#[inline]
pub fn be_fix_int_ser<S>(t: &S) -> Vec<u8>
where
    S: ?Sized + serde::Serialize,
{
    bincode::DefaultOptions::new()
        .with_big_endian()
        .with_fixint_encoding()
        .serialize(t)
        .expect("failed to serialize via be_fix_int_ser method")
}

pub(crate) fn iterator_bounds_with_range<K>(
    range: impl RangeBounds<K>,
) -> (Option<Vec<u8>>, Option<Vec<u8>>)
where
    K: Serialize,
{
    let iterator_lower_bound = match range.start_bound() {
        Bound::Included(lower_bound) => {
            // Rocksdb lower bound is inclusive by default so nothing to do
            Some(be_fix_int_ser(&lower_bound))
        }
        Bound::Excluded(lower_bound) => {
            let mut key_buf = be_fix_int_ser(&lower_bound);

            if is_max(&key_buf) {
                // No representable key strictly greater than the maximum at this byte
                // length. Append a zero byte so the lower bound is lexicographically
                // greater than any same-length key, ensuring the iterator yields
                // nothing — matching the user's intent of excluding the max key.
                key_buf.push(0);
            } else {
                // Since we want exclusive, we need to increment the key to exclude the previous
                big_endian_add_one(&mut key_buf);
            }
            Some(key_buf)
        }
        Bound::Unbounded => None,
    };
    let iterator_upper_bound = match range.end_bound() {
        Bound::Included(upper_bound) => {
            let mut key_buf = be_fix_int_ser(&upper_bound);

            if is_max(&key_buf) {
                // No same-length key is greater than the maximum, but a longer key
                // extending it still is. Append a zero byte so the (exclusive) upper
                // bound is the first such key, keeping any extension out of an
                // inclusive `..=max` range.
                key_buf.push(0);
            } else {
                // Rocksdb upper bound is exclusive, so increment to make the
                // caller's inclusive bound inclusive.
                big_endian_add_one(&mut key_buf);
            }
            Some(key_buf)
        }
        Bound::Excluded(upper_bound) => {
            // Rocksdb upper bound is exclusive by default so nothing to do
            Some(be_fix_int_ser(&upper_bound))
        }
        Bound::Unbounded => None,
    };
    (iterator_lower_bound, iterator_upper_bound)
}

/// Computes the raw byte bounds for a prefix scan over keys serialized with
/// [`be_fix_int_ser`].
///
/// `prefix` must serialize to a prefix of the column family's key encoding —
/// typically the leading field(s) of a tuple key. Returns the half-open byte
/// range `[ser(prefix), ser(prefix) + 1)`, which selects exactly the keys that
/// begin with `ser(prefix)`. When the serialized prefix is all `0xFF` there is
/// no representable upper bound, so the scan runs to the end of the column
/// family.
pub(crate) fn prefix_iterator_bounds<P>(prefix: &P) -> (Option<Vec<u8>>, Option<Vec<u8>>)
where
    P: ?Sized + Serialize,
{
    let lower = be_fix_int_ser(prefix);
    let upper = if is_max(&lower) {
        None
    } else {
        let mut upper = lower.clone();
        big_endian_add_one(&mut upper);
        Some(upper)
    };
    (Some(lower), upper)
}

/// Increments the big-endian integer in `v` by one, in place.
///
/// Callers must ensure `v` is not all-`0xFF` (each one checks `is_max` and
/// handles that case separately).
///
/// # Panics
///
/// Panics if `v` is all-`0xFF`. Incrementing it would overflow, and silently
/// wrapping to zero would emit a wrong key; panicking instead keeps a buggy
/// caller from corrupting the keyspace.
fn big_endian_add_one(v: &mut [u8]) {
    for i in (0..v.len()).rev() {
        if v[i] == u8::MAX {
            v[i] = 0;
        } else {
            v[i] += 1;
            return;
        }
    }
    // Reaching here means no byte could be incremented, i.e. `v` was all-`0xFF`
    // and the carry ran off the top — a violated precondition. Panic rather than
    // return a silently-wrapped, incorrect key.
    unreachable!("big_endian_add_one called on an all-0xFF value")
}

/// Check if all the bytes in the vector are 0xFF
fn is_max(v: &[u8]) -> bool {
    v.iter().all(|&x| x == u8::MAX)
}

#[expect(clippy::assign_op_pattern, clippy::manual_div_ceil)]
#[test]
fn test_helpers() {
    let v = vec![];
    assert!(is_max(&v));

    fn check_add(v: Vec<u8>) {
        let mut v = v;
        let num = Num32::from_big_endian(&v);
        big_endian_add_one(&mut v);
        assert!(num + 1 == Num32::from_big_endian(&v));
    }

    uint::construct_uint! {
        // 32 byte number
        struct Num32(4);
    }

    check_add(vec![1; 32]);
    check_add(vec![6; 32]);
    check_add(vec![254; 32]);

    // TBD: More tests coming with randomized arrays
}

#[test]
#[should_panic(expected = "all-0xFF")]
fn big_endian_add_one_panics_on_max() {
    big_endian_add_one(&mut [0xFFu8; 4]);
}

#[test]
fn test_inclusive_upper_bound_at_max() {
    // The missed case: an inclusive upper bound whose serialization is all-`0xFF`.
    // The exclusive rocksdb upper bound must be the first key PAST it
    // (`ser(hi) ++ [0]`), not `None`. With `None` the scan is unbounded, so a
    // longer key that extends the max (and therefore sorts *after* it) is wrongly
    // included in `..=hi`. Verify the actual inclusion/exclusion the bound yields,
    // for single-byte, multi-byte and array maxima.
    fn check_max<K: Serialize>(max: K) {
        let hi = be_fix_int_ser(&max);
        assert!(is_max(&hi), "test value must serialize to all-0xFF");
        let (_, upper) = iterator_bounds_with_range::<K>((Bound::Unbounded, Bound::Included(max)));
        let upper = upper.expect("inclusive upper at the max must be bounded, not None");

        // `hi` itself stays inside the (exclusive) bound; a key extending it does not.
        let mut extension = hi.clone();
        extension.push(0);
        assert!(hi < upper, "the max key itself must stay in range");
        assert!(
            extension >= upper,
            "a key extending the max must be excluded"
        );
        assert_eq!(upper, extension, "bound must be exactly ser(hi) ++ [0]");
    }
    check_max(u8::MAX);
    check_max(u64::MAX);
    check_max([0xFFu8; 32]);

    // Symmetric with the excluded-lower arm at the max.
    let (lower, _) = iterator_bounds_with_range::<u8>((Bound::Excluded(u8::MAX), Bound::Unbounded));
    assert_eq!(lower, Some(vec![0xFF, 0x00]));
}
