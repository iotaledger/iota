// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use derive_more::{Deref, From};

impl_id!(
    pub TransactionId,
    32,
    "A transaction identifier, the BLAKE2b-256 hash of the transaction bytes. See <https://www.blake2.net/> for more information."
);

#[cfg(feature = "serde")]
string_serde_impl!(TransactionId);

impl_id!(
    pub BlockId,
    32,
    "A block identifier, the BLAKE2b-256 hash of the block bytes. See <https://www.blake2.net/> for more information."
);

#[cfg(feature = "serde")]
string_serde_impl!(BlockId);

impl_id!(
    pub MilestoneId,
    32,
    "A milestone identifier, the BLAKE2b-256 hash of the milestone bytes. See <https://www.blake2.net/> for more information."
);

#[cfg(feature = "serde")]
string_serde_impl!(MilestoneId);

/// A wrapper around a `u32` that represents a milestone index.
#[repr(transparent)]
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    From,
    Deref,
    packable::Packable,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MilestoneIndex(pub u32);

impl MilestoneIndex {
    /// Creates a new [`MilestoneIndex`].
    pub fn new(index: u32) -> Self {
        Self(index)
    }
}
