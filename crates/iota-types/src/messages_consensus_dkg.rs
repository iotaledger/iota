// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! DKG (distributed key generation) types and helpers for [`super`].
//!
//! These pull in `fastcrypto-tbls` (BLS12-381 threshold signatures), which
//! isn't available on `wasm32-unknown-unknown`, so the whole module is gated
//! out of the wasm build.

use std::{
    collections::hash_map::DefaultHasher,
    fmt::{Debug, Formatter},
    hash::{Hash, Hasher},
    sync::Arc,
};

use fastcrypto::{error::FastCryptoResult, groups::bls12381};
use fastcrypto_tbls::dkg_v1;
use serde::{Deserialize, Serialize};

use super::{AuthorityName, ConsensusTransaction, ConsensusTransactionKind};

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VersionedDkgMessage {
    V1(dkg_v1::Message<bls12381::G2Element, bls12381::G2Element>),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VersionedDkgConfirmation {
    V1(dkg_v1::Confirmation<bls12381::G2Element>),
}

impl Debug for VersionedDkgMessage {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            VersionedDkgMessage::V1(msg) => write!(
                f,
                "DKG V1 Message with sender={}, vss_pk.degree={}, encrypted_shares.len()={}",
                msg.sender,
                msg.vss_pk.degree(),
                msg.encrypted_shares.len(),
            ),
        }
    }
}

impl VersionedDkgMessage {
    pub fn sender(&self) -> u16 {
        match self {
            VersionedDkgMessage::V1(msg) => msg.sender,
        }
    }

    pub fn create(
        dkg_version: u64,
        party: Arc<dkg_v1::Party<bls12381::G2Element, bls12381::G2Element>>,
    ) -> FastCryptoResult<VersionedDkgMessage> {
        assert_eq!(dkg_version, 1, "BUG: invalid DKG version");
        let msg = party.create_message(&mut rand::thread_rng())?;
        Ok(VersionedDkgMessage::V1(msg))
    }

    pub fn unwrap_v1(self) -> dkg_v1::Message<bls12381::G2Element, bls12381::G2Element> {
        match self {
            VersionedDkgMessage::V1(msg) => msg,
        }
    }

    pub fn is_valid_version(&self, dkg_version: u64) -> bool {
        matches!((self, dkg_version), (VersionedDkgMessage::V1(_), 1))
    }
}

impl VersionedDkgConfirmation {
    pub fn sender(&self) -> u16 {
        match self {
            VersionedDkgConfirmation::V1(msg) => msg.sender,
        }
    }

    pub fn num_of_complaints(&self) -> usize {
        match self {
            VersionedDkgConfirmation::V1(msg) => msg.complaints.len(),
        }
    }

    pub fn unwrap_v1(&self) -> &dkg_v1::Confirmation<bls12381::G2Element> {
        match self {
            VersionedDkgConfirmation::V1(msg) => msg,
        }
    }

    pub fn is_valid_version(&self, dkg_version: u64) -> bool {
        matches!((self, dkg_version), (VersionedDkgConfirmation::V1(_), 1))
    }
}

impl ConsensusTransaction {
    pub fn new_randomness_dkg_message(
        authority: AuthorityName,
        versioned_message: &VersionedDkgMessage,
    ) -> Self {
        let message =
            bcs::to_bytes(versioned_message).expect("message serialization should not fail");
        let mut hasher = DefaultHasher::new();
        message.hash(&mut hasher);
        let tracking_id = hasher.finish().to_le_bytes();
        Self {
            tracking_id,
            kind: ConsensusTransactionKind::RandomnessDkgMessage(authority, message),
        }
    }

    pub fn new_randomness_dkg_confirmation(
        authority: AuthorityName,
        versioned_confirmation: &VersionedDkgConfirmation,
    ) -> Self {
        let confirmation =
            bcs::to_bytes(versioned_confirmation).expect("message serialization should not fail");
        let mut hasher = DefaultHasher::new();
        confirmation.hash(&mut hasher);
        let tracking_id = hasher.finish().to_le_bytes();
        Self {
            tracking_id,
            kind: ConsensusTransactionKind::RandomnessDkgConfirmation(authority, confirmation),
        }
    }
}
