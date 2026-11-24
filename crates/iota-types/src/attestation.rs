// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;

use iota_sdk_types::{
    GasCostSummary, SenderSignedTransaction, SharedObjectReference, TransactionDigest, Version,
};
use serde::{Deserialize, Serialize};

use crate::{move_authenticator::MoveAuthenticatorExt, transaction::TransactionEnvelope};

/// Index of a validator in the current epoch's consensus committee. Kept as a
/// plain `u8` so `iota-types` does not depend on `starfish-config`, whose
/// `AuthorityIndex(u8)` is BCS-identical; the value is untrusted until checked
/// against the block author post-consensus.
pub(super) type AuthorityIndex = u8;

/// A pre-consensus claim produced by a trusted actor certifying that a specific
/// transaction has been validated before entering consensus. The attestation is
/// a separate artifact that travels alongside the transaction; the transaction
/// and the user's signature are completely unchanged.
///
/// An enum so that other attestor kinds can be appended later; the variant
/// order is protocol-significant.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Attestation {
    /// Produced by the block-proposing validator and authenticated by the
    /// block signature, so it carries no signature of its own.
    Authority {
        payload: AttestationData,
        /// Index of the attesting validator in the current epoch's committee
        authority_index: AuthorityIndex,
    },
}

/// The attested content carried by every [`Attestation`] variant.
///
/// Versioned to allow new fields to be introduced without breaking existing
/// match arms.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttestationData {
    V1 {
        /// Expected computation units billed by the attestor's dry-run
        /// (`computation_cost / gas_price`). Used by the sequencer to improve
        /// shared-object scheduling before execution.
        computation_units: u64,
        /// The version the attestor's dry run read of each shared object that
        /// a Move authenticator of the transaction names, as an argument or as
        /// its account, each with the [`SharedInputIndex`] that names it. Every
        /// such object has an entry, and an object named at two indexes has
        /// one for each.
        shared_object_versions: Vec<(SharedInputIndex, Version)>,
    },
}

/// Where a Move authenticator of a signed transaction names a shared object.
/// The variant order is protocol-significant: a new variant goes at the end.
#[cfg_attr(test, derive(iota_macros::EnumVariantOrder))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SharedInputIndex {
    /// The object is an argument of a Move authenticator: `authenticator` is
    /// the index of the authenticator among the signatures, and `argument`
    /// the index of the argument among its arguments.
    AuthenticatorArgument { authenticator: u8, argument: u8 },
    /// The object is the account of a Move authenticator: `authenticator` is
    /// the index of the authenticator among the signatures.
    AuthenticatorAccount { authenticator: u8 },
}

/// Returns each shared object that a Move authenticator of `transaction`
/// names, with its index: the arguments and then the account of each Move
/// authenticator, in the order of the signatures. An object named at two
/// indexes is returned for each. An object whose index does not fit
/// [`SharedInputIndex`] is left out.
pub fn list_move_authenticator_shared_objects(
    transaction: &SenderSignedTransaction,
) -> Vec<(SharedInputIndex, &SharedObjectReference)> {
    transaction
        .signatures()
        .iter()
        .enumerate()
        .filter_map(|(signature_index, signature)| {
            Some((
                u8::try_from(signature_index).ok()?,
                signature.as_opt_move_authenticator()?,
            ))
        })
        .flat_map(|(authenticator, move_authenticator)| {
            let arguments = move_authenticator
                .call_args()
                .iter()
                .enumerate()
                .filter_map(move |(argument_index, argument)| {
                    let index = SharedInputIndex::AuthenticatorArgument {
                        authenticator,
                        argument: u8::try_from(argument_index).ok()?,
                    };

                    Some((index, argument.as_opt_shared()?))
                });
            let account = move_authenticator
                .object_to_authenticate()
                .as_opt_shared()
                .map(|account| {
                    (
                        SharedInputIndex::AuthenticatorAccount { authenticator },
                        account,
                    )
                });

            arguments.chain(account)
        })
        .collect()
}

/// A user transaction bundled with its attestation. This is the inner payload
/// of `ConsensusTransactionKind::UserTransactionV1`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttestedTransaction {
    pub transaction: TransactionEnvelope,
    /// `None` until validators attest transactions. Reserved now so attaching
    /// an attestation later leaves the encoding of an unattested transaction
    /// unchanged.
    pub attestation: Option<Attestation>,
}

impl Attestation {
    /// An attestation by the block-proposing validator at `authority_index`.
    pub fn new_authority(payload: AttestationData, authority_index: AuthorityIndex) -> Self {
        Self::Authority {
            payload,
            authority_index,
        }
    }

    pub fn computation_units(&self) -> u64 {
        let Attestation::Authority { payload, .. } = self;
        let AttestationData::V1 {
            computation_units, ..
        } = payload;
        *computation_units
    }

    pub fn shared_object_versions(&self) -> &[(SharedInputIndex, Version)] {
        let Attestation::Authority { payload, .. } = self;
        let AttestationData::V1 {
            shared_object_versions,
            ..
        } = payload;

        shared_object_versions
    }
}

/// The computation cost of `gas_cost_summary` in gas units, independent of
/// the gas price the transaction paid. Rounds down, and is zero when
/// `gas_price` is zero.
pub fn computation_units(gas_cost_summary: &GasCostSummary, gas_price: u64) -> u64 {
    gas_cost_summary
        .computation_cost
        .checked_div(gas_price)
        .unwrap_or(0)
}

/// The verdict on an attested, executed transaction. The variant order is
/// protocol-significant: append, never reorder.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AttestationVerdict {
    /// Not refuted and, whenever the transaction body ran, the claimed
    /// computation units were within tolerance of the executed ones.
    Valid,
    /// Not refuted, but the claimed computation units were not within
    /// tolerance. An honest claim can land here when a shared object or
    /// dynamic field the dry run read moved before execution, so this is not
    /// by itself evidence against the attestor.
    Inaccurate,
    /// Authentication failed, and the judge holds the attestor accountable for
    /// it. `units` is the computation the failed runs consumed: execution's own
    /// run, whether or not the effects charge it, plus the judge's re-run at the
    /// recorded versions, when one was needed.
    Refuted { units: u64 },
    /// Authentication failed, and the judge could not decide whether the
    /// attestor or the account is accountable: its re-run at the recorded
    /// versions failed with an internal error. Nobody pays and the attestor is
    /// not penalized; certified, so that a node executing from a checkpoint
    /// charges nobody as well.
    Undecided,
}

impl AttestationVerdict {
    /// `refuted_units` carries the units of both failed runs when the failure
    /// refutes the attestation. `executed_units` is `None` when the
    /// transaction body never ran, so the claim cannot be assessed.
    /// `tolerance_percentage` bounds `|attested - executed|` as a percentage
    /// of the executed units; `None` disables the accuracy check.
    pub fn new(
        refuted_units: Option<u64>,
        attested_units: u64,
        executed_units: Option<u64>,
        tolerance_percentage: Option<u64>,
    ) -> Self {
        if let Some(units) = refuted_units {
            return Self::Refuted { units };
        }
        let accurate = match (executed_units, tolerance_percentage) {
            (Some(executed), Some(tolerance)) => {
                attested_units.abs_diff(executed).saturating_mul(100)
                    <= tolerance.saturating_mul(executed)
            }
            _ => true,
        };
        if accurate {
            Self::Valid
        } else {
            Self::Inaccurate
        }
    }

    pub fn is_valid(&self) -> bool {
        matches!(self, Self::Valid)
    }
}

/// The validator's verdict on an attested, executed transaction, certified in
/// the checkpoint summary unless `Valid`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AttestationRecord {
    /// Index of the attestor in the committee of the checkpoint's epoch.
    pub attestor: AuthorityIndex,
    pub verdict: AttestationVerdict,
}

impl AttestationRecord {
    pub fn new(attestation: &Attestation, verdict: AttestationVerdict) -> Self {
        let Attestation::Authority {
            authority_index, ..
        } = attestation;
        Self {
            attestor: *authority_index,
            verdict,
        }
    }
}

/// A record other than `Valid`, certified in the checkpoint summary at its
/// transaction's position in the checkpoint contents.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AttestationEntry {
    pub position: u32,
    pub record: AttestationRecord,
}

impl AttestationEntry {
    /// The records other than `Valid`, each at its position in `records`.
    ///
    /// # Panics
    ///
    /// If a position does not fit `u32`.
    pub fn from_records(records: &[Option<AttestationRecord>]) -> Vec<Self> {
        records
            .iter()
            .enumerate()
            .filter_map(|(position, record)| {
                let record = (*record).filter(|record| !record.verdict.is_valid())?;
                Some(Self {
                    position: u32::try_from(position).expect("checkpoint positions fit u32"),
                    record,
                })
            })
            .collect()
    }
}

/// Per-attestor totals of refuted attestations for the epoch so far,
/// certified in every checkpoint summary. An attestor without refuted
/// attestations has no entry.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttestationTotals {
    /// Committee index to the sum of the squared units of its `Refuted`
    /// verdicts since the epoch started; saturating.
    pub refuted_units_squared: BTreeMap<AuthorityIndex, u64>,
}

impl AttestationTotals {
    /// These totals plus the `Refuted` records of one more checkpoint.
    pub fn with_records(mut self, records: &[Option<AttestationRecord>]) -> Self {
        for record in records.iter().flatten() {
            if let AttestationVerdict::Refuted { units } = record.verdict {
                let sum = self
                    .refuted_units_squared
                    .entry(record.attestor)
                    .or_default();
                *sum = sum.saturating_add(units.saturating_pow(2));
            }
        }
        self
    }

    /// These totals plus `other`'s; saturating.
    pub fn with_totals(mut self, other: &Self) -> Self {
        for (&attestor, &units_squared) in &other.refuted_units_squared {
            let sum = self.refuted_units_squared.entry(attestor).or_default();
            *sum = sum.saturating_add(units_squared);
        }
        self
    }
}

impl AttestedTransaction {
    pub fn new(transaction: TransactionEnvelope, attestation: Option<Attestation>) -> Self {
        Self {
            transaction,
            attestation,
        }
    }

    pub fn digest(&self) -> &TransactionDigest {
        self.transaction.digest()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use iota_protocol_config::ProtocolConfig;
    use iota_sdk_types::{
        Address, MoveAuthenticator, MoveAuthenticatorV1, ObjectId, ProgrammableTransaction,
        Transaction, UserSignature,
    };

    use super::*;
    use crate::{
        base_types::random_object_ref,
        error::UserInputError,
        transaction::{CallArg, TransactionAPI},
        utils::create_fake_transaction,
    };

    fn make_attestation_data() -> AttestationData {
        AttestationData::V1 {
            computation_units: 1_000_000,
            shared_object_versions: vec![
                (
                    SharedInputIndex::AuthenticatorArgument {
                        authenticator: 0,
                        argument: 1,
                    },
                    Version::from(7u64),
                ),
                (
                    SharedInputIndex::AuthenticatorAccount { authenticator: 0 },
                    Version::from(7u64),
                ),
            ],
        }
    }

    /// An entry takes one byte for the variant of [`SharedInputIndex`], one for
    /// each `u8` index, and eight for the version.
    #[test]
    fn entries_take_eleven_bytes_or_ten_for_account() {
        let size = |index: SharedInputIndex| {
            bcs::to_bytes(&(index, Version::from(u64::MAX)))
                .unwrap()
                .len()
        };

        assert_eq!(
            size(SharedInputIndex::AuthenticatorArgument {
                authenticator: u8::MAX,
                argument: u8::MAX,
            }),
            11
        );
        assert_eq!(
            size(SharedInputIndex::AuthenticatorAccount {
                authenticator: u8::MAX
            }),
            10
        );
    }

    /// The largest record and the largest transaction together stay under the
    /// size limit of a consensus transaction. The record has an entry for every
    /// argument and the account of a Move authenticator for the sender and one
    /// for the sponsor.
    #[test]
    fn largest_record_fits_beside_largest_transaction() {
        let config = ProtocolConfig::get_for_max_version_UNSAFE();
        // The validity check of a Move authenticator allows fewer arguments
        // than `max_function_parameters` minus 3. The two assertions on
        // `validity_check()` tie this count to that rule.
        let max_arguments = config.max_function_parameters() - 4;
        let authenticator_with = |arguments: u64| {
            MoveAuthenticator::V1(MoveAuthenticatorV1::new(
                vec![CallArg::Pure(vec![]); usize::try_from(arguments).unwrap()],
                vec![],
                SharedObjectReference {
                    object_id: ObjectId::random(),
                    initial_shared_version: Version::from(1u64),
                    mutable: false,
                },
            ))
        };
        assert!(
            authenticator_with(max_arguments)
                .validity_check(&config)
                .is_ok()
        );
        assert!(matches!(
            authenticator_with(max_arguments + 1).validity_check(&config),
            Err(UserInputError::SizeLimitExceeded { .. })
        ));
        let version = Version::from(u64::MAX);
        let authenticator_inputs = (0..2).flat_map(|authenticator| {
            (0..max_arguments)
                .map(move |argument| {
                    let index = SharedInputIndex::AuthenticatorArgument {
                        authenticator,
                        argument: u8::try_from(argument).unwrap(),
                    };

                    (index, version)
                })
                .chain([(
                    SharedInputIndex::AuthenticatorAccount { authenticator },
                    version,
                )])
        });
        let attestation = Attestation::new_authority(
            AttestationData::V1 {
                computation_units: u64::MAX,
                shared_object_versions: authenticator_inputs.collect(),
            },
            u8::MAX,
        );

        let record_size = u64::try_from(bcs::serialized_size(&attestation).unwrap()).unwrap();

        assert!(
            config.max_tx_size_bytes() + record_size
                < config.consensus_max_transaction_size_bytes(),
            "a record of {record_size} bytes"
        );
    }

    /// The transaction names shared objects at its inputs 1 and 3, at the
    /// second argument of its Move authenticator and as the authenticator's
    /// account. [`list_move_authenticator_shared_objects()`] lists the last
    /// two with their indexes: the shared objects of the Move authenticators,
    /// as [`MoveAuthenticatorExt::shared_objects()`] gives them. It leaves out
    /// the pure value and the immutable object that the authenticator also
    /// takes.
    #[test]
    fn listing_names_shared_objects_of_move_authenticators() {
        let shared = |mutable| SharedObjectReference {
            object_id: ObjectId::random(),
            initial_shared_version: Version::from(1u64),
            mutable,
        };
        let (argument, account) = (shared(false), shared(false));
        let programmable = ProgrammableTransaction {
            inputs: vec![
                CallArg::Pure(vec![0]),
                CallArg::Shared(shared(true)),
                CallArg::ImmutableOrOwned(random_object_ref()),
                CallArg::Shared(shared(false)),
            ],
            commands: vec![],
        };
        let authenticator = MoveAuthenticator::V1(MoveAuthenticatorV1::new(
            vec![
                CallArg::Pure(vec![1]),
                CallArg::Shared(argument),
                CallArg::ImmutableOrOwned(random_object_ref()),
            ],
            vec![],
            account,
        ));
        let transaction = SenderSignedTransaction::new(
            Transaction::new_programmable(
                Address::ZERO,
                vec![random_object_ref()],
                programmable,
                1,
                1,
            ),
            vec![UserSignature::MoveAuthenticator(authenticator)],
        );

        let listed = list_move_authenticator_shared_objects(&transaction);

        assert_eq!(
            listed,
            vec![
                (
                    SharedInputIndex::AuthenticatorArgument {
                        authenticator: 0,
                        argument: 1,
                    },
                    &argument,
                ),
                (
                    SharedInputIndex::AuthenticatorAccount { authenticator: 0 },
                    &account,
                ),
            ]
        );
        assert_eq!(
            listed
                .iter()
                .map(|(_, object)| object.object_id)
                .collect::<BTreeSet<_>>(),
            transaction
                .move_authenticators()
                .iter()
                .flat_map(|authenticator| authenticator.shared_objects())
                .map(|object| object.object_id)
                .collect::<BTreeSet<_>>(),
            "the record covers every shared object that a Move authenticator names"
        );
    }

    #[test]
    fn attestation_data_bcs_round_trip() {
        let data = make_attestation_data();
        let encoded = bcs::to_bytes(&data).unwrap();
        let decoded: AttestationData = bcs::from_bytes(&encoded).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn attestation_authority_bcs_round_trip() {
        let attestation = Attestation::Authority {
            payload: make_attestation_data(),
            authority_index: 3,
        };
        let encoded = bcs::to_bytes(&attestation).unwrap();
        let decoded: Attestation = bcs::from_bytes(&encoded).unwrap();
        assert_eq!(decoded, attestation);
    }

    /// Both sides of the check are `computation_cost / gas_price`, which the
    /// gas meter rounds up to `gas_rounding_step`, so they are always multiples
    /// of it and the smallest difference they can show is one step.
    #[test]
    fn verdict_accuracy_band() {
        const STEP: u64 = 1_000;
        let judge = |attested, executed, tolerance| {
            AttestationVerdict::new(None, attested, Some(executed), tolerance)
        };

        // Under ten steps a tenth of the executed units is less than one step,
        // so only the same value is accurate.
        assert_eq!(
            judge(3 * STEP, 3 * STEP, Some(10)),
            AttestationVerdict::Valid
        );
        assert_eq!(
            judge(4 * STEP, 3 * STEP, Some(10)),
            AttestationVerdict::Inaccurate
        );
        assert_eq!(
            judge(2 * STEP, 3 * STEP, Some(10)),
            AttestationVerdict::Inaccurate
        );

        // At ten steps one step of drift is exactly the allowance.
        assert_eq!(
            judge(11 * STEP, 10 * STEP, Some(10)),
            AttestationVerdict::Valid
        );
        assert_eq!(
            judge(9 * STEP, 10 * STEP, Some(10)),
            AttestationVerdict::Valid
        );
        assert_eq!(
            judge(12 * STEP, 10 * STEP, Some(10)),
            AttestationVerdict::Inaccurate
        );

        // Ten steps either way on a hundred-step transaction.
        assert_eq!(
            judge(110 * STEP, 100 * STEP, Some(10)),
            AttestationVerdict::Valid
        );
        assert_eq!(
            judge(111 * STEP, 100 * STEP, Some(10)),
            AttestationVerdict::Inaccurate
        );

        // No tolerance configured disables the check.
        assert_eq!(judge(50 * STEP, STEP, None), AttestationVerdict::Valid);
        // A body that never ran cannot be assessed.
        assert_eq!(
            AttestationVerdict::new(None, 50 * STEP, None, Some(10)),
            AttestationVerdict::Valid
        );
        // A refutation wins over accuracy and keeps the units of both runs.
        assert_eq!(
            AttestationVerdict::new(Some(7 * STEP), 3 * STEP, Some(3 * STEP), Some(10)),
            AttestationVerdict::Refuted { units: 7 * STEP }
        );
    }

    #[test]
    fn entries_certify_every_record_other_than_valid() {
        let record = |attestor, verdict| Some(AttestationRecord { attestor, verdict });
        let entry = |position, attestor, verdict| AttestationEntry {
            position,
            record: AttestationRecord { attestor, verdict },
        };

        let entries = AttestationEntry::from_records(&[
            record(1, AttestationVerdict::Valid),
            record(2, AttestationVerdict::Inaccurate),
            None,
            record(0, AttestationVerdict::Refuted { units: 3 }),
            record(1, AttestationVerdict::Undecided),
        ]);

        assert_eq!(
            entries,
            vec![
                entry(1, 2, AttestationVerdict::Inaccurate),
                entry(3, 0, AttestationVerdict::Refuted { units: 3 }),
                entry(4, 1, AttestationVerdict::Undecided),
            ]
        );
    }

    #[test]
    fn totals_sum_the_squared_units_of_refuted_records() {
        let record = |attestor, verdict| Some(AttestationRecord { attestor, verdict });
        let refuted = |units| AttestationVerdict::Refuted { units };
        let totals = AttestationTotals::default().with_records(&[
            record(2, refuted(3)),
            record(1, AttestationVerdict::Valid),
            record(1, AttestationVerdict::Inaccurate),
            record(0, AttestationVerdict::Undecided),
            None,
            record(2, refuted(25)),
        ]);
        assert_eq!(totals.refuted_units_squared, BTreeMap::from([(2, 9 + 625)]));

        // A later checkpoint adds to the running sums.
        let totals = totals.with_records(&[record(0, refuted(1)), record(2, refuted(2))]);
        assert_eq!(
            totals.refuted_units_squared,
            BTreeMap::from([(0, 1), (2, 9 + 625 + 4)])
        );
        assert_eq!(
            bcs::to_bytes(&AttestationTotals::default()).unwrap(),
            vec![0],
            "empty totals cost one byte"
        );
    }

    #[test]
    fn attested_transaction_bcs_round_trip() {
        let attested = AttestedTransaction::new(
            create_fake_transaction(),
            Some(Attestation::Authority {
                payload: make_attestation_data(),
                authority_index: 0,
            }),
        );
        let encoded = bcs::to_bytes(&attested).unwrap();
        let decoded: AttestedTransaction = bcs::from_bytes(&encoded).unwrap();
        assert_eq!(decoded, attested);
    }
}
