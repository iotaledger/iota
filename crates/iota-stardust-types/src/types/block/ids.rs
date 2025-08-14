// Copyright 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

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
