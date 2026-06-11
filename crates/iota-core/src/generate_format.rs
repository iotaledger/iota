// Copyright (c) 2021, Facebook, Inc. and its affiliates
// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeMap, fs::File, io::Write};

use clap::*;
use iota_sdk_crypto::{
    Signer as _, ed25519::Ed25519PrivateKey, secp256k1::Secp256k1PrivateKey,
    secp256r1::Secp256r1PrivateKey,
};
use iota_sdk_types::{
    Argument, ChangeEpoch, Command, CommandArgumentError, ConsensusCommitPrologueV1,
    ConsensusDeterminedVersionAssignments, EndOfEpochTransactionKind, Event, ExecutionError,
    ExecutionStatus, GenesisObject, GenesisTransaction, Identifier, MoveLocation, MoveObjectType,
    ObjectId, Owner, PackageUpgradeError, RandomnessStateUpdate, SimpleSignature, StructTag,
    TransactionExpiration, TransactionKind, TypeArgumentError, TypeTag, UnchangedSharedKind,
    crypto::{Intent, IntentMessage, PersonalMessage},
    move_package::{MovePackage, TypeOrigin, UpgradeInfo},
};
use iota_types::{
    base_types::{
        self, ExecutionData, IotaAddress, ObjectDigest, TransactionDigest, TransactionEffectsDigest,
    },
    crypto::{
        AccountKeyPair, AggregateAuthoritySignature, AuthorityKeyPair, AuthorityPublicKeyBytes,
        AuthorityQuorumSignInfo, AuthoritySignature, AuthorityStrongQuorumSignInfo,
        Ed25519IotaSignature, KeypairTraits, Signature, Signer, ToFromBytes, get_key_pair,
    },
    digests::ConsensusCommitDigest,
    effects::{
        IDOperation, ObjectIn, ObjectOut, TransactionEffects, TransactionEffectsExtForTesting,
        TransactionEvents,
    },
    full_checkpoint_content::{CheckpointData, CheckpointTransaction},
    messages_checkpoint::{
        CertifiedCheckpointSummary, CheckpointCommitment, CheckpointContents,
        CheckpointContentsDigest, CheckpointDigest, CheckpointSummary, FullCheckpointContents,
    },
    messages_grpc::ObjectInfoRequestKind,
    multisig::{MultiSig, MultiSigPublicKey, MultisigMember},
    object::{Data, MoveObject, MoveObjectExt, ObjectInner},
    signature::GenericSignature,
    storage::DeleteKind,
    transaction::{
        CallArg, ProgrammableTransaction, SenderSignedData, SharedObjectRef, Transaction,
        TransactionData, TransactionDataAPI,
    },
};
use move_core_types::{account_address::AccountAddress, language_storage::ModuleId};
use pretty_assertions::assert_str_eq;
use rand::{SeedableRng, rngs::StdRng};
use roaring::RoaringBitmap;
use serde_reflection::{ContainerFormat, Format, Registry, Result, Samples, Tracer, TracerConfig};
use typed_store::TypedStoreError;

/// Generate a type format registry for IOTA types
///
/// Used for regression testing.
///
/// It uses [serde_reflection] for serializing the type system
/// which conveniently plugs into [serde].
///
/// The process is not automatic though, so all types that should
/// be tracked must be presented to the [Tracer]. Whenever possible the
/// [Tracer::trace_type] function should be used, but in cases when
/// custom [serde::Deserialize] is implemented for a type with additional
/// restrictions a [Tracer::trace_value] is likely necessary, so that [Tracer]
/// may verify the type formats. This later requirement seems to be transitive.
///
/// For example **TypeA** implements a custom serializer, hence necessitating
/// the use of [Tracer::trace_value], then every type that contains **TypeA**
/// will require a sample to be provided.
fn get_registry() -> Result<Registry> {
    let config = TracerConfig::default()
        .record_samples_for_structs(true)
        .record_samples_for_newtype_structs(true);
    let mut tracer = Tracer::new(config);
    let mut samples = Samples::new();
    // 1. Record samples for types with custom deserializers.
    // We want to call
    // tracer.trace_value(&mut samples, ...).unwrap();
    // with all the base types contained in messages, especially the ones with
    // custom serializers; or involving generics (see [serde_reflection documentation](https://novifinancial.github.io/serde-reflection/serde_reflection/index.html)).

    // Trace SDK Identifier, StructTag and TypeTag samples early - these use custom
    // serde that requires valid sample values to be provided before types
    // containing them are traced.
    let sdk_identifier = iota_sdk_types::Identifier::from_static("sample_identifier");
    tracer.trace_value(&mut samples, &sdk_identifier).unwrap();
    let struct_tag = StructTag::new_gas_coin();
    tracer.trace_value(&mut samples, &struct_tag).unwrap();

    // Trace all TypeTag variants since the SDK's TypeTag has custom serde
    tracer.trace_value(&mut samples, &TypeTag::Bool).unwrap();
    tracer.trace_value(&mut samples, &TypeTag::U8).unwrap();
    tracer.trace_value(&mut samples, &TypeTag::U16).unwrap();
    tracer.trace_value(&mut samples, &TypeTag::U32).unwrap();
    tracer.trace_value(&mut samples, &TypeTag::U64).unwrap();
    tracer.trace_value(&mut samples, &TypeTag::U128).unwrap();
    tracer.trace_value(&mut samples, &TypeTag::U256).unwrap();
    tracer.trace_value(&mut samples, &TypeTag::Address).unwrap();
    tracer.trace_value(&mut samples, &TypeTag::Signer).unwrap();
    tracer
        .trace_value(&mut samples, &TypeTag::Vector(Box::new(TypeTag::U8)))
        .unwrap();
    let type_tag_struct = TypeTag::from(struct_tag.clone());
    tracer.trace_value(&mut samples, &type_tag_struct).unwrap();

    // MoveObject.type_ uses MoveObjectType which has custom serde.
    // Trace all variants so the schema is complete:
    // Other (variant 0) - any non-special struct tag
    tracer
        .trace_value(
            &mut samples,
            &MoveObjectType::from(StructTag::new(
                IotaAddress::ZERO,
                Identifier::from_static("m"),
                Identifier::from_static("T"),
                Vec::new(),
            )),
        )
        .unwrap();
    // GasCoin (variant 1)
    tracer
        .trace_value(
            &mut samples,
            &MoveObjectType::from(StructTag::new_gas_coin()),
        )
        .unwrap();
    // StakedIota (variant 2)
    tracer
        .trace_value(
            &mut samples,
            &MoveObjectType::from(StructTag::new_staked_iota()),
        )
        .unwrap();
    // Coin (variant 3) - non-IOTA coin
    tracer
        .trace_value(
            &mut samples,
            &MoveObjectType::from(StructTag::new_coin(TypeTag::Bool)),
        )
        .unwrap();

    let m = ModuleId::new(
        AccountAddress::ZERO,
        move_core_types::identifier::Identifier::new("foo").unwrap(),
    );
    tracer.trace_value(&mut samples, &m).unwrap();
    tracer
        .trace_value(&mut samples, &Identifier::new("foo").unwrap())
        .unwrap();

    let (addr, kp): (_, AuthorityKeyPair) = get_key_pair();
    let (s_addr, s_kp): (_, AccountKeyPair) = get_key_pair();
    let pk: AuthorityPublicKeyBytes = kp.public().into();
    tracer.trace_value(&mut samples, &addr).unwrap();
    tracer.trace_value(&mut samples, &kp).unwrap();
    tracer.trace_value(&mut samples, &pk).unwrap();

    tracer.trace_value(&mut samples, &s_addr).unwrap();
    tracer.trace_value(&mut samples, &s_kp).unwrap();

    // We have two signature types: one for Authority Signatures, which don't
    // include the PubKey ...
    let sig: AuthoritySignature = Signer::sign(&kp, b"hello world");
    tracer.trace_value(&mut samples, &sig).unwrap();
    // ... and the user signature which does

    let sig: Signature = Signer::sign(&s_kp, b"hello world");
    tracer.trace_value(&mut samples, &sig).unwrap();

    let kp1 = Ed25519PrivateKey::generate(StdRng::from_seed([0; 32]));
    let kp2 = Secp256k1PrivateKey::generate(StdRng::from_seed([0; 32]));
    let kp3 = Secp256r1PrivateKey::generate(StdRng::from_seed([0; 32]));

    let multisig_pk = MultiSigPublicKey::new(
        vec![
            MultisigMember::new(kp1.public_key(), 1),
            MultisigMember::new(kp2.public_key(), 1),
            MultisigMember::new(kp3.public_key(), 1),
        ],
        2,
    )
    .unwrap();

    let digest = IntentMessage::new(
        Intent::iota_transaction(),
        PersonalMessage("Message".as_bytes().to_vec().into()),
    )
    .signing_digest();

    let sig1: SimpleSignature = kp1.sign(&*digest);
    let sig2: SimpleSignature = kp2.sign(&*digest);
    let sig3: SimpleSignature = kp3.sign(&*digest);

    let multi_sig = MultiSig::new(
        vec![
            sig1.clone().into(),
            sig2.clone().into(),
            sig3.clone().into(),
        ],
        multisig_pk,
    )
    .unwrap();
    tracer.trace_value(&mut samples, &multi_sig).unwrap();

    let generic_sig_multi = GenericSignature::MultiSig(multi_sig);
    tracer
        .trace_value(&mut samples, &generic_sig_multi)
        .unwrap();

    // Seed a `GenericSignature::Signature` sample so that when the tracer
    // later deserializes `CheckpointContents.user_signatures`
    // (`Vec<Vec<GenericSignature>>`) it has flag-0/1/2 bytes available.
    // Otherwise fastcrypto's `from_bytes` rejects synthesized bytes with
    // "Invalid signature was given to the function".
    tracer
        .trace_value(&mut samples, &GenericSignature::Signature(sig.clone()))
        .unwrap();

    tracer.trace_value(&mut samples, &sig1).unwrap();
    tracer.trace_value(&mut samples, &sig2).unwrap();
    tracer.trace_value(&mut samples, &sig3).unwrap();
    // ObjectID and IotaAddress are the same length
    let oid: ObjectId = addr.into();
    tracer.trace_value(&mut samples, &oid).unwrap();

    // ObjectDigest and Transaction digest use the `serde_as`speedup for ser/de =>
    // trace them
    let od = ObjectDigest::random();
    let td = TransactionDigest::random();
    tracer.trace_value(&mut samples, &od).unwrap();
    tracer.trace_value(&mut samples, &td).unwrap();

    let teff = TransactionEffectsDigest::random();
    tracer.trace_value(&mut samples, &teff).unwrap();

    let ccd = CheckpointContentsDigest::random();
    tracer.trace_value(&mut samples, &ccd).unwrap();

    let ccd = CheckpointDigest::random();
    tracer.trace_value(&mut samples, &ccd).unwrap();

    let tot = TypeOrigin {
        module_name: Identifier::from_static("module_name"),
        datatype_name: Identifier::from_static("datatype_name"),
        package: ObjectId::random(),
    };
    tracer.trace_value(&mut samples, &tot).unwrap();

    // We need Event sample here, because our GenesisTransaction contains an
    // Event while, sui's doesn't.
    let event = Event {
        package_id: ObjectId::random(),
        module: Identifier::from_static("foo"),
        sender: IotaAddress::ZERO,
        type_: struct_tag.clone(),
        contents: vec![0],
    };
    tracer.trace_value(&mut samples, &event).unwrap();

    // Seed both Data variants. trace_type::<Data> is skipped because the SDK's
    // MovePackage uses BTreeMap<Identifier, Vec<u8>> with serde_with, and
    // Identifier's custom serde (DisplayFromStr) is incompatible with
    // serde_reflection's tracing deserializer for map keys.
    let sample_move_obj = MoveObject::new_gas_coin(1u64.into(), ObjectId::ZERO, 0);
    tracer
        .trace_value(&mut samples, &Data::Struct(sample_move_obj))
        .unwrap();
    let sample_upgrade_info = UpgradeInfo {
        upgraded_id: ObjectId::ZERO,
        upgraded_version: 1u64.into(),
    };
    tracer
        .trace_value(&mut samples, &sample_upgrade_info)
        .unwrap();
    let sample_move_pkg = MovePackage {
        id: ObjectId::ZERO,
        version: 1u64.into(),
        modules: BTreeMap::from([(Identifier::from_static("module"), vec![0u8])]),
        type_origin_table: vec![tot.clone()],
        linkage_table: BTreeMap::from([(ObjectId::ZERO, sample_upgrade_info)]),
    };
    tracer.trace_value(&mut samples, &sample_move_pkg).unwrap();
    tracer
        .trace_value(&mut samples, &Data::Package(sample_move_pkg))
        .unwrap();

    // Trace SDK types with custom serde (ExecutionStatus, ExecutionError,
    // CommandArgumentError, PackageUpgradeError). These delegate to internal
    // Binary* helper types that serde_reflection cannot auto-discover through
    // trace_type alone.
    //
    // Strategy: seed with trace_value for the types containing custom-serde
    // fields (MoveLocation, both ExecutionStatus variants), then use repeated
    // trace_type_once calls to let the deserializer discover remaining variants.
    let move_location = MoveLocation {
        package: ObjectId::ZERO,
        module: Identifier::from_static("foo"),
        function: 0,
        instruction: 0,
        function_name: Some(Identifier::from_static("foo")),
    };
    tracer.trace_value(&mut samples, &move_location).unwrap();
    tracer.trace_type::<MoveLocation>(&samples).unwrap();

    tracer
        .trace_value(&mut samples, &ExecutionStatus::Success)
        .unwrap();
    tracer
        .trace_value(
            &mut samples,
            &ExecutionStatus::Failure {
                error: ExecutionError::InsufficientGas,
                command: Some(0),
            },
        )
        .unwrap();

    // Discover all remaining enum variants via deserialization. trace_type
    // loops internally until all variants of the (internal Binary*) enum are
    // found, using the samples we seeded above for custom-serde fields.
    tracer.trace_type::<ExecutionError>(&samples).unwrap();
    tracer.trace_type::<CommandArgumentError>(&samples).unwrap();
    tracer.trace_type::<PackageUpgradeError>(&samples).unwrap();

    // 2. Trace the main entry point(s) + every enum separately.
    tracer.trace_type::<Owner>(&samples).unwrap();
    // Trace all CallArg (= iota_sdk_types::Input) variants
    tracer
        .trace_value(&mut samples, &CallArg::Pure(vec![0u8]))
        .unwrap();
    tracer
        .trace_value(
            &mut samples,
            &CallArg::ImmutableOrOwned(iota_types::base_types::ObjectRef::new(
                ObjectId::ZERO,
                1u64.into(),
                ObjectDigest::random(),
            )),
        )
        .unwrap();
    tracer
        .trace_value(
            &mut samples,
            &CallArg::Shared(SharedObjectRef::new(ObjectId::ZERO, 1u64.into(), false)),
        )
        .unwrap();
    tracer
        .trace_value(
            &mut samples,
            &CallArg::Receiving(iota_types::base_types::ObjectRef::new(
                ObjectId::ZERO,
                1u64.into(),
                ObjectDigest::random(),
            )),
        )
        .unwrap();
    tracer.trace_type::<CallArg>(&samples).unwrap();
    tracer.trace_type::<TypedStoreError>(&samples).unwrap();
    tracer
        .trace_type::<ObjectInfoRequestKind>(&samples)
        .unwrap();

    // Trace all TransactionKind variants via trace_value
    let sample_pt = ProgrammableTransaction {
        inputs: vec![CallArg::Pure(vec![0u8])],
        commands: vec![Command::new_make_move_vector(None, vec![])],
    };
    tracer
        .trace_value(&mut samples, &TransactionKind::Programmable(sample_pt))
        .unwrap();
    let sample_genesis_obj = GenesisObject::new(
        Data::Struct(MoveObject::new_gas_coin(1u64.into(), ObjectId::ZERO, 0)),
        Owner::Address(IotaAddress::ZERO),
    );
    tracer
        .trace_value(
            &mut samples,
            &TransactionKind::Genesis(GenesisTransaction {
                objects: vec![sample_genesis_obj.clone()],
                events: vec![event.clone()],
            }),
        )
        .unwrap();
    tracer
        .trace_value(
            &mut samples,
            &TransactionKind::ConsensusCommitPrologueV1(ConsensusCommitPrologueV1 {
                epoch: 0,
                round: 0,
                sub_dag_index: Some(0),
                commit_timestamp_ms: 0,
                consensus_commit_digest: ConsensusCommitDigest::default(),
                consensus_determined_version_assignments:
                    ConsensusDeterminedVersionAssignments::CancelledTransactions {
                        cancelled_transactions: vec![],
                    },
            }),
        )
        .unwrap();
    // EndOfEpochTransaction variant is already covered by sender_data below
    tracer
        .trace_value(
            &mut samples,
            &TransactionKind::RandomnessStateUpdate(RandomnessStateUpdate {
                epoch: 0,
                randomness_round: 0u64.into(),
                random_bytes: vec![0u8],
                randomness_obj_initial_shared_version: 0u64.into(),
            }),
        )
        .unwrap();

    // Trace GenesisObject (single-variant enum)
    tracer
        .trace_value(&mut samples, &sample_genesis_obj)
        .unwrap();

    // Trace Object via trace_value. Object is a newtype wrapper around
    // Arc<ObjectInner>, but ObjectInner has #[serde(rename = "Object")],
    // so we need to trace ObjectInner directly to avoid a format conflict
    // (Struct vs NewTypeStruct both named "Object").
    let sample_obj_inner = ObjectInner {
        data: Data::Struct(MoveObject::new_gas_coin(1u64.into(), ObjectId::ZERO, 0)),
        owner: Owner::Address(IotaAddress::ZERO),
        previous_transaction: TransactionDigest::default(),
        storage_rebate: 0,
    };
    tracer.trace_value(&mut samples, &sample_obj_inner).unwrap();

    // Trace TransactionEvents via trace_value
    let sample_events = TransactionEvents(vec![Event {
        package_id: ObjectId::ZERO,
        module: Identifier::from_static("foo"),
        sender: IotaAddress::ZERO,
        type_: struct_tag.clone(),
        contents: vec![0],
    }]);
    tracer.trace_value(&mut samples, &sample_events).unwrap();

    tracer
        .trace_type::<base_types::IotaAddress>(&samples)
        .unwrap();
    tracer.trace_type::<DeleteKind>(&samples).unwrap();
    tracer.trace_type::<Argument>(&samples).unwrap();
    // Trace all Command variants explicitly — MoveCall contains Identifier and
    // TypeTag fields with custom serde, so trace_type alone cannot deserialize
    // them.
    tracer
        .trace_value(
            &mut samples,
            &Command::new_move_call(
                ObjectId::ZERO,
                Identifier::from_static("foo"),
                Identifier::from_static("bar"),
                vec![TypeTag::U64],
                vec![Argument::Gas],
            ),
        )
        .unwrap();
    tracer
        .trace_value(
            &mut samples,
            &Command::new_transfer_objects(vec![Argument::Input(0)], Argument::Gas),
        )
        .unwrap();
    tracer
        .trace_value(
            &mut samples,
            &Command::new_split_coins(Argument::Gas, vec![Argument::Input(0)]),
        )
        .unwrap();
    tracer
        .trace_value(
            &mut samples,
            &Command::new_merge_coins(Argument::Gas, vec![Argument::Input(0)]),
        )
        .unwrap();
    tracer
        .trace_value(
            &mut samples,
            &Command::new_publish(vec![vec![0u8]], vec![ObjectId::ZERO]),
        )
        .unwrap();
    tracer
        .trace_value(
            &mut samples,
            &Command::new_make_move_vector(None, vec![Argument::Gas]),
        )
        .unwrap();
    tracer
        .trace_value(
            &mut samples,
            &Command::new_make_move_vector(Some(TypeTag::U64), vec![Argument::Input(0)]),
        )
        .unwrap();
    tracer
        .trace_value(
            &mut samples,
            &Command::new_upgrade(
                vec![vec![0u8]],
                vec![ObjectId::ZERO],
                ObjectId::ZERO,
                Argument::Input(0),
            ),
        )
        .unwrap();
    tracer.trace_type::<TransactionKind>(&samples).unwrap();
    tracer.trace_type::<TypeArgumentError>(&samples).unwrap();
    tracer
        .trace_type::<TransactionExpiration>(&samples)
        .unwrap();
    tracer
        .trace_type::<EndOfEpochTransactionKind>(&samples)
        .unwrap();

    tracer.trace_type::<IDOperation>(&samples).unwrap();
    tracer.trace_type::<ObjectIn>(&samples).unwrap();
    tracer.trace_type::<ObjectOut>(&samples).unwrap();
    tracer.trace_type::<UnchangedSharedKind>(&samples).unwrap();
    tracer.trace_type::<TransactionEffects>(&samples).unwrap();

    tracer.trace_type::<CheckpointContents>(&samples).unwrap();
    tracer.trace_type::<CheckpointSummary>(&samples).unwrap();
    tracer.trace_type::<CheckpointCommitment>(&samples).unwrap();
    tracer
        .trace_type::<ConsensusDeterminedVersionAssignments>(&samples)
        .unwrap();

    let sender_data = SenderSignedData::new(
        TransactionData::new_with_gas_coins(
            TransactionKind::EndOfEpoch(vec![EndOfEpochTransactionKind::ChangeEpoch(
                ChangeEpoch {
                    epoch: 0,
                    protocol_version: 0,
                    storage_charge: 0,
                    computation_charge: 0,
                    storage_rebate: 0,
                    non_refundable_storage_fee: 0,
                    epoch_start_timestamp_ms: 0,
                    system_packages: vec![],
                },
            )]),
            IotaAddress::ZERO,
            vec![iota_types::base_types::ObjectRef::new(
                ObjectId::ZERO,
                1u64.into(),
                ObjectDigest::default(),
            )],
            0,
            0,
        ),
        // TODO remove conversion https://github.com/iotaledger/iota/issues/11590
        vec![GenericSignature::Signature(
            Signature::Ed25519IotaSignature(
                Ed25519IotaSignature::from_bytes(&sig1.to_bytes()).unwrap(),
            ),
        )],
    );
    tracer.trace_value(&mut samples, &sender_data).unwrap();

    let quorum_sig: AuthorityStrongQuorumSignInfo = AuthorityQuorumSignInfo {
        epoch: 0,
        signature: AggregateAuthoritySignature::default(),
        signers_map: RoaringBitmap::default(),
    };
    tracer.trace_value(&mut samples, &quorum_sig).unwrap();

    tracer
        .trace_type::<CertifiedCheckpointSummary>(&samples)
        .unwrap();

    // Trace FullCheckpointContents, CheckpointTransaction and CheckpointData
    // via trace_value (they transitively contain TypeTag).
    let sample_transaction = Transaction::new(sender_data.clone());
    let sample_effects = TransactionEffects::new_empty_v1_for_testing(TransactionDigest::default());
    let sample_exec_data = ExecutionData {
        transaction: sample_transaction.clone(),
        effects: sample_effects.clone(),
    };
    let sample_full_ckpt = FullCheckpointContents::new_with_causally_ordered_transactions(
        std::iter::once(sample_exec_data),
    );
    tracer.trace_value(&mut samples, &sample_full_ckpt).unwrap();

    // Use empty vecs for input_objects/output_objects because
    // Object(Arc<ObjectInner>) cannot be serialized through serde-reflection:
    // both Object and ObjectInner use serde name "Object" but register as
    // NewTypeStruct vs Struct respectively. The Object format is already
    // registered via the ObjectInner trace above. After tracing, we patch the
    // registry to replace Seq(Unknown) with Seq(TypeName("Object")) for these
    // fields.
    let sample_ckpt_tx = CheckpointTransaction {
        transaction: sample_transaction,
        effects: sample_effects,
        events: Some(sample_events),
        input_objects: vec![],
        output_objects: vec![],
    };
    tracer.trace_value(&mut samples, &sample_ckpt_tx).unwrap();

    let sample_ckpt_summary = CheckpointSummary {
        epoch: 0,
        sequence_number: 0,
        network_total_transactions: 0,
        content_digest: CheckpointContentsDigest::default(),
        previous_digest: None,
        epoch_rolling_gas_cost_summary: iota_sdk_types::gas::GasCostSummary::new(0, 0, 0, 0, 0),
        timestamp_ms: 0,
        checkpoint_commitments: vec![],
        end_of_epoch_data: None,
        version_specific_data: vec![],
    };
    let sample_ckpt_data = CheckpointData {
        checkpoint_summary: CertifiedCheckpointSummary::new_from_data_and_sig(
            sample_ckpt_summary,
            quorum_sig.clone(),
        ),
        checkpoint_contents: CheckpointContents::new_with_digests_only_for_tests(vec![]),
        transactions: vec![sample_ckpt_tx],
    };
    tracer.trace_value(&mut samples, &sample_ckpt_data).unwrap();

    // Use registry_unchecked() because trace_type::<TransactionEffects>
    // re-encounters ExecutionStatus during deserialization and marks it as
    // incomplete, even though all variants were already discovered above.
    let mut registry = tracer.registry_unchecked();

    // Clean up spurious high-index variants injected by serde_reflection's
    // deserializer when it re-encounters already-complete enums.
    for container in registry.values_mut() {
        if let ContainerFormat::Enum(variants) = container {
            variants.retain(|idx, _| *idx < u32::MAX / 2);
        }
    }

    // Patch CheckpointTransaction's input_objects and output_objects fields.
    // These were traced with empty vecs (producing Seq(Unknown)) because
    // Object(Arc<ObjectInner>) can't be serialized through serde-reflection
    // without a name collision between the Object newtype and ObjectInner's
    // #[serde(rename = "Object")]. The correct element type is already in the
    // registry from tracing ObjectInner directly.
    let object_seq = Format::Seq(Box::new(Format::TypeName("Object".into())));
    if let Some(ContainerFormat::Struct(fields)) = registry.get_mut("CheckpointTransaction") {
        for field in fields.iter_mut() {
            if field.name == "input_objects" || field.name == "output_objects" {
                field.value = object_seq.clone();
            }
        }
    }

    Ok(registry)
}

#[derive(Debug, Parser, Clone, Copy, ValueEnum)]
enum Action {
    Print,
    Test,
    Record,
}

#[derive(Debug, Parser)]
#[command(
    name = "IOTA format generator",
    about = "Trace serde (de)serialization to generate format descriptions for IOTA types"
)]
struct Options {
    #[arg(value_enum, default_value = "Print", ignore_case = true)]
    action: Action,
}

const FILE_PATH: &str = "iota-core/tests/staged/iota.yaml";

fn main() {
    let options = Options::parse();
    let registry = match get_registry() {
        Ok(registry) => registry,
        Err(e) => {
            eprintln!("Error generating registry: {}", e.explanation());
            std::process::exit(1);
        }
    };
    match options.action {
        Action::Print => {
            let content = serde_yaml::to_string(&registry).unwrap();
            println!("{content}");
        }
        Action::Record => {
            let content = serde_yaml::to_string(&registry).unwrap();
            let mut f = File::create(FILE_PATH).unwrap();
            writeln!(f, "{content}").unwrap();
        }
        Action::Test => {
            let reference = std::fs::read_to_string(FILE_PATH).unwrap();
            let content: String = serde_yaml::to_string(&registry).unwrap() + "\n";
            assert_str_eq!(&reference, &content);
        }
    }
}
