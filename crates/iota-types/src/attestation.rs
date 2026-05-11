// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Serialize};

use crate::{
    base_types::{IotaAddress, ObjectRef},
    signature::GenericSignature,
    transaction::Transaction,
};

/// A pre-consensus claim produced by a trusted actor certifying that a specific
/// transaction has been validated before entering consensus. The attestation is
/// a separate artifact that travels alongside the transaction; the transaction
/// and the user's signature are completely unchanged.
///
/// Two variants are supported:
/// - [`Attestation::Validator`]: produced by the block-proposing validator.
///   Authenticated implicitly by the block signature — no separate attestor
///   signature is needed.
/// - [`Attestation::Explicit`]: produced by a registered third-party attestor.
///   Requires a signature binding the attestation to the transaction.
#[non_exhaustive]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Attestation {
    Validator(AttestationData),
    Explicit {
        data: AttestationData,
        attestor_address: IotaAddress,
        /// Signs over `hash(transaction.digest() || BCS(data) ||
        /// attestor_address)`, binding the attestation to both the
        /// specific transaction and the attestor's identity.
        signature: GenericSignature,
    },
}

/// The attested content carried by all [`Attestation`] variants.
///
/// Versioned to allow new fields to be introduced without breaking existing
/// match arms. Both `Validator` and `Explicit` share the same `AttestationData`
/// so any extension applies uniformly across attestation types.
#[non_exhaustive]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AttestationData {
    V1 {
        /// Expected computation cost, in MIST, used by the sequencer to improve
        /// shared-object scheduling before execution.
        estimated_computation_cost: u64,
        /// Shared-object versions observed by the attestor during the dry-run.
        /// Used to determine whether a discrepancy between the attested
        /// estimate and the actual execution outcome is misbehavior or is
        /// explained by a legitimate state change between attestation time and
        /// execution time.
        object_versions: Vec<ObjectRef>,
    },
}

/// A user transaction bundled with its attestation. This is the inner payload
/// of `ConsensusTransactionKind::UserTransactionV2`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AttestedTransaction {
    pub transaction: Box<Transaction>,
    pub attestation: Attestation,
}

impl AttestedTransaction {
    pub fn new(transaction: Transaction, attestation: Attestation) -> Self {
        Self {
            transaction: Box::new(transaction),
            attestation,
        }
    }
}
