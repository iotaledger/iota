// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::atomic::AtomicU64;

use serde::{Deserialize, Serialize};

// MisbehaviorsV1 contains lists of all metrics used in v1 of the validator
// scoring mechanism, with a value for each metric. The metrics (misbeheaviors)
// include, faulty blocks, equivocation and missing proposal counts for each
// authority. This first version does not include any type of proof. Any metric
// contained in this struct must be guaranteed to be monotonically increasing,
// because of the way updates are applied from reports.
//
// The type parameter `T` determines the storage type for the metrics:
// - `T = Vec<u64>` for reports (one value per authority)
// - `T = Vec<AtomicU64>` for atomic metrics collected and stored locally (one
//   atomic per authority)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MisbehaviorsV1<T> {
    faulty_blocks_provable: T,
    faulty_blocks_unprovable: T,
    missing_proposals: T,
    equivocations: T,
}

impl<T> MisbehaviorsV1<T> {
    pub fn new(
        faulty_blocks_provable: T,
        faulty_blocks_unprovable: T,
        missing_proposals: T,
        equivocations: T,
    ) -> Self {
        Self {
            faulty_blocks_provable,
            faulty_blocks_unprovable,
            missing_proposals,
            equivocations,
        }
    }

    // Returns a reference to the faulty_blocks_provable field.
    pub fn faulty_blocks_provable(&self) -> &T {
        &self.faulty_blocks_provable
    }

    // Returns a reference to the faulty_blocks_unprovable field.
    pub fn faulty_blocks_unprovable(&self) -> &T {
        &self.faulty_blocks_unprovable
    }

    // Returns a reference to the missing_proposals field.
    pub fn missing_proposals(&self) -> &T {
        &self.missing_proposals
    }

    // Returns a reference to the equivocations field.
    pub fn equivocations(&self) -> &T {
        &self.equivocations
    }

    // Returns an iterator over references to all misbehavior fields.
    pub fn iter(&self) -> std::vec::IntoIter<&T> {
        vec![
            &self.faulty_blocks_provable,
            &self.faulty_blocks_unprovable,
            &self.missing_proposals,
            &self.equivocations,
        ]
        .into_iter()
    }

    // Returns an iterator over references to major misbehavior fields.
    // Major misbehaviors carry a higher penalty in the scoring system.
    pub fn iter_major_misbehaviors(&self) -> std::vec::IntoIter<&T> {
        vec![&self.equivocations].into_iter()
    }

    // Returns an iterator over references to minor misbehavior fields.
    // Minor misbehaviors carry a lower penalty in the scoring system.
    pub fn iter_minor_misbehaviors(&self) -> std::vec::IntoIter<&T> {
        vec![
            &self.faulty_blocks_provable,
            &self.faulty_blocks_unprovable,
            &self.missing_proposals,
        ]
        .into_iter()
    }
}

impl<T> FromIterator<T> for MisbehaviorsV1<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut iterator = iter.into_iter();
        Self {
            faulty_blocks_provable: iterator.next().expect("Not enough elements in iterator"),
            faulty_blocks_unprovable: iterator.next().expect("Not enough elements in iterator"),
            missing_proposals: iterator.next().expect("Not enough elements in iterator"),
            equivocations: iterator.next().expect("Not enough elements in iterator"),
        }
    }
}

impl MisbehaviorsV1<u64> {
    pub fn new_zeroed() -> Self {
        Self::new(0, 0, 0, 0)
    }
}

impl MisbehaviorsV1<Vec<u64>> {
    pub fn new_zeroed(committee_size: usize) -> Self {
        Self::new(
            (0..committee_size).map(|_| 0).collect(),
            (0..committee_size).map(|_| 0).collect(),
            (0..committee_size).map(|_| 0).collect(),
            (0..committee_size).map(|_| 0).collect(),
        )
    }

    // Verifies that all fields have the expected committee size.
    pub fn verify(&self, committee_size: usize) -> bool {
        self.iter().all(|metric| metric.len() == committee_size)
    }

    pub fn as_atomic(&self) -> MisbehaviorsV1<Vec<AtomicU64>> {
        self.iter()
            .map(|metric| {
                metric
                    .iter()
                    .map(|&x| AtomicU64::new(x))
                    .collect::<Vec<AtomicU64>>()
            })
            .collect::<MisbehaviorsV1<Vec<AtomicU64>>>()
    }

    pub fn misbehaviors_from_authority(&self, authority: usize) -> MisbehaviorsV1<u64> {
        self.iter()
            .map(|metric| metric[authority])
            .collect::<MisbehaviorsV1<u64>>()
    }
}

impl MisbehaviorsV1<Vec<AtomicU64>> {
    pub fn new_zeroed(committee_size: usize) -> Self {
        Self::new(
            (0..committee_size).map(|_| AtomicU64::new(0)).collect(),
            (0..committee_size).map(|_| AtomicU64::new(0)).collect(),
            (0..committee_size).map(|_| AtomicU64::new(0)).collect(),
            (0..committee_size).map(|_| AtomicU64::new(0)).collect(),
        )
    }

    pub fn as_non_atomic(&self) -> MisbehaviorsV1<Vec<u64>> {
        self.iter()
            .map(|metric| {
                metric
                    .iter()
                    .map(|x| x.load(std::sync::atomic::Ordering::Relaxed))
                    .collect::<Vec<u64>>()
            })
            .collect::<MisbehaviorsV1<Vec<u64>>>()
    }

    pub fn misbehaviors_from_authority(&self, authority: usize) -> MisbehaviorsV1<u64> {
        self.iter()
            .map(|metric| metric[authority].load(std::sync::atomic::Ordering::Relaxed))
            .collect::<MisbehaviorsV1<u64>>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iter_u64() {
        let original = MisbehaviorsV1::new(1_u64, 2, 3, 4);
        let new: MisbehaviorsV1<u64> = original.iter().copied().collect();
        assert_eq!(original, new);
    }

    #[test]
    #[should_panic]
    fn test_iter_u64_major() {
        let original = MisbehaviorsV1::new(1_u64, 2, 3, 4);
        let _: MisbehaviorsV1<u64> = original.iter_major_misbehaviors().copied().collect();
    }

    #[test]
    #[should_panic]
    fn test_iter_u64_minor() {
        let original = MisbehaviorsV1::new(1_u64, 2, 3, 4);
        let _: MisbehaviorsV1<u64> = original.iter_minor_misbehaviors().copied().collect();
    }

    #[test]
    fn test_iter_vec_u64() {
        let original = MisbehaviorsV1::new(
            vec![1_u64, 2, 3],
            vec![4, 5, 6],
            vec![7, 8, 9],
            vec![10, 11, 12],
        );
        let new: MisbehaviorsV1<Vec<u64>> = original.iter().cloned().collect();
        assert_eq!(original, new);
    }
    #[test]
    #[should_panic]
    fn test_iter_vec_u64_major() {
        let original = MisbehaviorsV1::new(
            vec![1_u64, 2, 3],
            vec![4, 5, 6],
            vec![7, 8, 9],
            vec![10, 11, 12],
        );
        let _: MisbehaviorsV1<Vec<u64>> = original.iter_major_misbehaviors().cloned().collect();
    }
    #[test]
    #[should_panic]
    fn test_iter_vec_u64_minor() {
        let original = MisbehaviorsV1::new(
            vec![1_u64, 2, 3],
            vec![4, 5, 6],
            vec![7, 8, 9],
            vec![10, 11, 12],
        );
        let _: MisbehaviorsV1<Vec<u64>> = original.iter_minor_misbehaviors().cloned().collect();
    }
}
