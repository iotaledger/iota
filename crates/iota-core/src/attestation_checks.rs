// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Checks on an attestation, shared by every path that validates an attested
//! transaction.

use std::collections::{HashMap, HashSet};

use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::{ObjectId, Version};
use iota_types::{
    attestation::{Attestation, SharedInputIndex, list_shared_inputs},
    error::{IotaError, IotaResult},
    transaction::{TransactionAPI, TransactionEnvelope},
};

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

/// The record of `attestation` must hold a version only at an index where
/// `transaction` names a shared object, at most one for each index, the same
/// version for an object named at two indexes, and a version for each shared
/// argument and account of a Move authenticator. Returns the error of the
/// first entry that breaks a rule, in the order of the record:
/// [`IotaError::AttestationIndexNamesNoSharedObject`],
/// [`IotaError::AttestationIndexRepeated`] or
/// [`IotaError::AttestationObjectVersionsDiffer`]. Otherwise, returns
/// [`IotaError::AttestationAuthenticatorInputMissing`] for the first shared
/// input of a Move authenticator that has no entry, in the order of
/// [`list_shared_inputs()`].
pub(crate) fn check_attested_shared_object_versions(
    transaction: &TransactionEnvelope,
    attestation: &Attestation,
) -> IotaResult {
    let shared_inputs = list_shared_inputs(transaction.data());
    let object_ids: HashMap<SharedInputIndex, ObjectId> = shared_inputs
        .iter()
        .map(|(index, object)| (*index, object.object_id))
        .collect();
    let mut recorded_indexes = HashSet::new();
    let mut recorded_versions: HashMap<ObjectId, Version> = HashMap::new();
    for &(index, version) in attestation.shared_object_versions() {
        let Some(&object_id) = object_ids.get(&index) else {
            return Err(IotaError::AttestationIndexNamesNoSharedObject { index });
        };
        if !recorded_indexes.insert(index) {
            return Err(IotaError::AttestationIndexRepeated { index });
        }
        let first = *recorded_versions.entry(object_id).or_insert(version);
        if first != version {
            return Err(IotaError::AttestationObjectVersionsDiffer {
                object_id,
                first,
                second: version,
            });
        }
    }

    // An honest attestor attests only after the authentication of its dry run
    // passed, and authentication fails on a deleted input, so it records every
    // shared input of the Move authenticators.
    let missing = shared_inputs
        .iter()
        .map(|(index, _)| *index)
        .find(|index| !recorded_indexes.contains(index));
    if let Some(index) = missing {
        return Err(IotaError::AttestationAuthenticatorInputMissing { index });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use iota_sdk_types::{
        Address, MoveAuthenticator, MoveAuthenticatorV1, ProgrammableTransaction,
        SharedObjectReference, Transaction, UserSignature,
    };
    use iota_types::{
        attestation::AttestationData, base_types::random_object_ref, transaction::CallArg,
    };

    use super::*;

    // The indexes at which the Move authenticators of the transaction of
    // `build_transaction_naming_shared_objects()` name a shared object.
    const CLOCK: SharedInputIndex = SharedInputIndex::AuthenticatorArgument {
        authenticator: 0,
        argument: 1,
    };
    const ACCOUNT: SharedInputIndex = SharedInputIndex::AuthenticatorAccount { authenticator: 0 };
    const ACCOUNT_AS_ARGUMENT: SharedInputIndex = SharedInputIndex::AuthenticatorArgument {
        authenticator: 1,
        argument: 0,
    };
    const SECOND_ACCOUNT: SharedInputIndex =
        SharedInputIndex::AuthenticatorAccount { authenticator: 1 };

    /// A record that an honest attestor can give for the transaction of
    /// `build_transaction_naming_shared_objects()`. The account has the same
    /// version at both of its indexes.
    const HONEST_RECORD: [(SharedInputIndex, u64); 4] = [
        (CLOCK, 2),
        (ACCOUNT, 7),
        (ACCOUNT_AS_ARGUMENT, 7),
        (SECOND_ACCOUNT, 5),
    ];

    /// Returns a transaction and the id of the account of its first Move
    /// authenticator. The transaction takes a pure value at input 0, a shared
    /// pool at input 1 and an owned object at input 2. Its first Move
    /// authenticator takes a pure value and then a shared clock as its
    /// arguments, and authenticates the account. Its second Move authenticator
    /// takes that account and then an immutable object as its arguments, and
    /// authenticates another account.
    fn build_transaction_naming_shared_objects() -> (TransactionEnvelope, ObjectId) {
        let shared = |mutable| SharedObjectReference {
            object_id: ObjectId::random(),
            initial_shared_version: Version::from(1u64),
            mutable,
        };
        let account = shared(false);
        let programmable = ProgrammableTransaction {
            inputs: vec![
                CallArg::Pure(vec![0]),
                CallArg::Shared(shared(true)),
                CallArg::ImmutableOrOwned(random_object_ref()),
            ],
            commands: vec![],
        };
        let first_authenticator = MoveAuthenticator::V1(MoveAuthenticatorV1::new(
            vec![CallArg::Pure(vec![1]), CallArg::Shared(shared(false))],
            vec![],
            account,
        ));
        let second_authenticator = MoveAuthenticator::V1(MoveAuthenticatorV1::new(
            vec![
                CallArg::Shared(account),
                CallArg::ImmutableOrOwned(random_object_ref()),
            ],
            vec![],
            shared(false),
        ));
        let transaction = TransactionEnvelope::from_user_sig_data(
            Transaction::new_programmable(
                Address::ZERO,
                vec![random_object_ref()],
                programmable,
                1,
                1,
            ),
            vec![
                UserSignature::MoveAuthenticator(first_authenticator),
                UserSignature::MoveAuthenticator(second_authenticator),
            ],
        );

        (transaction, account.object_id)
    }

    /// Runs [`check_attested_shared_object_versions()`] on `transaction` with
    /// a record of `entries`.
    fn check(transaction: &TransactionEnvelope, entries: &[(SharedInputIndex, u64)]) -> IotaResult {
        let attestation = Attestation::new_authority(
            AttestationData::V1 {
                computation_units: 1,
                shared_object_versions: entries
                    .iter()
                    .map(|&(index, version)| (index, Version::from(version)))
                    .collect(),
            },
            0,
        );

        check_attested_shared_object_versions(transaction, &attestation)
    }

    /// The record of an honest attestor passes. The shared pool that the
    /// transaction itself takes has no index and needs no entry.
    #[test]
    fn record_of_every_shared_input_passes() {
        let (transaction, _) = build_transaction_naming_shared_objects();

        assert_eq!(check(&transaction, &HONEST_RECORD), Ok(()));
    }

    /// The indexes name a pure value, an immutable object, nothing past the
    /// last argument, and a Move authenticator the transaction does not have.
    #[test]
    fn index_naming_no_shared_object_is_refused() {
        let (transaction, _) = build_transaction_naming_shared_objects();

        for index in [
            SharedInputIndex::AuthenticatorArgument {
                authenticator: 0,
                argument: 0,
            },
            SharedInputIndex::AuthenticatorArgument {
                authenticator: 1,
                argument: 1,
            },
            SharedInputIndex::AuthenticatorArgument {
                authenticator: 0,
                argument: 2,
            },
            SharedInputIndex::AuthenticatorAccount { authenticator: 2 },
        ] {
            let mut entries = HONEST_RECORD.to_vec();
            entries.push((index, 1));

            assert_eq!(
                check(&transaction, &entries),
                Err(IotaError::AttestationIndexNamesNoSharedObject { index }),
                "{index:?}"
            );
        }
    }

    /// An index recorded twice is refused even with the same version.
    #[test]
    fn repeated_index_is_refused() {
        let (transaction, _) = build_transaction_naming_shared_objects();
        let mut entries = HONEST_RECORD.to_vec();
        entries.push((CLOCK, 2));

        assert_eq!(
            check(&transaction, &entries),
            Err(IotaError::AttestationIndexRepeated { index: CLOCK })
        );
    }

    /// The transaction names the account at two indexes, and the record gives
    /// it a different version at each.
    #[test]
    fn object_with_two_versions_is_refused() {
        let (transaction, account_id) = build_transaction_naming_shared_objects();

        assert_eq!(
            check(
                &transaction,
                &[(ACCOUNT, 7), (CLOCK, 2), (ACCOUNT_AS_ARGUMENT, 8)]
            ),
            Err(IotaError::AttestationObjectVersionsDiffer {
                object_id: account_id,
                first: Version::from(7u64),
                second: Version::from(8u64),
            })
        );
    }

    /// Every shared argument and account of the Move authenticators needs an
    /// entry.
    #[test]
    fn missing_authenticator_input_is_refused() {
        let (transaction, _) = build_transaction_naming_shared_objects();

        for (missing, _) in HONEST_RECORD {
            let entries: Vec<_> = HONEST_RECORD
                .into_iter()
                .filter(|(index, _)| *index != missing)
                .collect();

            assert_eq!(
                check(&transaction, &entries),
                Err(IotaError::AttestationAuthenticatorInputMissing { index: missing }),
                "{missing:?}"
            );
        }
    }

    #[test]
    fn zero_gas_price_has_no_ceiling() {
        let gas_budget = 1;
        let gas_price = 0;
        let tx = Transaction::new_transfer(
            Address::ZERO,
            random_object_ref(),
            Address::ZERO,
            random_object_ref(),
            gas_budget,
            gas_price,
        );
        let attestation = Attestation::new_authority(
            AttestationData::V1 {
                computation_units: u64::MAX,
                shared_object_versions: vec![],
            },
            0,
        );
        assert!(
            check_attested_units(
                &ProtocolConfig::get_for_max_version_UNSAFE(),
                &TransactionEnvelope::from_data(tx, vec![]),
                &attestation,
            )
            .is_ok()
        );
    }
}
