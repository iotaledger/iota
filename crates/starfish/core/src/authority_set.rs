// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeSet, fmt};

use serde::{Deserialize, Serialize};
use starfish_config::AuthorityIndex;

/// Compact bitmask representing a subset of authorities.
/// Supports up to 256 authorities (AuthorityIndex is u8).
#[derive(Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AuthoritySet([u64; 4]);

impl AuthoritySet {
    /// Creates an empty authority set.
    pub fn new() -> Self {
        Self([0; 4])
    }

    /// Creates a set with two authorities pre-inserted.
    pub fn new_with(a: AuthorityIndex, b: AuthorityIndex) -> Self {
        let mut s = Self::new();
        s.insert(a);
        s.insert(b);
        s
    }

    /// Inserts an authority into the set. Returns true if the authority was
    /// not already present.
    pub fn insert(&mut self, index: AuthorityIndex) -> bool {
        let i = index.value();
        let array_index = i / 64;
        let bit_pos = i % 64;
        let mask = 1u64 << bit_pos;
        let already_present = (self.0[array_index] & mask) != 0;
        self.0[array_index] |= mask;
        !already_present
    }

    /// Returns true if the set contains the given authority.
    pub fn contains(&self, index: AuthorityIndex) -> bool {
        let i = index.value();
        let array_index = i / 64;
        let bit_pos = i % 64;
        (self.0[array_index] & (1u64 << bit_pos)) != 0
    }

    /// Returns true if the set is empty.
    pub fn is_empty(&self) -> bool {
        self.0.iter().all(|&bits| bits == 0)
    }

    /// Returns the number of authorities in the set.
    pub fn len(&self) -> usize {
        self.0.iter().map(|bits| bits.count_ones() as usize).sum()
    }

    /// Iterates over the authority indices in the set, in ascending order.
    pub fn iter(&self) -> impl Iterator<Item = AuthorityIndex> + '_ {
        self.0.iter().enumerate().flat_map(|(array_index, &bits)| {
            let base = array_index * 64;
            BitIter(bits).map(move |bit| AuthorityIndex::from((base + bit) as u8))
        })
    }

    /// Converts to a BTreeSet of AuthorityIndex.
    pub fn to_btreeset(self) -> BTreeSet<AuthorityIndex> {
        self.iter().collect()
    }
}

impl From<&BTreeSet<AuthorityIndex>> for AuthoritySet {
    fn from(set: &BTreeSet<AuthorityIndex>) -> Self {
        let mut result = Self::new();
        for &index in set {
            result.insert(index);
        }
        result
    }
}

impl fmt::Debug for AuthoritySet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let indices: Vec<_> = self.iter().collect();
        write!(f, "AuthoritySet({indices:?})")
    }
}

/// Iterator over set bits in a u64.
struct BitIter(u64);

impl Iterator for BitIter {
    type Item = usize;

    fn next(&mut self) -> Option<usize> {
        if self.0 == 0 {
            return None;
        }
        let bit = self.0.trailing_zeros() as usize;
        self.0 &= self.0 - 1;
        Some(bit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_set() {
        let set = AuthoritySet::new();
        assert!(set.is_empty());
        assert_eq!(set.len(), 0);
        assert_eq!(set.iter().count(), 0);
    }

    #[test]
    fn test_insert_and_contains() {
        let mut set = AuthoritySet::new();
        let idx = AuthorityIndex::new_for_test(5);

        assert!(!set.contains(idx));
        assert!(set.insert(idx));
        assert!(set.contains(idx));
        assert!(!set.insert(idx)); // duplicate
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn test_across_buckets() {
        let mut set = AuthoritySet::new();
        set.insert(AuthorityIndex::new_for_test(0));
        set.insert(AuthorityIndex::new_for_test(63));
        set.insert(AuthorityIndex::new_for_test(64));
        set.insert(AuthorityIndex::new_for_test(127));
        set.insert(AuthorityIndex::new_for_test(128));
        set.insert(AuthorityIndex::new_for_test(255));

        assert_eq!(set.len(), 6);
        assert!(set.contains(AuthorityIndex::new_for_test(0)));
        assert!(set.contains(AuthorityIndex::new_for_test(255)));
        assert!(!set.contains(AuthorityIndex::new_for_test(1)));
    }

    #[test]
    fn test_iter_order() {
        let mut set = AuthoritySet::new();
        set.insert(AuthorityIndex::new_for_test(200));
        set.insert(AuthorityIndex::new_for_test(3));
        set.insert(AuthorityIndex::new_for_test(100));
        set.insert(AuthorityIndex::new_for_test(65));

        let indices: Vec<_> = set.iter().map(|i| i.value()).collect();
        assert_eq!(indices, vec![3, 65, 100, 200]);
    }

    #[test]
    fn test_new_with() {
        let a = AuthorityIndex::new_for_test(3);
        let b = AuthorityIndex::new_for_test(7);
        let set = AuthoritySet::new_with(a, b);
        assert_eq!(set.len(), 2);
        assert!(set.contains(a));
        assert!(set.contains(b));
    }

    #[test]
    fn test_btreeset_roundtrip() {
        let mut btree = BTreeSet::new();
        btree.insert(AuthorityIndex::new_for_test(10));
        btree.insert(AuthorityIndex::new_for_test(50));
        btree.insert(AuthorityIndex::new_for_test(200));

        let authority_set = AuthoritySet::from(&btree);
        assert_eq!(authority_set.to_btreeset(), btree);
    }
}
