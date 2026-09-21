// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Attestation checks shared by the block verifier, the attested-transaction
//! ingress and post-consensus validation.

use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::{Address, TransactionDigest, UserSignature};
use iota_types::{
    attestation::{Attestation, explicit_attestation_digest},
    error::{IotaError, IotaResult},
    iota_system_state::attestor_registry::{
        AttestorSignatureError, EpochStartAttestorInfoV1, verify_attestor_signature,
    },
    transaction::{TransactionAPI, TransactionEnvelope},
};

use crate::authority::authority_per_epoch_store::AuthorityPerEpochStore;

/// Attested computation units must lie between the protocol floor and what
/// the transaction can pay for.
pub(crate) fn check_attested_units(
    protocol_config: &ProtocolConfig,
    transaction: &TransactionEnvelope,
    attestation: &Attestation,
) -> IotaResult {
    let min_attested_units = protocol_config
        .base_tx_cost_fixed()
        .min(protocol_config.gas_rounding_step());
    let attested_units = attestation.computation_units();
    let txn = transaction.data().transaction();
    let max_attested_units = txn
        .gas_budget()
        .checked_div(txn.gas_price())
        .unwrap_or(u64::MAX);
    if attested_units < min_attested_units {
        return Err(IotaError::AttestationUnitsBelowMinimum {
            actual: attested_units,
            minimum: min_attested_units,
        });
    }
    if attested_units > max_attested_units {
        return Err(IotaError::AttestationUnitsAboveBudget {
            actual: attested_units,
            maximum: max_attested_units,
        });
    }
    Ok(())
}

/// The epoch-start registry entry of an explicit attestation's attestor.
/// Fails when external attestation is disabled or the attestor is not in
/// this epoch's active set.
pub(crate) fn explicit_attestor_entry<'a>(
    epoch_store: &'a AuthorityPerEpochStore,
    attestor_address: &Address,
) -> IotaResult<&'a EpochStartAttestorInfoV1> {
    if !epoch_store.protocol_config().enable_external_attestation() {
        return Err(IotaError::UnsupportedFeature {
            error: "Explicit attestation not supported at current protocol version".into(),
        });
    }
    let attestor_set = epoch_store.attestor_set();
    attestor_set
        .by_address(attestor_address)
        .map(|(_, entry)| entry)
        .ok_or_else(|| IotaError::ExplicitAttestationUnknownAttestor {
            attestor_address: *attestor_address,
            epoch: attestor_set.epoch(),
        })
}

/// Verifies an explicit attestation for `tx_digest`: its attestor must be in
/// this epoch's set and its signature must verify with the registered key. A
/// validator attestation is authenticated by the block signature and passes.
pub(crate) fn verify_explicit_attestation(
    epoch_store: &AuthorityPerEpochStore,
    tx_digest: &TransactionDigest,
    attestation: &Attestation,
) -> IotaResult {
    let Attestation::Explicit {
        payload,
        attestor_address,
        signature,
    } = attestation
    else {
        return Ok(());
    };
    let attestor = explicit_attestor_entry(epoch_store, attestor_address)?;
    let attestor_address = *attestor_address;
    let UserSignature::Simple(signature) = signature.as_ref() else {
        return Err(IotaError::ExplicitAttestationSignatureInvalid {
            attestor_address,
            error: format!("unsupported signature scheme {:?}", signature.scheme()),
        });
    };
    let digest = explicit_attestation_digest(tx_digest, payload, attestor_address);
    verify_attestor_signature(&attestor.attestor_pubkey, signature, &digest).map_err(|e| match e {
        AttestorSignatureError::KeyMismatch => {
            IotaError::ExplicitAttestationKeyMismatch { attestor_address }
        }
        AttestorSignatureError::Invalid(error) => IotaError::ExplicitAttestationSignatureInvalid {
            attestor_address,
            error,
        },
    })
}
