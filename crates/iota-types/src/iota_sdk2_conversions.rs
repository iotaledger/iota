// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Module for conversions between iota-core types and iota-sdk types
//!
//! For now this module makes heavy use of the `bcs_convert_impl` macro to
//! implement the `From` trait for converting between core and external sdk
//! types, relying on the fact that the BCS format of these types are strictly
//! identical. As time goes on we'll slowly hand implement these impls
//! directly to avoid going through the BCS machinery.

use fastcrypto::traits::ToFromBytes;
use iota_sdk2::types::{
    object::{MovePackage, MoveStruct},
    *,
};
use move_core_types::language_storage::ModuleId;

use crate::transaction::TransactionDataAPI as _;

impl From<crate::object::Object> for Object {
    fn from(value: crate::object::Object) -> Self {
        Self {
            data: value.data.clone().into(),
            owner: value.owner.into(),
            previous_transaction: value.previous_transaction.into(),
            storage_rebate: value.storage_rebate,
        }
    }
}

impl From<Object> for crate::object::Object {
    fn from(value: Object) -> Self {
        Self::new_from_genesis(
            value.data.into(),
            value.owner.into(),
            value.previous_transaction.into(),
        )
    }
}

impl From<crate::object::Data> for ObjectData {
    fn from(value: crate::object::Data) -> Self {
        match value {
            crate::object::Data::Move(move_object) => Self::Struct(move_object_to_sdk(move_object)),
            crate::object::Data::Package(move_package) => {
                Self::Package(move_package_to_sdk(move_package))
            }
        }
    }
}

impl From<ObjectData> for crate::object::Data {
    fn from(value: ObjectData) -> Self {
        match value {
            ObjectData::Struct(move_object) => Self::Move(sdk_object_to_move(move_object)),
            ObjectData::Package(move_package) => Self::Package(sdk_package_to_move(move_package)),
        }
    }
}

fn move_type_tag_to_sdk(tt: move_core_types::language_storage::TypeTag) -> TypeTag {
    use move_core_types::language_storage::TypeTag as MoveTypeTag;
    match tt {
        MoveTypeTag::Bool => TypeTag::Bool,
        MoveTypeTag::U8 => TypeTag::U8,
        MoveTypeTag::U64 => TypeTag::U64,
        MoveTypeTag::U128 => TypeTag::U128,
        MoveTypeTag::Address => TypeTag::Address,
        MoveTypeTag::Signer => TypeTag::Signer,
        MoveTypeTag::Vector(type_tag) => TypeTag::Vector(Box::new(move_type_tag_to_sdk(*type_tag))),
        MoveTypeTag::Struct(struct_tag) => {
            TypeTag::Struct(Box::new(move_struct_tag_to_sdk(*struct_tag)))
        }
        MoveTypeTag::U16 => TypeTag::U16,
        MoveTypeTag::U32 => TypeTag::U32,
        MoveTypeTag::U256 => TypeTag::U256,
    }
}

fn move_struct_tag_to_sdk(st: move_core_types::language_storage::StructTag) -> StructTag {
    StructTag {
        address: Address::new(st.address.into_bytes()),
        module: Identifier::new(st.module.as_str()).expect("module identifier conversion failed"),
        name: Identifier::new(st.name.as_str()).expect("struct name identifier conversion failed"),
        type_params: st
            .type_params
            .into_iter()
            .map(move_type_tag_to_sdk)
            .collect(),
    }
}

fn move_package_to_sdk(package: crate::move_package::MovePackage) -> MovePackage {
    MovePackage {
        id: package.id().into(),
        version: package.version().value(),
        modules: package
            .module_map
            .into_iter()
            .map(|(name, bytes)| {
                (
                    Identifier::new(name).expect("package name identifier conversion failed"),
                    bytes,
                )
            })
            .collect(),
        type_origin_table: package
            .type_origin_table
            .into_iter()
            .map(move_type_origin_to_sdk)
            .collect(),
        linkage_table: package
            .linkage_table
            .into_iter()
            .map(|(id, info)| (id.into(), move_upgrade_info_to_sdk(info)))
            .collect(),
    }
}

fn sdk_package_to_move(package: MovePackage) -> crate::move_package::MovePackage {
    crate::move_package::MovePackage {
        id: package.id.into(),
        version: package.version.into(),
        module_map: package
            .modules
            .into_iter()
            .map(|(name, bytes)| (name.to_string(), bytes))
            .collect(),
        type_origin_table: package
            .type_origin_table
            .into_iter()
            .map(sdk_type_origin_to_move)
            .collect(),
        linkage_table: package
            .linkage_table
            .into_iter()
            .map(|(id, info)| (id.into(), sdk_upgrade_info_to_move(info)))
            .collect(),
    }
}

fn move_object_to_sdk(obj: crate::object::MoveObject) -> MoveStruct {
    MoveStruct {
        type_: move_object_type_to_sdk(obj.type_),
        version: obj.version.value(),
        contents: obj.contents,
    }
}

fn sdk_object_to_move(obj: MoveStruct) -> crate::object::MoveObject {
    crate::object::MoveObject {
        type_: sdk_object_type_to_move(obj.type_),
        version: obj.version.into(),
        contents: obj.contents,
    }
}

fn move_object_type_to_sdk(type_: crate::base_types::MoveObjectType) -> StructTag {
    move_struct_tag_to_sdk(move_core_types::language_storage::StructTag {
        address: type_.address(),
        module: type_.module().to_owned(),
        name: type_.name().to_owned(),
        type_params: type_.type_params(),
    })
}

fn sdk_object_type_to_move(type_: StructTag) -> crate::base_types::MoveObjectType {
    crate::base_types::MoveObjectType::from(move_core_types::language_storage::StructTag {
        address: move_core_types::account_address::AccountAddress::new(type_.address.into_inner()),
        module: crate::Identifier::new(type_.module.as_str())
            .expect("struct module name conversion failed"),
        name: crate::Identifier::new(type_.name.as_str()).expect("struct name conversion failed"),
        type_params: type_
            .type_params
            .into_iter()
            .map(type_tag_sdk_to_core)
            .collect(),
    })
}

fn move_type_origin_to_sdk(origin: crate::move_package::TypeOrigin) -> TypeOrigin {
    TypeOrigin {
        module_name: Identifier::new(&origin.module_name)
            .expect("module identifier conversion failed"),
        struct_name: Identifier::new(&origin.datatype_name)
            .expect("struct identifier conversion failed"),
        package: origin.package.into(),
    }
}

fn sdk_type_origin_to_move(origin: TypeOrigin) -> crate::move_package::TypeOrigin {
    crate::move_package::TypeOrigin {
        module_name: origin.module_name.to_string(),
        datatype_name: origin.struct_name.to_string(),
        package: origin.package.into(),
    }
}

fn move_upgrade_info_to_sdk(info: crate::move_package::UpgradeInfo) -> UpgradeInfo {
    UpgradeInfo {
        upgraded_id: info.upgraded_id.into(),
        upgraded_version: info.upgraded_version.value(),
    }
}

fn sdk_upgrade_info_to_move(info: UpgradeInfo) -> crate::move_package::UpgradeInfo {
    crate::move_package::UpgradeInfo {
        upgraded_id: info.upgraded_id.into(),
        upgraded_version: info.upgraded_version.into(),
    }
}

impl From<crate::transaction::TransactionData> for Transaction {
    fn from(value: crate::transaction::TransactionData) -> Self {
        Self {
            sender: Address::new(value.sender().to_inner()),
            gas_payment: GasPayment {
                objects: value
                    .gas()
                    .iter()
                    .map(|(id, seq, digest)| {
                        ObjectReference::new((*id).into(), seq.value(), (*digest).into())
                    })
                    .collect(),
                owner: Address::new(value.gas_data().owner.to_inner()),
                price: value.gas_data().price,
                budget: value.gas_data().budget,
            },
            expiration: match value.expiration() {
                crate::transaction::TransactionExpiration::None => TransactionExpiration::None,
                crate::transaction::TransactionExpiration::Epoch(e) => {
                    TransactionExpiration::Epoch(*e)
                }
            },
            kind: value.into_kind().into(),
        }
    }
}

impl From<Transaction> for crate::transaction::TransactionData {
    fn from(value: Transaction) -> Self {
        Self::new_with_gas_data(
            value.kind.into(),
            value.sender.into(),
            crate::transaction::GasData {
                payment: value
                    .gas_payment
                    .objects
                    .into_iter()
                    .map(ObjectReference::into_parts)
                    .map(|(id, seq, digest)| (id.into(), seq.into(), digest.into()))
                    .collect(),
                owner: value.gas_payment.owner.into(),
                price: value.gas_payment.price,
                budget: value.gas_payment.budget,
            },
        )
    }
}

impl From<crate::transaction::TransactionKind> for TransactionKind {
    fn from(value: crate::transaction::TransactionKind) -> Self {
        use crate::transaction::TransactionKind as InternalTxnKind;
        match value {
            InternalTxnKind::ProgrammableTransaction(programmable_transaction) => {
                TransactionKind::ProgrammableTransaction(ProgrammableTransaction {
                    inputs: programmable_transaction
                        .inputs
                        .into_iter()
                        .map(Into::into)
                        .collect(),
                    commands: programmable_transaction
                        .commands
                        .into_iter()
                        .map(Into::into)
                        .collect(),
                })
            }
            InternalTxnKind::Genesis(genesis_transaction) => {
                TransactionKind::Genesis(GenesisTransaction {
                    objects: genesis_transaction
                        .objects
                        .into_iter()
                        .map(|obj| match obj {
                            crate::transaction::GenesisObject::RawObject { data, owner } => {
                                GenesisObject {
                                    data: data.into(),
                                    owner: owner.into(),
                                }
                            }
                        })
                        .collect(),
                    events: genesis_transaction
                        .events
                        .into_iter()
                        .map(|event| Event {
                            package_id: event.package_id.into(),
                            module: Identifier::new(event.transaction_module.as_str())
                                .expect("invalid transaction module"),
                            sender: event.sender.into(),
                            type_: struct_tag_core_to_sdk(event.type_),
                            contents: event.contents,
                        })
                        .collect(),
                })
            }
            InternalTxnKind::ConsensusCommitPrologueV1(consensus_commit_prologue_v1) => {
                let consensus_determined_version_assignments = match consensus_commit_prologue_v1.consensus_determined_version_assignments {
                    crate::messages_consensus::ConsensusDeterminedVersionAssignments::CancelledTransactions(vec) =>
                        ConsensusDeterminedVersionAssignments::CancelledTransactions {
                            cancelled_transactions: vec.into_iter().map(|value| CancelledTransaction {
                                digest: value.0.into(),
                                version_assignments:
                                    value
                                        .1
                                        .into_iter()
                                        .map(|value| VersionAssignment { object_id: value.0.into(), version: value.1.value() })
                                        .collect(),
                            }).collect()
                        },
                };
                TransactionKind::ConsensusCommitPrologueV1(ConsensusCommitPrologueV1 {
                    epoch: consensus_commit_prologue_v1.epoch,
                    round: consensus_commit_prologue_v1.round,
                    sub_dag_index: consensus_commit_prologue_v1.sub_dag_index,
                    commit_timestamp_ms: consensus_commit_prologue_v1.commit_timestamp_ms,
                    consensus_commit_digest: consensus_commit_prologue_v1
                        .consensus_commit_digest
                        .into(),
                    consensus_determined_version_assignments,
                })
            }
            InternalTxnKind::AuthenticatorStateUpdateV1(authenticator_state_update_v1) => {
                TransactionKind::AuthenticatorStateUpdateV1(AuthenticatorStateUpdateV1 {
                    epoch: authenticator_state_update_v1.epoch,
                    round: authenticator_state_update_v1.round,
                    new_active_jwks: authenticator_state_update_v1
                        .new_active_jwks
                        .into_iter()
                        .map(|jwk| ActiveJwk {
                            jwk_id: JwkId {
                                iss: jwk.jwk_id.iss,
                                kid: jwk.jwk_id.kid,
                            },
                            jwk: Jwk {
                                kty: jwk.jwk.kty,
                                e: jwk.jwk.e,
                                n: jwk.jwk.n,
                                alg: jwk.jwk.alg,
                            },
                            epoch: jwk.epoch,
                        })
                        .collect(),
                    authenticator_obj_initial_shared_version: authenticator_state_update_v1
                        .authenticator_obj_initial_shared_version
                        .value(),
                })
            }
            InternalTxnKind::EndOfEpochTransaction(vec) => {
                TransactionKind::EndOfEpoch(vec.into_iter().map(Into::into).collect())
            }
            InternalTxnKind::RandomnessStateUpdate(randomness_state_update) => {
                TransactionKind::RandomnessStateUpdate(RandomnessStateUpdate {
                    epoch: randomness_state_update.epoch,
                    randomness_round: randomness_state_update.randomness_round.0,
                    random_bytes: randomness_state_update.random_bytes,
                    randomness_obj_initial_shared_version: randomness_state_update
                        .randomness_obj_initial_shared_version
                        .value(),
                })
            }
        }
    }
}

impl From<TransactionKind> for crate::transaction::TransactionKind {
    fn from(value: TransactionKind) -> Self {
        match value {
            TransactionKind::ProgrammableTransaction(programmable_transaction) => {
                Self::ProgrammableTransaction(crate::transaction::ProgrammableTransaction {
                    inputs: programmable_transaction
                        .inputs
                        .into_iter()
                        .map(Into::into)
                        .collect(),
                    commands: programmable_transaction
                        .commands
                        .into_iter()
                        .map(Into::into)
                        .collect(),
                })
            }
            TransactionKind::Genesis(genesis_transaction) => {
                Self::Genesis(crate::transaction::GenesisTransaction {
                    objects: genesis_transaction
                        .objects
                        .into_iter()
                        .map(|obj| crate::transaction::GenesisObject::RawObject {
                            data: obj.data.into(),
                            owner: obj.owner.into(),
                        })
                        .collect(),
                    events: genesis_transaction
                        .events
                        .into_iter()
                        .map(|event| crate::event::Event {
                            package_id: event.package_id.into(),
                            transaction_module: crate::Identifier::new(event.module.as_str())
                                .expect("invalid transaction module"),
                            sender: event.sender.into(),
                            type_: struct_tag_sdk_to_core(event.type_),
                            contents: event.contents,
                        })
                        .collect(),
                })
            }
            TransactionKind::ConsensusCommitPrologueV1(consensus_commit_prologue_v1) => {
                let consensus_determined_version_assignments = match consensus_commit_prologue_v1.consensus_determined_version_assignments {
                    ConsensusDeterminedVersionAssignments::CancelledTransactions{ cancelled_transactions } =>
                    crate::messages_consensus::ConsensusDeterminedVersionAssignments::CancelledTransactions(
                        cancelled_transactions.into_iter().map(|value|
                            (
                                value.digest.into(),
                                value
                                    .version_assignments
                                    .into_iter()
                                    .map(|value| (value.object_id.into(), value.version.into()))
                                    .collect()
                            )
                        ).collect()
                    ),
                };
                Self::ConsensusCommitPrologueV1(
                    crate::messages_consensus::ConsensusCommitPrologueV1 {
                        epoch: consensus_commit_prologue_v1.epoch,
                        round: consensus_commit_prologue_v1.round,
                        sub_dag_index: consensus_commit_prologue_v1.sub_dag_index,
                        commit_timestamp_ms: consensus_commit_prologue_v1.commit_timestamp_ms,
                        consensus_commit_digest: consensus_commit_prologue_v1
                            .consensus_commit_digest
                            .into(),
                        consensus_determined_version_assignments,
                    },
                )
            }
            TransactionKind::AuthenticatorStateUpdateV1(authenticator_state_update_v1) => {
                Self::AuthenticatorStateUpdateV1(crate::transaction::AuthenticatorStateUpdateV1 {
                    epoch: authenticator_state_update_v1.epoch,
                    round: authenticator_state_update_v1.round,
                    new_active_jwks: authenticator_state_update_v1
                        .new_active_jwks
                        .into_iter()
                        .map(|jwk| crate::authenticator_state::ActiveJwk {
                            jwk_id: crate::authenticator_state::JwkId {
                                iss: jwk.jwk_id.iss,
                                kid: jwk.jwk_id.kid,
                            },
                            jwk: crate::authenticator_state::JWK {
                                kty: jwk.jwk.kty,
                                e: jwk.jwk.e,
                                n: jwk.jwk.n,
                                alg: jwk.jwk.alg,
                            },
                            epoch: jwk.epoch,
                        })
                        .collect(),
                    authenticator_obj_initial_shared_version: authenticator_state_update_v1
                        .authenticator_obj_initial_shared_version
                        .into(),
                })
            }
            TransactionKind::EndOfEpoch(vec) => {
                Self::EndOfEpochTransaction(vec.into_iter().map(Into::into).collect())
            }
            TransactionKind::RandomnessStateUpdate(randomness_state_update) => {
                Self::RandomnessStateUpdate(crate::transaction::RandomnessStateUpdate {
                    epoch: randomness_state_update.epoch,
                    randomness_round: crate::crypto::RandomnessRound(
                        randomness_state_update.randomness_round,
                    ),
                    random_bytes: randomness_state_update.random_bytes,
                    randomness_obj_initial_shared_version: randomness_state_update
                        .randomness_obj_initial_shared_version
                        .into(),
                })
            }
        }
    }
}

impl From<crate::transaction::EndOfEpochTransactionKind> for EndOfEpochTransactionKind {
    fn from(value: crate::transaction::EndOfEpochTransactionKind) -> Self {
        match value {
            crate::transaction::EndOfEpochTransactionKind::ChangeEpoch(change_epoch) => {
                EndOfEpochTransactionKind::ChangeEpoch(ChangeEpoch {
                    epoch: change_epoch.epoch,
                    protocol_version: change_epoch.protocol_version.as_u64(),
                    storage_charge: change_epoch.storage_charge,
                    computation_charge: change_epoch.computation_charge,
                    storage_rebate: change_epoch.storage_rebate,
                    non_refundable_storage_fee: change_epoch.non_refundable_storage_fee,
                    epoch_start_timestamp_ms: change_epoch.epoch_start_timestamp_ms,
                    system_packages: change_epoch
                        .system_packages
                        .into_iter()
                        .map(|(version, modules, dependencies)| SystemPackage {
                            version: version.value(),
                            modules,
                            dependencies: dependencies.into_iter().map(Into::into).collect(),
                        })
                        .collect(),
                })
            }
            crate::transaction::EndOfEpochTransactionKind::AuthenticatorStateCreate => {
                EndOfEpochTransactionKind::AuthenticatorStateCreate
            }
            crate::transaction::EndOfEpochTransactionKind::AuthenticatorStateExpire(
                authenticator_state_expire,
            ) => EndOfEpochTransactionKind::AuthenticatorStateExpire(AuthenticatorStateExpire {
                min_epoch: authenticator_state_expire.min_epoch,
                authenticator_obj_initial_shared_version: authenticator_state_expire
                    .authenticator_obj_initial_shared_version
                    .value(),
            }),
            crate::transaction::EndOfEpochTransactionKind::BridgeStateCreate(chain_identifier) => {
                EndOfEpochTransactionKind::BridgeStateCreate {
                    chain_id: CheckpointDigest::new(chain_identifier.digest().into()),
                }
            }
            crate::transaction::EndOfEpochTransactionKind::BridgeCommitteeInit(sequence_number) => {
                EndOfEpochTransactionKind::BridgeCommitteeInit {
                    bridge_object_version: sequence_number.value(),
                }
            }
        }
    }
}

impl From<EndOfEpochTransactionKind> for crate::transaction::EndOfEpochTransactionKind {
    fn from(value: EndOfEpochTransactionKind) -> Self {
        match value {
            EndOfEpochTransactionKind::ChangeEpoch(change_epoch) => {
                Self::ChangeEpoch(crate::transaction::ChangeEpoch {
                    epoch: change_epoch.epoch,
                    protocol_version: change_epoch.protocol_version.into(),
                    storage_charge: change_epoch.storage_charge,
                    computation_charge: change_epoch.computation_charge,
                    storage_rebate: change_epoch.storage_rebate,
                    non_refundable_storage_fee: change_epoch.non_refundable_storage_fee,
                    epoch_start_timestamp_ms: change_epoch.epoch_start_timestamp_ms,
                    system_packages: change_epoch
                        .system_packages
                        .into_iter()
                        .map(|package| {
                            (
                                package.version.into(),
                                package.modules,
                                package.dependencies.into_iter().map(Into::into).collect(),
                            )
                        })
                        .collect(),
                })
            }
            EndOfEpochTransactionKind::AuthenticatorStateCreate => Self::AuthenticatorStateCreate,
            EndOfEpochTransactionKind::AuthenticatorStateExpire(authenticator_state_expire) => {
                Self::AuthenticatorStateExpire(crate::transaction::AuthenticatorStateExpire {
                    min_epoch: authenticator_state_expire.min_epoch,
                    authenticator_obj_initial_shared_version: authenticator_state_expire
                        .authenticator_obj_initial_shared_version
                        .into(),
                })
            }
            EndOfEpochTransactionKind::BridgeStateCreate { chain_id } => {
                Self::BridgeStateCreate(crate::digests::ChainIdentifier(chain_id.into()))
            }
            EndOfEpochTransactionKind::BridgeCommitteeInit {
                bridge_object_version,
            } => Self::BridgeCommitteeInit(bridge_object_version.into()),
        }
    }
}

impl From<crate::transaction::CallArg> for InputArgument {
    fn from(value: crate::transaction::CallArg) -> Self {
        match value {
            crate::transaction::CallArg::Pure(vec) => Self::Pure { value: vec },
            crate::transaction::CallArg::Object(object_arg) => match object_arg {
                crate::transaction::ObjectArg::ImmOrOwnedObject(obj_ref) => {
                    Self::ImmutableOrOwned(core_obj_ref_to_sdk(obj_ref))
                }
                crate::transaction::ObjectArg::SharedObject {
                    id,
                    initial_shared_version,
                    mutable,
                } => Self::Shared {
                    object_id: id.into(),
                    initial_shared_version: initial_shared_version.value(),
                    mutable,
                },
                crate::transaction::ObjectArg::Receiving(obj_ref) => {
                    Self::Receiving(core_obj_ref_to_sdk(obj_ref))
                }
            },
        }
    }
}

impl From<InputArgument> for crate::transaction::CallArg {
    fn from(value: InputArgument) -> Self {
        use crate::transaction::ObjectArg;
        match value {
            InputArgument::Pure { value } => Self::Pure(value),
            InputArgument::ImmutableOrOwned(object_reference) => Self::Object(
                ObjectArg::ImmOrOwnedObject(sdk_obj_ref_to_core(object_reference)),
            ),
            InputArgument::Shared {
                object_id,
                initial_shared_version,
                mutable,
            } => Self::Object(ObjectArg::SharedObject {
                id: object_id.into(),
                initial_shared_version: initial_shared_version.into(),
                mutable,
            }),
            InputArgument::Receiving(object_reference) => {
                Self::Object(ObjectArg::Receiving(sdk_obj_ref_to_core(object_reference)))
            }
        }
    }
}

fn core_obj_ref_to_sdk(obj_ref: crate::base_types::ObjectRef) -> ObjectReference {
    ObjectReference::new(obj_ref.0.into(), obj_ref.1.value(), obj_ref.2.into())
}

fn sdk_obj_ref_to_core(obj_ref: ObjectReference) -> crate::base_types::ObjectRef {
    let (id, seq, digest) = obj_ref.into_parts();
    (id.into(), seq.into(), digest.into())
}

impl From<crate::effects::TransactionEffects> for TransactionEffects {
    fn from(value: crate::effects::TransactionEffects) -> Self {
        match value {
            crate::effects::TransactionEffects::V1(effects) => {
                Self::V1(Box::new(TransactionEffectsV1 {
                    epoch: effects.executed_epoch,
                    gas_used: GasCostSummary::new(
                        effects.gas_used.computation_cost,
                        effects.gas_used.computation_cost_burned,
                        effects.gas_used.storage_cost,
                        effects.gas_used.storage_rebate,
                        effects.gas_used.non_refundable_storage_fee,
                    ),
                    gas_object_index: effects.gas_object_index,
                    transaction_digest: effects.transaction_digest.into(),
                    events_digest: effects.events_digest.map(Into::into),
                    dependencies: effects.dependencies.into_iter().map(Into::into).collect(),
                    lamport_version: effects.lamport_version.value(),
                    changed_objects: effects
                        .changed_objects
                        .into_iter()
                        .map(|(id, change)| ChangedObject {
                            object_id: id.into(),
                            change: EffectsObjectChange {
                                input_state: match change.input_state {
                                    crate::effects::ObjectIn::NotExist => ObjectIn::NotExist,
                                    crate::effects::ObjectIn::Exist(((version, digest), owner)) => {
                                        ObjectIn::Exist {
                                            version: version.value(),
                                            digest: digest.into(),
                                            owner: owner.into(),
                                        }
                                    }
                                },
                                output_state: match change.output_state {
                                    crate::effects::ObjectOut::NotExist => ObjectOut::NotExist,
                                    crate::effects::ObjectOut::ObjectWrite((digest, owner)) => {
                                        ObjectOut::ObjectWrite {
                                            digest: digest.into(),
                                            owner: owner.into(),
                                        }
                                    }
                                    crate::effects::ObjectOut::PackageWrite((seq, digest)) => {
                                        ObjectOut::PackageWrite {
                                            version: seq.value(),
                                            digest: digest.into(),
                                        }
                                    }
                                },
                                id_operation: match change.id_operation {
                                    crate::effects::IDOperation::None => IdOperation::None,
                                    crate::effects::IDOperation::Created => IdOperation::Created,
                                    crate::effects::IDOperation::Deleted => IdOperation::Deleted,
                                },
                            },
                        })
                        .collect(),
                    unchanged_shared_objects: effects
                        .unchanged_shared_objects
                        .into_iter()
                        .map(|(id, kind)| UnchangedSharedObject {
                            object_id: id.into(),
                            kind: match kind {
                                crate::effects::UnchangedSharedKind::ReadOnlyRoot((
                                    version,
                                    digest,
                                )) => UnchangedSharedKind::ReadOnlyRoot {
                                    version: version.value(),
                                    digest: digest.into(),
                                },
                                crate::effects::UnchangedSharedKind::MutateDeleted(
                                    sequence_number,
                                ) => UnchangedSharedKind::MutateDeleted {
                                    version: sequence_number.value(),
                                },
                                crate::effects::UnchangedSharedKind::ReadDeleted(
                                    sequence_number,
                                ) => UnchangedSharedKind::ReadDeleted {
                                    version: sequence_number.value(),
                                },
                                crate::effects::UnchangedSharedKind::Cancelled(sequence_number) => {
                                    UnchangedSharedKind::Cancelled {
                                        version: sequence_number.value(),
                                    }
                                }
                                crate::effects::UnchangedSharedKind::PerEpochConfig => {
                                    UnchangedSharedKind::PerEpochConfig
                                }
                            },
                        })
                        .collect(),
                    auxiliary_data_digest: effects.aux_data_digest.map(Into::into),
                    status: effects.status.into(),
                }))
            }
        }
    }
}

impl From<TransactionEffects> for crate::effects::TransactionEffects {
    fn from(value: TransactionEffects) -> Self {
        match value {
            TransactionEffects::V1(transaction_effects_v1) => {
                crate::effects::effects_v1::TransactionEffectsV1 {
                    status: transaction_effects_v1.status.into(),
                    executed_epoch: transaction_effects_v1.epoch,
                    gas_used: crate::gas::GasCostSummary::new(
                        transaction_effects_v1.gas_used.computation_cost,
                        transaction_effects_v1.gas_used.computation_cost_burned,
                        transaction_effects_v1.gas_used.storage_cost,
                        transaction_effects_v1.gas_used.storage_rebate,
                        transaction_effects_v1.gas_used.non_refundable_storage_fee,
                    ),
                    transaction_digest: transaction_effects_v1.transaction_digest.into(),
                    gas_object_index: transaction_effects_v1.gas_object_index,
                    events_digest: transaction_effects_v1.events_digest.map(Into::into),
                    dependencies: transaction_effects_v1
                        .dependencies
                        .into_iter()
                        .map(Into::into)
                        .collect(),
                    lamport_version: transaction_effects_v1.lamport_version.into(),
                    changed_objects: transaction_effects_v1
                        .changed_objects
                        .into_iter()
                        .map(|obj| {
                            (
                                obj.object_id.into(),
                                crate::effects::EffectsObjectChange {
                                    input_state: match obj.change.input_state {
                                        ObjectIn::NotExist => crate::effects::ObjectIn::NotExist,
                                        ObjectIn::Exist {
                                            version,
                                            digest,
                                            owner,
                                        } => crate::effects::ObjectIn::Exist((
                                            (version.into(), digest.into()),
                                            owner.into(),
                                        )),
                                    },
                                    output_state: match obj.change.output_state {
                                        ObjectOut::NotExist => crate::effects::ObjectOut::NotExist,
                                        ObjectOut::ObjectWrite { digest, owner } => {
                                            crate::effects::ObjectOut::ObjectWrite((
                                                digest.into(),
                                                owner.into(),
                                            ))
                                        }
                                        ObjectOut::PackageWrite { version, digest } => {
                                            crate::effects::ObjectOut::PackageWrite((
                                                version.into(),
                                                digest.into(),
                                            ))
                                        }
                                    },
                                    id_operation: match obj.change.id_operation {
                                        IdOperation::None => crate::effects::IDOperation::None,
                                        IdOperation::Created => {
                                            crate::effects::IDOperation::Created
                                        }
                                        IdOperation::Deleted => {
                                            crate::effects::IDOperation::Deleted
                                        }
                                    },
                                },
                            )
                        })
                        .collect(),
                    unchanged_shared_objects: transaction_effects_v1
                        .unchanged_shared_objects
                        .into_iter()
                        .map(|obj| {
                            (
                                obj.object_id.into(),
                                match obj.kind {
                                    UnchangedSharedKind::ReadOnlyRoot { version, digest } => {
                                        crate::effects::UnchangedSharedKind::ReadOnlyRoot((
                                            version.into(),
                                            digest.into(),
                                        ))
                                    }
                                    UnchangedSharedKind::MutateDeleted { version } => {
                                        crate::effects::UnchangedSharedKind::MutateDeleted(
                                            version.into(),
                                        )
                                    }
                                    UnchangedSharedKind::ReadDeleted { version } => {
                                        crate::effects::UnchangedSharedKind::ReadDeleted(
                                            version.into(),
                                        )
                                    }
                                    UnchangedSharedKind::Cancelled { version } => {
                                        crate::effects::UnchangedSharedKind::Cancelled(
                                            version.into(),
                                        )
                                    }
                                    UnchangedSharedKind::PerEpochConfig => {
                                        crate::effects::UnchangedSharedKind::PerEpochConfig
                                    }
                                },
                            )
                        })
                        .collect(),
                    aux_data_digest: transaction_effects_v1.auxiliary_data_digest.map(Into::into),
                }
                .into()
            }
        }
    }
}

macro_rules! impl_convert_digest {
    ($name:ident) => {
        impl From<crate::digests::$name> for $name {
            fn from(value: crate::digests::$name) -> Self {
                Self::new(value.into_inner())
            }
        }

        impl From<$name> for crate::digests::$name {
            fn from(value: $name) -> Self {
                Self::new(value.into_inner())
            }
        }
    };
}

impl_convert_digest!(Digest);
impl_convert_digest!(ObjectDigest);
impl_convert_digest!(CheckpointDigest);
impl_convert_digest!(TransactionDigest);
impl_convert_digest!(TransactionEffectsDigest);
impl_convert_digest!(TransactionEventsDigest);
impl_convert_digest!(CheckpointContentsDigest);
impl_convert_digest!(ConsensusCommitDigest);

impl From<crate::digests::EffectsAuxDataDigest> for EffectsAuxiliaryDataDigest {
    fn from(value: crate::digests::EffectsAuxDataDigest) -> Self {
        Self::new(value.into_inner())
    }
}

impl From<EffectsAuxiliaryDataDigest> for crate::digests::EffectsAuxDataDigest {
    fn from(value: EffectsAuxiliaryDataDigest) -> Self {
        Self::new(value.into_inner())
    }
}

impl From<crate::execution_status::ExecutionStatus> for ExecutionStatus {
    fn from(value: crate::execution_status::ExecutionStatus) -> Self {
        match value {
            crate::execution_status::ExecutionStatus::Success => Self::Success,
            crate::execution_status::ExecutionStatus::Failure { error, command } => Self::Failure {
                error: error.into(),
                command: command.map(|v| v as u64),
            },
        }
    }
}

impl From<ExecutionStatus> for crate::execution_status::ExecutionStatus {
    fn from(value: ExecutionStatus) -> Self {
        match value {
            ExecutionStatus::Success => Self::Success,
            ExecutionStatus::Failure { error, command } => Self::Failure {
                error: error.into(),
                command: command.map(|v| v as usize),
            },
        }
    }
}

impl From<crate::execution_status::ExecutionFailureStatus> for ExecutionError {
    fn from(value: crate::execution_status::ExecutionFailureStatus) -> Self {
        use crate::execution_status::ExecutionFailureStatus;
        match value {
            ExecutionFailureStatus::InsufficientGas => Self::InsufficientGas,
            ExecutionFailureStatus::InvalidGasObject => Self::InvalidGasObject,
            ExecutionFailureStatus::InvariantViolation => Self::InvariantViolation,
            ExecutionFailureStatus::FeatureNotYetSupported => Self::FeatureNotYetSupported,
            ExecutionFailureStatus::MoveObjectTooBig {
                object_size,
                max_object_size,
            } => Self::ObjectTooBig {
                object_size,
                max_object_size,
            },
            ExecutionFailureStatus::MovePackageTooBig {
                object_size,
                max_object_size,
            } => Self::PackageTooBig {
                object_size,
                max_object_size,
            },
            ExecutionFailureStatus::CircularObjectOwnership { object } => {
                Self::CircularObjectOwnership {
                    object: object.into(),
                }
            }
            ExecutionFailureStatus::InsufficientCoinBalance => Self::InsufficientCoinBalance,
            ExecutionFailureStatus::CoinBalanceOverflow => Self::CoinBalanceOverflow,
            ExecutionFailureStatus::PublishErrorNonZeroAddress => Self::PublishErrorNonZeroAddress,
            ExecutionFailureStatus::IotaMoveVerificationError => Self::IotaMoveVerificationError,
            ExecutionFailureStatus::MovePrimitiveRuntimeError(move_location_opt) => {
                Self::MovePrimitiveRuntimeError {
                    location: move_location_opt.0.map(Into::into),
                }
            }
            ExecutionFailureStatus::MoveAbort(move_location, code) => Self::MoveAbort {
                location: move_location.into(),
                code,
            },
            ExecutionFailureStatus::VMVerificationOrDeserializationError => {
                Self::VmVerificationOrDeserializationError
            }
            ExecutionFailureStatus::VMInvariantViolation => Self::VmInvariantViolation,
            ExecutionFailureStatus::FunctionNotFound => Self::FunctionNotFound,
            ExecutionFailureStatus::ArityMismatch => Self::ArityMismatch,
            ExecutionFailureStatus::TypeArityMismatch => Self::TypeArityMismatch,
            ExecutionFailureStatus::NonEntryFunctionInvoked => Self::NonEntryFunctionInvoked,
            ExecutionFailureStatus::CommandArgumentError { arg_idx, kind } => {
                use crate::execution_status::CommandArgumentError as InternalCmdArgErr;
                Self::CommandArgumentError {
                    argument: arg_idx,
                    kind: match kind {
                        InternalCmdArgErr::TypeMismatch => CommandArgumentError::TypeMismatch,
                        InternalCmdArgErr::InvalidBCSBytes => CommandArgumentError::InvalidBcsBytes,
                        InternalCmdArgErr::InvalidUsageOfPureArg => {
                            CommandArgumentError::InvalidUsageOfPureArgument
                        }
                        InternalCmdArgErr::InvalidArgumentToPrivateEntryFunction => {
                            CommandArgumentError::InvalidArgumentToPrivateEntryFunction
                        }
                        InternalCmdArgErr::IndexOutOfBounds { idx } => {
                            CommandArgumentError::IndexOutOfBounds { index: idx }
                        }
                        InternalCmdArgErr::SecondaryIndexOutOfBounds {
                            result_idx,
                            secondary_idx,
                        } => CommandArgumentError::SecondaryIndexOutOfBounds {
                            result: result_idx,
                            subresult: secondary_idx,
                        },
                        InternalCmdArgErr::InvalidResultArity { result_idx } => {
                            CommandArgumentError::InvalidResultArity { result: result_idx }
                        }
                        InternalCmdArgErr::InvalidGasCoinUsage => {
                            CommandArgumentError::InvalidGasCoinUsage
                        }
                        InternalCmdArgErr::InvalidValueUsage => {
                            CommandArgumentError::InvalidValueUsage
                        }
                        InternalCmdArgErr::InvalidObjectByValue => {
                            CommandArgumentError::InvalidObjectByValue
                        }
                        InternalCmdArgErr::InvalidObjectByMutRef => {
                            CommandArgumentError::InvalidObjectByMutRef
                        }
                        InternalCmdArgErr::SharedObjectOperationNotAllowed => {
                            CommandArgumentError::SharedObjectOperationNotAllowed
                        }
                    },
                }
            }
            ExecutionFailureStatus::TypeArgumentError { argument_idx, kind } => {
                use crate::execution_status::TypeArgumentError as InternalTypeArgErr;
                Self::TypeArgumentError {
                    type_argument: argument_idx,
                    kind: match kind {
                        InternalTypeArgErr::TypeNotFound => TypeArgumentError::TypeNotFound,
                        InternalTypeArgErr::ConstraintNotSatisfied => {
                            TypeArgumentError::ConstraintNotSatisfied
                        }
                    },
                }
            }
            ExecutionFailureStatus::UnusedValueWithoutDrop {
                result_idx,
                secondary_idx,
            } => Self::UnusedValueWithoutDrop {
                result: result_idx,
                subresult: secondary_idx,
            },
            ExecutionFailureStatus::InvalidPublicFunctionReturnType { idx } => {
                Self::InvalidPublicFunctionReturnType { index: idx }
            }
            ExecutionFailureStatus::InvalidTransferObject => Self::InvalidTransferObject,
            ExecutionFailureStatus::EffectsTooLarge {
                current_size,
                max_size,
            } => Self::EffectsTooLarge {
                current_size,
                max_size,
            },
            ExecutionFailureStatus::PublishUpgradeMissingDependency => {
                Self::PublishUpgradeMissingDependency
            }
            ExecutionFailureStatus::PublishUpgradeDependencyDowngrade => {
                Self::PublishUpgradeDependencyDowngrade
            }
            ExecutionFailureStatus::PackageUpgradeError { upgrade_error } => {
                use crate::execution_status::PackageUpgradeError as InternalPkgUpgradeErr;
                Self::PackageUpgradeError {
                    kind: match upgrade_error {
                        InternalPkgUpgradeErr::UnableToFetchPackage { package_id } => {
                            PackageUpgradeError::UnableToFetchPackage {
                                package_id: package_id.into(),
                            }
                        }
                        InternalPkgUpgradeErr::NotAPackage { object_id } => {
                            PackageUpgradeError::NotAPackage {
                                object_id: object_id.into(),
                            }
                        }
                        InternalPkgUpgradeErr::IncompatibleUpgrade => {
                            PackageUpgradeError::IncompatibleUpgrade
                        }
                        InternalPkgUpgradeErr::DigestDoesNotMatch { digest } => {
                            PackageUpgradeError::DigestDoesNotMatch {
                                digest: Digest::from_bytes(digest).expect("invalid digest bytes"),
                            }
                        }
                        InternalPkgUpgradeErr::UnknownUpgradePolicy { policy } => {
                            PackageUpgradeError::UnknownUpgradePolicy { policy }
                        }
                        InternalPkgUpgradeErr::PackageIDDoesNotMatch {
                            package_id,
                            ticket_id,
                        } => PackageUpgradeError::PackageIdDoesNotMatch {
                            package_id: package_id.into(),
                            ticket_id: ticket_id.into(),
                        },
                    },
                }
            }
            ExecutionFailureStatus::WrittenObjectsTooLarge {
                current_size,
                max_size,
            } => Self::WrittenObjectsTooLarge {
                object_size: current_size,
                max_object_size: max_size,
            },
            ExecutionFailureStatus::CertificateDenied => Self::CertificateDenied,
            ExecutionFailureStatus::IotaMoveVerificationTimeout => {
                Self::IotaMoveVerificationTimeout
            }
            ExecutionFailureStatus::SharedObjectOperationNotAllowed => {
                Self::SharedObjectOperationNotAllowed
            }
            ExecutionFailureStatus::InputObjectDeleted => Self::InputObjectDeleted,
            ExecutionFailureStatus::ExecutionCancelledDueToSharedObjectCongestion {
                congested_objects,
            } => Self::ExecutionCancelledDueToSharedObjectCongestion {
                congested_objects: congested_objects.0.into_iter().map(Into::into).collect(),
            },
            ExecutionFailureStatus::AddressDeniedForCoin { address, coin_type } => {
                Self::AddressDeniedForCoin {
                    address: address.into(),
                    coin_type,
                }
            }
            ExecutionFailureStatus::CoinTypeGlobalPause { coin_type } => {
                Self::CoinTypeGlobalPause { coin_type }
            }
            ExecutionFailureStatus::ExecutionCancelledDueToRandomnessUnavailable => {
                Self::ExecutionCancelledDueToRandomnessUnavailable
            }
        }
    }
}

impl From<ExecutionError> for crate::execution_status::ExecutionFailureStatus {
    fn from(value: ExecutionError) -> Self {
        match value {
            ExecutionError::InsufficientGas => Self::InsufficientGas,
            ExecutionError::InvalidGasObject => Self::InvalidGasObject,
            ExecutionError::InvariantViolation => Self::InvariantViolation,
            ExecutionError::FeatureNotYetSupported => Self::FeatureNotYetSupported,
            ExecutionError::ObjectTooBig {
                object_size,
                max_object_size,
            } => Self::MoveObjectTooBig {
                object_size,
                max_object_size,
            },
            ExecutionError::PackageTooBig {
                object_size,
                max_object_size,
            } => Self::MovePackageTooBig {
                object_size,
                max_object_size,
            },
            ExecutionError::CircularObjectOwnership { object } => Self::CircularObjectOwnership {
                object: object.into(),
            },
            ExecutionError::InsufficientCoinBalance => Self::InsufficientCoinBalance,
            ExecutionError::CoinBalanceOverflow => Self::CoinBalanceOverflow,
            ExecutionError::PublishErrorNonZeroAddress => Self::PublishErrorNonZeroAddress,
            ExecutionError::IotaMoveVerificationError => Self::IotaMoveVerificationError,
            ExecutionError::MovePrimitiveRuntimeError { location } => {
                Self::MovePrimitiveRuntimeError(crate::execution_status::MoveLocationOpt(
                    location.map(Into::into),
                ))
            }
            ExecutionError::MoveAbort { location, code } => Self::MoveAbort(location.into(), code),
            ExecutionError::VmVerificationOrDeserializationError => {
                Self::VMVerificationOrDeserializationError
            }
            ExecutionError::VmInvariantViolation => Self::VMInvariantViolation,
            ExecutionError::FunctionNotFound => Self::FunctionNotFound,
            ExecutionError::ArityMismatch => Self::ArityMismatch,
            ExecutionError::TypeArityMismatch => Self::TypeArityMismatch,
            ExecutionError::NonEntryFunctionInvoked => Self::NonEntryFunctionInvoked,
            ExecutionError::CommandArgumentError { argument, kind } => {
                use crate::execution_status::CommandArgumentError as InternalCmdArgErr;
                Self::CommandArgumentError {
                    arg_idx: argument,
                    kind: match kind {
                        CommandArgumentError::TypeMismatch => InternalCmdArgErr::TypeMismatch,
                        CommandArgumentError::InvalidBcsBytes => InternalCmdArgErr::InvalidBCSBytes,
                        CommandArgumentError::InvalidUsageOfPureArgument => {
                            InternalCmdArgErr::InvalidUsageOfPureArg
                        }
                        CommandArgumentError::InvalidArgumentToPrivateEntryFunction => {
                            InternalCmdArgErr::InvalidArgumentToPrivateEntryFunction
                        }
                        CommandArgumentError::IndexOutOfBounds { index } => {
                            InternalCmdArgErr::IndexOutOfBounds { idx: index }
                        }
                        CommandArgumentError::SecondaryIndexOutOfBounds { result, subresult } => {
                            InternalCmdArgErr::SecondaryIndexOutOfBounds {
                                result_idx: result,
                                secondary_idx: subresult,
                            }
                        }
                        CommandArgumentError::InvalidResultArity { result } => {
                            InternalCmdArgErr::InvalidResultArity { result_idx: result }
                        }
                        CommandArgumentError::InvalidGasCoinUsage => {
                            InternalCmdArgErr::InvalidGasCoinUsage
                        }
                        CommandArgumentError::InvalidValueUsage => {
                            InternalCmdArgErr::InvalidValueUsage
                        }
                        CommandArgumentError::InvalidObjectByValue => {
                            InternalCmdArgErr::InvalidObjectByValue
                        }
                        CommandArgumentError::InvalidObjectByMutRef => {
                            InternalCmdArgErr::InvalidObjectByMutRef
                        }
                        CommandArgumentError::SharedObjectOperationNotAllowed => {
                            InternalCmdArgErr::SharedObjectOperationNotAllowed
                        }
                    },
                }
            }
            ExecutionError::TypeArgumentError {
                type_argument,
                kind,
            } => {
                use crate::execution_status::TypeArgumentError as InternalTypeArgErr;
                Self::TypeArgumentError {
                    argument_idx: type_argument,
                    kind: match kind {
                        TypeArgumentError::TypeNotFound => InternalTypeArgErr::TypeNotFound,
                        TypeArgumentError::ConstraintNotSatisfied => {
                            InternalTypeArgErr::ConstraintNotSatisfied
                        }
                    },
                }
            }
            ExecutionError::UnusedValueWithoutDrop { result, subresult } => {
                Self::UnusedValueWithoutDrop {
                    result_idx: result,
                    secondary_idx: subresult,
                }
            }
            ExecutionError::InvalidPublicFunctionReturnType { index } => {
                Self::InvalidPublicFunctionReturnType { idx: index }
            }
            ExecutionError::InvalidTransferObject => Self::InvalidTransferObject,
            ExecutionError::EffectsTooLarge {
                current_size,
                max_size,
            } => Self::EffectsTooLarge {
                current_size,
                max_size,
            },
            ExecutionError::PublishUpgradeMissingDependency => {
                Self::PublishUpgradeMissingDependency
            }
            ExecutionError::PublishUpgradeDependencyDowngrade => {
                Self::PublishUpgradeDependencyDowngrade
            }
            ExecutionError::PackageUpgradeError { kind } => {
                use crate::execution_status::PackageUpgradeError as InternalPkgUpgradeErr;
                Self::PackageUpgradeError {
                    upgrade_error: match kind {
                        PackageUpgradeError::UnableToFetchPackage { package_id } => {
                            InternalPkgUpgradeErr::UnableToFetchPackage {
                                package_id: package_id.into(),
                            }
                        }
                        PackageUpgradeError::NotAPackage { object_id } => {
                            InternalPkgUpgradeErr::NotAPackage {
                                object_id: object_id.into(),
                            }
                        }
                        PackageUpgradeError::IncompatibleUpgrade => {
                            InternalPkgUpgradeErr::IncompatibleUpgrade
                        }
                        PackageUpgradeError::DigestDoesNotMatch { digest } => {
                            InternalPkgUpgradeErr::DigestDoesNotMatch {
                                digest: digest.as_bytes().to_vec(),
                            }
                        }
                        PackageUpgradeError::UnknownUpgradePolicy { policy } => {
                            InternalPkgUpgradeErr::UnknownUpgradePolicy { policy }
                        }
                        PackageUpgradeError::PackageIdDoesNotMatch {
                            package_id,
                            ticket_id,
                        } => InternalPkgUpgradeErr::PackageIDDoesNotMatch {
                            package_id: package_id.into(),
                            ticket_id: ticket_id.into(),
                        },
                    },
                }
            }
            ExecutionError::WrittenObjectsTooLarge {
                object_size,
                max_object_size,
            } => Self::WrittenObjectsTooLarge {
                current_size: object_size,
                max_size: max_object_size,
            },
            ExecutionError::CertificateDenied => Self::CertificateDenied,
            ExecutionError::IotaMoveVerificationTimeout => Self::IotaMoveVerificationTimeout,
            ExecutionError::SharedObjectOperationNotAllowed => {
                Self::SharedObjectOperationNotAllowed
            }
            ExecutionError::InputObjectDeleted => Self::InputObjectDeleted,
            ExecutionError::ExecutionCancelledDueToSharedObjectCongestion { congested_objects } => {
                Self::ExecutionCancelledDueToSharedObjectCongestion {
                    congested_objects: crate::execution_status::CongestedObjects(
                        congested_objects.into_iter().map(Into::into).collect(),
                    ),
                }
            }
            ExecutionError::AddressDeniedForCoin { address, coin_type } => {
                Self::AddressDeniedForCoin {
                    address: address.into(),
                    coin_type,
                }
            }
            ExecutionError::CoinTypeGlobalPause { coin_type } => {
                Self::CoinTypeGlobalPause { coin_type }
            }
            ExecutionError::ExecutionCancelledDueToRandomnessUnavailable => {
                Self::ExecutionCancelledDueToRandomnessUnavailable
            }
        }
    }
}

impl From<crate::execution_status::MoveLocation> for MoveLocation {
    fn from(value: crate::execution_status::MoveLocation) -> Self {
        Self {
            package: ObjectId::new(value.module.address().into_bytes()),
            module: Identifier::new(value.module.name().as_str()).expect("invalid module name"),
            function: value.function,
            instruction: value.instruction,
            function_name: value
                .function_name
                .map(|name| Identifier::new(name).expect("invalid function name")),
        }
    }
}

impl From<MoveLocation> for crate::execution_status::MoveLocation {
    fn from(value: MoveLocation) -> Self {
        Self {
            module: ModuleId::new(
                move_core_types::account_address::AccountAddress::new(value.package.into_inner()),
                crate::Identifier::new(value.module.as_str()).expect("invalid module name"),
            ),
            function: value.function,
            instruction: value.instruction,
            function_name: value.function_name.map(|name| name.to_string()),
        }
    }
}

impl From<crate::messages_checkpoint::CheckpointContents> for CheckpointContents {
    fn from(value: crate::messages_checkpoint::CheckpointContents) -> Self {
        Self(
            value
                .into_iter_with_signatures()
                .map(|(digests, signatures)| CheckpointTransactionInfo {
                    transaction: digests.transaction.into(),
                    effects: digests.effects.into(),
                    signatures: signatures.into_iter().map(Into::into).collect(),
                })
                .collect(),
        )
    }
}

impl From<CheckpointContents> for crate::messages_checkpoint::CheckpointContents {
    fn from(value: CheckpointContents) -> Self {
        let (transactions, user_signatures) = value.0.into_iter().fold(
            (Vec::new(), Vec::new()),
            |(mut transactions, mut user_signatures), info| {
                transactions.push(crate::base_types::ExecutionDigests {
                    transaction: info.transaction.into(),
                    effects: info.effects.into(),
                });
                user_signatures.push(info.signatures.into_iter().map(Into::into).collect());
                (transactions, user_signatures)
            },
        );
        crate::messages_checkpoint::CheckpointContents::new_with_digests_and_signatures(
            transactions,
            user_signatures,
        )
    }
}

impl From<crate::full_checkpoint_content::CheckpointData> for CheckpointData {
    fn from(value: crate::full_checkpoint_content::CheckpointData) -> Self {
        Self {
            checkpoint_summary: value.checkpoint_summary.into(),
            checkpoint_contents: value.checkpoint_contents.into(),
            transactions: value.transactions.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<CheckpointData> for crate::full_checkpoint_content::CheckpointData {
    fn from(value: CheckpointData) -> Self {
        Self {
            checkpoint_summary: value.checkpoint_summary.into(),
            checkpoint_contents: value.checkpoint_contents.into(),
            transactions: value.transactions.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<crate::full_checkpoint_content::CheckpointTransaction> for CheckpointTransaction {
    fn from(value: crate::full_checkpoint_content::CheckpointTransaction) -> Self {
        Self {
            transaction: value.transaction.into(),
            effects: value.effects.into(),
            events: value.events.map(Into::into),
            input_objects: value.input_objects.into_iter().map(Into::into).collect(),
            output_objects: value.output_objects.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<CheckpointTransaction> for crate::full_checkpoint_content::CheckpointTransaction {
    fn from(value: CheckpointTransaction) -> Self {
        Self {
            transaction: value.transaction.into(),
            effects: value.effects.into(),
            events: value.events.map(Into::into),
            input_objects: value.input_objects.into_iter().map(Into::into).collect(),
            output_objects: value.output_objects.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<crate::signature::GenericSignature> for UserSignature {
    fn from(value: crate::signature::GenericSignature) -> Self {
        bcs::from_bytes(&bcs::to_bytes(&value).expect("invalid signature"))
            .expect("invalid signature")
    }
}

impl From<UserSignature> for crate::signature::GenericSignature {
    fn from(value: UserSignature) -> Self {
        bcs::from_bytes(&bcs::to_bytes(&value).expect("invalid signature"))
            .expect("invalid signature")
    }
}

impl From<crate::effects::TransactionEvents> for TransactionEvents {
    fn from(value: crate::effects::TransactionEvents) -> Self {
        Self(value.data.into_iter().map(Into::into).collect())
    }
}

impl From<TransactionEvents> for crate::effects::TransactionEvents {
    fn from(value: TransactionEvents) -> Self {
        Self {
            data: value.0.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<crate::event::Event> for Event {
    fn from(value: crate::event::Event) -> Self {
        Self {
            package_id: value.package_id.into(),
            module: Identifier::new(value.transaction_module.as_str())
                .expect("invalid event module identifier"),
            sender: value.sender.into(),
            type_: struct_tag_core_to_sdk(value.type_),
            contents: value.contents,
        }
    }
}

impl From<Event> for crate::event::Event {
    fn from(value: Event) -> Self {
        Self {
            package_id: value.package_id.into(),
            transaction_module: crate::Identifier::new(value.module.as_str())
                .expect("invalid event module identifier"),
            sender: value.sender.into(),
            type_: struct_tag_sdk_to_core(value.type_),
            contents: value.contents,
        }
    }
}

impl From<crate::transaction::Command> for Command {
    fn from(value: crate::transaction::Command) -> Self {
        use crate::transaction::Command as InternalCmd;
        match value {
            InternalCmd::MoveCall(programmable_move_call) => Self::MoveCall(MoveCall {
                package: programmable_move_call.package.into(),
                module: Identifier::new(programmable_move_call.module.as_str())
                    .expect("invalid move call module identifier"),
                function: Identifier::new(programmable_move_call.function.as_str())
                    .expect("invalid move call function identifier"),
                type_arguments: programmable_move_call
                    .type_arguments
                    .into_iter()
                    .map(type_tag_core_to_sdk)
                    .collect(),
                arguments: programmable_move_call
                    .arguments
                    .into_iter()
                    .map(Into::into)
                    .collect(),
            }),
            InternalCmd::TransferObjects(objects, address) => {
                Self::TransferObjects(TransferObjects {
                    objects: objects.into_iter().map(Into::into).collect(),
                    address: address.into(),
                })
            }
            InternalCmd::SplitCoins(coin, amounts) => Self::SplitCoins(SplitCoins {
                coin: coin.into(),
                amounts: amounts.into_iter().map(Into::into).collect(),
            }),
            InternalCmd::MergeCoins(argument, coins_to_merge) => Self::MergeCoins(MergeCoins {
                coin: argument.into(),
                coins_to_merge: coins_to_merge.into_iter().map(Into::into).collect(),
            }),
            InternalCmd::Publish(modules, dependencies) => Self::Publish(Publish {
                modules,
                dependencies: dependencies.into_iter().map(Into::into).collect(),
            }),
            InternalCmd::MakeMoveVec(type_tag, elements) => Self::MakeMoveVector(MakeMoveVector {
                type_: type_tag.map(type_tag_core_to_sdk),
                elements: elements.into_iter().map(Into::into).collect(),
            }),
            InternalCmd::Upgrade(modules, dependencies, package, ticket) => {
                Self::Upgrade(Upgrade {
                    modules,
                    dependencies: dependencies.into_iter().map(Into::into).collect(),
                    package: package.into(),
                    ticket: ticket.into(),
                })
            }
        }
    }
}

impl From<Command> for crate::transaction::Command {
    fn from(value: Command) -> Self {
        match value {
            Command::MoveCall(move_call) => {
                Self::MoveCall(Box::new(crate::transaction::ProgrammableMoveCall {
                    package: move_call.package.into(),
                    module: crate::Identifier::new(move_call.module.as_str())
                        .expect("invalid move call module identifier"),
                    function: crate::Identifier::new(move_call.function.as_str())
                        .expect("invalid move call function identifier"),
                    type_arguments: move_call
                        .type_arguments
                        .into_iter()
                        .map(type_tag_sdk_to_core)
                        .collect(),
                    arguments: move_call.arguments.into_iter().map(Into::into).collect(),
                }))
            }
            Command::TransferObjects(transfer_objects) => Self::TransferObjects(
                transfer_objects
                    .objects
                    .into_iter()
                    .map(Into::into)
                    .collect(),
                transfer_objects.address.into(),
            ),
            Command::SplitCoins(split_coins) => Self::SplitCoins(
                split_coins.coin.into(),
                split_coins.amounts.into_iter().map(Into::into).collect(),
            ),
            Command::MergeCoins(merge_coins) => Self::MergeCoins(
                merge_coins.coin.into(),
                merge_coins
                    .coins_to_merge
                    .into_iter()
                    .map(Into::into)
                    .collect(),
            ),
            Command::Publish(publish) => Self::Publish(
                publish.modules,
                publish.dependencies.into_iter().map(Into::into).collect(),
            ),
            Command::MakeMoveVector(make_move_vector) => Self::MakeMoveVec(
                make_move_vector.type_.map(type_tag_sdk_to_core),
                make_move_vector
                    .elements
                    .into_iter()
                    .map(Into::into)
                    .collect(),
            ),
            Command::Upgrade(upgrade) => Self::Upgrade(
                upgrade.modules,
                upgrade.dependencies.into_iter().map(Into::into).collect(),
                upgrade.package.into(),
                upgrade.ticket.into(),
            ),
        }
    }
}

impl From<crate::transaction::Argument> for Argument {
    fn from(value: crate::transaction::Argument) -> Self {
        match value {
            crate::transaction::Argument::GasCoin => Self::Gas,
            crate::transaction::Argument::Input(idx) => Self::Input(idx),
            crate::transaction::Argument::Result(idx) => Self::Result(idx),
            crate::transaction::Argument::NestedResult(idx1, idx2) => {
                Self::NestedResult(idx1, idx2)
            }
        }
    }
}

impl From<Argument> for crate::transaction::Argument {
    fn from(value: Argument) -> Self {
        match value {
            Argument::Gas => Self::GasCoin,
            Argument::Input(idx) => Self::Input(idx),
            Argument::Result(idx) => Self::Result(idx),
            Argument::NestedResult(idx1, idx2) => Self::NestedResult(idx1, idx2),
        }
    }
}

impl From<crate::gas::GasCostSummary> for GasCostSummary {
    fn from(value: crate::gas::GasCostSummary) -> Self {
        Self::new(
            value.computation_cost,
            value.computation_cost_burned,
            value.storage_cost,
            value.storage_rebate,
            value.non_refundable_storage_fee,
        )
    }
}

impl From<GasCostSummary> for crate::gas::GasCostSummary {
    fn from(value: GasCostSummary) -> Self {
        Self::new(
            value.computation_cost,
            value.computation_cost_burned,
            value.storage_cost,
            value.storage_rebate,
            value.non_refundable_storage_fee,
        )
    }
}

impl From<crate::messages_checkpoint::EndOfEpochData> for EndOfEpochData {
    fn from(value: crate::messages_checkpoint::EndOfEpochData) -> Self {
        Self {
            next_epoch_committee: value
                .next_epoch_committee
                .into_iter()
                .map(|(public_key, stake)| ValidatorCommitteeMember {
                    public_key: Bls12381PublicKey::new(public_key.0),
                    stake,
                })
                .collect(),
            next_epoch_protocol_version: value.next_epoch_protocol_version.as_u64(),
            epoch_commitments: value
                .epoch_commitments
                .into_iter()
                .map(Into::into)
                .collect(),
            epoch_supply_change: value.epoch_supply_change,
        }
    }
}

impl From<EndOfEpochData> for crate::messages_checkpoint::EndOfEpochData {
    fn from(value: EndOfEpochData) -> Self {
        Self {
            next_epoch_committee: value
                .next_epoch_committee
                .into_iter()
                .map(|v| (v.public_key.into(), v.stake))
                .collect(),
            next_epoch_protocol_version: value.next_epoch_protocol_version.into(),
            epoch_commitments: value
                .epoch_commitments
                .into_iter()
                .map(Into::into)
                .collect(),
            epoch_supply_change: value.epoch_supply_change,
        }
    }
}

impl From<crate::messages_checkpoint::CheckpointCommitment> for CheckpointCommitment {
    fn from(value: crate::messages_checkpoint::CheckpointCommitment) -> Self {
        let crate::messages_checkpoint::CheckpointCommitment::ECMHLiveObjectSetDigest(digest) =
            value;
        Self::EcmhLiveObjectSet {
            digest: Digest::new(digest.digest.into_inner()),
        }
    }
}

impl From<CheckpointCommitment> for crate::messages_checkpoint::CheckpointCommitment {
    fn from(value: CheckpointCommitment) -> Self {
        let CheckpointCommitment::EcmhLiveObjectSet { digest } = value;
        Self::ECMHLiveObjectSetDigest(crate::messages_checkpoint::ECMHLiveObjectSetDigest {
            digest: crate::digests::Digest::new(digest.into_inner()),
        })
    }
}

impl From<crate::messages_checkpoint::CheckpointSummary> for CheckpointSummary {
    fn from(value: crate::messages_checkpoint::CheckpointSummary) -> Self {
        Self {
            epoch: value.epoch,
            sequence_number: value.sequence_number,
            network_total_transactions: value.network_total_transactions,
            content_digest: value.content_digest.into(),
            previous_digest: value.previous_digest.map(Into::into),
            epoch_rolling_gas_cost_summary: value.epoch_rolling_gas_cost_summary.into(),
            timestamp_ms: value.timestamp_ms,
            checkpoint_commitments: value
                .checkpoint_commitments
                .into_iter()
                .map(Into::into)
                .collect(),
            end_of_epoch_data: value.end_of_epoch_data.map(Into::into),
            version_specific_data: value.version_specific_data,
        }
    }
}

impl From<CheckpointSummary> for crate::messages_checkpoint::CheckpointSummary {
    fn from(value: CheckpointSummary) -> Self {
        Self {
            epoch: value.epoch,
            sequence_number: value.sequence_number,
            network_total_transactions: value.network_total_transactions,
            content_digest: value.content_digest.into(),
            previous_digest: value.previous_digest.map(Into::into),
            epoch_rolling_gas_cost_summary: value.epoch_rolling_gas_cost_summary.into(),
            timestamp_ms: value.timestamp_ms,
            checkpoint_commitments: value
                .checkpoint_commitments
                .into_iter()
                .map(Into::into)
                .collect(),
            end_of_epoch_data: value.end_of_epoch_data.map(Into::into),
            version_specific_data: value.version_specific_data,
        }
    }
}

impl From<crate::messages_checkpoint::CertifiedCheckpointSummary> for SignedCheckpointSummary {
    fn from(value: crate::messages_checkpoint::CertifiedCheckpointSummary) -> Self {
        let (data, sig) = value.into_data_and_sig();
        Self {
            checkpoint: data.into(),
            signature: sig.into(),
        }
    }
}

impl From<SignedCheckpointSummary> for crate::messages_checkpoint::CertifiedCheckpointSummary {
    fn from(value: SignedCheckpointSummary) -> Self {
        Self::new_from_data_and_sig(
            crate::messages_checkpoint::CheckpointSummary::from(value.checkpoint),
            crate::crypto::AuthorityQuorumSignInfo::<true>::from(value.signature),
        )
    }
}

impl<const T: bool> From<crate::crypto::AuthorityQuorumSignInfo<T>>
    for ValidatorAggregatedSignature
{
    fn from(value: crate::crypto::AuthorityQuorumSignInfo<T>) -> Self {
        let crate::crypto::AuthorityQuorumSignInfo {
            epoch,
            signature,
            signers_map,
        } = value;

        Self {
            epoch,
            signature: Bls12381Signature::from_bytes(signature.as_ref()).unwrap(),
            bitmap: signers_map,
        }
    }
}

impl<const T: bool> From<ValidatorAggregatedSignature>
    for crate::crypto::AuthorityQuorumSignInfo<T>
{
    fn from(value: ValidatorAggregatedSignature) -> Self {
        let ValidatorAggregatedSignature {
            epoch,
            signature,
            bitmap,
        } = value;

        Self {
            epoch,
            signature: crate::crypto::AggregateAuthoritySignature::from_bytes(signature.as_bytes())
                .unwrap(),
            signers_map: bitmap,
        }
    }
}

impl From<crate::object::Owner> for Owner {
    fn from(value: crate::object::Owner) -> Self {
        match value {
            crate::object::Owner::AddressOwner(address) => Self::Address(address.into()),
            crate::object::Owner::ObjectOwner(object_id) => Self::Object(object_id.into()),
            crate::object::Owner::Shared {
                initial_shared_version,
            } => Self::Shared(initial_shared_version.value()),
            crate::object::Owner::Immutable => Self::Immutable,
        }
    }
}

impl From<Owner> for crate::object::Owner {
    fn from(value: Owner) -> Self {
        match value {
            Owner::Address(address) => crate::object::Owner::AddressOwner(address.into()),
            Owner::Object(object_id) => crate::object::Owner::ObjectOwner(object_id.into()),
            Owner::Shared(initial_shared_version) => crate::object::Owner::Shared {
                initial_shared_version: initial_shared_version.into(),
            },
            Owner::Immutable => crate::object::Owner::Immutable,
        }
    }
}

impl From<crate::base_types::IotaAddress> for Address {
    fn from(value: crate::base_types::IotaAddress) -> Self {
        Self::new(value.to_inner())
    }
}

impl From<Address> for crate::base_types::IotaAddress {
    fn from(value: Address) -> Self {
        crate::base_types::ObjectID::new(value.into_inner()).into()
    }
}

impl From<crate::base_types::ObjectID> for ObjectId {
    fn from(value: crate::base_types::ObjectID) -> Self {
        Self::new(value.into_bytes())
    }
}

impl From<ObjectId> for crate::base_types::ObjectID {
    fn from(value: ObjectId) -> Self {
        Self::new(value.into_inner())
    }
}

impl From<crate::base_types::IotaAddress> for ObjectId {
    fn from(value: crate::base_types::IotaAddress) -> Self {
        Self::new(value.to_inner())
    }
}

impl From<ObjectId> for crate::base_types::IotaAddress {
    fn from(value: ObjectId) -> Self {
        crate::base_types::ObjectID::new(value.into_inner()).into()
    }
}

impl From<crate::transaction::SenderSignedData> for SignedTransaction {
    fn from(value: crate::transaction::SenderSignedData) -> Self {
        let crate::transaction::SenderSignedTransaction {
            intent_message,
            tx_signatures,
        } = value.into_inner();

        Self {
            transaction: intent_message.value.into(),
            signatures: tx_signatures.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<SignedTransaction> for crate::transaction::SenderSignedData {
    fn from(value: SignedTransaction) -> Self {
        let SignedTransaction {
            transaction,
            signatures,
        } = value;

        Self::new(
            transaction.into(),
            signatures.into_iter().map(Into::into).collect(),
        )
    }
}

impl From<crate::transaction::Transaction> for SignedTransaction {
    fn from(value: crate::transaction::Transaction) -> Self {
        value.into_data().into()
    }
}

impl From<SignedTransaction> for crate::transaction::Transaction {
    fn from(value: SignedTransaction) -> Self {
        Self::new(value.into())
    }
}

pub fn type_tag_core_to_sdk(value: move_core_types::language_storage::TypeTag) -> TypeTag {
    match value {
        move_core_types::language_storage::TypeTag::Bool => TypeTag::Bool,
        move_core_types::language_storage::TypeTag::U8 => TypeTag::U8,
        move_core_types::language_storage::TypeTag::U64 => TypeTag::U64,
        move_core_types::language_storage::TypeTag::U128 => TypeTag::U128,
        move_core_types::language_storage::TypeTag::Address => TypeTag::Address,
        move_core_types::language_storage::TypeTag::Signer => TypeTag::Signer,
        move_core_types::language_storage::TypeTag::Vector(type_tag) => {
            TypeTag::Vector(Box::new(type_tag_core_to_sdk(*type_tag)))
        }
        move_core_types::language_storage::TypeTag::Struct(struct_tag) => {
            TypeTag::Struct(Box::new(struct_tag_core_to_sdk(*struct_tag)))
        }
        move_core_types::language_storage::TypeTag::U16 => TypeTag::U16,
        move_core_types::language_storage::TypeTag::U32 => TypeTag::U32,
        move_core_types::language_storage::TypeTag::U256 => TypeTag::U256,
    }
}

pub fn type_tag_sdk_to_core(value: TypeTag) -> move_core_types::language_storage::TypeTag {
    match value {
        TypeTag::Bool => move_core_types::language_storage::TypeTag::Bool,
        TypeTag::U8 => move_core_types::language_storage::TypeTag::U8,
        TypeTag::U64 => move_core_types::language_storage::TypeTag::U64,
        TypeTag::U128 => move_core_types::language_storage::TypeTag::U128,
        TypeTag::Address => move_core_types::language_storage::TypeTag::Address,
        TypeTag::Signer => move_core_types::language_storage::TypeTag::Signer,
        TypeTag::Vector(type_tag) => move_core_types::language_storage::TypeTag::Vector(Box::new(
            type_tag_sdk_to_core(*type_tag),
        )),
        TypeTag::Struct(struct_tag) => move_core_types::language_storage::TypeTag::Struct(
            Box::new(struct_tag_sdk_to_core(*struct_tag)),
        ),
        TypeTag::U16 => move_core_types::language_storage::TypeTag::U16,
        TypeTag::U32 => move_core_types::language_storage::TypeTag::U32,
        TypeTag::U256 => move_core_types::language_storage::TypeTag::U256,
    }
}

pub fn struct_tag_core_to_sdk(value: move_core_types::language_storage::StructTag) -> StructTag {
    let move_core_types::language_storage::StructTag {
        address,
        module,
        name,
        type_params,
    } = value;

    let address = Address::new(address.into_bytes());
    let module = Identifier::new(module.as_str()).unwrap();
    let name = Identifier::new(name.as_str()).unwrap();
    let type_params = type_params.into_iter().map(type_tag_core_to_sdk).collect();
    StructTag {
        address,
        module,
        name,
        type_params,
    }
}

pub fn struct_tag_sdk_to_core(value: StructTag) -> move_core_types::language_storage::StructTag {
    let StructTag {
        address,
        module,
        name,
        type_params,
    } = value;

    let address = move_core_types::account_address::AccountAddress::new(address.into_inner());
    let module = move_core_types::identifier::Identifier::new(module.into_inner()).unwrap();
    let name = move_core_types::identifier::Identifier::new(name.into_inner()).unwrap();
    let type_params = type_params.into_iter().map(type_tag_sdk_to_core).collect();
    move_core_types::language_storage::StructTag {
        address,
        module,
        name,
        type_params,
    }
}

impl From<crate::committee::Committee> for ValidatorCommittee {
    fn from(value: crate::committee::Committee) -> Self {
        Self {
            epoch: value.epoch(),
            members: value
                .voting_rights
                .into_iter()
                .map(|(name, stake)| ValidatorCommitteeMember {
                    public_key: name.into(),
                    stake,
                })
                .collect(),
        }
    }
}

impl From<ValidatorCommittee> for crate::committee::Committee {
    fn from(value: ValidatorCommittee) -> Self {
        let ValidatorCommittee { epoch, members } = value;

        Self::new(
            epoch,
            members
                .into_iter()
                .map(|member| (member.public_key.into(), member.stake))
                .collect(),
        )
    }
}

impl From<crate::crypto::AuthorityPublicKeyBytes> for Bls12381PublicKey {
    fn from(value: crate::crypto::AuthorityPublicKeyBytes) -> Self {
        Self::new(value.0)
    }
}

impl From<Bls12381PublicKey> for crate::crypto::AuthorityPublicKeyBytes {
    fn from(value: Bls12381PublicKey) -> Self {
        Self::new(value.into_inner())
    }
}

impl From<UnchangedSharedKind> for crate::effects::UnchangedSharedKind {
    fn from(value: UnchangedSharedKind) -> Self {
        match value {
            UnchangedSharedKind::ReadOnlyRoot { version, digest } => {
                Self::ReadOnlyRoot((version.into(), digest.into()))
            }
            UnchangedSharedKind::MutateDeleted { version } => Self::MutateDeleted(version.into()),
            UnchangedSharedKind::ReadDeleted { version } => Self::ReadDeleted(version.into()),
            UnchangedSharedKind::Cancelled { version } => Self::Cancelled(version.into()),
            UnchangedSharedKind::PerEpochConfig => Self::PerEpochConfig,
        }
    }
}

impl From<crate::effects::UnchangedSharedKind> for UnchangedSharedKind {
    fn from(value: crate::effects::UnchangedSharedKind) -> Self {
        match value {
            crate::effects::UnchangedSharedKind::ReadOnlyRoot((version, digest)) => {
                Self::ReadOnlyRoot {
                    version: version.into(),
                    digest: digest.into(),
                }
            }
            crate::effects::UnchangedSharedKind::MutateDeleted(version) => Self::MutateDeleted {
                version: version.into(),
            },
            crate::effects::UnchangedSharedKind::ReadDeleted(version) => Self::ReadDeleted {
                version: version.into(),
            },
            crate::effects::UnchangedSharedKind::Cancelled(version) => Self::Cancelled {
                version: version.into(),
            },
            crate::effects::UnchangedSharedKind::PerEpochConfig => Self::PerEpochConfig,
        }
    }
}

impl From<crate::transaction::TransactionExpiration> for TransactionExpiration {
    fn from(value: crate::transaction::TransactionExpiration) -> Self {
        match value {
            crate::transaction::TransactionExpiration::None => Self::None,
            crate::transaction::TransactionExpiration::Epoch(epoch) => Self::Epoch(epoch),
        }
    }
}

impl From<TransactionExpiration> for crate::transaction::TransactionExpiration {
    fn from(value: TransactionExpiration) -> Self {
        match value {
            TransactionExpiration::None => Self::None,
            TransactionExpiration::Epoch(epoch) => Self::Epoch(epoch),
        }
    }
}
