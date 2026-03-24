// Copyright (c) 2021, Facebook, Inc. and its affiliates
// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeMap, fs::File, io::Write, str::FromStr};

use clap::*;
use iota_sdk_types::crypto::{Intent, IntentMessage, PersonalMessage};
use iota_types::{
    base_types::{
        self, Identifier, IotaAddress, ObjectDigest, ObjectID, StructTag, TransactionDigest,
        TransactionEffectsDigest, TypeTag,
    },
    crypto::{
        AccountKeyPair, AggregateAuthoritySignature, AuthorityKeyPair, AuthorityPublicKeyBytes,
        AuthorityQuorumSignInfo, AuthoritySignature, AuthorityStrongQuorumSignInfo, IotaKeyPair,
        KeypairTraits, Signature, Signer, get_key_pair, get_key_pair_from_rng,
    },
    effects::{
        IDOperation, ObjectIn, ObjectOut, TransactionEffects, TransactionEvents,
        UnchangedSharedKind,
    },
    event::Event,
    execution_status::{
        CommandArgumentError, ExecutionFailureStatus, ExecutionStatus, MoveLocation,
        PackageUpgradeError, TypeArgumentError,
    },
    full_checkpoint_content::{CheckpointData, CheckpointTransaction},
    messages_checkpoint::{
        CertifiedCheckpointSummary, CheckpointCommitment, CheckpointContents,
        CheckpointContentsDigest, CheckpointDigest, CheckpointSummary, FullCheckpointContents,
    },
    messages_consensus::ConsensusDeterminedVersionAssignments,
    messages_grpc::ObjectInfoRequestKind,
    move_package::{MovePackage, TypeOrigin},
    multisig::{MultiSig, MultiSigPublicKey},
    object::{Data, MoveObject, Object, Owner},
    signature::GenericSignature,
    storage::DeleteKind,
    transaction::{
        Argument, CallArg, Command, EndOfEpochTransactionKind, GenesisObject, SenderSignedData,
        SharedObjectRef, TransactionData, TransactionExpiration, TransactionKind,
    },
    type_input::{StructInput, TypeInput},
};
use move_core_types::{account_address::AccountAddress, language_storage::ModuleId};
use pretty_assertions::assert_str_eq;
use rand::{SeedableRng, rngs::StdRng};
use roaring::RoaringBitmap;
use serde_reflection::{ContainerFormat, Registry, Result, Samples, Tracer, TracerConfig};
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
    let sdk_identifier = iota_types::base_types::Identifier::from_static("sample_identifier");
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
            &iota_sdk_types::MoveObjectType::from(StructTag::new(
                iota_sdk_types::Address::ZERO,
                iota_sdk_types::Identifier::from_static("m"),
                iota_sdk_types::Identifier::from_static("T"),
                Vec::new(),
            )),
        )
        .unwrap();
    // GasCoin (variant 1)
    tracer
        .trace_value(
            &mut samples,
            &iota_sdk_types::MoveObjectType::from(StructTag::new_gas_coin()),
        )
        .unwrap();
    // StakedIota (variant 2)
    tracer
        .trace_value(
            &mut samples,
            &iota_sdk_types::MoveObjectType::from(StructTag::new_staked_iota()),
        )
        .unwrap();
    // Coin (variant 3) - non-IOTA coin
    tracer
        .trace_value(
            &mut samples,
            &iota_sdk_types::MoveObjectType::from(StructTag::new_coin(TypeTag::Bool)),
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

    let kp1: IotaKeyPair =
        IotaKeyPair::Ed25519(get_key_pair_from_rng(&mut StdRng::from_seed([0; 32])).1);
    let kp2: IotaKeyPair =
        IotaKeyPair::Secp256k1(get_key_pair_from_rng(&mut StdRng::from_seed([0; 32])).1);
    let kp3: IotaKeyPair =
        IotaKeyPair::Secp256r1(get_key_pair_from_rng(&mut StdRng::from_seed([0; 32])).1);
    let multisig_pk = MultiSigPublicKey::new(
        vec![kp1.public(), kp2.public(), kp3.public()],
        vec![1, 1, 1],
        2,
    )
    .unwrap();

    let msg = IntentMessage::new(
        Intent::iota_transaction(),
        PersonalMessage("Message".as_bytes().to_vec().into()),
    );

    let sig1: GenericSignature = Signature::new_secure(&msg, &kp1).into();
    let sig2: GenericSignature = Signature::new_secure(&msg, &kp2).into();
    let sig3: GenericSignature = Signature::new_secure(&msg, &kp3).into();
    let sig4: GenericSignature = GenericSignature::from_str("BiVYDmenOnqS+thmz5m5SrZnWaKXZLVxgh+rri6LHXs25B0AAAAAnQF7InR5cGUiOiJ3ZWJhdXRobi5nZXQiLCAiY2hhbGxlbmdlIjoiQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQSIsIm9yaWdpbiI6Imh0dHA6Ly9sb2NhbGhvc3Q6NTE3MyIsImNyb3NzT3JpZ2luIjpmYWxzZSwgInVua25vd24iOiAidW5rbm93biJ9YgJMwqcOmZI7F/N+K5SMe4DRYCb4/cDWW68SFneSHoD2GxKKhksbpZ5rZpdrjSYABTCsFQQBpLORzTvbj4edWKd/AsEBeovrGvHR9Ku7critg6k7qvfFlPUngujXfEzXd8Eg").unwrap();

    let multi_sig =
        MultiSig::combine(vec![sig1.clone(), sig2.clone(), sig3.clone()], multisig_pk).unwrap();
    tracer.trace_value(&mut samples, &multi_sig).unwrap();

    let generic_sig_multi = GenericSignature::MultiSig(multi_sig);
    tracer
        .trace_value(&mut samples, &generic_sig_multi)
        .unwrap();

    tracer.trace_value(&mut samples, &sig1).unwrap();
    tracer.trace_value(&mut samples, &sig2).unwrap();
    tracer.trace_value(&mut samples, &sig3).unwrap();
    tracer.trace_value(&mut samples, &sig4).unwrap();
    // ObjectID and IotaAddress are the same length
    let oid: ObjectID = addr.into();
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
        package: ObjectID::random(),
    };
    tracer.trace_value(&mut samples, &tot).unwrap();

    let si = StructInput {
        address: IotaAddress::ZERO,
        module: "foo".to_owned(),
        name: "bar".to_owned(),
        type_params: vec![TypeInput::Bool],
    };
    tracer.trace_value(&mut samples, &si).unwrap();

    // We need Event sample here, because our GenesisTransaction contains an
    // Event while, sui's doesn't.
    let event = Event {
        package_id: ObjectID::random(),
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
    let sample_move_obj = MoveObject::new_gas_coin(1u64.into(), ObjectID::ZERO, 0);
    tracer
        .trace_value(&mut samples, &Data::Move(sample_move_obj))
        .unwrap();
    let sample_upgrade_info = iota_types::move_package::UpgradeInfo {
        upgraded_id: ObjectID::ZERO,
        upgraded_version: 1u64.into(),
    };
    tracer
        .trace_value(&mut samples, &sample_upgrade_info)
        .unwrap();
    let sample_move_pkg = MovePackage {
        id: ObjectID::ZERO,
        version: 1u64.into(),
        modules: BTreeMap::from([(Identifier::from_static("module"), vec![0u8])]),
        type_origin_table: vec![tot.clone()],
        linkage_table: BTreeMap::from([(ObjectID::ZERO, sample_upgrade_info)]),
    };
    tracer.trace_value(&mut samples, &sample_move_pkg).unwrap();
    tracer
        .trace_value(&mut samples, &Data::Package(sample_move_pkg))
        .unwrap();

    // Trace SDK types with custom serde (ExecutionStatus, ExecutionFailureStatus,
    // CommandArgumentError, PackageUpgradeError). These delegate to internal
    // Binary* helper types that serde_reflection cannot auto-discover through
    // trace_type alone.
    //
    // Strategy: seed with trace_value for the types containing custom-serde
    // fields (MoveLocation, both ExecutionStatus variants), then use repeated
    // trace_type_once calls to let the deserializer discover remaining variants.
    let move_location = MoveLocation {
        package: ObjectID::ZERO,
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
                error: ExecutionFailureStatus::InsufficientGas,
                command: Some(0),
            },
        )
        .unwrap();

    // Discover all remaining enum variants via deserialization. trace_type
    // loops internally until all variants of the (internal Binary*) enum are
    // found, using the samples we seeded above for custom-serde fields.
    tracer
        .trace_type::<ExecutionFailureStatus>(&samples)
        .unwrap();
    tracer.trace_type::<CommandArgumentError>(&samples).unwrap();
    tracer.trace_type::<PackageUpgradeError>(&samples).unwrap();

    // 2. Trace the main entry point(s) + every enum separately.
    tracer.trace_type::<StructInput>(&samples).unwrap();
    tracer.trace_type::<TypeInput>(&samples).unwrap();
    tracer.trace_type::<Owner>(&samples).unwrap();
    // Trace all CallArg (= iota_sdk_types::Input) variants
    tracer
        .trace_value(&mut samples, &CallArg::Pure(vec![0u8]))
        .unwrap();
    tracer
        .trace_value(
            &mut samples,
            &CallArg::ImmutableOrOwned(iota_types::base_types::ObjectRef::new(
                ObjectID::ZERO,
                1u64.into(),
                ObjectDigest::random(),
            )),
        )
        .unwrap();
    tracer
        .trace_value(
            &mut samples,
            &CallArg::Shared(SharedObjectRef {
                object_id: ObjectID::ZERO,
                initial_shared_version: 1u64.into(),
                mutable: false,
            }),
        )
        .unwrap();
    tracer
        .trace_value(
            &mut samples,
            &CallArg::Receiving(iota_types::base_types::ObjectRef::new(
                ObjectID::ZERO,
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
    tracer.trace_type::<TransactionKind>(&samples).unwrap();
    tracer
        .trace_type::<base_types::IotaAddress>(&samples)
        .unwrap();
    tracer.trace_type::<DeleteKind>(&samples).unwrap();
    tracer.trace_type::<Argument>(&samples).unwrap();
    tracer.trace_type::<Command>(&samples).unwrap();
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

    tracer
        .trace_type::<FullCheckpointContents>(&samples)
        .unwrap();
    tracer.trace_type::<CheckpointContents>(&samples).unwrap();
    tracer.trace_type::<CheckpointSummary>(&samples).unwrap();
    tracer.trace_type::<CheckpointCommitment>(&samples).unwrap();
    tracer.trace_type::<GenesisObject>(&samples).unwrap();
    tracer
        .trace_type::<ConsensusDeterminedVersionAssignments>(&samples)
        .unwrap();

    let sender_data = SenderSignedData::new(
        TransactionData::new_with_gas_coins(
            TransactionKind::EndOfEpochTransaction(Vec::new()),
            IotaAddress::ZERO,
            Vec::new(),
            0,
            0,
        ),
        Vec::new(),
    );
    tracer.trace_value(&mut samples, &sender_data).unwrap();
    tracer.trace_type::<TransactionData>(&samples).unwrap();

    let quorum_sig: AuthorityStrongQuorumSignInfo = AuthorityQuorumSignInfo {
        epoch: 0,
        signature: AggregateAuthoritySignature::default(),
        signers_map: RoaringBitmap::default(),
    };
    tracer.trace_value(&mut samples, &quorum_sig).unwrap();

    tracer
        .trace_type::<CertifiedCheckpointSummary>(&samples)
        .unwrap();

    tracer.trace_type::<Object>(&samples).unwrap();

    tracer.trace_type::<TransactionEvents>(&samples).unwrap();
    tracer
        .trace_type::<CheckpointTransaction>(&samples)
        .unwrap();

    tracer.trace_type::<CheckpointData>(&samples).unwrap();

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
    let registry = get_registry().unwrap();
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
