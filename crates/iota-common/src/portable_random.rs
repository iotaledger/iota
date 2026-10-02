// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Sampling routines whose results depend only on the bytes an RNG produces,
//! never on which version of `rand` is in the dependency graph.
//!
//! `rand`'s own `shuffle`, `choose`, `choose_multiple` and
//! `choose_multiple_weighted` are free to change between releases, and they
//! have: 0.9 replaced the range sampler and the weighted-selection wrapper, so
//! the same seed produces a different permutation. That is fine for a test
//! fixture and unacceptable for a permutation every validator has to agree on.
//!
//! The routines here reproduce `rand` 0.8's algorithms and are fixed by
//! tests against captured vectors, so callers that need a permutation to
//! stay the same forever can bump `rand` freely.
//!
//! Two rules for callers:
//!
//! - Seed [`rand::rngs::ChaCha12Rng`], not `StdRng`. `StdRng` is documented as
//!   non-portable — `rand` reserves the right to swap the algorithm under it —
//!   whereas `ChaCha12Rng` names the generator and is what `StdRng` currently
//!   happens to be.
//! - Draw floats through [`f64_unit`] rather than `rand`'s `random::<f64>()`,
//!   which is another conversion `rand` owns and could change.

use rand::Rng;

/// Draws a `f64` uniformly from `[0, 1)`, using the top 53 bits of one 64-bit
/// word.
pub fn f64_unit<R: Rng + ?Sized>(rng: &mut R) -> f64 {
    const SCALE: f64 = 1.0 / ((1u64 << 53) as f64);
    SCALE * ((rng.next_u64() >> 11) as f64)
}

/// Draws a `u32` uniformly from `low..=high`.
///
/// Lemire's multiply-shift with rejection: multiply a random word by the range
/// width and keep the high half, rejecting the low half's biased tail.
fn uniform_u32_inclusive<R: Rng + ?Sized>(rng: &mut R, low: u32, high: u32) -> u32 {
    let range = high.wrapping_sub(low).wrapping_add(1);
    if range == 0 {
        // `low..=high` spans the whole of u32, so every word is in range.
        return rng.next_u32();
    }
    let zone = (range << range.leading_zeros()).wrapping_sub(1);
    loop {
        let product = u64::from(rng.next_u32()) * u64::from(range);
        let (hi, lo) = ((product >> 32) as u32, product as u32);
        if lo <= zone {
            return low.wrapping_add(hi);
        }
    }
}

/// Draws an index uniformly from `0..ubound`.
///
/// Sampling is done in 32 bits so that the result does not depend on the width
/// of `usize`. Panics if `ubound` is 0 or exceeds `u32::MAX`.
pub fn index<R: Rng + ?Sized>(rng: &mut R, ubound: usize) -> usize {
    assert!(ubound > 0, "cannot draw an index from an empty range");
    assert!(
        ubound <= u32::MAX as usize,
        "index sampling is limited to u32::MAX to stay independent of usize width"
    );
    uniform_u32_inclusive(rng, 0, ubound as u32 - 1) as usize
}

/// Permutes `slice` in place (Fisher-Yates, from the back).
pub fn shuffle<T, R: Rng + ?Sized>(rng: &mut R, slice: &mut [T]) {
    for i in (1..slice.len()).rev() {
        slice.swap(i, index(rng, i + 1));
    }
}

/// Returns a uniformly drawn element, or `None` if `slice` is empty.
pub fn choose<'a, T, R: Rng + ?Sized>(rng: &mut R, slice: &'a [T]) -> Option<&'a T> {
    if slice.is_empty() {
        None
    } else {
        Some(&slice[index(rng, slice.len())])
    }
}

/// Draws `amount` distinct indices from `0..length`, in random order.
///
/// Which of the two algorithms runs is decided by `length` and `amount` alone,
/// so it is as reproducible as the draws themselves. Panics if `amount`
/// exceeds `length`, or if the pair falls in the range where `rand` 0.8 used a
/// third algorithm — unreachable for `amount == length`, which is how every
/// caller here uses it, and better to fail loudly than to diverge quietly.
pub fn sample_indices<R: Rng + ?Sized>(rng: &mut R, length: usize, amount: usize) -> Vec<usize> {
    assert!(
        amount <= length,
        "cannot sample more indices than are available"
    );
    assert!(
        length <= u32::MAX as usize,
        "index sampling is limited to u32::MAX to stay independent of usize width"
    );
    let (amount, length) = (amount as u32, length as u32);

    // Thresholds carried over verbatim from `rand` 0.8; they trade set-up cost
    // against draw count and have no meaning beyond picking the cheaper
    // algorithm.
    if amount < 163 {
        let scale = if length < 500_000 {
            (1.6, 10.0)
        } else {
            (8.0 / 45.0, 70.0 / 9.0)
        };
        let amount_fp = amount as f32;
        if amount > 11 && (length as f32) < (scale.1 + scale.0 * amount_fp) * amount_fp {
            sample_in_place(rng, length, amount)
        } else {
            sample_floyd(rng, length, amount)
        }
    } else {
        let scale = if length < 500_000 { 270.0 } else { 330.0 / 9.0 };
        assert!(
            (length as f32) < scale * (amount as f32),
            "sampling {amount} of {length} indices needs the algorithm this module does not carry"
        );
        sample_in_place(rng, length, amount)
    }
}

/// Floyd's combination algorithm, fully shuffled.
fn sample_floyd<R: Rng + ?Sized>(rng: &mut R, length: u32, amount: u32) -> Vec<usize> {
    // Floyd's insert keeps the result shuffled as it goes, but `Vec::insert`
    // gets expensive, so past this size the shuffle is done in one pass after.
    let shuffle_while_inserting = amount < 50;

    let mut indices: Vec<u32> = Vec::with_capacity(amount as usize);
    for j in length - amount..length {
        let t = uniform_u32_inclusive(rng, 0, j);
        if shuffle_while_inserting {
            if let Some(pos) = indices.iter().position(|&x| x == t) {
                indices.insert(pos, j);
                continue;
            }
        } else if indices.contains(&t) {
            indices.push(j);
            continue;
        }
        indices.push(t);
    }
    if !shuffle_while_inserting {
        for i in (1..amount).rev() {
            let j = uniform_u32_inclusive(rng, 0, i) as usize;
            indices.swap(i as usize, j);
        }
    }
    indices.into_iter().map(|i| i as usize).collect()
}

/// Partial Fisher-Yates over the whole index range.
fn sample_in_place<R: Rng + ?Sized>(rng: &mut R, length: u32, amount: u32) -> Vec<usize> {
    let mut indices: Vec<u32> = (0..length).collect();
    for i in 0..amount {
        let j = uniform_u32_inclusive(rng, i, length - 1);
        indices.swap(i as usize, j as usize);
    }
    indices.truncate(amount as usize);
    indices.into_iter().map(|i| i as usize).collect()
}

/// Draws `amount` distinct indices from `0..length` with probability
/// proportional to weight, in descending order of the keys drawn.
///
/// Efraimidis and Spirakis, <https://doi.org/10.1016/j.ipl.2005.11.003>: give
/// index `i` the key `u^(1/w_i)` for a uniform `u`, then take the largest keys.
/// Passing `amount == length` therefore yields a weighted permutation of every
/// index.
///
/// Panics if any weight is negative or not a number.
pub fn sample_weighted<R, F>(rng: &mut R, length: usize, weight: F, amount: usize) -> Vec<usize>
where
    R: Rng + ?Sized,
    F: Fn(usize) -> f64,
{
    let amount = amount.min(length);
    // `rand` 0.8 pushes every key onto a `std` `BinaryHeap` and pops the
    // largest, which decides the order of equal keys. The heap is written out
    // here, following `std`'s push and pop, so that order stays fixed whatever
    // `std` does later.
    let mut heap: Vec<(f64, usize)> = Vec::with_capacity(length);
    for i in 0..length {
        let w = weight(i);
        assert!(
            w >= 0.0,
            "weights must be non-negative and not NaN, got {w}"
        );
        heap.push((f64_unit(rng).powf(1.0 / w), i));
        heap_sift_up(&mut heap, i);
    }
    let mut result = Vec::with_capacity(amount);
    while result.len() < amount {
        let last = heap.pop().expect("heap holds at least `amount` keys");
        let largest = if heap.is_empty() {
            last
        } else {
            let largest = std::mem::replace(&mut heap[0], last);
            heap_sift_down_to_bottom(&mut heap);
            largest
        };
        result.push(largest.1);
    }
    result
}

/// Moves the entry at `pos` towards the root of the max-heap until its parent's
/// key is no smaller, and returns where it ends up.
fn heap_sift_up(heap: &mut [(f64, usize)], mut pos: usize) -> usize {
    let entry = heap[pos];
    while pos > 0 {
        let parent = (pos - 1) / 2;
        if entry.0 <= heap[parent].0 {
            break;
        }
        heap[pos] = heap[parent];
        pos = parent;
    }
    heap[pos] = entry;
    pos
}

/// Restores the max-heap after the root has been replaced: walks the root's
/// entry down to a leaf along the larger children, then back up.
fn heap_sift_down_to_bottom(heap: &mut [(f64, usize)]) {
    let end = heap.len();
    let entry = heap[0];
    let mut pos = 0;
    let mut child = 1;
    while child + 1 < end {
        // On equal keys the right child is taken, as `std` does.
        if heap[child].0 <= heap[child + 1].0 {
            child += 1;
        }
        heap[pos] = heap[child];
        pos = child;
        child = 2 * pos + 1;
    }
    if child + 1 == end {
        heap[pos] = heap[child];
        pos = child;
    }
    heap[pos] = entry;
    heap_sift_up(heap, pos);
}

#[cfg(test)]
mod tests {
    use rand::{SeedableRng, rngs::ChaCha12Rng};

    use super::*;

    fn rng(seed: u8) -> ChaCha12Rng {
        ChaCha12Rng::from_seed([seed; 32])
    }

    /// Vectors captured from `rand` 0.8, whose algorithms these routines
    /// reproduce. They are the permutations consensus already agreed on, so a
    /// change here is a change to the protocol, not a test to re-record.
    #[test]
    fn matches_captured_vectors() {
        let mut v: Vec<u32> = (0..10).collect();
        shuffle(&mut rng(0), &mut v);
        assert_eq!(v, [6, 8, 3, 0, 1, 9, 2, 5, 7, 4]);

        let mut v: Vec<u32> = (0..40).collect();
        shuffle(&mut rng(7), &mut v);
        assert_eq!(v[..8], [13, 36, 32, 11, 39, 30, 6, 24]);

        let items: Vec<u32> = (0..10).collect();
        assert_eq!(choose(&mut rng(0), &items), Some(&4));
        assert_eq!(choose(&mut rng(1), &items), Some(&8));

        // Under 12 indices draws Floyd's, over it draws partial Fisher-Yates,
        // and 50 crosses Floyd's shuffle-as-you-go cutoff.
        assert_eq!(
            sample_indices(&mut rng(0), 10, 10),
            [1, 8, 4, 9, 7, 6, 3, 0, 5, 2]
        );
        assert_eq!(
            sample_indices(&mut rng(0), 20, 20),
            [
                8, 7, 3, 17, 5, 14, 11, 13, 2, 6, 0, 1, 18, 12, 4, 16, 19, 15, 10, 9
            ]
        );
        assert_eq!(
            sample_indices(&mut rng(3), 63, 63)[..8],
            [32, 62, 58, 27, 54, 2, 56, 26]
        );

        let weights = [1u64, 5, 2, 8, 3];
        assert_eq!(
            sample_weighted(
                &mut rng(0),
                weights.len(),
                |i| weights[i] as f64,
                weights.len()
            ),
            [1, 4, 3, 0, 2]
        );
        assert_eq!(
            sample_weighted(&mut rng(9), weights.len(), |i| weights[i] as f64, 2),
            [1, 0]
        );

        // Zero weights all draw the key 0.0; `rand` 0.8 gave them this order,
        // not index order.
        let weights = [0u64, 3, 0, 0, 5, 0, 1, 0];
        assert_eq!(
            sample_weighted(
                &mut rng(0),
                weights.len(),
                |i| weights[i] as f64,
                weights.len()
            ),
            [4, 1, 6, 5, 2, 7, 0, 3]
        );
        assert_eq!(
            sample_weighted(&mut rng(5), weights.len(), |i| weights[i] as f64, 6),
            [1, 4, 6, 5, 2, 7]
        );
    }

    #[test]
    fn f64_unit_lies_in_the_unit_interval() {
        let mut gen = rng(0);
        for _ in 0..10_000 {
            let x = f64_unit(&mut gen);
            assert!((0.0..1.0).contains(&x), "{x} outside [0, 1)");
        }
        // The conversion `rand` performs, for as long as both exist.
        assert_eq!(
            f64_unit(&mut rng(4)),
            rand::RngExt::random::<f64>(&mut rng(4))
        );
    }

    #[test]
    fn sampling_every_index_yields_a_permutation() {
        for len in [1usize, 2, 11, 12, 13, 49, 50, 51, 162, 163, 200] {
            for seed in 0..4 {
                let mut got = sample_indices(&mut rng(seed), len, len);
                got.sort_unstable();
                assert_eq!(got, (0..len).collect::<Vec<_>>(), "len {len} seed {seed}");

                let mut got = sample_weighted(&mut rng(seed), len, |i| (i % 7 + 1) as f64, len);
                got.sort_unstable();
                assert_eq!(got, (0..len).collect::<Vec<_>>(), "len {len} seed {seed}");
            }
        }
    }

    #[test]
    fn zero_weight_sorts_last() {
        let weights = [0u64, 4, 4, 4];
        let order = sample_weighted(&mut rng(2), weights.len(), |i| weights[i] as f64, 4);
        assert_eq!(order.last(), Some(&0));
    }
}
