// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashSet, env, fs::File, io::Read, path::PathBuf};

use expect_test::expect;
use iota_framework::BuiltInFramework;
use iota_move_build::{BuildConfig, check_unpublished_dependencies, gather_published_ids};
use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::{
    ExecutionError, ExecutionStatus, Identifier, ObjectData, ObjectId, Owner, Transaction,
};
use iota_types::{
    crypto::{AccountPrivateKey, get_key_pair},
    effects::TransactionEffectsAPI,
    error::{IotaError, UserInputError},
    object::ObjectRead,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::{TEST_ONLY_GAS_UNIT_FOR_PUBLISH, TransactionAPI},
    utils::to_sender_signed_transaction,
};
use move_binary_format::{
    CompiledModule,
    file_format::{
        Bytecode, CodeUnit, FunctionDefinition, FunctionHandle, FunctionHandleIndex,
        IdentifierIndex, Signature, SignatureIndex, SignatureToken, StructDefinitionIndex,
        Visibility,
    },
    file_format_common::{BinaryConstants, VERSION_6},
};
use move_package::source_package::manifest_parser;

use crate::authority::{
    authority_tests::{call_move, init_state_with_ids, send_and_confirm_transaction},
    move_integration_tests::{
        build_and_publish_test_package, build_multi_publish_txns, build_package,
        build_test_package, run_multi_txns,
    },
};

#[tokio::test]
#[cfg_attr(msim, ignore)]
async fn test_publishing_with_unpublished_deps() {
    let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
    let gas = ObjectId::random();
    let authority = init_state_with_ids(vec![(sender, gas)]).await;

    let package = build_and_publish_test_package(
        &authority,
        &sender,
        &sender_key,
        &gas,
        "depends_on_basics",
        // with_unpublished_deps
        true,
    )
    .await;

    let ObjectRead::Exists(read_ref, package_obj, _) =
        authority.get_object_read(&package.object_id).unwrap()
    else {
        panic!("Can't read package")
    };

    assert_eq!(package, read_ref);
    let ObjectData::Package(move_package) = package_obj.into_inner().data else {
        panic!("Not a package")
    };

    // Check that the published package includes its depended upon module.
    assert_eq!(
        move_package
            .serialized_module_map()
            .keys()
            .map(<Identifier>::as_str)
            .collect::<HashSet<_>>(),
        HashSet::from(["depends_on_basics", "object_basics"]),
    );

    let effects = call_move(
        &authority,
        &gas,
        &sender,
        &sender_key,
        &package.object_id,
        "depends_on_basics",
        "delegate",
        vec![],
        vec![],
    )
    .await
    .unwrap();

    assert!(effects.status().is_success());
    assert_eq!(effects.created().len(), 1);
    let created = effects.created()[0];
    let (object_ref, owner) = (*created.reference(), *created.owner());
    let v = object_ref.version;

    // Check that calling the function does what we expect
    assert!(matches!(
        owner,
        Owner::Shared(initial) if initial == v
    ));
}

#[tokio::test]
#[cfg_attr(msim, ignore)]
async fn test_publish_empty_package() {
    let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
    let gas = ObjectId::random();
    let authority = init_state_with_ids(vec![(sender, gas)]).await;
    let rgp = authority.reference_gas_price_for_testing().unwrap();
    let gas_object = authority.get_object(&gas);
    let gas_object_ref = gas_object.unwrap().object_ref();

    // empty package
    let tx = Transaction::new_module(
        sender,
        gas_object_ref,
        vec![],
        vec![],
        rgp * TEST_ONLY_GAS_UNIT_FOR_PUBLISH,
        rgp,
    );
    let transaction = to_sender_signed_transaction(tx, &sender_key);
    let err = send_and_confirm_transaction(&authority, transaction)
        .await
        .unwrap_err();
    assert_eq!(
        err,
        IotaError::UserInput {
            error: UserInputError::EmptyCommandInput
        }
    );

    // empty module
    let tx = Transaction::new_module(
        sender,
        gas_object_ref,
        vec![vec![]],
        vec![],
        rgp * TEST_ONLY_GAS_UNIT_FOR_PUBLISH,
        rgp,
    );
    let transaction = to_sender_signed_transaction(tx, &sender_key);
    let result = send_and_confirm_transaction(&authority, transaction)
        .await
        .unwrap()
        .1;
    assert_eq!(
        result.status(),
        &ExecutionStatus::Failure {
            error: ExecutionError::VmVerificationOrDeserializationError,
            command: Some(0)
        }
    )
}

#[tokio::test]
#[cfg_attr(msim, ignore)]
async fn test_publish_duplicate_modules() {
    let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
    let gas = ObjectId::random();
    let authority = init_state_with_ids(vec![(sender, gas)]).await;
    let gas_object = authority.get_object(&gas);
    let gas_object_ref = gas_object.unwrap().object_ref();
    let rgp = authority.reference_gas_price_for_testing().unwrap();

    // empty package
    let mut modules = build_test_package("object_owner", /* with_unpublished_deps */ false);
    assert_eq!(modules.len(), 1);
    modules.push(modules[0].clone());
    let tx = Transaction::new_module(
        sender,
        gas_object_ref,
        modules,
        BuiltInFramework::all_package_ids(),
        rgp * TEST_ONLY_GAS_UNIT_FOR_PUBLISH,
        rgp,
    );
    let transaction = to_sender_signed_transaction(tx, &sender_key);
    let result = send_and_confirm_transaction(&authority, transaction)
        .await
        .unwrap()
        .1;
    assert_eq!(
        result.status(),
        &ExecutionStatus::Failure {
            error: ExecutionError::VmVerificationOrDeserializationError,
            command: Some(0)
        }
    )
}

#[tokio::test]
#[cfg_attr(msim, ignore)]
async fn test_generate_lock_file() {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.extend(["src", "unit_tests", "data", "generate_move_lock_file"]);

    let tmp_dir = iota_common::tempdir();
    let lock_file_path = tmp_dir.path().join("Move.lock");

    let mut build_config = BuildConfig::new_for_testing();
    build_config.config.lock_file = Some(lock_file_path.clone());
    build_config
        .clone()
        .build(&path)
        .expect("Move package did not build");
    // Update the lock file with placeholder compiler version so this isn't bumped
    // every release.
    build_config
        .config
        .update_lock_file_toolchain_version(&path, "0.0.1".into())
        .expect("Could not update lock file");

    let mut lock_file_contents = String::new();
    File::open(lock_file_path)
        .expect("Cannot open lock file")
        .read_to_string(&mut lock_file_contents)
        .expect("Error reading Move.lock file");

    let expected = expect![[r##"
        # @generated by Move, please check-in and do not edit manually.

        [move]
        version = 3
        manifest_digest = "37689E9F9E5809521FA06520D55141982F5B8F26F5C55DE4E88FA63D73E1FEFF"
        deps_digest = "3C4103934B1E040BB6B23F1D610B4EF9F2F1166A50A104EADCF77467C004C600"
        dependencies = [
          { id = "Examples", name = "Examples" },
          { id = "Iota", name = "Iota" },
        ]

        [[move.package]]
        id = "Examples"
        source = { local = "../object_basics" }

        dependencies = [
          { id = "Iota", name = "Iota" },
        ]

        [[move.package]]
        id = "Iota"
        source = { local = "../../../../../iota-framework/packages/iota-framework" }

        dependencies = [
          { id = "MoveStdlib", name = "MoveStdlib" },
        ]

        [[move.package]]
        id = "MoveStdlib"
        source = { local = "../../../../../iota-framework/packages/move-stdlib" }

        [move.toolchain-version]
        compiler-version = "0.0.1"
        edition = "2024"
        flavor = "iota"
    "##]];
    expected.assert_eq(lock_file_contents.as_str());
}

#[tokio::test]
#[cfg_attr(msim, ignore)]
async fn test_custom_property_parse_published_at() {
    let build_config = BuildConfig::new_for_testing();
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.extend(["src", "unit_tests", "data", "custom_properties_in_manifest"]);

    build_config
        .build(&path)
        .expect("Move package did not build");
    let manifest = manifest_parser::parse_move_manifest_from_file(path.as_path())
        .expect("Could not parse Move.toml");
    let properties = manifest
        .package
        .custom_properties
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect::<Vec<_>>();

    let expected = expect![[r#"
        [
            (
                "published-at",
                "0x777",
            ),
        ]
    "#]];
    expected.assert_debug_eq(&properties)
}

#[tokio::test]
#[cfg_attr(msim, ignore)]
async fn test_custom_property_check_unpublished_dependencies() {
    let build_config = BuildConfig::new_for_testing();
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.extend([
        "src",
        "unit_tests",
        "data",
        "custom_properties_in_manifest_ensure_published_at",
    ]);

    let resolution_graph = build_config
        .config
        .resolution_graph_for_package(&path, None, &mut std::io::sink())
        .expect("Could not build resolution graph.");

    let IotaError::ModulePublishFailure { error } = check_unpublished_dependencies(
        &gather_published_ids(&resolution_graph, None).1.unpublished,
    )
    .err()
    .unwrap() else {
        panic!("Expected ModulePublishFailure")
    };

    let expected = expect![[r#"
        Package dependency "CustomPropertiesInManifestDependencyMissingPublishedAt" does not specify a published address (the Move.toml manifest for "CustomPropertiesInManifestDependencyMissingPublishedAt" does not contain a 'published-at' field, nor is there a 'published-id' in the Move.lock). You can use `iota move manage-package` to record the on-chain address for "CustomPropertiesInManifestDependencyMissingPublishedAt".
        If this is intentional, you may use the --with-unpublished-dependencies flag to continue publishing these dependencies as part of your package (they won't be linked against existing packages on-chain)."#]];
    expected.assert_eq(&error)
}

#[tokio::test]
#[cfg_attr(msim, ignore)]
async fn test_publish_extraneous_bytes_modules() {
    let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
    let gas = ObjectId::random();
    let authority = init_state_with_ids(vec![(sender, gas)]).await;
    let gas_object = authority.get_object(&gas);
    let gas_object_ref = gas_object.unwrap().object_ref();
    let rgp = authority.reference_gas_price_for_testing().unwrap();

    // test valid module bytes
    let correct_modules =
        build_test_package("object_owner", /* with_unpublished_deps */ false);
    assert_eq!(correct_modules.len(), 1);
    let tx = Transaction::new_module(
        sender,
        gas_object_ref,
        correct_modules.clone(),
        BuiltInFramework::all_package_ids(),
        rgp * TEST_ONLY_GAS_UNIT_FOR_PUBLISH,
        rgp,
    );
    let transaction = to_sender_signed_transaction(tx, &sender_key);
    let result = send_and_confirm_transaction(&authority, transaction)
        .await
        .unwrap()
        .1;
    assert_eq!(result.status(), &ExecutionStatus::Success);

    // make the bytes invalid
    let gas_object = authority.get_object(&gas);
    let gas_object_ref = gas_object.unwrap().object_ref();
    let mut modules = correct_modules.clone();
    modules[0].push(0);
    assert_eq!(modules.len(), 1);
    let tx = Transaction::new_module(
        sender,
        gas_object_ref,
        modules,
        BuiltInFramework::all_package_ids(),
        rgp * TEST_ONLY_GAS_UNIT_FOR_PUBLISH,
        rgp,
    );
    let transaction = to_sender_signed_transaction(tx, &sender_key);
    let result = send_and_confirm_transaction(&authority, transaction)
        .await
        .unwrap()
        .1;
    assert_eq!(
        result.status(),
        &ExecutionStatus::Failure {
            error: ExecutionError::VmVerificationOrDeserializationError,
            command: Some(0)
        }
    );

    // make the bytes invalid, in a different way
    let gas_object = authority.get_object(&gas);
    let gas_object_ref = gas_object.unwrap().object_ref();
    let mut modules = correct_modules.clone();
    let first_module = modules[0].clone();
    modules[0].extend(first_module);
    assert_eq!(modules.len(), 1);
    let tx = Transaction::new_module(
        sender,
        gas_object_ref,
        modules,
        BuiltInFramework::all_package_ids(),
        rgp * TEST_ONLY_GAS_UNIT_FOR_PUBLISH,
        rgp,
    );
    let transaction = to_sender_signed_transaction(tx, &sender_key);
    let result = send_and_confirm_transaction(&authority, transaction)
        .await
        .unwrap()
        .1;
    assert_eq!(
        result.status(),
        &ExecutionStatus::Failure {
            error: ExecutionError::VmVerificationOrDeserializationError,
            command: Some(0)
        }
    );

    // make the bytes invalid by adding metadata
    let gas_object = authority.get_object(&gas);
    let gas_object_ref = gas_object.unwrap().object_ref();
    let mut modules = correct_modules.clone();
    let new_bytes = {
        let mut m = CompiledModule::deserialize_with_defaults(&modules[0]).unwrap();
        m.metadata.push(move_core_types::metadata::Metadata {
            key: vec![0],
            value: vec![1],
        });
        let mut buf = vec![];
        m.serialize_with_version(m.version, &mut buf).unwrap();
        buf
    };
    modules[0] = new_bytes;
    assert_eq!(modules.len(), 1);
    let tx = Transaction::new_module(
        sender,
        gas_object_ref,
        modules,
        BuiltInFramework::all_package_ids(),
        rgp * TEST_ONLY_GAS_UNIT_FOR_PUBLISH,
        rgp,
    );
    let transaction = to_sender_signed_transaction(tx, &sender_key);
    let result = send_and_confirm_transaction(&authority, transaction)
        .await
        .unwrap()
        .1;
    assert_eq!(
        result.status(),
        &ExecutionStatus::Failure {
            error: ExecutionError::VmVerificationOrDeserializationError,
            command: Some(0)
        }
    )
}

/// Publish a version 6 module as the serializer wrote it, which must succeed,
/// then again with the given high byte in the version field.
async fn publish_v6_module_with_flavor_byte(flavor_byte: u8) -> ExecutionStatus {
    let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
    let gas = ObjectId::random();
    let authority = init_state_with_ids(vec![(sender, gas)]).await;
    let rgp = authority.reference_gas_price_for_testing().unwrap();

    let modules = build_test_package("object_owner", /* with_unpublished_deps */ false);
    assert_eq!(modules.len(), 1);

    // Below binary format version 7 the header carries no flavor, so the high byte
    // of the version field is the part that must be zero.
    let v6_module = {
        let module = CompiledModule::deserialize_with_defaults(&modules[0]).unwrap();
        let mut buf = vec![];
        module.serialize_with_version(VERSION_6, &mut buf).unwrap();
        buf
    };
    let publish = |modules: Vec<Vec<u8>>| {
        let gas_object_ref = authority.get_object(&gas).unwrap().object_ref();
        let tx = Transaction::new_module(
            sender,
            gas_object_ref,
            modules,
            BuiltInFramework::all_package_ids(),
            rgp * TEST_ONLY_GAS_UNIT_FOR_PUBLISH,
            rgp,
        );
        to_sender_signed_transaction(tx, &sender_key)
    };

    let canonical = send_and_confirm_transaction(&authority, publish(vec![v6_module.clone()]))
        .await
        .unwrap()
        .1;
    assert_eq!(canonical.status(), &ExecutionStatus::Success);

    let mut doctored = v6_module;
    doctored[BinaryConstants::MOVE_MAGIC_SIZE + 3] = flavor_byte;
    let effects = send_and_confirm_transaction(&authority, publish(vec![doctored]))
        .await
        .unwrap()
        .1;

    effects.status().clone()
}

#[tokio::test]
#[cfg_attr(msim, ignore)]
async fn test_publish_non_canonical_version_header() {
    // Any non-zero high byte masks off to the same version, so the header would
    // otherwise be a second encoding of the canonical module. `0x05` is the flavor
    // the serializer writes from version 7 on, and is no more acceptable here.
    let rejected = ExecutionStatus::Failure {
        error: ExecutionError::VmVerificationOrDeserializationError,
        command: Some(0),
    };
    for flavor_byte in [0x01, 0x05, 0xFF] {
        assert_eq!(
            publish_v6_module_with_flavor_byte(flavor_byte).await,
            rejected,
            "flavor byte {flavor_byte:#04x}"
        );
    }
}

#[tokio::test]
#[cfg_attr(msim, ignore)]
async fn test_publish_non_canonical_version_header_before_the_check() {
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_check_canonical_module_version_header_for_testing(false);
        config
    });

    // Replaying a protocol version from before the check must still accept what it
    // accepted then.
    assert_eq!(
        publish_v6_module_with_flavor_byte(0xFF).await,
        ExecutionStatus::Success
    );
}

/// A module using one of the deprecated global storage instructions passes the
/// Move verifier, so the transaction is signed and executed; it is the IOTA
/// verifier that rejects it, during execution.
#[tokio::test]
#[cfg_attr(msim, ignore)]
async fn test_publish_deprecated_bytes_modules() {
    let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
    let gas = ObjectId::random();
    let authority = init_state_with_ids(vec![(sender, gas)]).await;
    let gas_object = authority.get_object(&gas);
    let gas_object_ref = gas_object.unwrap().object_ref();
    let rgp = authority.reference_gas_price_for_testing().unwrap();

    // test valid module bytes
    let correct_modules =
        build_test_package("object_owner", /* with_unpublished_deps */ false);
    assert_eq!(correct_modules.len(), 1);
    let tx = Transaction::new_module(
        sender,
        gas_object_ref,
        correct_modules.clone(),
        BuiltInFramework::all_package_ids(),
        rgp * TEST_ONLY_GAS_UNIT_FOR_PUBLISH,
        rgp,
    );
    let transaction = to_sender_signed_transaction(tx, &sender_key);
    let result = send_and_confirm_transaction(&authority, transaction)
        .await
        .unwrap()
        .1;
    assert_eq!(result.status(), &ExecutionStatus::Success);

    // give the module a private function that uses a deprecated global storage
    // instruction
    let gas_object = authority.get_object(&gas);
    let gas_object_ref = gas_object.unwrap().object_ref();
    let mut modules = correct_modules.clone();
    let new_bytes = {
        let mut m = CompiledModule::deserialize_with_defaults(&modules[0]).unwrap();

        // `exists` needs a struct with `key` and no type parameters, so that the
        // non-generic form of the instruction applies
        let key_struct = m
            .struct_defs
            .iter()
            .position(|def| {
                let handle = m.datatype_handle_at(def.struct_handle);
                handle.abilities.has_key() && handle.type_parameters.is_empty()
            })
            .expect("test package must declare a non-generic object");

        // reuse the signatures if they are already there: `DuplicationChecker`
        // rejects a module with two identical ones
        let empty = match m.signatures.iter().position(|sig| sig.0.is_empty()) {
            Some(idx) => idx,
            None => {
                m.signatures.push(Signature(vec![]));
                m.signatures.len() - 1
            }
        };
        let empty = SignatureIndex(empty as u16);
        let address = match m
            .signatures
            .iter()
            .position(|sig| sig.0.len() == 1 && sig.0[0] == SignatureToken::Address)
        {
            Some(idx) => idx,
            None => {
                m.signatures.push(Signature(vec![SignatureToken::Address]));
                m.signatures.len() - 1
            }
        };
        let address = SignatureIndex(address as u16);

        let name = IdentifierIndex(m.identifiers.len() as u16);
        m.identifiers
            .push(move_core_types::identifier::Identifier::new("uses_global_storage").unwrap());

        let function = FunctionHandleIndex(m.function_handles.len() as u16);
        m.function_handles.push(FunctionHandle {
            module: m.self_module_handle_idx,
            name,
            parameters: address,
            return_: empty,
            type_parameters: vec![],
        });
        m.function_defs.push(FunctionDefinition {
            function,
            visibility: Visibility::Private,
            is_entry: false,
            // `exists` does not acquire the resource it looks up
            acquires_global_resources: vec![],
            code: Some(CodeUnit {
                locals: empty,
                code: vec![
                    Bytecode::CopyLoc(0),
                    Bytecode::ExistsDeprecated(StructDefinitionIndex(key_struct as u16)),
                    Bytecode::Pop,
                    Bytecode::Ret,
                ],
                jump_tables: vec![],
            }),
        });

        let mut buf = vec![];
        m.serialize_with_version(m.version, &mut buf).unwrap();
        buf
    };
    modules[0] = new_bytes;
    let tx = Transaction::new_module(
        sender,
        gas_object_ref,
        modules,
        BuiltInFramework::all_package_ids(),
        rgp * TEST_ONLY_GAS_UNIT_FOR_PUBLISH,
        rgp,
    );
    let transaction = to_sender_signed_transaction(tx, &sender_key);
    let result = send_and_confirm_transaction(&authority, transaction)
        .await
        .unwrap()
        .1;
    // Raised at execution by `global_storage_access_verifier::verify_module` in
    // `iota-execution/latest/iota-verifier`, which rejects every deprecated
    // global storage instruction.
    assert_eq!(
        result.status(),
        &ExecutionStatus::Failure {
            error: ExecutionError::IotaMoveVerificationError,
            command: Some(0)
        }
    )
}

#[tokio::test]
#[cfg_attr(msim, ignore)]
async fn test_publish_max_packages() {
    let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
    let gas_object_id = ObjectId::random();
    let authority = init_state_with_ids(vec![(sender, gas_object_id)]).await;

    let (_, modules, dependencies) = build_package("object_basics", false);

    // push max number of packages allowed to publish
    let max_pub_cmd = authority
        .epoch_store_for_testing()
        .protocol_config()
        .max_publish_or_upgrade_per_ptb_as_option()
        .unwrap_or(0);
    assert!(max_pub_cmd > 0);
    let packages = vec![(modules, dependencies); max_pub_cmd as usize];

    let mut builder = ProgrammableTransactionBuilder::new();
    build_multi_publish_txns(&mut builder, sender, packages);
    let result = run_multi_txns(&authority, sender, &sender_key, &gas_object_id, builder)
        .await
        .unwrap()
        .1;
    let effects = result.into_data();
    assert_eq!(effects.status(), &ExecutionStatus::Success);
}

#[tokio::test]
#[cfg_attr(msim, ignore)]
async fn test_publish_more_than_max_packages_error() {
    let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
    let gas_object_id = ObjectId::random();
    let authority = init_state_with_ids(vec![(sender, gas_object_id)]).await;

    let (_, modules, dependencies) = build_package("object_basics", false);

    // push max number of packages allowed to publish
    let max_pub_cmd = authority
        .epoch_store_for_testing()
        .protocol_config()
        .max_publish_or_upgrade_per_ptb_as_option()
        .unwrap_or(0);
    assert!(max_pub_cmd > 0);
    let packages = vec![(modules, dependencies); (max_pub_cmd + 1) as usize];

    let mut builder = ProgrammableTransactionBuilder::new();
    build_multi_publish_txns(&mut builder, sender, packages);
    let err = run_multi_txns(&authority, sender, &sender_key, &gas_object_id, builder)
        .await
        .unwrap_err();
    assert_eq!(
        err,
        IotaError::UserInput {
            error: UserInputError::MaxPublishCountExceeded {
                max_publish_commands: max_pub_cmd,
                publish_count: max_pub_cmd + 1,
            }
        }
    );
}
