// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Checks on an attestation, shared by every path that validates an attested
//! transaction.

use iota_protocol_config::ProtocolConfig;
use iota_types::{
    attestation::Attestation,
    error::{IotaError, IotaResult},
    transaction::{TransactionAPI, TransactionEnvelope},
};

/// Attested computation units must lie between the protocol floor and what
/// the transaction can pay for; a zero gas price can pay for nothing.
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
    let max_attested_units = txn.gas_budget().checked_div(txn.gas_price()).unwrap_or(0);
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
