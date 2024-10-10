// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub(crate) mod consensus_output_api;

/// An unique integer ID for a validator used by consensus.
/// In Mysticeti, this is used the same way as the AuthorityIndex type there.
pub type AuthorityIndex = u32;
