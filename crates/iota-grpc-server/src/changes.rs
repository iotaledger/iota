// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Synchronous derivation of balance and object changes from a transaction's
//! effects and its already-fetched input/output objects. Operates purely on
//! in-memory data — no storage lookups.
//!
//! Objects missing from the provided sets (e.g. pruned from the object store)
//! are an error rather than being skipped: a missing input coin would make a
//! balance delta numerically wrong and a missing object would silently drop
//! an object change, with no way for the client to detect the incomplete
//! result. Erroring lets the client retry without the derived change fields.

use std::{
    collections::{BTreeMap, HashSet},
    ops::Neg,
};

use iota_grpc_types::v1::transaction as grpc_tx;
use iota_sdk_types::{
    Address, ExecutionStatus, ObjectDigest, ObjectId, Owner, StructTag, TypeTag, Version,
};
use iota_types::{
    coin::Coin,
    effects::{ObjectRemoveKind, TransactionEffects, TransactionEffectsAPI, TransactionEffectsExt},
    gas_coin::GAS,
    object::Object,
    storage::WriteKind,
};

/// Error deriving balance or object changes from a transaction's effects.
#[derive(Debug, PartialEq, Eq)]
pub enum DeriveChangesError {
    /// A required object was not in the provided input/output sets. On the
    /// ledger read path this means it was pruned from the object store; on
    /// the execute/simulate/checkpoint paths the sets are always complete,
    /// so a miss there indicates a server bug.
    MissingObject {
        object_id: ObjectId,
        version: Version,
    },
    /// An object whose type is a coin had contents that are not a valid coin.
    MalformedCoin {
        object_id: ObjectId,
        version: Version,
    },
}

impl std::fmt::Display for DeriveChangesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingObject { object_id, version } => write!(
                f,
                "object {object_id} at version {version} is unavailable (possibly pruned)"
            ),
            Self::MalformedCoin { object_id, version } => write!(
                f,
                "coin object {object_id} at version {version} has malformed contents"
            ),
        }
    }
}

impl std::error::Error for DeriveChangesError {}

impl From<DeriveChangesError> for crate::error::RpcError {
    fn from(error: DeriveChangesError) -> Self {
        Self::new(
            tonic::Code::FailedPrecondition,
            format!(
                "cannot derive the requested change fields: {error}; \
                 retry without balance_changes/object_changes in the read mask"
            ),
        )
    }
}

/// The change in balance of one coin type for one owner, derived from a
/// transaction's effects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedBalanceChange {
    pub owner: Owner,
    pub coin_type: TypeTag,
    /// Negative amount means the net flow of value is away from the owner.
    pub amount: i128,
}

/// A change to an object caused by a transaction, derived from its effects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DerivedObjectChange {
    Published {
        package_id: ObjectId,
        version: Version,
        digest: ObjectDigest,
        modules: Vec<String>,
    },
    Mutated {
        sender: Address,
        owner: Owner,
        object_type: StructTag,
        object_id: ObjectId,
        version: Version,
        previous_version: Version,
        digest: ObjectDigest,
    },
    Deleted {
        sender: Address,
        object_type: StructTag,
        object_id: ObjectId,
        version: Version,
    },
    Wrapped {
        sender: Address,
        object_type: StructTag,
        object_id: ObjectId,
        version: Version,
    },
    Unwrapped {
        sender: Address,
        owner: Owner,
        object_type: StructTag,
        object_id: ObjectId,
        version: Version,
        digest: ObjectDigest,
    },
    Created {
        sender: Address,
        owner: Owner,
        object_type: StructTag,
        object_id: ObjectId,
        version: Version,
        digest: ObjectDigest,
    },
}

/// Derive the balance changes of a transaction from its effects and its
/// input/output objects.
///
/// `input_objects` must hold the objects at the versions given by
/// `effects.modified_at_versions()` and `output_objects` the objects written
/// by the transaction (created, mutated, unwrapped); this is exactly what the
/// existing input/output object fetch paths produce.
///
/// For a failed transaction only the gas charge is reported (which requires
/// no objects). `mocked_coin` excludes a gas coin mocked during simulation
/// from the result.
///
/// Errors if a required object is missing from the sets — a missing input
/// coin would silently corrupt the delta of any coin whose output is present.
pub fn derive_balance_changes(
    effects: &TransactionEffects,
    input_objects: &[Object],
    output_objects: &[Object],
    mocked_coin: Option<ObjectId>,
) -> Result<Vec<DerivedBalanceChange>, DeriveChangesError> {
    let (_, gas_owner) = effects.gas_object();

    // Only charge gas when the tx fails, skip all object parsing
    if effects.status() != &ExecutionStatus::Success {
        return Ok(vec![DerivedBalanceChange {
            owner: gas_owner,
            coin_type: GAS::type_tag(),
            amount: (effects.gas_cost_summary().net_gas_usage() as i128).neg(),
        }]);
    }

    let objects: BTreeMap<(ObjectId, Version), &Object> = input_objects
        .iter()
        .chain(output_objects)
        .map(|o| ((o.id(), o.version()), o))
        .collect();

    let unwrapped_then_deleted = effects
        .unwrapped_then_deleted()
        .iter()
        .map(|object_ref| object_ref.object_id)
        .collect::<HashSet<_>>();

    let mut balances = BTreeMap::<(Owner, TypeTag), i128>::new();

    // 1. subtract all input coins
    for (id, version) in effects.modified_at_versions() {
        // Skip the mocked gas coin, which is not present in the input objects
        if matches!(mocked_coin, Some(coin) if id == coin) {
            continue;
        }
        // Unwrapped-then-deleted objects have no stored input version
        if unwrapped_then_deleted.contains(&id) {
            continue;
        }
        if let Some((owner, coin_type, amount)) = coin_owner_type_value(&objects, id, version)? {
            *balances.entry((owner, coin_type)).or_default() -= amount as i128;
        }
    }

    // 2. add all mutated coins
    for (object_ref, _, _) in effects.all_changed_objects() {
        // Skip the mocked gas coin, which is not present in the output objects
        if matches!(mocked_coin, Some(coin) if object_ref.object_id == coin) {
            continue;
        }
        if let Some((owner, coin_type, amount)) =
            coin_owner_type_value(&objects, object_ref.object_id, object_ref.version)?
        {
            *balances.entry((owner, coin_type)).or_default() += amount as i128;
        }
    }

    Ok(balances
        .into_iter()
        .filter_map(|((owner, coin_type), amount)| {
            if amount == 0 {
                return None;
            }
            Some(DerivedBalanceChange {
                owner,
                coin_type,
                amount,
            })
        })
        .collect())
}

/// Look up an object and return its owner, coin type and balance if it is a
/// coin. Returns `None` for non-coins; errors if the object is missing from
/// the set or its coin contents are malformed.
fn coin_owner_type_value(
    objects: &BTreeMap<(ObjectId, Version), &Object>,
    id: ObjectId,
    version: Version,
) -> Result<Option<(Owner, TypeTag, u64)>, DeriveChangesError> {
    let Some(object) = objects.get(&(id, version)) else {
        return Err(DeriveChangesError::MissingObject {
            object_id: id,
            version,
        });
    };
    let Some(move_object_type) = object.type_() else {
        return Ok(None);
    };
    if !move_object_type.is_coin() {
        return Ok(None);
    }
    let malformed_coin = || DeriveChangesError::MalformedCoin {
        object_id: id,
        version,
    };
    let coin_type = move_object_type
        .type_params()
        .first()
        .ok_or_else(malformed_coin)?
        .clone();
    let value = Coin::extract_balance_if_coin(object)
        .ok()
        .flatten()
        .ok_or_else(malformed_coin)?;
    Ok(Some((object.owner, coin_type, value)))
}

/// Derive the object changes of a transaction from its effects and its
/// input/output objects.
///
/// `output_objects` must hold the objects written by the transaction and
/// `input_objects` the objects at the versions given by
/// `effects.modified_at_versions()` — the latter provide the types of deleted
/// and wrapped objects.
///
/// Errors if a required object is missing from the sets — skipping it would
/// silently drop an object change from the result.
pub fn derive_object_changes(
    sender: Address,
    effects: &TransactionEffects,
    input_objects: &[Object],
    output_objects: &[Object],
) -> Result<Vec<DerivedObjectChange>, DeriveChangesError> {
    let mut object_changes = vec![];

    let modified_at_versions = effects
        .modified_at_versions()
        .into_iter()
        .collect::<BTreeMap<_, _>>();

    let outputs: BTreeMap<(ObjectId, Version), &Object> = output_objects
        .iter()
        .map(|o| ((o.id(), o.version()), o))
        .collect();

    // Input objects are the objects at their modified-at versions, so they are
    // unique per id and provide the pre-transaction state of removed objects.
    let inputs_by_id: BTreeMap<ObjectId, &Object> =
        input_objects.iter().map(|o| (o.id(), o)).collect();

    for (changed_object, owner, kind) in effects.all_changed_objects() {
        let object_id = changed_object.object_id;
        let version = changed_object.version;
        let digest = changed_object.digest;
        let Some(object) = outputs.get(&(object_id, version)) else {
            return Err(DeriveChangesError::MissingObject { object_id, version });
        };
        if let Some(move_object_type) = object.type_() {
            let object_type: StructTag = move_object_type.clone().into();

            match kind {
                WriteKind::Mutate => object_changes.push(DerivedObjectChange::Mutated {
                    sender,
                    owner,
                    object_type,
                    object_id,
                    version,
                    // modified_at_versions should always hold mutated objects
                    previous_version: modified_at_versions
                        .get(&object_id)
                        .copied()
                        .unwrap_or_default(),
                    digest,
                }),
                WriteKind::Create => object_changes.push(DerivedObjectChange::Created {
                    sender,
                    owner,
                    object_type,
                    object_id,
                    version,
                    digest,
                }),
                WriteKind::Unwrap => object_changes.push(DerivedObjectChange::Unwrapped {
                    sender,
                    owner,
                    object_type,
                    object_id,
                    version,
                    digest,
                }),
            }
        } else if let Some(package) = object.data.as_opt_package() {
            if kind == WriteKind::Create {
                object_changes.push(DerivedObjectChange::Published {
                    package_id: package.id(),
                    version: package.version(),
                    digest,
                    modules: package
                        .serialized_module_map()
                        .keys()
                        .map(|k| k.to_string())
                        .collect(),
                })
            }
        }
    }

    for (removed_object, kind) in effects.all_removed_objects() {
        let object_id = removed_object.object_id;
        let version = removed_object.version;
        let Some(object) = inputs_by_id.get(&object_id) else {
            // The input object lives at its modified-at version, not at the
            // removed ref's (tombstone) version
            return Err(DeriveChangesError::MissingObject {
                object_id,
                version: modified_at_versions
                    .get(&object_id)
                    .copied()
                    .unwrap_or(version),
            });
        };
        // Packages cannot be removed; skip non-Move objects
        if let Some(move_object_type) = object.type_() {
            let object_type: StructTag = move_object_type.clone().into();
            match kind {
                ObjectRemoveKind::Delete => object_changes.push(DerivedObjectChange::Deleted {
                    sender,
                    object_type,
                    object_id,
                    version,
                }),
                ObjectRemoveKind::Wrap => object_changes.push(DerivedObjectChange::Wrapped {
                    sender,
                    object_type,
                    object_id,
                    version,
                }),
            }
        }
    }

    Ok(object_changes)
}

impl From<DerivedBalanceChange> for grpc_tx::BalanceChange {
    fn from(change: DerivedBalanceChange) -> Self {
        Self::default()
            .with_owner(change.owner)
            .with_coin_type(&change.coin_type)
            .with_amount(change.amount.to_be_bytes().to_vec())
    }
}

/// A Move object's type is always a struct; the proto carries it as the more
/// general `TypeTag`, matching `BalanceChange.coin_type`.
fn object_type_to_proto(object_type: StructTag) -> iota_grpc_types::v1::types::TypeTag {
    (&TypeTag::Struct(Box::new(object_type))).into()
}

impl From<DerivedObjectChange> for grpc_tx::ObjectChange {
    fn from(change: DerivedObjectChange) -> Self {
        match change {
            DerivedObjectChange::Published {
                package_id,
                version,
                digest,
                modules,
            } => Self::default().with_published(
                grpc_tx::ObjectChangePublished::default()
                    .with_package_id(package_id)
                    .with_version(version.as_u64())
                    .with_digest(digest)
                    .with_modules(modules),
            ),
            DerivedObjectChange::Mutated {
                sender,
                owner,
                object_type,
                object_id,
                version,
                previous_version,
                digest,
            } => Self::default().with_mutated(
                grpc_tx::ObjectChangeMutated::default()
                    .with_sender(sender)
                    .with_owner(owner)
                    .with_object_type(object_type_to_proto(object_type))
                    .with_object_id(object_id)
                    .with_version(version.as_u64())
                    .with_previous_version(previous_version.as_u64())
                    .with_digest(digest),
            ),
            DerivedObjectChange::Deleted {
                sender,
                object_type,
                object_id,
                version,
            } => Self::default().with_deleted(
                grpc_tx::ObjectChangeDeleted::default()
                    .with_sender(sender)
                    .with_object_type(object_type_to_proto(object_type))
                    .with_object_id(object_id)
                    .with_version(version.as_u64()),
            ),
            DerivedObjectChange::Wrapped {
                sender,
                object_type,
                object_id,
                version,
            } => Self::default().with_wrapped(
                grpc_tx::ObjectChangeWrapped::default()
                    .with_sender(sender)
                    .with_object_type(object_type_to_proto(object_type))
                    .with_object_id(object_id)
                    .with_version(version.as_u64()),
            ),
            DerivedObjectChange::Unwrapped {
                sender,
                owner,
                object_type,
                object_id,
                version,
                digest,
            } => Self::default().with_unwrapped(
                grpc_tx::ObjectChangeUnwrapped::default()
                    .with_sender(sender)
                    .with_owner(owner)
                    .with_object_type(object_type_to_proto(object_type))
                    .with_object_id(object_id)
                    .with_version(version.as_u64())
                    .with_digest(digest),
            ),
            DerivedObjectChange::Created {
                sender,
                owner,
                object_type,
                object_id,
                version,
                digest,
            } => Self::default().with_created(
                grpc_tx::ObjectChangeCreated::default()
                    .with_sender(sender)
                    .with_owner(owner)
                    .with_object_type(object_type_to_proto(object_type))
                    .with_object_id(object_id)
                    .with_version(version.as_u64())
                    .with_digest(digest),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use iota_sdk_types::TransactionDigest;
    use iota_types::{
        effects::{TestEffectsBuilder, TransactionEffectsAPIForTesting as _},
        full_checkpoint_content::CheckpointTransaction,
        test_checkpoint_data_builder::TestCheckpointDataBuilder,
        transaction::TransactionDataAPI as _,
    };

    use super::*;

    const SENDER: u8 = 0;
    const RECIPIENT: u8 = 1;

    fn sender_address() -> Address {
        TestCheckpointDataBuilder::derive_address(SENDER)
    }

    fn recipient_address() -> Address {
        TestCheckpointDataBuilder::derive_address(RECIPIENT)
    }

    fn object_id(object_idx: u64) -> ObjectId {
        TestCheckpointDataBuilder::derive_object_id(object_idx)
    }

    /// A minimal transaction whose gas input is at version 0, so effects
    /// built from it assign lamport version 1 — matching the initial version
    /// of objects created by the `*_for_testing` constructors. Also returns
    /// the gas coin at its input version (0) and output version (1) so the
    /// derivation finds the mutated gas object.
    fn version_zero_gas_transaction() -> (iota_types::transaction::SenderSignedData, Object, Object)
    {
        use iota_sdk_types::ObjectReference;
        use iota_types::{
            programmable_transaction_builder::ProgrammableTransactionBuilder,
            transaction::{SenderSignedData, TransactionData},
        };

        let gas_id = ObjectId::random();
        let gas_ref = ObjectReference::new(gas_id, 0u64.into(), ObjectDigest::MIN);
        let transaction_data = TransactionData::new(
            iota_sdk_types::TransactionKind::Programmable(
                ProgrammableTransactionBuilder::new().finish(),
            ),
            sender_address(),
            gas_ref,
            1,
            1,
        );
        let gas_owner = Owner::Address(sender_address());
        (
            SenderSignedData::new(transaction_data, vec![]),
            Object::with_id_owner_version_for_testing(gas_id, 0u64.into(), gas_owner),
            Object::with_id_owner_version_for_testing(gas_id, 1u64.into(), gas_owner),
        )
    }

    /// Build a single-transaction checkpoint and return the builder along
    /// with that transaction.
    fn build_single_tx(
        builder: TestCheckpointDataBuilder,
        f: impl FnOnce(TestCheckpointDataBuilder) -> TestCheckpointDataBuilder,
    ) -> (TestCheckpointDataBuilder, CheckpointTransaction) {
        let mut builder = f(builder.start_transaction(SENDER)).finish_transaction();
        let checkpoint = builder.build_checkpoint();
        (builder, checkpoint.transactions.into_iter().next().unwrap())
    }

    fn balance_changes(tx: &CheckpointTransaction) -> Vec<DerivedBalanceChange> {
        derive_balance_changes(&tx.effects, &tx.input_objects, &tx.output_objects, None).unwrap()
    }

    fn object_changes(tx: &CheckpointTransaction) -> Vec<DerivedObjectChange> {
        derive_object_changes(
            sender_address(),
            &tx.effects,
            &tx.input_objects,
            &tx.output_objects,
        )
        .unwrap()
    }

    #[test]
    fn balance_changes_for_coin_transfer() {
        let builder = TestCheckpointDataBuilder::new(1);
        let (builder, _) = build_single_tx(builder, |b| b.create_iota_object(0, 100));
        let (_, tx) = build_single_tx(builder, |b| b.transfer_coin_balance(0, 1, RECIPIENT, 30));

        let changes = balance_changes(&tx);
        assert_eq!(
            changes,
            // BTreeMap iteration order: sorted by (owner, coin type)
            {
                let mut expected = vec![
                    DerivedBalanceChange {
                        owner: Owner::Address(sender_address()),
                        coin_type: GAS::type_tag(),
                        amount: -30,
                    },
                    DerivedBalanceChange {
                        owner: Owner::Address(recipient_address()),
                        coin_type: GAS::type_tag(),
                        amount: 30,
                    },
                ];
                expected.sort_by(|a, b| (a.owner, &a.coin_type).cmp(&(b.owner, &b.coin_type)));
                expected
            }
        );
    }

    #[test]
    fn balance_changes_grouped_by_owner_and_coin_type() {
        let builder = TestCheckpointDataBuilder::new(1);
        let (builder, _) = build_single_tx(builder, |b| {
            b.create_iota_object(0, 100).create_iota_object(1, 50)
        });
        // Two coins of the same type transferred to the same recipient must
        // fold into a single entry per owner
        let (_, tx) = build_single_tx(builder, |b| {
            b.transfer_coin_balance(0, 2, RECIPIENT, 10)
                .transfer_coin_balance(1, 3, RECIPIENT, 20)
        });

        let changes = balance_changes(&tx);
        assert_eq!(changes.len(), 2);
        for change in &changes {
            match change.owner {
                Owner::Address(address) if address == sender_address() => {
                    assert_eq!(change.amount, -30)
                }
                Owner::Address(address) if address == recipient_address() => {
                    assert_eq!(change.amount, 30)
                }
                _ => panic!("unexpected owner: {:?}", change.owner),
            }
        }
    }

    #[test]
    fn balance_changes_zero_delta_dropped() {
        let builder = TestCheckpointDataBuilder::new(1);
        let (builder, _) = build_single_tx(builder, |b| b.create_iota_object(0, 100));
        // Mutating a coin without changing its balance must produce no entry
        // (this also covers the automatically mutated gas coin)
        let (_, tx) = build_single_tx(builder, |b| b.mutate_owned_object(0));

        assert_eq!(balance_changes(&tx), vec![]);
    }

    #[test]
    fn balance_changes_non_coin_objects_ignored() {
        // A created non-coin Move object (a treasury cap) must not produce a
        // balance change
        let treasury_cap = Object::treasury_cap_for_testing(
            iota_sdk_types::StructTag::new_gas(),
            iota_types::coin::TreasuryCap {
                id: iota_types::id::UID::new(ObjectId::random()),
                total_supply: iota_types::balance::Supply { value: 0 },
            },
        );
        let (transaction, gas_input, gas_output) = version_zero_gas_transaction();
        let effects = TestEffectsBuilder::new(&transaction)
            .with_created_objects([(treasury_cap.id(), *treasury_cap.owner())])
            .build();

        // The mutated gas coin cancels out; the treasury cap is not a coin
        assert_eq!(
            derive_balance_changes(&effects, &[gas_input], &[gas_output, treasury_cap], None),
            Ok(vec![])
        );
    }

    #[test]
    fn balance_changes_failed_tx_charges_gas_only() {
        let builder = TestCheckpointDataBuilder::new(1);
        let (_, tx) = build_single_tx(builder, |b| b.create_iota_object(0, 100));

        // Rebuild the effects as a failed execution with a non-zero gas charge
        let mut effects = TestEffectsBuilder::new(tx.transaction.data())
            .with_status(ExecutionStatus::Failure {
                error: iota_sdk_types::ExecutionError::InsufficientGas,
                command: None,
            })
            .build();
        effects.gas_cost_summary_mut_for_testing().computation_cost = 1000;

        // The failed path needs no objects at all
        let changes = derive_balance_changes(&effects, &[], &[], None);
        assert_eq!(
            changes,
            Ok(vec![DerivedBalanceChange {
                owner: Owner::Address(sender_address()),
                coin_type: GAS::type_tag(),
                amount: -1000,
            }])
        );
    }

    #[test]
    fn balance_changes_mocked_coin_excluded() {
        let builder = TestCheckpointDataBuilder::new(1);
        let (builder, _) = build_single_tx(builder, |b| b.create_iota_object(0, 100));
        let (_, tx) = build_single_tx(builder, |b| b.transfer_coin_balance(0, 1, RECIPIENT, 30));

        let changes = derive_balance_changes(
            &tx.effects,
            &tx.input_objects,
            &tx.output_objects,
            Some(object_id(0)),
        );
        assert_eq!(
            changes,
            Ok(vec![DerivedBalanceChange {
                owner: Owner::Address(recipient_address()),
                coin_type: GAS::type_tag(),
                amount: 30,
            }])
        );
    }

    #[test]
    fn balance_changes_missing_objects_error() {
        let builder = TestCheckpointDataBuilder::new(1);
        let (builder, _) = build_single_tx(builder, |b| b.create_iota_object(0, 100));
        let (_, tx) = build_single_tx(builder, |b| b.transfer_coin_balance(0, 1, RECIPIENT, 30));

        // A missing input coin would corrupt the delta, so the derivation
        // must refuse to produce a result
        assert!(matches!(
            derive_balance_changes(&tx.effects, &[], &tx.output_objects, None),
            Err(DeriveChangesError::MissingObject { .. })
        ));
    }

    #[test]
    fn object_changes_created_and_mutated() {
        let builder = TestCheckpointDataBuilder::new(1);
        let (builder, _) = build_single_tx(builder, |b| b.create_owned_object(0));
        let (_, tx) = build_single_tx(builder, |b| {
            b.create_owned_object(1).transfer_object(0, RECIPIENT)
        });

        let changes = object_changes(&tx);
        // created object 1, transferred (mutated) object 0, mutated gas coin
        assert_eq!(changes.len(), 3);
        assert!(changes.iter().any(|change| matches!(
            change,
            DerivedObjectChange::Created { object_id: id, sender, owner, .. }
                if *id == object_id(1)
                    && *sender == sender_address()
                    && *owner == Owner::Address(sender_address())
        )));
        let previous_version = tx
            .effects
            .modified_at_versions()
            .into_iter()
            .find_map(|(id, version)| (id == object_id(0)).then_some(version))
            .unwrap();
        assert!(changes.iter().any(|change| matches!(
            change,
            DerivedObjectChange::Mutated {
                object_id: id,
                owner,
                previous_version: previous,
                ..
            } if *id == object_id(0)
                && *owner == Owner::Address(recipient_address())
                && *previous == previous_version
        )));
    }

    #[test]
    fn object_changes_wrapped_deleted_unwrapped() {
        let builder = TestCheckpointDataBuilder::new(1);
        let (builder, _) =
            build_single_tx(builder, |b| b.create_owned_object(0).create_owned_object(1));
        let (builder, tx) = build_single_tx(builder, |b| b.wrap_object(0).delete_object(1));

        let changes = object_changes(&tx);
        assert!(changes.iter().any(|change| matches!(
            change,
            DerivedObjectChange::Wrapped { object_id: id, sender, .. }
                if *id == object_id(0) && *sender == sender_address()
        )));
        assert!(changes.iter().any(|change| matches!(
            change,
            DerivedObjectChange::Deleted { object_id: id, .. } if *id == object_id(1)
        )));

        let (_, tx) = build_single_tx(builder, |b| b.unwrap_object(0));
        let changes = object_changes(&tx);
        assert!(changes.iter().any(|change| matches!(
            change,
            DerivedObjectChange::Unwrapped { object_id: id, .. } if *id == object_id(0)
        )));
    }

    #[test]
    fn object_changes_published_package() {
        let module = move_binary_format::file_format::empty_module();
        let package =
            Object::new_package_for_testing(&[module], TransactionDigest::GENESIS_MARKER, [])
                .unwrap();
        let package_id = package.id();

        let (transaction, gas_input, gas_output) = version_zero_gas_transaction();
        let effects = TestEffectsBuilder::new(&transaction)
            .with_created_objects([(package_id, Owner::Immutable)])
            .build();
        let created_version = effects
            .all_changed_objects()
            .into_iter()
            .find_map(|(object_ref, _, kind)| {
                (kind == WriteKind::Create).then_some(object_ref.version)
            })
            .unwrap();
        assert_eq!(
            created_version,
            package.version(),
            "test setup: package version must match the effects' created version"
        );

        let changes = derive_object_changes(
            sender_address(),
            &effects,
            &[gas_input],
            &[gas_output, package],
        )
        .unwrap();
        assert!(changes.iter().any(|change| matches!(
            change,
            DerivedObjectChange::Published { package_id: id, modules, .. }
                if *id == package_id && !modules.is_empty()
        )));
    }

    #[test]
    fn object_changes_missing_objects_error() {
        let builder = TestCheckpointDataBuilder::new(1);
        let (_, tx) = build_single_tx(builder, |b| b.create_owned_object(0));

        // A missing object would silently drop an entry from the result, so
        // the derivation must refuse to produce one
        assert!(matches!(
            derive_object_changes(sender_address(), &tx.effects, &[], &[]),
            Err(DeriveChangesError::MissingObject { .. })
        ));
    }
}
