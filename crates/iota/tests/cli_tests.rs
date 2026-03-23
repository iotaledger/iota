// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[cfg(target_os = "windows")]
use std::os::windows::fs::FileExt;
#[cfg(not(msim))]
use std::str::FromStr;
use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    env,
    fmt::Write,
    fs::{self, read_dir},
    io::{self, Read, Seek, SeekFrom, Write as IoWrite},
    path::{Path, PathBuf},
    str, thread,
    time::Duration,
};

use expect_test::expect;
use fastcrypto::encoding::{Base64, Encoding};
use iota::{
    PrintableResult,
    client_commands::{
        DisplayOption, GasDataArgs, IotaClientCommandResult, IotaClientCommands, PaymentArgs,
        SwitchResponse, TxProcessingArgs, estimate_gas_budget,
    },
    client_ptb::ptb::{PTB, PTBCommandResult},
    iota_commands::{IotaCommand, IotaEnvConfig},
    key_identity::{KeyIdentity, get_identity_address},
};
use iota_config::IOTA_CLIENT_CONFIG;
use iota_json::IotaJsonValue;
use iota_json_rpc_types::{
    IotaExecutionStatus, IotaObjectData, IotaObjectDataFilter, IotaObjectDataOptions,
    IotaObjectResponse, IotaObjectResponseQuery, IotaRawData, IotaTransactionBlockDataAPI,
    IotaTransactionBlockEffects, IotaTransactionBlockEffectsAPI, ObjectChange, OwnedObjectRef,
    get_new_package_obj_from_response,
};
use iota_keys::keystore::AccountKeystore;
use iota_macros::sim_test;
use iota_move_build::{BuildConfig, IotaPackageHooks};
use iota_sdk::{IotaClient, PagedFn, wallet_context::WalletContext};
use iota_swarm_config::genesis_config::{AccountConfig, GenesisConfig};
use iota_test_transaction_builder::batch_make_transfer_transactions;
use iota_types::{
    base_types::{IotaAddress, ObjectID},
    crypto::{
        AccountKeyPair, Ed25519IotaSignature, IotaKeyPair, IotaSignatureInner,
        Secp256k1IotaSignature, SignatureScheme, get_key_pair,
    },
    error::IotaObjectResponseError,
    gas_coin::GasCoin,
    move_package::{MovePackage, UpgradeInfo},
    object::Owner,
    transaction::{
        TEST_ONLY_GAS_UNIT_FOR_GENERIC, TEST_ONLY_GAS_UNIT_FOR_OBJECT_BASICS,
        TEST_ONLY_GAS_UNIT_FOR_PUBLISH, TEST_ONLY_GAS_UNIT_FOR_SPLIT_COIN,
        TEST_ONLY_GAS_UNIT_FOR_TRANSFER, TransactionDataAPI,
    },
};
use move_package::{BuildConfig as MoveBuildConfig, lock_file::schema::ManagedPackage};
use serde_json::json;
use tempfile::TempDir;
use test_cluster::{TestCluster, TestClusterBuilder};
use tokio::time::sleep;

const TEST_DATA_DIR: &str = "tests/data/";

struct TreeShakingTest {
    test_cluster: TestCluster,
    client: IotaClient,
    rgp: u64,
    gas_obj_id: ObjectID,
    temp_dir: TempDir,
}

impl TreeShakingTest {
    async fn new() -> Result<Self, anyhow::Error> {
        let mut test_cluster = TestClusterBuilder::new().build().await;
        let rgp = test_cluster.get_reference_gas_price().await;
        let address = test_cluster.get_address_0();
        let context = &mut test_cluster.wallet;
        let client = context.get_client().await?;

        let object_refs = client
            .read_api()
            .get_owned_objects(
                address,
                Some(IotaObjectResponseQuery::new_with_options(
                    IotaObjectDataOptions::new()
                        .with_type()
                        .with_owner()
                        .with_previous_transaction(),
                )),
                None,
                None,
            )
            .await?
            .data;

        let gas_obj_id = object_refs.first().unwrap().object().unwrap().object_id;

        // Setup temp directory with test data
        let temp_dir = tempfile::Builder::new().prefix("tree_shaking").tempdir()?;
        std::fs::create_dir_all(temp_dir.path()).unwrap();
        let tests_dir = PathBuf::from(TEST_DATA_DIR);
        let framework_pkgs = PathBuf::from("../iota-framework/packages");
        copy_dir_all(tests_dir, temp_dir.path())?;
        copy_dir_all(framework_pkgs, temp_dir.path().join("system-packages"))?;

        Ok(Self {
            test_cluster,
            client,
            rgp,
            gas_obj_id,
            temp_dir,
        })
    }

    fn package_path(&self, name: &str) -> PathBuf {
        self.temp_dir
            .path()
            .to_path_buf()
            .join("tree_shaking")
            .join(name)
    }

    async fn publish_package(
        &mut self,
        package_name: &str,
        with_unpublished_dependencies: bool,
    ) -> Result<(ObjectID, ObjectID), anyhow::Error> {
        publish_package(
            self.package_path(package_name),
            self.test_cluster.wallet_mut(),
            self.rgp,
            self.gas_obj_id,
            with_unpublished_dependencies,
        )
        .await
    }

    async fn publish_package_without_tree_shaking(&mut self, package_name: &str) -> ObjectID {
        let package_path = self.package_path(package_name);

        let obj_ref = iota_test_transaction_builder::publish_package(
            self.test_cluster.wallet_mut(),
            package_path.clone(),
        )
        .await;

        obj_ref.0
    }

    async fn upgrade_package(
        &mut self,
        package_name: &str,
        upgrade_capability: ObjectID,
    ) -> Result<ObjectID, anyhow::Error> {
        let mut build_config = BuildConfig::new_for_testing().config;
        build_config.lock_file = Some(self.package_path(package_name).join("Move.lock"));
        let resp = IotaClientCommands::Upgrade {
            package_path: self.package_path(package_name),
            upgrade_capability,
            build_config,
            skip_dependency_verification: false,
            verify_deps: false,
            verify_compatibility: true,
            with_unpublished_dependencies: false,
            payment: PaymentArgs {
                gas: vec![self.gas_obj_id],
            },
            gas_data: GasDataArgs {
                gas_budget: Some(self.rgp * TEST_ONLY_GAS_UNIT_FOR_PUBLISH),
                ..Default::default()
            },
            processing: TxProcessingArgs::default(),
        }
        .execute(self.test_cluster.wallet_mut())
        .await?;

        let IotaClientCommandResult::TransactionBlock(publish_response) = resp else {
            unreachable!("Invalid response");
        };

        let IotaTransactionBlockEffects::V1(effects) = publish_response.effects.unwrap();
        assert!(effects.status.is_ok());

        let package_a_v1 = effects
            .created()
            .iter()
            .find(|refe| matches!(refe.owner, Owner::Immutable))
            .unwrap();
        Ok(package_a_v1.object_id())
    }

    async fn fetch_linkage_table(&self, pkg: ObjectID) -> BTreeMap<ObjectID, UpgradeInfo> {
        let move_pkg = fetch_move_packages(&self.client, vec![pkg]).await;
        move_pkg.first().unwrap().linkage_table().clone()
    }
}

/// Publishes a package and returns the package object id and the upgrade
/// capability object id Note that this sets the `Move.lock` file to be written
/// to the root of the package path.
async fn publish_package(
    package_path: PathBuf,
    context: &mut WalletContext,
    rgp: u64,
    gas_obj_id: ObjectID,
    with_unpublished_dependencies: bool,
) -> Result<(ObjectID, ObjectID), anyhow::Error> {
    let mut build_config = BuildConfig::new_for_testing().config;
    let move_lock_path = package_path.clone().join("Move.lock");
    build_config.lock_file = Some(move_lock_path.clone());
    let resp = IotaClientCommands::Publish {
        package_path: package_path.clone(),
        build_config: build_config.clone(),
        skip_dependency_verification: false,
        verify_deps: false,
        with_unpublished_dependencies,
        payment: PaymentArgs {
            gas: vec![gas_obj_id],
        },
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_PUBLISH),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;

    let IotaClientCommandResult::TransactionBlock(publish_response) = resp else {
        unreachable!("Invalid response");
    };

    let IotaTransactionBlockEffects::V1(effects) = publish_response.effects.unwrap();

    assert!(effects.status.is_ok());
    let package_a = effects
        .created()
        .iter()
        .find(|refe| matches!(refe.owner, Owner::Immutable))
        .unwrap();
    let cap = effects
        .created()
        .iter()
        .find(|refe| matches!(refe.owner, Owner::AddressOwner(_)))
        .unwrap();

    Ok((package_a.reference.object_id, cap.reference.object_id))
}

// Recursively copy a directory and all its contents
fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> io::Result<()> {
    fs::create_dir_all(&dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
        }
    }
    Ok(())
}

/// Fetch move packages based on the provided package IDs.
pub async fn fetch_move_packages(
    client: &IotaClient,
    package_ids: Vec<ObjectID>,
) -> Vec<MovePackage> {
    let objects = client
        .read_api()
        .multi_get_object_with_options(package_ids, IotaObjectDataOptions::bcs_lossless())
        .await
        .unwrap();

    objects
        .into_iter()
        .map(|o| {
            let o = o.into_object().unwrap();
            let Some(IotaRawData::Package(p)) = o.bcs else {
                panic!("Expected package");
            };
            p.to_move_package(u64::MAX /* safe as this pkg comes from the network */)
                .unwrap()
        })
        .collect()
}

/// Adds the `published-at` field to the Move.toml file. Pass in the
/// `address_id` if you want to set the `addresses` field in the Move.toml file.
///
/// Note that address_id works only if there's one item in the addresses
/// section. It does not know how to handle multiple addresses / addresses from
/// deps.
fn add_ids_to_manifest(
    package_path: &Path,
    published_at_id: &ObjectID,
    address_id: Option<ObjectID>,
) -> Result<(), anyhow::Error> {
    let content = std::fs::read_to_string(package_path.join("Move.toml"))?;
    let mut toml: toml::Value = toml::from_str(&content)?;
    if let Some(tbl) = toml.get_mut("package") {
        if let Some(tbl) = tbl.as_table_mut() {
            tbl.insert(
                "published-at".to_string(),
                toml::Value::String(published_at_id.to_hex_uncompressed()),
            );
        }
    }

    if let (Some(address_id), Some(tbl)) = (address_id, toml.get_mut("addresses")) {
        if let Some(tbl) = tbl.as_table_mut() {
            // Get the first address item
            let first_key = tbl.keys().next().unwrap();
            tbl.insert(
                first_key.to_string(),
                toml::Value::String(address_id.to_hex_uncompressed()),
            );
        }
    }

    let toml_str = toml::to_string(&toml)?;
    std::fs::write(package_path.join("Move.toml"), toml_str)?;
    Ok(())
}

#[tokio::test]
async fn test_addresses_command() -> Result<(), anyhow::Error> {
    let test_cluster = TestClusterBuilder::new().build().await;
    let mut context = test_cluster.wallet;

    // Add 3 accounts
    for _ in 0..3 {
        context
            .config_mut()
            .keystore_mut()
            .add_key(None, IotaKeyPair::Ed25519(get_key_pair().1))?;
    }

    // Print all addresses
    IotaClientCommands::Addresses {
        sort_by_alias: true,
    }
    .execute(&mut context)
    .await
    .unwrap()
    .print(true);

    Ok(())
}

#[sim_test]
async fn test_objects_command() -> Result<(), anyhow::Error> {
    let mut test_cluster = TestClusterBuilder::new()
        .with_num_validators(2)
        .build()
        .await;
    let address = test_cluster.get_address_0();
    let context = &mut test_cluster.wallet;
    let alias = context
        .config()
        .keystore()
        .get_alias_by_address(&address)
        .unwrap();
    // Print objects owned by `address`
    IotaClientCommands::Objects {
        address: Some(KeyIdentity::Address(address)),
    }
    .execute(context)
    .await?
    .print(true);
    // Print objects owned by `address`, passing its alias
    IotaClientCommands::Objects {
        address: Some(KeyIdentity::Alias(alias)),
    }
    .execute(context)
    .await?
    .print(true);
    let client = context.get_client().await?;
    let _object_refs = client
        .read_api()
        .get_owned_objects(
            address,
            Some(IotaObjectResponseQuery::new_with_options(
                IotaObjectDataOptions::new()
                    .with_type()
                    .with_owner()
                    .with_previous_transaction(),
            )),
            None,
            None,
        )
        .await?;

    Ok(())
}

#[sim_test]
async fn test_ptb_publish_and_complex_arg_resolution() -> Result<(), anyhow::Error> {
    // Publish the package
    move_package::package_hooks::register_package_hooks(Box::new(IotaPackageHooks));
    let mut test_cluster = TestClusterBuilder::new()
        .with_num_validators(2)
        .build()
        .await;
    let rgp = test_cluster.get_reference_gas_price().await;
    let address = test_cluster.get_address_0();
    let context = &mut test_cluster.wallet;
    let client = context.get_client().await?;
    let object_refs = client
        .read_api()
        .get_owned_objects(
            address,
            Some(IotaObjectResponseQuery::new_with_options(
                IotaObjectDataOptions::new()
                    .with_type()
                    .with_owner()
                    .with_previous_transaction(),
            )),
            None,
            None,
        )
        .await?
        .data;

    // Check log output contains all object ids.
    let gas_obj_id = object_refs.first().unwrap().object().unwrap().object_id;

    // Provide path to well formed package sources
    let mut package_path = PathBuf::from(TEST_DATA_DIR);
    package_path.push("ptb_complex_args_test_functions");
    let build_config = BuildConfig::new_for_testing().config;
    let resp = IotaClientCommands::Publish {
        package_path: package_path.clone(),
        build_config,
        skip_dependency_verification: false,
        verify_deps: true,
        with_unpublished_dependencies: false,
        payment: PaymentArgs {
            gas: vec![gas_obj_id],
        },
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_PUBLISH),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;

    // Print it out to CLI/logs
    resp.print(true);

    let IotaClientCommandResult::TransactionBlock(response) = resp else {
        unreachable!("Invalid response");
    };

    let IotaTransactionBlockEffects::V1(effects) = response.effects.unwrap();

    assert!(effects.status.is_ok());
    assert_eq!(effects.gas_object().object_id(), gas_obj_id);
    let package = effects
        .created()
        .iter()
        .find(|refe| matches!(refe.owner, Owner::Immutable))
        .unwrap();
    let package_id_str = package.reference.object_id.to_string();

    let start_call_result = IotaClientCommands::Call {
        package: package.reference.object_id,
        module: "test_module".to_string(),
        function: "new_shared".to_string(),
        type_args: vec![],
        args: vec![],
        payment: PaymentArgs::default(),
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_GENERIC),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;

    let shared_id_str =
        if let IotaClientCommandResult::TransactionBlock(response) = start_call_result {
            response.effects.unwrap().created().to_vec()[0]
                .reference
                .object_id
                .to_string()
        } else {
            unreachable!("Invalid response");
        };

    let complex_ptb_string = format!(
        r#"
         --assign p @{package_id_str}
         --assign s @{shared_id_str}
         # Use the shared object by immutable reference first
         --move-call "p::test_module::use_immut" s
         # Now use mutably -- we need to update the mutability of the object
         --move-call "p::test_module::use_mut" s
         # Make sure we handle different more complex pure arguments
         --move-call "p::test_module::use_ascii_string" "'foo bar baz'"
         --move-call "p::test_module::use_utf8_string" "'foo †††˚˚¬¬'"
         --gas-budget 100000000
        "#
    );

    let args = shlex::split(&complex_ptb_string).unwrap();
    iota::client_ptb::ptb::PTB {
        args: args.clone(),
        display: HashSet::new(),
    }
    .execute(context)
    .await?;

    let delete_object_ptb_string = format!(
        r#"
         --assign p @{package_id_str}
         --assign s @{shared_id_str}
         # Use the shared object by immutable reference first
         --move-call "p::test_module::use_immut" s
         --move-call "p::test_module::delete_shared_object" s
         --gas-budget 100000000
        "#
    );

    let args = shlex::split(&delete_object_ptb_string).unwrap();
    iota::client_ptb::ptb::PTB {
        args: args.clone(),
        display: HashSet::new(),
    }
    .execute(context)
    .await?;

    Ok(())
}

#[sim_test]
async fn test_ptb_publish() -> Result<(), anyhow::Error> {
    move_package::package_hooks::register_package_hooks(Box::new(IotaPackageHooks));
    let mut test_cluster = TestClusterBuilder::new()
        .with_num_validators(2)
        .build()
        .await;
    let context = &mut test_cluster.wallet;
    let mut package_path = PathBuf::from(TEST_DATA_DIR);
    package_path.push("ptb_complex_args_test_functions");

    let publish_ptb_string = format!(
        r#"
         --move-call iota::tx_context::sender
         --assign sender
         --publish {}
         --assign upgrade_cap
         --transfer-objects "[upgrade_cap]" sender
        "#,
        package_path.display()
    );
    let args = shlex::split(&publish_ptb_string).unwrap();
    iota::client_ptb::ptb::PTB {
        args: args.clone(),
        display: HashSet::new(),
    }
    .execute(context)
    .await?;
    Ok(())
}

#[sim_test]
async fn test_ptb_publish_upgrade() -> Result<(), anyhow::Error> {
    move_package::package_hooks::register_package_hooks(Box::new(IotaPackageHooks));
    let mut test_cluster = TestClusterBuilder::new()
        .with_num_validators(2)
        .build()
        .await;
    let context = &mut test_cluster.wallet;
    let mut package_path = PathBuf::from(TEST_DATA_DIR);
    package_path.push("ptb_complex_args_test_functions");
    let mut package_path_2 = PathBuf::from(TEST_DATA_DIR);
    package_path_2.push("clever_errors");

    let publish_ptb_string = format!(
        r#"
        --move-call iota::tx_context::sender
        --assign sender
        --publish {}
        --assign upgrade_cap
        --publish {}
        --assign upgrade_cap_2
        --transfer-objects "[upgrade_cap, upgrade_cap_2]" sender
        "#,
        package_path.display(),
        package_path_2.display()
    );
    let args = shlex::split(&publish_ptb_string).unwrap();
    let PTBCommandResult::CommandResult(res) = iota::client_ptb::ptb::PTB {
        args: args.clone(),
        display: HashSet::new(),
    }
    .execute(context)
    .await?
    else {
        panic!("unexpected PTB result");
    };
    let IotaClientCommandResult::TransactionBlock(transaction_response) = *res else {
        panic!("unexpected PTB result");
    };

    let object_changes = transaction_response.object_changes.unwrap();

    let upgrade_capabilities: Vec<ObjectID> = object_changes
        .iter()
        .filter_map(|c| {
            if let iota_json_rpc_types::ObjectChange::Created { object_type, .. } = c {
                if object_type == &iota_types::move_package::UpgradeCap::type_() {
                    Some(c.object_id())
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();

    let client = context.get_client().await?;
    let mut packages_with_upgrade_cap = Vec::new();
    for cap_id in upgrade_capabilities {
        let cap_object = client
            .read_api()
            .get_object_with_options(cap_id, IotaObjectDataOptions::default().with_content())
            .await?
            .into_object()
            .unwrap();

        let move_obj = cap_object.content.unwrap();
        if let iota_json_rpc_types::IotaParsedData::MoveObject(parsed) = move_obj {
            let fields_map = match parsed.fields {
                iota_json_rpc_types::IotaMoveStruct::WithFields(f) => f,
                _ => panic!("Unexpected struct type"),
            };
            let package_value = &fields_map["package"];
            let package_addr =
                IotaAddress::from_str(package_value.clone().to_json_value().as_str().unwrap())
                    .unwrap();

            let package_object = client
                .read_api()
                .get_object_with_options(
                    package_addr.into(),
                    IotaObjectDataOptions::default().with_content(),
                )
                .await?
                .into_object()
                .unwrap();

            let is_clever_errors = if let Some(iota_json_rpc_types::IotaParsedData::Package(pkg)) =
                &package_object.content
            {
                pkg.disassembled.contains_key("clever_errors")
            } else {
                false
            };
            let pkg_path = if is_clever_errors {
                package_path_2.clone()
            } else {
                package_path.clone()
            };

            packages_with_upgrade_cap.push((pkg_path, package_addr, cap_id));
        } else {
            panic!("Expected MoveObject");
        }
    }

    // Update lock file for both packages
    for (pkg_path, package_id, _) in &packages_with_upgrade_cap {
        let mut build_config = BuildConfig::new_for_testing().config;
        build_config.lock_file = Some(pkg_path.join("Move.lock"));
        iota_package_management::update_lock_file_with_package_id(
            context,
            iota_package_management::LockCommand::Publish,
            build_config.install_dir,
            build_config.lock_file,
            (*package_id).into(),
            1,
        )
        .await?;
    }

    let publish_ptb_string = format!(
        r#"
        --move-call iota::tx_context::sender
        --assign sender
        --upgrade {} @{}
        --upgrade {} @{}
        "#,
        packages_with_upgrade_cap[0].0.display(),
        packages_with_upgrade_cap[0].2,
        packages_with_upgrade_cap[1].0.display(),
        packages_with_upgrade_cap[1].2,
    );
    let args = shlex::split(&publish_ptb_string).unwrap();
    iota::client_ptb::ptb::PTB {
        args: args.clone(),
        display: HashSet::new(),
    }
    .execute(context)
    .await?;

    Ok(())
}

#[sim_test]
async fn test_custom_genesis() -> Result<(), anyhow::Error> {
    // Create and save genesis config file
    // Create 4 authorities, 1 account with 1 gas object with custom id

    let mut config = GenesisConfig::for_local_testing();
    config.accounts.clear();
    config.accounts.push(AccountConfig {
        address: None,
        gas_amounts: vec![500],
    });
    let mut cluster = TestClusterBuilder::new()
        .set_genesis_config(config)
        .build()
        .await;
    let address = cluster.get_address_0();
    let context = cluster.wallet_mut();

    assert_eq!(1, context.config().keystore().addresses().len());

    // Print objects owned by `address`
    IotaClientCommands::Objects {
        address: Some(KeyIdentity::Address(address)),
    }
    .execute(context)
    .await?
    .print(true);

    Ok(())
}

#[sim_test]
async fn test_object_info_get_command() -> Result<(), anyhow::Error> {
    let mut test_cluster = TestClusterBuilder::new()
        .with_num_validators(2)
        .build()
        .await;

    let address = test_cluster.get_address_0();
    let context = &mut test_cluster.wallet;
    let client = context.get_client().await?;

    let object_refs = client
        .read_api()
        .get_owned_objects(
            address,
            Some(IotaObjectResponseQuery::new_with_options(
                IotaObjectDataOptions::new(),
            )),
            None,
            None,
        )
        .await?
        .data;

    // Check log output contains all object ids.
    let object_id = object_refs.first().unwrap().object().unwrap().object_id;

    IotaClientCommands::Object {
        id: object_id,
        bcs: false,
    }
    .execute(context)
    .await?
    .print(true);

    IotaClientCommands::Object {
        id: object_id,
        bcs: true,
    }
    .execute(context)
    .await?
    .print(true);

    Ok(())
}

#[sim_test]
async fn test_gas_command() -> Result<(), anyhow::Error> {
    let mut test_cluster = TestClusterBuilder::new()
        .with_num_validators(2)
        .build()
        .await;
    let rgp = test_cluster.get_reference_gas_price().await;
    let address = test_cluster.get_address_0();
    let context = &mut test_cluster.wallet;
    let alias = context
        .config()
        .keystore()
        .get_alias_by_address(&address)
        .unwrap();

    let client = context.get_client().await?;
    let object_refs = client
        .read_api()
        .get_owned_objects(
            address,
            Some(IotaObjectResponseQuery::new_with_options(
                IotaObjectDataOptions::full_content(),
            )),
            None,
            None,
        )
        .await?;

    let object_id = object_refs
        .data
        .first()
        .unwrap()
        .object()
        .unwrap()
        .object_id;
    let object_to_send = object_refs.data.get(1).unwrap().object().unwrap().object_id;

    IotaClientCommands::Gas {
        address: Some(KeyIdentity::Address(address)),
    }
    .execute(context)
    .await?
    .print(true);

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Send an object
    IotaClientCommands::Transfer {
        to: KeyIdentity::Address(IotaAddress::random_for_testing_only()),
        object_id: object_to_send,
        payment: PaymentArgs {
            gas: vec![object_id],
        },
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;

    // Fetch gas again, and use the alias instead of the address
    IotaClientCommands::Gas {
        address: Some(KeyIdentity::Alias(alias)),
    }
    .execute(context)
    .await?
    .print(true);

    Ok(())
}

#[sim_test]
async fn test_move_call_args_linter_command() -> Result<(), anyhow::Error> {
    let mut test_cluster = TestClusterBuilder::new()
        .with_num_validators(2)
        .build()
        .await;
    let rgp = test_cluster.get_reference_gas_price().await;
    let address1 = test_cluster.get_address_0();
    let context = &mut test_cluster.wallet;

    let address2 = IotaAddress::random_for_testing_only();

    let client = context.get_client().await?;
    // publish the object basics package
    let object_refs = client
        .read_api()
        .get_owned_objects(
            address1,
            Some(IotaObjectResponseQuery::new_with_options(
                IotaObjectDataOptions::full_content(),
            )),
            None,
            None,
        )
        .await?
        .data;
    let gas_obj_id = object_refs.first().unwrap().object().unwrap().object_id;
    let mut package_path = PathBuf::from(TEST_DATA_DIR);
    package_path.push("move_call_args_linter");
    let build_config = BuildConfig::new_for_testing().config;
    let resp = IotaClientCommands::Publish {
        package_path,
        build_config,
        skip_dependency_verification: false,
        verify_deps: true,
        with_unpublished_dependencies: false,
        payment: PaymentArgs {
            gas: vec![gas_obj_id],
        },
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_PUBLISH),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;

    let package = if let IotaClientCommandResult::TransactionBlock(response) = resp {
        assert!(
            response.status_ok().unwrap(),
            "Command failed: {response:?}"
        );
        assert_eq!(
            response.effects.as_ref().unwrap().gas_object().object_id(),
            gas_obj_id
        );
        response
            .effects
            .unwrap()
            .created()
            .iter()
            .find(
                |OwnedObjectRef {
                     owner,
                     reference: _,
                 }| matches!(owner, Owner::Immutable),
            )
            .unwrap()
            .reference
            .object_id
    } else {
        unreachable!("Invalid response");
    };

    // Print objects owned by `address1`
    IotaClientCommands::Objects {
        address: Some(KeyIdentity::Address(address1)),
    }
    .execute(context)
    .await?
    .print(true);
    tokio::time::sleep(Duration::from_millis(2000)).await;
    let client = context.get_client().await?;
    let object_refs = client
        .read_api()
        .get_owned_objects(
            address1,
            Some(IotaObjectResponseQuery::new_with_options(
                IotaObjectDataOptions::new()
                    .with_type()
                    .with_owner()
                    .with_previous_transaction(),
            )),
            None,
            None,
        )
        .await?
        .data;

    // Create an object for address1 using Move call

    // Certain prep work
    // Get a gas object
    let coins: Vec<_> = object_refs
        .iter()
        .filter(|object_ref| object_ref.object().unwrap().is_gas_coin())
        .collect();
    let gas = coins.first().unwrap().object()?.object_id;
    let obj = coins.get(1).unwrap().object()?.object_id;

    // Create the args
    let args = vec![
        IotaJsonValue::new(json!("123"))?,
        IotaJsonValue::new(json!(address1))?,
    ];

    // Test case with no gas specified
    let resp = IotaClientCommands::Call {
        package,
        module: "object_basics".to_string(),
        function: "create".to_string(),
        type_args: vec![],
        args,
        payment: PaymentArgs::default(),
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_OBJECT_BASICS),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;
    resp.print(true);

    // Get the created object
    let created_obj: ObjectID = if let IotaClientCommandResult::TransactionBlock(resp) = resp {
        resp.effects
            .unwrap()
            .created()
            .first()
            .unwrap()
            .reference
            .object_id
    } else {
        panic!();
    };

    // Try a bad argument: decimal
    let args_json = json!([0.3f32, address1]);
    assert!(IotaJsonValue::new(args_json.as_array().unwrap().first().unwrap().clone()).is_err());

    // Try a bad argument: too few args
    let args_json = json!([300usize]);
    let mut args = vec![];
    for a in args_json.as_array().unwrap() {
        args.push(IotaJsonValue::new(a.clone()).unwrap());
    }

    let resp = IotaClientCommands::Call {
        package,
        module: "object_basics".to_string(),
        function: "create".to_string(),
        type_args: vec![],
        args: args.to_vec(),
        payment: PaymentArgs { gas: vec![gas] },
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_OBJECT_BASICS),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await;

    assert!(resp.is_err());

    let err_string = format!("{} ", resp.err().unwrap());
    assert!(err_string.contains("Expected 2 args, found 1"));

    // Try a transfer
    // This should fail due to mismatch of object being sent
    let args = [
        IotaJsonValue::new(json!(obj))?,
        IotaJsonValue::new(json!(address2))?,
    ];

    let resp = IotaClientCommands::Call {
        package,
        module: "object_basics".to_string(),
        function: "transfer".to_string(),
        type_args: vec![],
        args: args.to_vec(),
        payment: PaymentArgs { gas: vec![gas] },
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_OBJECT_BASICS),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await;

    assert!(resp.is_err());

    // Try a transfer with explicitly set gas price.
    // It should fail due to that gas price is below RGP.
    let args = [
        IotaJsonValue::new(json!(created_obj))?,
        IotaJsonValue::new(json!(address2))?,
    ];

    let resp = IotaClientCommands::Call {
        package,
        module: "object_basics".to_string(),
        function: "transfer".to_string(),
        type_args: vec![],
        args: args.to_vec(),
        payment: PaymentArgs { gas: vec![gas] },
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_OBJECT_BASICS),
            gas_price: Some(1),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await;

    assert!(resp.is_err());
    let err_string = format!("{} ", resp.err().unwrap());
    assert!(err_string.contains("Gas price 1 under reference gas price"));

    // FIXME: uncomment once we figure out what is going on with
    // `resolve_and_type_check` let err_string = format!("{} ",
    // resp.err().unwrap()); let framework_addr =
    // IOTA_FRAMEWORK_ADDRESS.to_hex_literal(); let package_addr =
    // package.to_hex_literal(); assert!(err_string.contains(&format!("Expected
    // argument of type {package_addr}::object_basics::Object, but found type
    // {framework_addr}::coin::Coin<{framework_addr}::iota::IOTA>")));

    // Try a proper transfer
    let args = [
        IotaJsonValue::new(json!(created_obj))?,
        IotaJsonValue::new(json!(address2))?,
    ];

    IotaClientCommands::Call {
        package,
        module: "object_basics".to_string(),
        function: "transfer".to_string(),
        type_args: vec![],
        args: args.to_vec(),
        payment: PaymentArgs { gas: vec![gas] },
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_OBJECT_BASICS),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;

    // Try a call with customized gas price.
    let args = vec![
        IotaJsonValue::new(json!("123"))?,
        IotaJsonValue::new(json!(address1))?,
    ];

    let result = IotaClientCommands::Call {
        package,
        module: "object_basics".to_string(),
        function: "create".to_string(),
        type_args: vec![],
        args,
        payment: PaymentArgs::default(),
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_OBJECT_BASICS),
            gas_price: Some(12345),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;

    if let IotaClientCommandResult::TransactionBlock(txn_response) = result {
        assert_eq!(
            txn_response.transaction.unwrap().data.gas_data().price,
            12345
        );
    } else {
        panic!("Command failed with unexpected result.")
    };

    Ok(())
}

#[sim_test]
async fn test_package_publish_command() -> Result<(), anyhow::Error> {
    let mut test_cluster = TestClusterBuilder::new()
        .with_num_validators(2)
        .build()
        .await;
    let rgp = test_cluster.get_reference_gas_price().await;
    let address = test_cluster.get_address_0();
    let context = &mut test_cluster.wallet;

    let client = context.get_client().await?;
    let object_refs = client
        .read_api()
        .get_owned_objects(
            address,
            Some(IotaObjectResponseQuery::new_with_options(
                IotaObjectDataOptions::new()
                    .with_type()
                    .with_owner()
                    .with_previous_transaction(),
            )),
            None,
            None,
        )
        .await?
        .data;

    // Check log output contains all object ids.
    let gas_obj_id = object_refs.first().unwrap().object().unwrap().object_id;

    // Provide path to well formed package sources
    let mut package_path = PathBuf::from(TEST_DATA_DIR);
    package_path.push("dummy_modules_publish");
    let build_config = BuildConfig::new_for_testing().config;
    let resp = IotaClientCommands::Publish {
        package_path,
        build_config,
        skip_dependency_verification: false,
        verify_deps: true,
        with_unpublished_dependencies: false,
        payment: PaymentArgs {
            gas: vec![gas_obj_id],
        },
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_PUBLISH),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;

    // Print it out to CLI/logs
    resp.print(true);

    let obj_ids = if let IotaClientCommandResult::TransactionBlock(response) = resp {
        assert_eq!(
            response.effects.as_ref().unwrap().gas_object().object_id(),
            gas_obj_id
        );
        response
            .effects
            .as_ref()
            .unwrap()
            .created()
            .iter()
            .map(|refe| refe.reference.object_id)
            .collect::<Vec<_>>()
    } else {
        unreachable!("Invalid response");
    };

    // Check the objects
    for obj_id in obj_ids {
        get_parsed_object_assert_existence(obj_id, context).await;
    }

    Ok(())
}

#[sim_test]
async fn test_package_management_on_publish_command() -> Result<(), anyhow::Error> {
    let mut test_cluster = TestClusterBuilder::new()
        .with_num_validators(2)
        .build()
        .await;
    let rgp = test_cluster.get_reference_gas_price().await;
    let address = test_cluster.get_address_0();
    let context = &mut test_cluster.wallet;

    let client = context.get_client().await?;
    let object_refs = client
        .read_api()
        .get_owned_objects(
            address,
            Some(IotaObjectResponseQuery::new_with_options(
                IotaObjectDataOptions::new()
                    .with_type()
                    .with_owner()
                    .with_previous_transaction(),
            )),
            None,
            None,
        )
        .await?
        .data;

    // Check log output contains all object ids.
    let gas_obj_id = object_refs.first().unwrap().object().unwrap().object_id;

    // Provide path to well formed package sources
    let mut package_path = PathBuf::from(TEST_DATA_DIR);
    package_path.push("dummy_modules_publish");
    let build_config = BuildConfig::new_for_testing().config;
    // Publish the package
    let resp = IotaClientCommands::Publish {
        package_path,
        build_config: build_config.clone(),
        skip_dependency_verification: false,
        verify_deps: true,
        with_unpublished_dependencies: false,
        payment: PaymentArgs {
            gas: vec![gas_obj_id],
        },
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_PUBLISH),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;

    // Get Package ID and version
    let (expect_original_id, expect_version, _) =
        if let IotaClientCommandResult::TransactionBlock(response) = resp {
            assert_eq!(
                response.effects.as_ref().unwrap().gas_object().object_id(),
                gas_obj_id
            );
            get_new_package_obj_from_response(&response)
                .ok_or_else(|| anyhow::anyhow!("No package object response"))?
        } else {
            unreachable!("Invalid response");
        };

    // Get lock file that recorded Package ID and version
    let lock_file = build_config.lock_file.expect("Lock file for testing");
    let mut lock_file = std::fs::File::open(lock_file).unwrap();
    let envs = ManagedPackage::read(&mut lock_file).unwrap();
    let localnet = envs.get("localnet").unwrap();
    assert_eq!(
        expect_original_id.to_string(),
        localnet.original_published_id,
    );
    assert_eq!(expect_original_id.to_string(), localnet.latest_published_id);
    assert_eq!(
        expect_version.value(),
        localnet.version.parse::<u64>().unwrap(),
    );
    Ok(())
}

#[sim_test]
async fn test_delete_shared_object() -> Result<(), anyhow::Error> {
    let mut test_cluster = TestClusterBuilder::new()
        .with_num_validators(2)
        .build()
        .await;
    let rgp = test_cluster.get_reference_gas_price().await;
    let address = test_cluster.get_address_0();
    let context = &mut test_cluster.wallet;

    let client = context.get_client().await?;
    let object_refs = client
        .read_api()
        .get_owned_objects(
            address,
            Some(IotaObjectResponseQuery::new_with_options(
                IotaObjectDataOptions::new()
                    .with_type()
                    .with_owner()
                    .with_previous_transaction(),
            )),
            None,
            None,
        )
        .await?
        .data;

    let gas_obj_id = object_refs.first().unwrap().object().unwrap().object_id;

    // Provide path to well formed package sources
    let mut package_path = PathBuf::from(TEST_DATA_DIR);
    package_path.push("sod");
    let build_config = BuildConfig::new_for_testing().config;
    let resp = IotaClientCommands::Publish {
        package_path,
        build_config,
        skip_dependency_verification: false,
        verify_deps: true,
        with_unpublished_dependencies: false,
        payment: PaymentArgs {
            gas: vec![gas_obj_id],
        },
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_PUBLISH),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;

    let owned_obj_ids = if let IotaClientCommandResult::TransactionBlock(response) = resp {
        assert_eq!(
            response.effects.as_ref().unwrap().gas_object().object_id(),
            gas_obj_id
        );
        let x = response.effects.unwrap();
        x.created().to_vec()
    } else {
        unreachable!("Invalid response");
    };

    // Check the objects
    for OwnedObjectRef { reference, .. } in &owned_obj_ids {
        get_parsed_object_assert_existence(reference.object_id, context).await;
    }

    let package_id = owned_obj_ids
        .into_iter()
        .find(|OwnedObjectRef { owner, .. }| owner == &Owner::Immutable)
        .expect("Must find published package ID")
        .reference;

    // Start and then receive the object
    let start_call_result = IotaClientCommands::Call {
        package: (*package_id.object_id).into(),
        module: "sod".to_string(),
        function: "start".to_string(),
        type_args: vec![],
        args: vec![],
        payment: PaymentArgs::default(),
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_PUBLISH),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;

    let shared_id = if let IotaClientCommandResult::TransactionBlock(response) = start_call_result {
        response.effects.unwrap().created().to_vec()[0]
            .reference
            .object_id
    } else {
        unreachable!("Invalid response");
    };

    let delete_result = IotaClientCommands::Call {
        package: (*package_id.object_id).into(),
        module: "sod".to_string(),
        function: "delete".to_string(),
        type_args: vec![],
        args: vec![IotaJsonValue::from_str(&shared_id.to_string()).unwrap()],
        payment: PaymentArgs::default(),
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_PUBLISH),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;

    if let IotaClientCommandResult::TransactionBlock(response) = delete_result {
        assert!(response.effects.unwrap().into_status().is_ok());
    } else {
        unreachable!("Invalid response");
    };

    Ok(())
}

#[sim_test]
async fn test_receive_argument() -> Result<(), anyhow::Error> {
    let mut test_cluster = TestClusterBuilder::new()
        .with_num_validators(2)
        .build()
        .await;
    let rgp = test_cluster.get_reference_gas_price().await;
    let address = test_cluster.get_address_0();
    let context = &mut test_cluster.wallet;

    let client = context.get_client().await?;
    let object_refs = client
        .read_api()
        .get_owned_objects(
            address,
            Some(IotaObjectResponseQuery::new_with_options(
                IotaObjectDataOptions::new()
                    .with_type()
                    .with_owner()
                    .with_previous_transaction(),
            )),
            None,
            None,
        )
        .await?
        .data;

    let gas_obj_id = object_refs.first().unwrap().object().unwrap().object_id;

    // Provide path to well formed package sources
    let mut package_path = PathBuf::from(TEST_DATA_DIR);
    package_path.push("tto");
    let build_config = BuildConfig::new_for_testing().config;
    let resp = IotaClientCommands::Publish {
        package_path,
        build_config,
        skip_dependency_verification: false,
        verify_deps: true,
        with_unpublished_dependencies: false,
        payment: PaymentArgs {
            gas: vec![gas_obj_id],
        },
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_PUBLISH),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;

    let owned_obj_ids = if let IotaClientCommandResult::TransactionBlock(response) = resp {
        assert_eq!(
            response.effects.as_ref().unwrap().gas_object().object_id(),
            gas_obj_id
        );
        let x = response.effects.unwrap();
        x.created().to_vec()
    } else {
        unreachable!("Invalid response");
    };

    // Check the objects
    for OwnedObjectRef { reference, .. } in &owned_obj_ids {
        get_parsed_object_assert_existence(reference.object_id, context).await;
    }

    let package_id = owned_obj_ids
        .into_iter()
        .find(|OwnedObjectRef { owner, .. }| owner == &Owner::Immutable)
        .expect("Must find published package ID")
        .reference;

    // Start and then receive the object
    let start_call_result = IotaClientCommands::Call {
        package: (*package_id.object_id).into(),
        module: "tto".to_string(),
        function: "start".to_string(),
        type_args: vec![],
        args: vec![],
        payment: PaymentArgs::default(),
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_PUBLISH),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;

    let (parent, child) =
        if let IotaClientCommandResult::TransactionBlock(response) = start_call_result {
            let created = response.effects.unwrap().created().to_vec();
            let owners: BTreeSet<ObjectID> = created
                .iter()
                .flat_map(|refe| {
                    refe.owner
                        .get_address_owner_address()
                        .ok()
                        .map(|x| x.into())
                })
                .collect();
            let child = created
                .iter()
                .find(|refe| !owners.contains(&refe.reference.object_id))
                .unwrap();
            let parent = created
                .iter()
                .find(|refe| owners.contains(&refe.reference.object_id))
                .unwrap();
            (parent.reference.clone(), child.reference.clone())
        } else {
            unreachable!("Invalid response");
        };

    let receive_result = IotaClientCommands::Call {
        package: (*package_id.object_id).into(),
        module: "tto".to_string(),
        function: "receiver".to_string(),
        type_args: vec![],
        args: vec![
            IotaJsonValue::from_str(&parent.object_id.to_string()).unwrap(),
            IotaJsonValue::from_str(&child.object_id.to_string()).unwrap(),
        ],
        payment: PaymentArgs::default(),
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_PUBLISH),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;

    if let IotaClientCommandResult::TransactionBlock(response) = receive_result {
        assert!(response.effects.unwrap().into_status().is_ok());
    } else {
        unreachable!("Invalid response");
    };

    Ok(())
}

#[sim_test]
async fn test_receive_argument_by_immut_ref() -> Result<(), anyhow::Error> {
    let mut test_cluster = TestClusterBuilder::new()
        .with_num_validators(2)
        .build()
        .await;
    let rgp = test_cluster.get_reference_gas_price().await;
    let address = test_cluster.get_address_0();
    let context = &mut test_cluster.wallet;

    let client = context.get_client().await?;
    let object_refs = client
        .read_api()
        .get_owned_objects(
            address,
            Some(IotaObjectResponseQuery::new_with_options(
                IotaObjectDataOptions::new()
                    .with_type()
                    .with_owner()
                    .with_previous_transaction(),
            )),
            None,
            None,
        )
        .await?
        .data;

    let gas_obj_id = object_refs.first().unwrap().object().unwrap().object_id;

    // Provide path to well formed package sources
    let mut package_path = PathBuf::from(TEST_DATA_DIR);
    package_path.push("tto");
    let build_config = BuildConfig::new_for_testing().config;
    let resp = IotaClientCommands::Publish {
        package_path,
        build_config,
        skip_dependency_verification: false,
        verify_deps: true,
        with_unpublished_dependencies: false,
        payment: PaymentArgs {
            gas: vec![gas_obj_id],
        },
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_PUBLISH),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;

    let owned_obj_ids = if let IotaClientCommandResult::TransactionBlock(response) = resp {
        assert_eq!(
            response.effects.as_ref().unwrap().gas_object().object_id(),
            gas_obj_id
        );
        let x = response.effects.unwrap();
        x.created().to_vec()
    } else {
        unreachable!("Invalid response");
    };

    // Check the objects
    for OwnedObjectRef { reference, .. } in &owned_obj_ids {
        get_parsed_object_assert_existence(reference.object_id, context).await;
    }

    let package_id = owned_obj_ids
        .into_iter()
        .find(|OwnedObjectRef { owner, .. }| owner == &Owner::Immutable)
        .expect("Must find published package ID")
        .reference;

    // Start and then receive the object
    let start_call_result = IotaClientCommands::Call {
        package: (*package_id.object_id).into(),
        module: "tto".to_string(),
        function: "start".to_string(),
        type_args: vec![],
        args: vec![],
        payment: PaymentArgs::default(),
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_PUBLISH),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;

    let (parent, child) =
        if let IotaClientCommandResult::TransactionBlock(response) = start_call_result {
            let created = response.effects.unwrap().created().to_vec();
            let owners: BTreeSet<ObjectID> = created
                .iter()
                .flat_map(|refe| {
                    refe.owner
                        .get_address_owner_address()
                        .ok()
                        .map(|x| x.into())
                })
                .collect();
            let child = created
                .iter()
                .find(|refe| !owners.contains(&refe.reference.object_id))
                .unwrap();
            let parent = created
                .iter()
                .find(|refe| owners.contains(&refe.reference.object_id))
                .unwrap();
            (parent.reference.clone(), child.reference.clone())
        } else {
            unreachable!("Invalid response");
        };

    let receive_result = IotaClientCommands::Call {
        package: (*package_id.object_id).into(),
        module: "tto".to_string(),
        function: "invalid_call_immut_ref".to_string(),
        type_args: vec![],
        args: vec![
            IotaJsonValue::from_str(&parent.object_id.to_string()).unwrap(),
            IotaJsonValue::from_str(&child.object_id.to_string()).unwrap(),
        ],
        payment: PaymentArgs::default(),
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_PUBLISH),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;

    if let IotaClientCommandResult::TransactionBlock(response) = receive_result {
        assert!(response.effects.unwrap().into_status().is_ok());
    } else {
        unreachable!("Invalid response");
    };

    Ok(())
}

#[sim_test]
async fn test_receive_argument_by_mut_ref() -> Result<(), anyhow::Error> {
    let mut test_cluster = TestClusterBuilder::new()
        .with_num_validators(2)
        .build()
        .await;
    let rgp = test_cluster.get_reference_gas_price().await;
    let address = test_cluster.get_address_0();
    let context = &mut test_cluster.wallet;

    let client = context.get_client().await?;
    let object_refs = client
        .read_api()
        .get_owned_objects(
            address,
            Some(IotaObjectResponseQuery::new_with_options(
                IotaObjectDataOptions::new()
                    .with_type()
                    .with_owner()
                    .with_previous_transaction(),
            )),
            None,
            None,
        )
        .await?
        .data;

    let gas_obj_id = object_refs.first().unwrap().object().unwrap().object_id;

    // Provide path to well formed package sources
    let mut package_path = PathBuf::from(TEST_DATA_DIR);
    package_path.push("tto");
    let build_config = BuildConfig::new_for_testing().config;
    let resp = IotaClientCommands::Publish {
        package_path,
        build_config,
        skip_dependency_verification: false,
        with_unpublished_dependencies: false,
        verify_deps: true,
        payment: PaymentArgs {
            gas: vec![gas_obj_id],
        },
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_PUBLISH),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;

    let owned_obj_ids = if let IotaClientCommandResult::TransactionBlock(response) = resp {
        assert_eq!(
            response.effects.as_ref().unwrap().gas_object().object_id(),
            gas_obj_id
        );
        let x = response.effects.unwrap();
        x.created().to_vec()
    } else {
        unreachable!("Invalid response");
    };

    // Check the objects
    for OwnedObjectRef { reference, .. } in &owned_obj_ids {
        get_parsed_object_assert_existence(reference.object_id, context).await;
    }

    let package_id = owned_obj_ids
        .into_iter()
        .find(|OwnedObjectRef { owner, .. }| owner == &Owner::Immutable)
        .expect("Must find published package ID")
        .reference;

    // Start and then receive the object
    let start_call_result = IotaClientCommands::Call {
        package: (*package_id.object_id).into(),
        module: "tto".to_string(),
        function: "start".to_string(),
        type_args: vec![],
        args: vec![],
        payment: PaymentArgs::default(),
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_PUBLISH),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;

    let (parent, child) =
        if let IotaClientCommandResult::TransactionBlock(response) = start_call_result {
            let created = response.effects.unwrap().created().to_vec();
            let owners: BTreeSet<ObjectID> = created
                .iter()
                .flat_map(|refe| {
                    refe.owner
                        .get_address_owner_address()
                        .ok()
                        .map(|x| x.into())
                })
                .collect();
            let child = created
                .iter()
                .find(|refe| !owners.contains(&refe.reference.object_id))
                .unwrap();
            let parent = created
                .iter()
                .find(|refe| owners.contains(&refe.reference.object_id))
                .unwrap();
            (parent.reference.clone(), child.reference.clone())
        } else {
            unreachable!("Invalid response");
        };

    let receive_result = IotaClientCommands::Call {
        package: (*package_id.object_id).into(),
        module: "tto".to_string(),
        function: "invalid_call_mut_ref".to_string(),
        type_args: vec![],
        args: vec![
            IotaJsonValue::from_str(&parent.object_id.to_string()).unwrap(),
            IotaJsonValue::from_str(&child.object_id.to_string()).unwrap(),
        ],
        payment: PaymentArgs::default(),
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_PUBLISH),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;

    if let IotaClientCommandResult::TransactionBlock(response) = receive_result {
        assert!(response.effects.unwrap().into_status().is_ok());
    } else {
        unreachable!("Invalid response");
    };

    Ok(())
}

#[sim_test]
async fn test_package_publish_command_with_unpublished_dependency_succeeds()
-> Result<(), anyhow::Error> {
    let with_unpublished_dependencies = true; // Value under test, results in successful response.

    let mut test_cluster = TestClusterBuilder::new()
        .with_num_validators(2)
        .build()
        .await;
    let address = test_cluster.get_address_0();
    let context = &mut test_cluster.wallet;

    let client = context.get_client().await?;
    let object_refs = client
        .read_api()
        .get_owned_objects(
            address,
            Some(IotaObjectResponseQuery::new_with_options(
                IotaObjectDataOptions::new()
                    .with_type()
                    .with_owner()
                    .with_previous_transaction(),
            )),
            None,
            None,
        )
        .await?
        .data;

    let gas_obj_id = object_refs.first().unwrap().object()?.object_id;

    let mut package_path = PathBuf::from(TEST_DATA_DIR);
    package_path.push("module_publish_with_unpublished_dependency");
    let build_config = BuildConfig::new_for_testing().config;
    let resp = IotaClientCommands::Publish {
        package_path,
        build_config,
        skip_dependency_verification: false,
        verify_deps: false,
        with_unpublished_dependencies,
        payment: PaymentArgs::default(),
        gas_data: GasDataArgs::default(),
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;

    // Print it out to CLI/logs
    resp.print(true);

    let obj_ids = if let IotaClientCommandResult::TransactionBlock(response) = resp {
        assert_eq!(
            response.effects.as_ref().unwrap().gas_object().object_id(),
            gas_obj_id
        );
        response
            .effects
            .as_ref()
            .unwrap()
            .created()
            .iter()
            .map(|refe| refe.reference.object_id)
            .collect::<Vec<_>>()
    } else {
        unreachable!("Invalid response");
    };

    // Check the objects
    for obj_id in obj_ids {
        get_parsed_object_assert_existence(obj_id, context).await;
    }

    Ok(())
}

#[sim_test]
async fn test_package_publish_command_with_unpublished_dependency_fails()
-> Result<(), anyhow::Error> {
    let with_unpublished_dependencies = false; // Value under test, results in error response.

    let mut test_cluster = TestClusterBuilder::new()
        .with_num_validators(2)
        .build()
        .await;
    let rgp = test_cluster.get_reference_gas_price().await;
    let address = test_cluster.get_address_0();
    let context = &mut test_cluster.wallet;
    let client = context.get_client().await?;
    let object_refs = client
        .read_api()
        .get_owned_objects(
            address,
            Some(IotaObjectResponseQuery::new_with_options(
                IotaObjectDataOptions::new()
                    .with_type()
                    .with_owner()
                    .with_previous_transaction(),
            )),
            None,
            None,
        )
        .await?
        .data;

    let gas_obj_id = object_refs.first().unwrap().object().unwrap().object_id;

    let mut package_path = PathBuf::from(TEST_DATA_DIR);
    package_path.push("module_publish_with_unpublished_dependency");
    let build_config = BuildConfig::new_for_testing().config;
    let result = IotaClientCommands::Publish {
        package_path,
        build_config,
        skip_dependency_verification: false,
        verify_deps: true,
        with_unpublished_dependencies,
        payment: PaymentArgs {
            gas: vec![gas_obj_id],
        },
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_PUBLISH),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await;

    let expect = expect![[r#"
        Err(
            ModulePublishFailure {
                error: "Package dependency \"Unpublished\" does not specify a published address (the Move.toml manifest for \"Unpublished\" does not contain a 'published-at' field, nor is there a 'published-id' in the Move.lock). You can use `iota move manage-package` to record the on-chain address for \"Unpublished\".\nIf this is intentional, you may use the --with-unpublished-dependencies flag to continue publishing these dependencies as part of your package (they won't be linked against existing packages on-chain).",
            },
        )
    "#]];
    expect.assert_debug_eq(&result);
    Ok(())
}

#[sim_test]
async fn test_package_publish_command_non_zero_unpublished_dep_fails() -> Result<(), anyhow::Error>
{
    let with_unpublished_dependencies = true; // Value under test, incompatible with dependencies that specify non-zero
    // address.

    let mut test_cluster = TestClusterBuilder::new()
        .with_num_validators(2)
        .build()
        .await;
    let rgp = test_cluster.get_reference_gas_price().await;
    let address = test_cluster.get_address_0();
    let context = &mut test_cluster.wallet;

    let client = context.get_client().await?;
    let object_refs = client
        .read_api()
        .get_owned_objects(address, None, None, None)
        .await?
        .data;

    let gas_obj_id = object_refs.first().unwrap().object().unwrap().object_id;

    let mut package_path = PathBuf::from(TEST_DATA_DIR);
    package_path.push("module_publish_with_unpublished_dependency_with_non_zero_address");
    let build_config = BuildConfig::new_for_testing().config;
    let result = IotaClientCommands::Publish {
        package_path,
        build_config,
        skip_dependency_verification: false,
        verify_deps: true,
        with_unpublished_dependencies,
        payment: PaymentArgs {
            gas: vec![gas_obj_id],
        },
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_PUBLISH),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await;

    let expect = expect![[r#"
        Err(
            ModulePublishFailure {
                error: "The following modules in package dependencies set a non-zero self-address:\n - 0000000000000000000000000000000000000000000000000000000000000bad::non_zero in dependency UnpublishedNonZeroAddress\nIf these packages really are unpublished, their self-addresses should be set to \"0x0\" in the [addresses] section of the manifest when publishing. If they are already published, ensure they specify the address in the `published-at` of their Move.toml manifest.",
            },
        )
    "#]];
    expect.assert_debug_eq(&result);
    Ok(())
}

#[sim_test]
async fn test_package_publish_command_failure_invalid() -> Result<(), anyhow::Error> {
    let with_unpublished_dependencies = true; // Invalid packages should fail to publish, even if we allow unpublished
    // dependencies.

    let mut test_cluster = TestClusterBuilder::new()
        .with_num_validators(2)
        .build()
        .await;
    let rgp = test_cluster.get_reference_gas_price().await;
    let address = test_cluster.get_address_0();
    let context = &mut test_cluster.wallet;

    let client = context.get_client().await?;
    let object_refs = client
        .read_api()
        .get_owned_objects(
            address,
            Some(IotaObjectResponseQuery::new_with_options(
                IotaObjectDataOptions::new()
                    .with_type()
                    .with_owner()
                    .with_previous_transaction(),
            )),
            None,
            None,
        )
        .await?
        .data;

    let gas_obj_id = object_refs.first().unwrap().object().unwrap().object_id;

    let mut package_path = PathBuf::from(TEST_DATA_DIR);
    package_path.push("module_publish_failure_invalid");
    let build_config = BuildConfig::new_for_testing().config;
    let result = IotaClientCommands::Publish {
        package_path,
        build_config,
        skip_dependency_verification: false,
        verify_deps: true,
        with_unpublished_dependencies,
        payment: PaymentArgs {
            gas: vec![gas_obj_id],
        },
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_PUBLISH),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await;

    let expect = expect![[r#"
        Err(
            ModulePublishFailure {
                error: "Package dependency \"Invalid\" does not specify a valid published address: could not parse value \"mystery\" for 'published-at' field in Move.toml or 'published-id' in Move.lock file.",
            },
        )
    "#]];
    expect.assert_debug_eq(&result);
    Ok(())
}

#[sim_test]
async fn test_package_publish_nonexistent_dependency() -> Result<(), anyhow::Error> {
    let mut test_cluster = TestClusterBuilder::new()
        .with_num_validators(2)
        .build()
        .await;
    let rgp = test_cluster.get_reference_gas_price().await;
    let address = test_cluster.get_address_0();
    let context = &mut test_cluster.wallet;
    let client = context.get_client().await?;
    let object_refs = client
        .read_api()
        .get_owned_objects(address, None, None, None)
        .await?
        .data;

    let gas_obj_id = object_refs.first().unwrap().object().unwrap().object_id;

    let mut package_path = PathBuf::from(TEST_DATA_DIR);
    package_path.push("module_publish_with_nonexistent_dependency");
    let build_config = BuildConfig::new_for_testing().config;
    let result = IotaClientCommands::Publish {
        package_path,
        build_config,
        skip_dependency_verification: false,
        verify_deps: true,
        with_unpublished_dependencies: false,
        payment: PaymentArgs {
            gas: vec![gas_obj_id],
        },
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_PUBLISH),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await;

    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("Failed to fetch package Nonexistent"),
        "{}",
        err
    );
    Ok(())
}

#[sim_test]
async fn test_package_publish_test_flag() -> Result<(), anyhow::Error> {
    let mut test_cluster = TestClusterBuilder::new()
        .with_num_validators(2)
        .build()
        .await;
    let rgp = test_cluster.get_reference_gas_price().await;
    let address = test_cluster.get_address_0();
    let context = &mut test_cluster.wallet;
    let client = context.get_client().await?;
    let object_refs = client
        .read_api()
        .get_owned_objects(address, None, None, None)
        .await?
        .data;

    let gas_obj_id = object_refs.first().unwrap().object().unwrap().object_id;

    let mut package_path = PathBuf::from(TEST_DATA_DIR);
    package_path.push("module_publish_with_nonexistent_dependency");
    let mut build_config: MoveBuildConfig = BuildConfig::new_for_testing().config;
    // this would have been the result of calling `iota client publish --test`
    build_config.test_mode = true;

    let result = IotaClientCommands::Publish {
        package_path,
        build_config,
        skip_dependency_verification: false,
        verify_deps: true,
        with_unpublished_dependencies: false,
        payment: PaymentArgs {
            gas: vec![gas_obj_id],
        },
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_PUBLISH),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await;

    let expect = expect![[r#"
        Err(
            ModulePublishFailure {
                error: "The `publish` subcommand should not be used with the `--test` flag\n\nCode in published packages must not depend on test code.\nIn order to fix this and publish the package without `--test`, remove any non-test dependencies on test-only code.\nYou can ensure all test-only dependencies have been removed by compiling the package normally with `iota move build`.",
            },
        )
    "#]];
    expect.assert_debug_eq(&result);
    Ok(())
}

#[sim_test]
async fn test_package_publish_empty() -> Result<(), anyhow::Error> {
    let mut test_cluster = TestClusterBuilder::new().build().await;
    let rgp = test_cluster.get_reference_gas_price().await;
    let address = test_cluster.get_address_0();
    let context = &mut test_cluster.wallet;

    let client = context.get_client().await?;
    let object_refs = client
        .read_api()
        .get_owned_objects(
            address,
            Some(IotaObjectResponseQuery::new_with_options(
                IotaObjectDataOptions::new()
                    .with_type()
                    .with_owner()
                    .with_previous_transaction(),
            )),
            None,
            None,
        )
        .await?
        .data;

    // Check log output contains all object ids.
    let gas_obj_id = object_refs.first().unwrap().object().unwrap().object_id;

    // Provide path to well formed package sources
    let mut package_path = PathBuf::from(TEST_DATA_DIR);
    package_path.push("empty");
    let build_config = BuildConfig::new_for_testing().config;
    let result = IotaClientCommands::Publish {
        package_path,
        build_config,
        skip_dependency_verification: false,
        verify_deps: true,
        with_unpublished_dependencies: false,
        payment: PaymentArgs {
            gas: vec![gas_obj_id],
        },
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_PUBLISH),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await;

    // should return error
    let expect = expect![[r#"
        Err(
            ModulePublishFailure {
                error: "No modules found in the package",
            },
        )
    "#]];

    expect.assert_debug_eq(&result);
    Ok(())
}

#[sim_test]
async fn test_package_upgrade_command() -> Result<(), anyhow::Error> {
    move_package::package_hooks::register_package_hooks(Box::new(IotaPackageHooks));
    let mut test_cluster = TestClusterBuilder::new()
        .with_num_validators(2)
        .build()
        .await;
    let rgp = test_cluster.get_reference_gas_price().await;
    let address = test_cluster.get_address_0();
    let context = &mut test_cluster.wallet;
    let client = context.get_client().await?;
    let object_refs = client
        .read_api()
        .get_owned_objects(
            address,
            Some(IotaObjectResponseQuery::new_with_options(
                IotaObjectDataOptions::new()
                    .with_type()
                    .with_owner()
                    .with_previous_transaction(),
            )),
            None,
            None,
        )
        .await?
        .data;

    // Check log output contains all object ids.
    let gas_obj_id = object_refs.first().unwrap().object().unwrap().object_id;

    // Provide path to well formed package sources
    let mut package_path = PathBuf::from(TEST_DATA_DIR);
    package_path.push("dummy_modules_upgrade");
    let build_config = BuildConfig::new_for_testing().config;
    let resp = IotaClientCommands::Publish {
        package_path: package_path.clone(),
        build_config,
        skip_dependency_verification: false,
        verify_deps: true,
        with_unpublished_dependencies: false,
        payment: PaymentArgs {
            gas: vec![gas_obj_id],
        },
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_PUBLISH),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;

    // Print it out to CLI/logs
    resp.print(true);

    let IotaClientCommandResult::TransactionBlock(response) = resp else {
        unreachable!("Invalid response");
    };

    let IotaTransactionBlockEffects::V1(effects) = response.effects.unwrap();

    assert!(effects.status.is_ok());
    assert_eq!(effects.gas_object().object_id(), gas_obj_id);
    let package = effects
        .created()
        .iter()
        .find(|refe| matches!(refe.owner, Owner::Immutable))
        .unwrap();

    let cap = effects
        .created()
        .iter()
        .find(|refe| matches!(refe.owner, Owner::AddressOwner(_)))
        .unwrap();

    // Hacky for now: we need to add the correct `published-at` field to the Move
    // toml file. In the future once we have automated address management
    // replace this logic!
    let tmp_dir = tempfile::tempdir().unwrap();
    fs_extra::dir::copy(
        &package_path,
        tmp_dir.path(),
        &fs_extra::dir::CopyOptions::default(),
    )
    .unwrap();
    let mut upgrade_pkg_path = tmp_dir.path().to_path_buf();
    upgrade_pkg_path.extend(["dummy_modules_upgrade", "Move.toml"]);
    let mut move_toml = std::fs::File::options()
        .read(true)
        .write(true)
        .open(&upgrade_pkg_path)
        .unwrap();
    upgrade_pkg_path.pop();

    let mut buf = String::new();
    move_toml.read_to_string(&mut buf).unwrap();

    // Add a `published-at = "0x<package_object_id>"` to the Move manifest.
    let mut lines: Vec<String> = buf.split('\n').map(|x| x.to_string()).collect();
    let idx = lines.iter().position(|s| s == "[package]").unwrap();
    lines.insert(
        idx + 1,
        format!(
            "published-at = \"{}\"",
            package.reference.object_id.to_hex_uncompressed()
        ),
    );
    let new = lines.join("\n");

    move_toml.seek(SeekFrom::Start(0))?;
    move_toml.set_len(0)?; // Truncate the file
    move_toml.write_all(new.as_bytes())?;

    // Now run the upgrade
    let build_config = BuildConfig::new_for_testing().config;
    let resp = IotaClientCommands::Upgrade {
        package_path: upgrade_pkg_path,
        upgrade_capability: cap.reference.object_id,
        build_config,
        verify_compatibility: true,
        skip_dependency_verification: false,
        verify_deps: true,
        with_unpublished_dependencies: false,
        payment: PaymentArgs {
            gas: vec![gas_obj_id],
        },
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_PUBLISH),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;

    resp.print(true);

    let IotaClientCommandResult::TransactionBlock(response) = resp else {
        unreachable!("Invalid response");
    };
    let IotaTransactionBlockEffects::V1(effects) = response.effects.unwrap();

    assert!(effects.status.is_ok());
    assert_eq!(effects.gas_object().object_id(), gas_obj_id);

    let obj_ids = effects
        .created()
        .iter()
        .map(|refe| refe.reference.object_id)
        .collect::<Vec<_>>();

    // Check the objects
    for obj_id in obj_ids {
        get_parsed_object_assert_existence(obj_id, context).await;
    }

    Ok(())
}

#[sim_test]
async fn test_package_management_on_upgrade_command() -> Result<(), anyhow::Error> {
    move_package::package_hooks::register_package_hooks(Box::new(IotaPackageHooks));
    let mut test_cluster = TestClusterBuilder::new()
        .with_num_validators(2)
        .build()
        .await;
    let rgp = test_cluster.get_reference_gas_price().await;
    let address = test_cluster.get_address_0();
    let context = &mut test_cluster.wallet;
    let client = context.get_client().await?;
    let object_refs = client
        .read_api()
        .get_owned_objects(
            address,
            Some(IotaObjectResponseQuery::new_with_options(
                IotaObjectDataOptions::new()
                    .with_type()
                    .with_owner()
                    .with_previous_transaction(),
            )),
            None,
            None,
        )
        .await?
        .data;

    // Check log output contains all object ids.
    let gas_obj_id = object_refs.first().unwrap().object().unwrap().object_id;

    // Provide path to well formed package sources
    let mut package_path = PathBuf::from(TEST_DATA_DIR);
    package_path.push("dummy_modules_upgrade");
    let mut build_config = BuildConfig::new_for_testing().config;
    let resp = IotaClientCommands::Publish {
        package_path: package_path.clone(),
        build_config: build_config.clone(),
        skip_dependency_verification: false,
        verify_deps: true,
        with_unpublished_dependencies: false,
        payment: PaymentArgs {
            gas: vec![gas_obj_id],
        },
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_PUBLISH),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;

    let IotaClientCommandResult::TransactionBlock(publish_response) = resp else {
        unreachable!("Invalid response");
    };

    let IotaTransactionBlockEffects::V1(effects) = publish_response.clone().effects.unwrap();

    assert!(effects.status.is_ok());
    assert_eq!(effects.gas_object().object_id(), gas_obj_id);
    let cap = effects
        .created()
        .iter()
        .find(|refe| matches!(refe.owner, Owner::AddressOwner(_)))
        .unwrap();

    // We will upgrade the package in a `tmp_dir` using the `Move.lock` resulting
    // from publish, so as not to clobber anything.
    // The `Move.lock` needs to point to the root directory of the
    // package-to-be-upgraded. The core implementation does not use support an
    // arbitrary `lock_file` path specified in `BuildConfig` when the
    // `Move.lock` file is an input for upgrades, so we change the `BuildConfig`
    // `lock_file` to point to the root directory of package-to-be-upgraded.
    let tmp_dir = tempfile::tempdir().unwrap();
    fs_extra::dir::copy(
        &package_path,
        tmp_dir.path(),
        &fs_extra::dir::CopyOptions::default(),
    )
    .unwrap();
    let mut upgrade_pkg_path = tmp_dir.path().to_path_buf();
    upgrade_pkg_path.extend(["dummy_modules_upgrade", "Move.toml"]);
    upgrade_pkg_path.pop();
    // Place the `Move.lock` after publishing in the tmp dir for upgrading.
    let published_lock_file_path = build_config.lock_file.clone().unwrap();
    let mut upgrade_lock_file_path = upgrade_pkg_path.clone();
    upgrade_lock_file_path.push("Move.lock");
    std::fs::copy(
        published_lock_file_path.clone(),
        upgrade_lock_file_path.clone(),
    )?;
    // Point the `BuildConfig` lock_file to the package root.
    build_config.lock_file = Some(upgrade_pkg_path.join("Move.lock"));

    // Now run the upgrade
    let upgrade_response = IotaClientCommands::Upgrade {
        package_path: upgrade_pkg_path,
        upgrade_capability: cap.reference.object_id,
        build_config: build_config.clone(),
        verify_compatibility: true,
        skip_dependency_verification: false,
        verify_deps: true,
        with_unpublished_dependencies: false,
        payment: PaymentArgs {
            gas: vec![gas_obj_id],
        },
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_PUBLISH),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;

    // Get Original Package ID and version
    let (expect_original_id, _, _) = get_new_package_obj_from_response(&publish_response)
        .ok_or_else(|| anyhow::anyhow!("No package object response"))?;

    // Get Upgraded Package ID and version
    let (expect_upgrade_latest_id, expect_upgrade_version, _) =
        if let IotaClientCommandResult::TransactionBlock(response) = upgrade_response {
            assert_eq!(
                response.effects.as_ref().unwrap().gas_object().object_id(),
                gas_obj_id
            );
            get_new_package_obj_from_response(&response)
                .ok_or_else(|| anyhow::anyhow!("No package object response"))?
        } else {
            unreachable!("Invalid response");
        };

    // Get lock file that recorded Package ID and version
    let lock_file = build_config.lock_file.expect("Lock file for testing");
    let mut lock_file = std::fs::File::open(lock_file).unwrap();
    let envs = ManagedPackage::read(&mut lock_file).unwrap();
    let localnet = envs.get("localnet").unwrap();
    // Original ID should correspond to first published package.
    assert_eq!(
        expect_original_id.to_string(),
        localnet.original_published_id,
    );
    // Upgrade ID should correspond to upgraded package.
    assert_eq!(
        expect_upgrade_latest_id.to_string(),
        localnet.latest_published_id,
    );
    // Version should correspond to upgraded package.
    assert_eq!(
        expect_upgrade_version.value(),
        localnet.version.parse::<u64>().unwrap(),
    );
    Ok(())
}

#[sim_test]
async fn test_package_management_on_upgrade_command_conflict() -> Result<(), anyhow::Error> {
    move_package::package_hooks::register_package_hooks(Box::new(IotaPackageHooks));
    let mut test_cluster = TestClusterBuilder::new()
        .with_num_validators(2)
        .build()
        .await;
    let rgp = test_cluster.get_reference_gas_price().await;
    let address = test_cluster.get_address_0();
    let context = &mut test_cluster.wallet;
    let client = context.get_client().await?;
    let object_refs = client
        .read_api()
        .get_owned_objects(
            address,
            Some(IotaObjectResponseQuery::new_with_options(
                IotaObjectDataOptions::new()
                    .with_type()
                    .with_owner()
                    .with_previous_transaction(),
            )),
            None,
            None,
        )
        .await?
        .data;

    let gas_obj_id = object_refs.first().unwrap().object().unwrap().object_id;

    // Provide path to well formed package sources
    let mut package_path = PathBuf::from(TEST_DATA_DIR);
    package_path.push("dummy_modules_upgrade");
    let build_config_publish = BuildConfig::new_for_testing().config;
    let resp = IotaClientCommands::Publish {
        package_path: package_path.clone(),
        build_config: build_config_publish.clone(),
        skip_dependency_verification: false,
        verify_deps: true,
        with_unpublished_dependencies: false,
        payment: PaymentArgs {
            gas: vec![gas_obj_id],
        },
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_PUBLISH),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;

    let IotaClientCommandResult::TransactionBlock(publish_response) = resp else {
        unreachable!("Invalid response");
    };

    let IotaTransactionBlockEffects::V1(effects) = publish_response.clone().effects.unwrap();

    assert!(effects.status.is_ok());
    assert_eq!(effects.gas_object().object_id(), gas_obj_id);
    let package = effects
        .created()
        .iter()
        .find(|refe| matches!(refe.owner, Owner::Immutable))
        .unwrap();

    let cap = effects
        .created()
        .iter()
        .find(|refe| matches!(refe.owner, Owner::AddressOwner(_)))
        .unwrap();

    // Set up a temporary working directory  for upgrading.
    let tmp_dir = tempfile::tempdir().unwrap();
    fs_extra::dir::copy(
        &package_path,
        tmp_dir.path(),
        &fs_extra::dir::CopyOptions::default(),
    )
    .unwrap();
    let mut upgrade_pkg_path = tmp_dir.path().to_path_buf();
    upgrade_pkg_path.extend(["dummy_modules_upgrade", "Move.toml"]);
    let mut move_toml = std::fs::File::options()
        .read(true)
        .write(true)
        .open(&upgrade_pkg_path)
        .unwrap();
    upgrade_pkg_path.pop();
    let mut buf = String::new();
    move_toml.read_to_string(&mut buf).unwrap();
    let mut lines: Vec<String> = buf.split('\n').map(|x| x.to_string()).collect();
    let idx = lines.iter().position(|s| s == "[package]").unwrap();
    // Purposely add a conflicting `published-at` address to the Move manifest.
    lines.insert(idx + 1, "published-at = \"0xbad\"".to_string());
    let new = lines.join("\n");

    move_toml.seek(SeekFrom::Start(0))?;
    move_toml.set_len(0)?; // Truncate the file
    move_toml.write_all(new.as_bytes())?;

    // Create a new build config for the upgrade. Initialize its lock file to the
    // package we published.
    let build_config_upgrade = BuildConfig::new_for_testing().config;
    let mut upgrade_lock_file_path = upgrade_pkg_path.clone();
    upgrade_lock_file_path.push("Move.lock");
    let publish_lock_file_path = build_config_publish.lock_file.unwrap();
    std::fs::copy(
        publish_lock_file_path.clone(),
        upgrade_lock_file_path.clone(),
    )?;

    // Now run the upgrade
    let upgrade_response = IotaClientCommands::Upgrade {
        package_path: upgrade_pkg_path,
        upgrade_capability: cap.reference.object_id,
        build_config: build_config_upgrade.clone(),
        verify_compatibility: true,
        skip_dependency_verification: false,
        verify_deps: true,
        with_unpublished_dependencies: false,
        payment: PaymentArgs {
            gas: vec![gas_obj_id],
        },
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_PUBLISH),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await;

    let err_string = upgrade_response.unwrap_err().to_string();
    let err_string = err_string.replace(&package.object_id().to_string(), "<elided-for-test>");

    let expect = expect![[r#"
Conflicting published package address: `Move.toml` contains published-at address 0x0000000000000000000000000000000000000000000000000000000000000bad but `Move.lock` file contains published-at address <elided-for-test>. You may want to:
 - delete the published-at address in the `Move.toml` if the `Move.lock` address is correct; OR
 - update the `Move.lock` address using the `iota manage-package` command to be the same as the `Move.toml`; OR
 - check that your `iota active-env` (currently localnet) corresponds to the chain on which the package is published (i.e., devnet, testnet, mainnet); OR
 - contact the maintainer if this package is a dependency and request resolving the conflict."#]];
    expect.assert_eq(&err_string);
    Ok(())
}

#[sim_test]
async fn test_native_transfer() -> Result<(), anyhow::Error> {
    let mut test_cluster = TestClusterBuilder::new()
        .with_num_validators(2)
        .build()
        .await;
    let rgp = test_cluster.get_reference_gas_price().await;
    let address = test_cluster.get_address_0();
    let context = &mut test_cluster.wallet;
    let recipient = IotaAddress::random_for_testing_only();
    let client = context.get_client().await?;
    let object_refs = client
        .read_api()
        .get_owned_objects(
            address,
            Some(IotaObjectResponseQuery::new_with_options(
                IotaObjectDataOptions::new()
                    .with_type()
                    .with_owner()
                    .with_previous_transaction(),
            )),
            None,
            None,
        )
        .await?
        .data;

    // Check log output contains all object ids.
    let gas_obj_id = object_refs.first().unwrap().object().unwrap().object_id;
    let obj_id = object_refs.get(1).unwrap().object().unwrap().object_id;

    let resp = IotaClientCommands::Transfer {
        to: KeyIdentity::Address(recipient),
        object_id: obj_id,
        payment: PaymentArgs {
            gas: vec![gas_obj_id],
        },
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;

    // Print it out to CLI/logs
    resp.print(true);

    // Get the mutated objects
    let (mut_obj1, mut_obj2) = if let IotaClientCommandResult::TransactionBlock(response) = resp {
        assert!(
            response.status_ok().unwrap(),
            "Command failed: {response:?}"
        );
        assert_eq!(
            response.effects.as_ref().unwrap().gas_object().object_id(),
            gas_obj_id
        );
        (
            response
                .effects
                .as_ref()
                .unwrap()
                .mutated()
                .first()
                .unwrap()
                .reference
                .object_id,
            response
                .effects
                .as_ref()
                .unwrap()
                .mutated()
                .get(1)
                .unwrap()
                .reference
                .object_id,
        )
    } else {
        panic!()
    };

    // Check the objects
    let resp = IotaClientCommands::Object {
        id: mut_obj1,
        bcs: false,
    }
    .execute(context)
    .await?;
    let mut_obj1 = if let IotaClientCommandResult::Object(resp) = resp {
        if let Some(obj) = resp.data {
            obj
        } else {
            panic!()
        }
    } else {
        panic!();
    };

    let resp2 = IotaClientCommands::Object {
        id: mut_obj2,
        bcs: false,
    }
    .execute(context)
    .await?;
    let mut_obj2 = if let IotaClientCommandResult::Object(resp2) = resp2 {
        if let Some(obj) = resp2.data {
            obj
        } else {
            panic!()
        }
    } else {
        panic!();
    };

    let (gas, obj) = if mut_obj1.owner.unwrap().get_owner_address().unwrap() == address {
        (mut_obj1, mut_obj2)
    } else {
        (mut_obj2, mut_obj1)
    };

    assert_eq!(gas.owner.unwrap().get_owner_address().unwrap(), address);
    assert_eq!(obj.owner.unwrap().get_owner_address().unwrap(), recipient);

    let object_refs = client
        .read_api()
        .get_owned_objects(
            address,
            Some(IotaObjectResponseQuery::new_with_options(
                IotaObjectDataOptions::new()
                    .with_type()
                    .with_owner()
                    .with_previous_transaction(),
            )),
            None,
            None,
        )
        .await?;

    // Check log output contains all object ids.
    let obj_id = object_refs.data.get(1).unwrap().object().unwrap().object_id;

    let resp = IotaClientCommands::Transfer {
        to: KeyIdentity::Address(recipient),
        object_id: obj_id,
        payment: PaymentArgs::default(),
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;

    // Print it out to CLI/logs
    resp.print(true);

    // Get the mutated objects
    let (_mut_obj1, _mut_obj2) = if let IotaClientCommandResult::TransactionBlock(response) = resp {
        (
            response
                .effects
                .as_ref()
                .unwrap()
                .mutated()
                .first()
                .unwrap()
                .reference
                .object_id,
            response
                .effects
                .as_ref()
                .unwrap()
                .mutated()
                .get(1)
                .unwrap()
                .reference
                .object_id,
        )
    } else {
        panic!()
    };

    Ok(())
}

#[test]
// Test for issue https://github.com/iotaledger/iota/issues/1078
fn test_bug_1078() {
    let read = IotaClientCommandResult::Object(IotaObjectResponse::new_with_error(
        IotaObjectResponseError::NotExists {
            object_id: ObjectID::random(),
        },
    ));
    let mut writer = String::new();
    // fmt ObjectRead should not fail.
    write!(writer, "{read}").unwrap();
    write!(writer, "{read:?}").unwrap();
}

#[sim_test]
async fn test_switch_command() -> Result<(), anyhow::Error> {
    let mut cluster = TestClusterBuilder::new().build().await;
    let addr2 = cluster.get_address_1();
    let context = cluster.wallet_mut();

    // Get the active address
    let addr1 = context.active_address()?;

    // Run a command with address omitted
    let os = IotaClientCommands::Objects { address: None }
        .execute(context)
        .await?;

    let mut cmd_objs = if let IotaClientCommandResult::Objects(v) = os {
        v
    } else {
        panic!("Command failed")
    };

    // Check that we indeed fetched for addr1
    let client = context.get_client().await?;
    let mut actual_objs = client
        .read_api()
        .get_owned_objects(
            addr1,
            Some(IotaObjectResponseQuery::new_with_options(
                IotaObjectDataOptions::full_content(),
            )),
            None,
            None,
        )
        .await
        .unwrap()
        .data;
    cmd_objs.sort();
    actual_objs.sort();
    assert_eq!(cmd_objs, actual_objs);

    // Switch the address
    let resp = IotaClientCommands::Switch {
        address: Some(KeyIdentity::Address(addr2)),
        env: None,
    }
    .execute(context)
    .await?;
    assert_eq!(addr2, context.active_address()?);
    assert_ne!(addr1, context.active_address()?);
    assert_eq!(
        format!("{resp}"),
        format!(
            "{}",
            IotaClientCommandResult::Switch(SwitchResponse {
                address: Some(addr2.to_string()),
                env: None
            })
        )
    );

    // Wipe all the address info
    context.config_mut().set_active_address(None);

    // Create a new address
    let os = IotaClientCommands::NewAddress {
        key_scheme: SignatureScheme::ED25519,
        alias: None,
        derivation_path: None,
        word_length: None,
    }
    .execute(context)
    .await?;
    let new_addr = if let IotaClientCommandResult::NewAddress(x) = os {
        x.address
    } else {
        panic!("Command failed")
    };

    // Check that we can switch to this address
    // Switch the address
    let resp = IotaClientCommands::Switch {
        address: Some(KeyIdentity::Address(new_addr)),
        env: None,
    }
    .execute(context)
    .await?;
    assert_eq!(new_addr, context.active_address()?);
    assert_eq!(
        format!("{resp}"),
        format!(
            "{}",
            IotaClientCommandResult::Switch(SwitchResponse {
                address: Some(new_addr.to_string()),
                env: None
            })
        )
    );
    Ok(())
}

#[sim_test]
async fn test_new_address_command_by_flag() -> Result<(), anyhow::Error> {
    let mut cluster = TestClusterBuilder::new().build().await;
    let context = cluster.wallet_mut();

    // keypairs loaded from config are Ed25519
    assert_eq!(
        context
            .config()
            .keystore()
            .keys()
            .iter()
            .filter(|k| k.public().flag() == Ed25519IotaSignature::SCHEME.flag())
            .count(),
        5
    );

    IotaClientCommands::NewAddress {
        key_scheme: SignatureScheme::Secp256k1,
        alias: None,
        derivation_path: None,
        word_length: None,
    }
    .execute(context)
    .await?;

    // new keypair generated is Secp256k1
    assert_eq!(
        context
            .config()
            .keystore()
            .keys()
            .iter()
            .filter(|k| k.public().flag() == Secp256k1IotaSignature::SCHEME.flag())
            .count(),
        1
    );

    Ok(())
}

#[sim_test]
async fn test_remove_address_command() -> Result<(), anyhow::Error> {
    let mut cluster = TestClusterBuilder::new().build().await;
    let context = cluster.wallet_mut();

    let address = context
        .config()
        .keystore()
        .addresses()
        .get(1)
        .cloned()
        .unwrap();

    IotaClientCommands::RemoveAddress {
        address: address.into(),
    }
    .execute(context)
    .await?;

    assert_eq!(
        context
            .config()
            .keystore()
            .addresses()
            .iter()
            .filter(|k| *k == &address)
            .count(),
        0
    );

    Ok(())
}

#[sim_test]
async fn test_active_address_command() -> Result<(), anyhow::Error> {
    let mut cluster = TestClusterBuilder::new().build().await;
    let context = cluster.wallet_mut();

    // Get the active address
    let addr1 = context.active_address()?;

    // Run a command with address omitted
    let os = IotaClientCommands::ActiveAddress {}
        .execute(context)
        .await?;

    let a = if let IotaClientCommandResult::ActiveAddress(Some(v)) = os {
        v
    } else {
        panic!("Command failed")
    };
    assert_eq!(a, addr1);

    let addr2 = context
        .config()
        .keystore()
        .addresses()
        .get(1)
        .cloned()
        .unwrap();
    let resp = IotaClientCommands::Switch {
        address: Some(KeyIdentity::Address(addr2)),
        env: None,
    }
    .execute(context)
    .await?;
    assert_eq!(
        format!("{resp}"),
        format!(
            "{}",
            IotaClientCommandResult::Switch(SwitchResponse {
                address: Some(addr2.to_string()),
                env: None
            })
        )
    );

    // switch back to addr1 by using its alias
    let alias1 = context
        .config()
        .keystore()
        .get_alias_by_address(&addr1)
        .unwrap();
    let resp = IotaClientCommands::Switch {
        address: Some(KeyIdentity::Alias(alias1)),
        env: None,
    }
    .execute(context)
    .await?;
    assert_eq!(
        format!("{resp}"),
        format!(
            "{}",
            IotaClientCommandResult::Switch(SwitchResponse {
                address: Some(addr1.to_string()),
                env: None
            })
        )
    );

    Ok(())
}

fn get_gas_value(o: &IotaObjectData) -> u64 {
    GasCoin::try_from(o).unwrap().value()
}

async fn get_object(id: ObjectID, context: &WalletContext) -> Option<IotaObjectData> {
    let client = context.get_client().await.unwrap();
    let response = client
        .read_api()
        .get_object_with_options(id, IotaObjectDataOptions::full_content())
        .await
        .unwrap();
    response.data
}

async fn get_parsed_object_assert_existence(
    object_id: ObjectID,
    context: &WalletContext,
) -> IotaObjectData {
    get_object(object_id, context)
        .await
        .unwrap_or_else(|| panic!("Object {object_id} does not exist."))
}

#[sim_test]
async fn test_merge_coin() -> Result<(), anyhow::Error> {
    let mut test_cluster = TestClusterBuilder::new()
        .with_num_validators(2)
        .build()
        .await;
    let rgp = test_cluster.get_reference_gas_price().await;
    let address = test_cluster.get_address_0();
    let context = &mut test_cluster.wallet;

    let client = context.get_client().await?;
    let object_refs = client
        .read_api()
        .get_owned_objects(
            address,
            Some(IotaObjectResponseQuery::new_with_options(
                IotaObjectDataOptions::new()
                    .with_type()
                    .with_owner()
                    .with_previous_transaction(),
            )),
            None,
            None,
        )
        .await?
        .data;

    // Check log output contains all object ids.
    let gas = object_refs.first().unwrap().object().unwrap().object_id;
    let primary_coin = object_refs.get(1).unwrap().object().unwrap().object_id;
    let coin_to_merge = object_refs.get(2).unwrap().object().unwrap().object_id;

    let total_value = get_gas_value(&get_object(primary_coin, context).await.unwrap())
        + get_gas_value(&get_object(coin_to_merge, context).await.unwrap());

    // Test with gas specified
    let resp = IotaClientCommands::MergeCoin {
        primary_coin,
        coin_to_merge,
        payment: PaymentArgs { gas: vec![gas] },
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_GENERIC),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;
    let g = if let IotaClientCommandResult::TransactionBlock(r) = resp {
        assert!(r.status_ok().unwrap(), "Command failed: {r:?}");
        assert_eq!(r.effects.as_ref().unwrap().gas_object().object_id(), gas);
        let object_id = r
            .effects
            .as_ref()
            .unwrap()
            .mutated_excluding_gas()
            .into_iter()
            .next()
            .unwrap()
            .reference
            .object_id;
        get_parsed_object_assert_existence(object_id, context).await
    } else {
        panic!("Command failed")
    };

    // Check total value is expected
    assert_eq!(get_gas_value(&g), total_value);

    // Check that old coin is deleted
    assert_eq!(get_object(coin_to_merge, context).await, None);

    let object_refs = client
        .read_api()
        .get_owned_objects(
            address,
            Some(IotaObjectResponseQuery::new_with_options(
                IotaObjectDataOptions::new()
                    .with_type()
                    .with_owner()
                    .with_previous_transaction(),
            )),
            None,
            None,
        )
        .await?;

    let primary_coin = object_refs.data.get(1).unwrap().object()?.object_id;
    let coin_to_merge = object_refs.data.get(2).unwrap().object()?.object_id;

    let total_value = get_gas_value(&get_object(primary_coin, context).await.unwrap())
        + get_gas_value(&get_object(coin_to_merge, context).await.unwrap());

    // Test with no gas specified
    let resp = IotaClientCommands::MergeCoin {
        primary_coin,
        coin_to_merge,
        payment: PaymentArgs::default(),
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_GENERIC),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;

    let g = if let IotaClientCommandResult::TransactionBlock(r) = resp {
        let object_id = r
            .effects
            .as_ref()
            .unwrap()
            .mutated_excluding_gas()
            .into_iter()
            .next()
            .unwrap()
            .reference
            .object_id;
        get_parsed_object_assert_existence(object_id, context).await
    } else {
        panic!("Command failed")
    };

    // Check total value is expected
    assert_eq!(get_gas_value(&g), total_value);

    // Check that old coin is deleted
    assert_eq!(get_object(coin_to_merge, context).await, None);

    Ok(())
}

#[sim_test]
async fn test_split_coin() -> Result<(), anyhow::Error> {
    let mut test_cluster = TestClusterBuilder::new()
        .with_num_validators(2)
        .build()
        .await;
    let rgp = test_cluster.get_reference_gas_price().await;
    let address = test_cluster.get_address_0();
    let address_1 = test_cluster.get_address_1();
    let context = &mut test_cluster.wallet;
    let client = context.get_client().await?;
    let object_refs = client
        .read_api()
        .get_owned_objects(
            address,
            Some(IotaObjectResponseQuery::new_with_options(
                IotaObjectDataOptions::new()
                    .with_type()
                    .with_owner()
                    .with_previous_transaction(),
            )),
            None,
            None,
        )
        .await?;

    // Check log output contains all object ids.
    let gas = object_refs.data.first().unwrap().object()?.object_id;
    let mut coin = object_refs.data.get(1).unwrap().object()?.object_id;

    let orig_value = get_gas_value(&get_object(coin, context).await.unwrap());

    // Test with gas specified
    let resp = IotaClientCommands::SplitCoin {
        coin_id: coin,
        amounts: Some(vec![1000, 10]),
        count: None,
        payment: PaymentArgs { gas: vec![gas] },
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_SPLIT_COIN),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;

    let (updated_coin, new_coins) = if let IotaClientCommandResult::TransactionBlock(r) = resp {
        assert!(r.status_ok().unwrap(), "Command failed: {r:?}");
        assert_eq!(r.effects.as_ref().unwrap().gas_object().object_id(), gas);
        let updated_object_id = r
            .effects
            .as_ref()
            .unwrap()
            .mutated_excluding_gas()
            .into_iter()
            .next()
            .unwrap()
            .reference
            .object_id;
        let updated_obj = get_parsed_object_assert_existence(updated_object_id, context).await;
        let new_object_refs = r.effects.unwrap().created().to_vec();
        let mut new_objects = Vec::with_capacity(new_object_refs.len());
        for obj_ref in new_object_refs {
            new_objects.push(
                get_parsed_object_assert_existence(obj_ref.reference.object_id, context).await,
            );
        }
        (updated_obj, new_objects)
    } else {
        panic!("Command failed")
    };

    // Check values expected
    assert_eq!(get_gas_value(&updated_coin) + 1000 + 10, orig_value);
    assert!((get_gas_value(&new_coins[0]) == 1000) || (get_gas_value(&new_coins[0]) == 10));
    assert!((get_gas_value(&new_coins[1]) == 1000) || (get_gas_value(&new_coins[1]) == 10));
    let client = context.get_client().await?;
    let object_refs = client
        .read_api()
        .get_owned_objects(
            address,
            Some(IotaObjectResponseQuery::new_with_options(
                IotaObjectDataOptions::new()
                    .with_type()
                    .with_owner()
                    .with_previous_transaction(),
            )),
            None,
            None,
        )
        .await?
        .data;

    // Get another coin
    for c in object_refs {
        let coin_data = c.into_object().unwrap();
        if get_gas_value(&get_object(coin_data.object_id, context).await.unwrap()) > 2000 {
            coin = coin_data.object_id;
        }
    }
    let orig_value = get_gas_value(&get_object(coin, context).await.unwrap());

    // Test split coin into equal parts
    let resp = IotaClientCommands::SplitCoin {
        coin_id: coin,
        amounts: None,
        count: Some(3),
        payment: PaymentArgs::default(),
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_SPLIT_COIN),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;

    let (updated_coin, new_coins) = if let IotaClientCommandResult::TransactionBlock(r) = resp {
        assert!(r.status_ok().unwrap(), "Command failed: {r:?}");
        let updated_object_id = r
            .effects
            .as_ref()
            .unwrap()
            .mutated_excluding_gas()
            .into_iter()
            .next()
            .unwrap()
            .reference
            .object_id;
        let updated_obj = get_parsed_object_assert_existence(updated_object_id, context).await;
        let new_object_refs = r.effects.unwrap().created().to_vec();
        let mut new_objects = Vec::with_capacity(new_object_refs.len());
        for obj_ref in new_object_refs {
            new_objects.push(
                get_parsed_object_assert_existence(obj_ref.reference.object_id, context).await,
            );
        }
        (updated_obj, new_objects)
    } else {
        panic!("Command failed")
    };

    // Check values expected
    assert_eq!(
        get_gas_value(&updated_coin),
        orig_value / 3 + orig_value % 3
    );
    assert_eq!(get_gas_value(&new_coins[0]), orig_value / 3);
    assert_eq!(get_gas_value(&new_coins[1]), orig_value / 3);

    let object_refs = client
        .read_api()
        .get_owned_objects(
            address,
            Some(IotaObjectResponseQuery::new_with_options(
                IotaObjectDataOptions::new()
                    .with_type()
                    .with_owner()
                    .with_previous_transaction(),
            )),
            None,
            None,
        )
        .await?
        .data;

    // Get another coin
    for c in object_refs {
        let coin_data = c.into_object().unwrap();
        if get_gas_value(&get_object(coin_data.object_id, context).await.unwrap()) > 2000 {
            coin = coin_data.object_id;
        }
    }
    let orig_value = get_gas_value(&get_object(coin, context).await.unwrap());

    // Test with no gas specified
    let resp = IotaClientCommands::SplitCoin {
        coin_id: coin,
        amounts: Some(vec![1000, 10]),
        count: None,
        payment: PaymentArgs::default(),
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_SPLIT_COIN),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;

    let (updated_coin, new_coins) = if let IotaClientCommandResult::TransactionBlock(r) = resp {
        assert!(r.status_ok().unwrap(), "Command failed: {r:?}");
        let updated_object_id = r
            .effects
            .as_ref()
            .unwrap()
            .mutated_excluding_gas()
            .into_iter()
            .next()
            .unwrap()
            .reference
            .object_id;
        let updated_obj = get_parsed_object_assert_existence(updated_object_id, context).await;
        let new_object_refs = r.effects.unwrap().created().to_vec();
        let mut new_objects = Vec::with_capacity(new_object_refs.len());
        for obj_ref in new_object_refs {
            new_objects.push(
                get_parsed_object_assert_existence(obj_ref.reference.object_id, context).await,
            );
        }
        (updated_obj, new_objects)
    } else {
        panic!("Command failed")
    };

    // Check values expected
    assert_eq!(get_gas_value(&updated_coin) + 1000 + 10, orig_value);
    assert!((get_gas_value(&new_coins[0]) == 1000) || (get_gas_value(&new_coins[0]) == 10));
    assert!((get_gas_value(&new_coins[1]) == 1000) || (get_gas_value(&new_coins[1]) == 10));

    // Test with single gas coin
    context.config_mut().set_active_address(Some(address_1));
    let object_refs = client
        .coin_read_api()
        .get_coins(address_1, None, None, None)
        .await?;
    // First merge all coins so we only have a single one left
    let resp = IotaClientCommands::PayAllIota {
        input_coins: object_refs.data.iter().map(|o| o.coin_object_id).collect(),
        recipient: KeyIdentity::Address(address_1),
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;
    if let IotaClientCommandResult::TransactionBlock(r) = resp {
        assert!(r.status_ok().unwrap(), "Command PayAllIota failed: {r:?}");
    } else {
        panic!("Command PayAllIota failed")
    };

    let object_refs = client
        .coin_read_api()
        .get_coins(address_1, None, None, None)
        .await?;
    assert_eq!(object_refs.data.len(), 1, "More than one coin");

    let gas = object_refs.data.first().unwrap().coin_object_id;
    let resp = IotaClientCommands::SplitCoin {
        coin_id: gas,
        amounts: Some(vec![10, 1000]),
        count: None,
        payment: PaymentArgs::default(),
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_SPLIT_COIN),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;

    let new_coins = if let IotaClientCommandResult::TransactionBlock(r) = resp {
        assert!(r.status_ok().unwrap(), "Command SplitCoin failed: {r:?}");
        let effects = r.effects.as_ref().unwrap();
        assert_eq!(effects.gas_object().object_id(), gas);

        let new_object_refs = effects.created().to_vec();
        let mut new_objects = Vec::with_capacity(new_object_refs.len());
        for obj_ref in new_object_refs {
            new_objects.push(
                get_parsed_object_assert_existence(obj_ref.reference.object_id, context).await,
            );
        }
        new_objects
    } else {
        panic!("Command SplitCoin failed")
    };
    assert!((get_gas_value(&new_coins[0]) == 10) || (get_gas_value(&new_coins[0]) == 1000));
    assert!((get_gas_value(&new_coins[1]) == 1000) || (get_gas_value(&new_coins[1]) == 10));
    Ok(())
}

#[sim_test]
async fn test_signature_flag() -> Result<(), anyhow::Error> {
    let res = SignatureScheme::from_flag("0");
    assert!(res.is_ok());
    assert_eq!(res.unwrap().flag(), SignatureScheme::ED25519.flag());

    let res = SignatureScheme::from_flag("1");
    assert!(res.is_ok());
    assert_eq!(res.unwrap().flag(), SignatureScheme::Secp256k1.flag());

    let res = SignatureScheme::from_flag("2");
    assert!(res.is_ok());
    assert_eq!(res.unwrap().flag(), SignatureScheme::Secp256r1.flag());

    let res = SignatureScheme::from_flag("something");
    assert!(res.is_err());
    Ok(())
}

#[sim_test]
async fn test_execute_signed_tx() -> Result<(), anyhow::Error> {
    let mut test_cluster = TestClusterBuilder::new()
        .with_num_validators(2)
        .build()
        .await;
    let context = &mut test_cluster.wallet;
    let mut txns = batch_make_transfer_transactions(context, 1).await;
    let txn = txns.swap_remove(0);

    let (tx_data, signatures) = txn.to_tx_bytes_and_signatures();
    IotaClientCommands::ExecuteSignedTx {
        tx_bytes: tx_data.encoded(),
        signatures: signatures.into_iter().map(|s| s.encoded()).collect(),
    }
    .execute(context)
    .await?;
    Ok(())
}

#[sim_test]
async fn test_serialize_tx() -> Result<(), anyhow::Error> {
    let mut test_cluster = TestClusterBuilder::new()
        .with_num_validators(2)
        .build()
        .await;
    let rgp = test_cluster.get_reference_gas_price().await;
    let address = test_cluster.get_address_0();
    let address1 = test_cluster.get_address_1();
    let context = &mut test_cluster.wallet;
    let alias1 = context
        .config()
        .keystore()
        .get_alias_by_address(&address1)
        .unwrap();
    let client = context.get_client().await?;
    let object_refs = client
        .read_api()
        .get_owned_objects(
            address,
            Some(IotaObjectResponseQuery::new_with_options(
                IotaObjectDataOptions::new()
                    .with_type()
                    .with_owner()
                    .with_previous_transaction(),
            )),
            None,
            None,
        )
        .await?
        .data;
    let coin = object_refs.get(1).unwrap().object().unwrap().object_id;

    IotaClientCommands::PayIota {
        input_coins: Some(vec![coin]),
        recipients: vec![KeyIdentity::Address(address1)],
        amounts: vec![1],
        gas_data: GasDataArgs::default(),
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;

    IotaClientCommands::PayIota {
        input_coins: Some(vec![coin]),
        recipients: vec![KeyIdentity::Address(address1)],
        amounts: vec![1],
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;

    // use alias for transfer
    IotaClientCommands::PayIota {
        input_coins: Some(vec![coin]),
        recipients: vec![KeyIdentity::Alias(alias1)],
        amounts: vec![1],
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;

    let ptb_args = vec![
        "--split-coins".to_string(),
        "gas".to_string(),
        "[1000]".to_string(),
        "--assign".to_string(),
        "new_coin".to_string(),
        "--transfer-objects".to_string(),
        "[new_coin]".to_string(),
        format!("@{}", address1),
        "--gas-budget".to_string(),
        "50000000".to_string(),
    ];
    let mut args = ptb_args.clone();
    args.push("--serialize-signed-transaction".to_string());
    let ptb = PTB {
        args,
        display: HashSet::new(),
    };
    IotaClientCommands::PTB(ptb).execute(context).await.unwrap();
    let mut args = ptb_args.clone();
    args.push("--serialize-unsigned-transaction".to_string());
    let ptb = PTB {
        args,
        display: HashSet::new(),
    };
    IotaClientCommands::PTB(ptb).execute(context).await.unwrap();

    Ok(())
}

#[tokio::test]
async fn test_stake_with_none_amount() -> Result<(), anyhow::Error> {
    let mut test_cluster = TestClusterBuilder::new()
        .with_num_validators(2)
        .build()
        .await;
    let address = test_cluster.get_address_0();
    let context = &mut test_cluster.wallet;

    let client = context.get_client().await?;
    let coins = client
        .coin_read_api()
        .get_coins(address, None, None, None)
        .await?
        .data;

    let config_path = test_cluster.swarm.dir().join(IOTA_CLIENT_CONFIG);

    // Here we test the staking transaction to a committee member.
    let committee_member_addr = client
        .governance_api()
        .get_latest_iota_system_state()
        .await?
        .iter_committee_members()
        .next()
        .unwrap()
        .iota_address;

    test_with_iota_binary(&[
        "client",
        "--client.config",
        config_path.to_str().unwrap(),
        "call",
        "--package",
        "0x3",
        "--module",
        "iota_system",
        "--function",
        "request_add_stake_mul_coin",
        "--args",
        "0x5",
        &format!("[{}]", coins.first().unwrap().coin_object_id),
        "[]",
        &committee_member_addr.to_string(),
        "--gas-budget",
        "1000000000",
    ])
    .await?;

    let stake = client.governance_api().get_stakes(address).await?;

    assert_eq!(1, stake.len());
    assert_eq!(
        coins.first().unwrap().balance,
        stake.first().unwrap().stakes.first().unwrap().principal
    );
    Ok(())
}

#[tokio::test]
async fn test_stake_with_u64_amount() -> Result<(), anyhow::Error> {
    let mut test_cluster = TestClusterBuilder::new()
        .with_num_validators(2)
        .build()
        .await;
    let address = test_cluster.get_address_0();
    let context = &mut test_cluster.wallet;

    let client = context.get_client().await?;
    let coins = client
        .coin_read_api()
        .get_coins(address, None, None, None)
        .await?
        .data;

    let config_path = test_cluster.swarm.dir().join(IOTA_CLIENT_CONFIG);

    // Here we test the staking transaction to a committee member.
    let committee_member_addr = client
        .governance_api()
        .get_latest_iota_system_state()
        .await?
        .iter_committee_members()
        .next()
        .unwrap()
        .iota_address;

    test_with_iota_binary(&[
        "client",
        "--client.config",
        config_path.to_str().unwrap(),
        "call",
        "--package",
        "0x3",
        "--module",
        "iota_system",
        "--function",
        "request_add_stake_mul_coin",
        "--args",
        "0x5",
        &format!("[{}]", coins.first().unwrap().coin_object_id),
        "[1000000000]",
        &committee_member_addr.to_string(),
        "--gas-budget",
        "1000000000",
    ])
    .await?;

    let stake = client.governance_api().get_stakes(address).await?;

    assert_eq!(1, stake.len());
    assert_eq!(
        1000000000,
        stake.first().unwrap().stakes.first().unwrap().principal
    );
    Ok(())
}

async fn test_with_iota_binary(args: &[&str]) -> Result<(), anyhow::Error> {
    let mut cmd = assert_cmd::Command::cargo_bin("iota").unwrap();
    let args = args.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    // test cluster will not response if this call is in the same thread
    let out = thread::spawn(move || cmd.args(args).assert());
    while !out.is_finished() {
        sleep(Duration::from_millis(100)).await;
    }
    out.join().unwrap().success();
    Ok(())
}

#[sim_test]
async fn test_get_owned_objects_owned_by_address_and_check_pagination() -> Result<(), anyhow::Error>
{
    let mut test_cluster = TestClusterBuilder::new()
        .with_num_validators(2)
        .build()
        .await;
    let address = test_cluster.get_address_0();
    let context = &mut test_cluster.wallet;

    let client = context.get_client().await?;
    let object_responses = client
        .read_api()
        .get_owned_objects(
            address,
            Some(IotaObjectResponseQuery::new(
                Some(IotaObjectDataFilter::StructType(GasCoin::type_())),
                Some(
                    IotaObjectDataOptions::new()
                        .with_type()
                        .with_owner()
                        .with_previous_transaction(),
                ),
            )),
            None,
            None,
        )
        .await?;

    // assert that all the objects_returned are owned by the address
    for resp in &object_responses.data {
        let obj_owner = resp.object().unwrap().owner.unwrap();
        assert_eq!(
            obj_owner.get_owner_address().unwrap().to_string(),
            address.to_string()
        )
    }
    // assert that has next page is false
    assert!(!object_responses.has_next_page);

    // Pagination check
    let response_data = PagedFn::collect::<Vec<_>>(async |cursor| {
        client
            .read_api()
            .get_owned_objects(
                address,
                Some(IotaObjectResponseQuery::new(
                    Some(IotaObjectDataFilter::StructType(GasCoin::type_())),
                    Some(
                        IotaObjectDataOptions::new()
                            .with_type()
                            .with_owner()
                            .with_previous_transaction(),
                    ),
                )),
                cursor,
                Some(1),
            )
            .await
    })
    .await?;

    assert_eq!(&response_data, &object_responses.data);

    Ok(())
}

#[tokio::test]
async fn test_linter_suppression_stats() -> Result<(), anyhow::Error> {
    const LINTER_MSG: &str = "Total number of linter warnings suppressed: 5 (unique lints: 3)";
    let mut cmd = assert_cmd::Command::cargo_bin("iota").unwrap();
    let args = vec!["move", "test", "--path", "tests/data/linter"];
    let output = cmd
        .args(&args)
        .output()
        .expect("failed to run 'iota move test'");
    let out_str = str::from_utf8(&output.stderr).unwrap();
    assert!(
        out_str.contains(LINTER_MSG),
        "Expected to match {LINTER_MSG}, got: {out_str}"
    );
    // test no-lint suppresses
    let args = vec!["move", "test", "--no-lint", "--path", "tests/data/linter"];
    let output = cmd
        .args(&args)
        .output()
        .expect("failed to run 'iota move test'");
    let out_str = str::from_utf8(&output.stderr).unwrap();
    assert!(
        !out_str.contains(LINTER_MSG),
        "Expected _not to_ match {LINTER_MSG}, got: {out_str}"
    );
    Ok(())
}

#[tokio::test]
async fn key_identity_test() {
    let mut test_cluster = TestClusterBuilder::new()
        .with_num_validators(2)
        .build()
        .await;
    let address = test_cluster.get_address_0();
    let context = &mut test_cluster.wallet;
    let alias = context
        .config()
        .keystore()
        .get_alias_by_address(&address)
        .unwrap();

    // by alias
    assert_eq!(
        address,
        get_identity_address(Some(KeyIdentity::Alias(alias)), context)
            .await
            .unwrap()
    );
    // by address
    assert_eq!(
        address,
        get_identity_address(Some(KeyIdentity::Address(address)), context)
            .await
            .unwrap()
    );
    // alias does not exist
    assert!(
        get_identity_address(Some(KeyIdentity::Alias("alias".to_string())), context)
            .await
            .is_err()
    );

    // get active address instead when no alias/address is given
    assert_eq!(
        context.active_address().unwrap(),
        get_identity_address(None, context).await.unwrap()
    );
}

fn assert_dry_run(dry_run: IotaClientCommandResult, object_id: ObjectID, command: &str) {
    if let IotaClientCommandResult::DryRun(response) = dry_run {
        assert_eq!(
            *response.effects.status(),
            IotaExecutionStatus::Success,
            "{command} dry run test effects is not success"
        );
        assert_eq!(
            response.effects.gas_object().object_id(),
            object_id,
            "{command} dry run test failed, gas object used is not the expected one"
        );
    } else {
        panic!("{command} dry run failed");
    }
}

#[sim_test]
async fn test_dry_run() -> Result<(), anyhow::Error> {
    let mut test_cluster = TestClusterBuilder::new()
        .with_num_validators(2)
        .build()
        .await;
    let rgp = test_cluster.get_reference_gas_price().await;
    let address = test_cluster.get_address_0();
    let context = &mut test_cluster.wallet;
    let client = context.get_client().await?;
    let object_refs = client
        .read_api()
        .get_owned_objects(
            address,
            Some(IotaObjectResponseQuery::new_with_options(
                IotaObjectDataOptions::full_content(),
            )),
            None,
            None,
        )
        .await?;

    let object_id = object_refs
        .data
        .first()
        .unwrap()
        .object()
        .unwrap()
        .object_id;
    let object_to_send = object_refs.data.get(1).unwrap().object().unwrap().object_id;

    // === TRANSFER === //
    let transfer_dry_run = IotaClientCommands::Transfer {
        to: KeyIdentity::Address(IotaAddress::random_for_testing_only()),
        object_id: object_to_send,
        payment: PaymentArgs {
            gas: vec![object_id],
        },
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER),
            ..Default::default()
        },
        processing: TxProcessingArgs {
            dry_run: true,
            ..Default::default()
        },
    }
    .execute(context)
    .await?;

    assert_dry_run(transfer_dry_run, object_id, "Transfer");

    // === PAY === //
    let pay_dry_run = IotaClientCommands::Pay {
        input_coins: vec![object_id],
        recipients: vec![KeyIdentity::Address(IotaAddress::random_for_testing_only())],
        amounts: vec![1],
        payment: PaymentArgs::default(),
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER),
            ..Default::default()
        },
        processing: TxProcessingArgs {
            dry_run: true,
            ..Default::default()
        },
    }
    .execute(context)
    .await?;

    if let IotaClientCommandResult::DryRun(response) = pay_dry_run {
        assert_eq!(*response.effects.status(), IotaExecutionStatus::Success);
        assert_ne!(response.effects.gas_object().object_id(), object_id);
    } else {
        panic!("Pay dry run failed");
    }

    // specify which gas object to use
    let gas_coin_id = object_refs.data.last().unwrap().object().unwrap().object_id;
    let pay_dry_run = IotaClientCommands::Pay {
        input_coins: vec![object_id],
        recipients: vec![KeyIdentity::Address(IotaAddress::random_for_testing_only())],
        amounts: vec![1],
        payment: PaymentArgs {
            gas: vec![gas_coin_id],
        },
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER),
            ..Default::default()
        },
        processing: TxProcessingArgs {
            dry_run: true,
            ..Default::default()
        },
    }
    .execute(context)
    .await?;

    assert_dry_run(pay_dry_run, gas_coin_id, "Pay");

    // === PAY IOTA === //
    let pay_iota_dry_run = IotaClientCommands::PayIota {
        input_coins: Some(vec![object_id]),
        recipients: vec![KeyIdentity::Address(IotaAddress::random_for_testing_only())],
        amounts: vec![1],
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER),
            ..Default::default()
        },
        processing: TxProcessingArgs {
            dry_run: true,
            ..Default::default()
        },
    }
    .execute(context)
    .await?;

    assert_dry_run(pay_iota_dry_run, object_id, "PayIota");

    // === PAY ALL IOTA === //
    let pay_all_iota_dry_run = IotaClientCommands::PayAllIota {
        input_coins: vec![object_id],
        recipient: KeyIdentity::Address(IotaAddress::random_for_testing_only()),
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER),
            ..Default::default()
        },
        processing: TxProcessingArgs {
            dry_run: true,
            ..Default::default()
        },
    }
    .execute(context)
    .await?;

    assert_dry_run(pay_all_iota_dry_run, object_id, "PayAllIota");

    Ok(())
}

async fn test_cluster_helper() -> (
    TestCluster,
    IotaClient,
    u64,
    [ObjectID; 3],
    [KeyIdentity; 2],
    [IotaAddress; 2],
) {
    let mut test_cluster = TestClusterBuilder::new()
        .with_num_validators(2)
        .build()
        .await;
    let rgp = test_cluster.get_reference_gas_price().await;
    let address1 = test_cluster.get_address_0();
    let context = &mut test_cluster.wallet;
    let client = context.get_client().await.unwrap();
    let object_refs = client
        .read_api()
        .get_owned_objects(
            address1,
            Some(IotaObjectResponseQuery::new_with_options(
                IotaObjectDataOptions::full_content(),
            )),
            None,
            None,
        )
        .await
        .unwrap();

    let object_id1 = object_refs
        .data
        .first()
        .unwrap()
        .object()
        .unwrap()
        .object_id;
    let object_id2 = object_refs.data.get(1).unwrap().object().unwrap().object_id;
    let object_id3 = object_refs.data.get(2).unwrap().object().unwrap().object_id;
    let address2 = IotaAddress::random_for_testing_only();
    let address3 = IotaAddress::random_for_testing_only();
    let recipient1 = KeyIdentity::Address(address2);
    let recipient2 = KeyIdentity::Address(address3);

    (
        test_cluster,
        client,
        rgp,
        [object_id1, object_id2, object_id3],
        [recipient1, recipient2],
        [address2, address3],
    )
}

#[sim_test]
async fn test_pay() -> Result<(), anyhow::Error> {
    let (mut test_cluster, client, rgp, objects, recipients, addresses) =
        test_cluster_helper().await;
    let (object_id1, object_id2, object_id3) = (objects[0], objects[1], objects[2]);
    let (recipient1, recipient2) = (&recipients[0], &recipients[1]);
    let (address2, address3) = (addresses[0], addresses[1]);
    let context = &mut test_cluster.wallet;
    let pay = IotaClientCommands::Pay {
        input_coins: vec![object_id1, object_id2],
        recipients: vec![recipient1.clone(), recipient2.clone()],
        amounts: vec![5000, 10000],
        payment: PaymentArgs {
            gas: vec![object_id1],
        },
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await;

    // we passed the gas object to be one of the input coins, which should fail
    assert!(pay.is_err());

    let amounts = [5000, 10000];
    // we expect this to be the gas coin used
    let pay = IotaClientCommands::Pay {
        input_coins: vec![object_id1, object_id2],
        recipients: vec![recipient1.clone(), recipient2.clone()],
        amounts: amounts.into(),
        payment: PaymentArgs::default(),
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;

    // Pay command takes the input coins and transfers the given amounts from each
    // input coin (in order) to the recipients
    // this test checks if the recipients have received the objects, and if the gas
    // object used is the right one (not one of the input coins, and in this
    // setup it's the 3rd coin of sender) we also check if the balances are
    // right!
    if let IotaClientCommandResult::TransactionBlock(response) = pay {
        // check tx status
        assert!(response.status_ok().unwrap());
        // check gas coin used
        assert_eq!(
            response.effects.as_ref().unwrap().gas_object().object_id(),
            object_id3
        );
        let objs_refs = client
            .read_api()
            .get_owned_objects(
                address2,
                Some(IotaObjectResponseQuery::new_with_options(
                    IotaObjectDataOptions::full_content(),
                )),
                None,
                None,
            )
            .await?;
        assert!(!objs_refs.has_next_page);
        assert_eq!(objs_refs.data.len(), 1);
        assert_eq!(
            client
                .coin_read_api()
                .get_balance(address2, None)
                .await?
                .total_balance,
            amounts[0] as u128
        );
        let objs_refs = client
            .read_api()
            .get_owned_objects(
                address3,
                Some(IotaObjectResponseQuery::new_with_options(
                    IotaObjectDataOptions::full_content(),
                )),
                None,
                None,
            )
            .await?;
        assert!(response.status_ok().unwrap());
        assert!(!objs_refs.has_next_page);
        assert_eq!(objs_refs.data.len(), 1);
        assert_eq!(
            client
                .coin_read_api()
                .get_balance(address3, None)
                .await?
                .total_balance,
            amounts[1] as u128
        );
    } else {
        panic!("Pay test failed");
    }

    Ok(())
}

#[sim_test]
async fn test_pay_iota() -> Result<(), anyhow::Error> {
    let (mut test_cluster, client, rgp, objects, recipients, addresses) =
        test_cluster_helper().await;
    let (object_id1, object_id2) = (objects[0], objects[1]);
    let (recipient1, recipient2) = (&recipients[0], &recipients[1]);
    let (address2, address3) = (addresses[0], addresses[1]);
    let context = &mut test_cluster.wallet;
    let amounts = [1000, 5000];
    let pay_iota = IotaClientCommands::PayIota {
        input_coins: Some(vec![object_id1, object_id2]),
        recipients: vec![recipient1.clone(), recipient2.clone()],
        amounts: amounts.into(),
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;

    // pay iota takes the input coins and transfers from each of them (in order) the
    // amounts to the respective recipients.
    // check if each recipient has one object, if the tx status is success,
    // and if the gas object used was the first object in the input coins
    // we also check if the balances of each recipient are right!
    if let IotaClientCommandResult::TransactionBlock(response) = pay_iota {
        assert!(response.status_ok().unwrap());
        // check gas coin used
        assert_eq!(
            response.effects.as_ref().unwrap().gas_object().object_id(),
            object_id1
        );
        let objs_refs = client
            .read_api()
            .get_owned_objects(
                address2,
                Some(IotaObjectResponseQuery::new_with_options(
                    IotaObjectDataOptions::full_content(),
                )),
                None,
                None,
            )
            .await?;
        assert!(!objs_refs.has_next_page);
        assert_eq!(objs_refs.data.len(), 1);
        assert_eq!(
            client
                .coin_read_api()
                .get_balance(address2, None)
                .await?
                .total_balance,
            amounts[0] as u128
        );
        let objs_refs = client
            .read_api()
            .get_owned_objects(
                address3,
                Some(IotaObjectResponseQuery::new_with_options(
                    IotaObjectDataOptions::full_content(),
                )),
                None,
                None,
            )
            .await?;
        assert!(response.status_ok().unwrap());
        assert!(!objs_refs.has_next_page);
        assert_eq!(objs_refs.data.len(), 1);
        assert_eq!(
            client
                .coin_read_api()
                .get_balance(address3, None)
                .await?
                .total_balance,
            amounts[1] as u128
        );
    } else {
        panic!("PayIota test failed");
    }
    Ok(())
}

#[sim_test]
async fn test_pay_all_iota() -> Result<(), anyhow::Error> {
    let (mut test_cluster, client, rgp, objects, recipients, addresses) =
        test_cluster_helper().await;
    let (object_id1, object_id2) = (objects[0], objects[1]);
    let recipient1 = &recipients[0];
    let address2 = addresses[0];
    let context = &mut test_cluster.wallet;
    let pay_all_iota = IotaClientCommands::PayAllIota {
        input_coins: vec![object_id1, object_id2],
        recipient: recipient1.clone(),
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;

    // pay all iota will take the input coins and smash them into one coin and
    // transfer that coin to the recipient, so we check that the recipient has
    // one object, if the tx status is success, and if the gas object used was
    // the first object in the input coins
    if let IotaClientCommandResult::TransactionBlock(response) = pay_all_iota {
        let objs_refs = client
            .read_api()
            .get_owned_objects(
                address2,
                Some(IotaObjectResponseQuery::new_with_options(
                    IotaObjectDataOptions::full_content(),
                )),
                None,
                None,
            )
            .await?;
        assert!(response.status_ok().unwrap());
        assert!(!objs_refs.has_next_page);
        assert_eq!(objs_refs.data.len(), 1);
        assert_eq!(
            response.effects.unwrap().gas_object().object_id(),
            object_id1
        );
    } else {
        panic!("PayAllIota test failed");
    }

    Ok(())
}

#[sim_test]
async fn test_transfer() -> Result<(), anyhow::Error> {
    let (mut test_cluster, client, rgp, objects, recipients, addresses) =
        test_cluster_helper().await;
    let (object_id1, object_id2) = (objects[0], objects[1]);
    let recipient1 = &recipients[0];
    let address2 = addresses[0];
    let context = &mut test_cluster.wallet;
    let transfer = IotaClientCommands::Transfer {
        to: KeyIdentity::Address(address2),
        object_id: object_id1,
        payment: PaymentArgs {
            gas: vec![object_id1],
        },
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await;

    // passed the gas object to be the object to transfer, which should fail
    assert!(transfer.is_err());

    let transfer = IotaClientCommands::Transfer {
        to: recipient1.clone(),
        object_id: object_id1,
        payment: PaymentArgs::default(),
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;
    // transfer command will transfer the object_id1 to address2, and use object_id2
    // as gas we check if object1 is owned by address 2 and if the gas object
    // used is object_id2
    if let IotaClientCommandResult::TransactionBlock(response) = transfer {
        assert!(response.status_ok().unwrap());
        assert_eq!(
            response.effects.as_ref().unwrap().gas_object().object_id(),
            object_id2
        );
        let objs_refs = client
            .read_api()
            .get_owned_objects(
                address2,
                Some(IotaObjectResponseQuery::new_with_options(
                    IotaObjectDataOptions::full_content(),
                )),
                None,
                None,
            )
            .await?;
        assert!(!objs_refs.has_next_page);
        assert_eq!(objs_refs.data.len(), 1);
        assert_eq!(
            objs_refs.data.first().unwrap().object().unwrap().object_id,
            object_id1
        );
    } else {
        panic!("Transfer test failed");
    }
    Ok(())
}

#[sim_test]
async fn test_transfer_gas_smash() -> Result<(), anyhow::Error> {
    // Like `test_transfer` but using multiple gas objects.
    let (mut test_cluster, client, rgp, objects, recipients, addresses) =
        test_cluster_helper().await;
    let (object_id0, object_id1, object_id2) = (objects[0], objects[1], objects[2]);
    let recipient1 = &recipients[0];
    let address2 = addresses[0];
    let context = &mut test_cluster.wallet;
    let transfer = IotaClientCommands::Transfer {
        to: KeyIdentity::Address(address2),
        object_id: object_id1,
        payment: PaymentArgs {
            gas: vec![object_id0, object_id1],
        },
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await;

    // Overlap between the object being transferred and the gas objects should fail.
    assert!(transfer.is_err());

    let transfer = IotaClientCommands::Transfer {
        to: recipient1.clone(),
        object_id: object_id2,
        payment: PaymentArgs {
            gas: vec![object_id0, object_id1],
        },
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;

    // transfer command will transfer the object_id2 to address2, and use
    // object_id0, and object_id1 as gas we check if object1 is owned by address
    // 2 and the gas object used.
    let IotaClientCommandResult::TransactionBlock(response) = transfer else {
        panic!("Transfer test failed");
    };

    assert!(response.status_ok().unwrap());
    assert_eq!(
        response.effects.as_ref().unwrap().gas_object().object_id(),
        object_id0
    );
    let objs_refs = client
        .read_api()
        .get_owned_objects(
            address2,
            Some(IotaObjectResponseQuery::new_with_options(
                IotaObjectDataOptions::full_content(),
            )),
            None,
            None,
        )
        .await?;
    assert!(!objs_refs.has_next_page);
    assert_eq!(objs_refs.data.len(), 1);
    assert_eq!(
        objs_refs.data.first().unwrap().object().unwrap().object_id,
        object_id2
    );

    Ok(())
}

#[sim_test]
async fn test_transfer_sponsored() -> Result<(), anyhow::Error> {
    // Like `test_transfer` but the gas is sponsored by the recipient.
    let (mut cluster, _, rgp, o, _, _) = test_cluster_helper().await;
    let a0 = cluster.get_address_0();
    let a1 = cluster.get_address_1();
    let context = &mut cluster.wallet;

    // A0 sends O1 to A1
    let transfer = IotaClientCommands::Transfer {
        to: KeyIdentity::Address(a1),
        object_id: o[1],
        payment: PaymentArgs::default(),
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;

    let IotaClientCommandResult::TransactionBlock(response) = transfer else {
        panic!("Failed to set-up test")
    };

    assert_eq!(response.status_ok(), Some(true));

    // A1 sends 01 back to A0, but sponsored by A0.
    let transfer_back = IotaClientCommands::Transfer {
        to: KeyIdentity::Address(a0),
        object_id: o[1],
        payment: PaymentArgs::default(),
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER),
            gas_sponsor: Some(a0),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;

    let IotaClientCommandResult::TransactionBlock(response) = transfer_back else {
        panic!("Failed to run sponsored transfer")
    };

    let Some(tx) = &response.transaction else {
        panic!("TransactionBlock response should contain a transaction");
    };

    assert_eq!(response.status_ok(), Some(true));
    assert_eq!(tx.data.gas_data().owner, a0);
    assert_eq!(tx.data.sender(), &a1);

    Ok(())
}

#[sim_test]
async fn test_transfer_serialized_data() -> Result<(), anyhow::Error> {
    // Like `test_transfer` but the transaction is pre-generated and serialized into
    // a Base64 string containing a Base64-encoded TransactionData.
    let (mut cluster, client, rgp, o, _, a) = test_cluster_helper().await;
    let context = &mut cluster.wallet;

    // Build the transaction without running it.
    let transfer = IotaClientCommands::Transfer {
        to: KeyIdentity::Address(a[1]),
        object_id: o[0],
        payment: PaymentArgs { gas: vec![o[1]] },
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER),
            ..Default::default()
        },
        processing: TxProcessingArgs {
            serialize_unsigned_transaction: true,
            ..Default::default()
        },
    }
    .execute(context)
    .await?;

    let IotaClientCommandResult::SerializedUnsignedTransaction(tx_data) = transfer else {
        panic!("Expected SerializedUnsignedTransaction result");
    };

    let tx_bytes = Base64::encode(bcs::to_bytes(&tx_data)?);
    let transfer_serialized = IotaClientCommands::SerializedTx {
        tx_bytes,
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;

    let IotaClientCommandResult::TransactionBlock(response) = transfer_serialized else {
        panic!("Expected TransactionBlock result");
    };

    let Some(effects) = &response.effects else {
        panic!("TransactionBlock response should contain effects");
    };

    assert!(effects.status().is_ok());
    assert_eq!(effects.gas_object().object_id(), o[1]);

    let a1_objs = client
        .read_api()
        .get_owned_objects(a[1], None, None, None)
        .await?;

    assert!(!a1_objs.has_next_page);

    let page = a1_objs.data;
    assert_eq!(page.len(), 1);
    assert_eq!(page.first().unwrap().object().unwrap().object_id, o[0]);

    Ok(())
}

#[sim_test]
async fn test_transfer_serialized_kind() -> Result<(), anyhow::Error> {
    // Like `test_transfer` but the transaction is pre-generated and serialized into
    // a Base64 string containing a Base64-encoded TransactionKind.
    let (mut cluster, client, rgp, o, _, a) = test_cluster_helper().await;
    let context = &mut cluster.wallet;

    // Build the transaction without running it.
    let transfer = IotaClientCommands::Transfer {
        to: KeyIdentity::Address(a[1]),
        object_id: o[0],
        payment: PaymentArgs::default(),
        gas_data: GasDataArgs::default(),
        processing: TxProcessingArgs {
            serialize_unsigned_transaction: true,
            ..Default::default()
        },
    }
    .execute(context)
    .await?;

    let IotaClientCommandResult::SerializedUnsignedTransaction(tx_data) = transfer else {
        panic!("Expected SerializedUnsignedTransaction result");
    };

    let tx_bytes = Base64::encode(bcs::to_bytes(tx_data.kind())?);
    let transfer_serialized = IotaClientCommands::SerializedTxKind {
        tx_bytes,
        payment: PaymentArgs { gas: vec![o[1]] },
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;

    let IotaClientCommandResult::TransactionBlock(response) = transfer_serialized else {
        panic!("Expected TransactionBlock result");
    };

    let Some(effects) = &response.effects else {
        panic!("TransactionBlock response should contain effects");
    };

    assert!(effects.status().is_ok());
    assert_eq!(effects.gas_object().object_id(), o[1]);

    let a1_objs = client
        .read_api()
        .get_owned_objects(a[1], None, None, None)
        .await?;

    assert!(!a1_objs.has_next_page);

    let page = a1_objs.data;
    assert_eq!(page.len(), 1);
    assert_eq!(page.first().unwrap().object().unwrap().object_id, o[0]);

    Ok(())
}

#[sim_test]
async fn test_gas_estimation() -> Result<(), anyhow::Error> {
    let (mut test_cluster, client, rgp, objects, _, addresses) = test_cluster_helper().await;
    let object_id1 = objects[0];
    let address2 = addresses[0];
    let context = &mut test_cluster.wallet;
    let amount = 1000;
    let sender = context.active_address().unwrap();
    let tx_builder = client.transaction_builder();
    let tx_kind = tx_builder.transfer_iota_tx_kind(address2, Some(amount));
    let gas_estimate = estimate_gas_budget(context, sender, tx_kind, rgp, vec![], None).await;
    assert!(gas_estimate.is_ok());

    let pay_iota_cmd = IotaClientCommands::PayIota {
        recipients: vec![KeyIdentity::Address(address2)],
        input_coins: Some(vec![object_id1]),
        amounts: vec![amount],
        gas_data: GasDataArgs::default(),
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await
    .unwrap();
    if let IotaClientCommandResult::TransactionBlock(response) = pay_iota_cmd {
        assert!(response.status_ok().unwrap());
        let gas_used = response.effects.as_ref().unwrap().gas_object().object_id();
        assert_eq!(gas_used, object_id1);
        assert!(
            response
                .effects
                .as_ref()
                .unwrap()
                .gas_cost_summary()
                .gas_used()
                <= gas_estimate.unwrap()
        );
    } else {
        panic!("PayIota failed in gas estimation test");
    }
    Ok(())
}

#[sim_test]
async fn test_clever_errors() -> Result<(), anyhow::Error> {
    // Publish the package
    move_package::package_hooks::register_package_hooks(Box::new(IotaPackageHooks));
    let mut test_cluster = TestClusterBuilder::new()
        .with_num_validators(2)
        .build()
        .await;
    let rgp = test_cluster.get_reference_gas_price().await;
    let address = test_cluster.get_address_0();
    let context = &mut test_cluster.wallet;
    let client = context.get_client().await?;
    let object_refs = client
        .read_api()
        .get_owned_objects(
            address,
            Some(IotaObjectResponseQuery::new_with_options(
                IotaObjectDataOptions::new()
                    .with_type()
                    .with_owner()
                    .with_previous_transaction(),
            )),
            None,
            None,
        )
        .await?
        .data;

    // Check log output contains all object ids.
    let gas_obj_id = object_refs.first().unwrap().object().unwrap().object_id;

    // Provide path to well formed package sources
    let mut package_path = PathBuf::from(TEST_DATA_DIR);
    package_path.push("clever_errors");
    let build_config = BuildConfig::new_for_testing().config;
    let resp = IotaClientCommands::Publish {
        package_path: package_path.clone(),
        build_config,
        skip_dependency_verification: false,
        verify_deps: true,
        with_unpublished_dependencies: false,
        payment: PaymentArgs {
            gas: vec![gas_obj_id],
        },
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_PUBLISH),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;

    // Print it out to CLI/logs
    resp.print(true);

    let IotaClientCommandResult::TransactionBlock(response) = resp else {
        unreachable!("Invalid response");
    };

    let IotaTransactionBlockEffects::V1(effects) = response.effects.unwrap();

    assert!(effects.status.is_ok());
    assert_eq!(effects.gas_object().object_id(), gas_obj_id);
    let package = effects
        .created()
        .iter()
        .find(|refe| matches!(refe.owner, Owner::Immutable))
        .unwrap();

    let elide_transaction_digest = |s: String| -> String {
        let mut x = s.splitn(5, '\'').collect::<Vec<_>>();
        x[1] = "ELIDED_TRANSACTION_DIGEST";
        let tmp = format!("ELIDED_ADDRESS{}", &x[3][66..]);
        x[3] = &tmp;
        x.join("'")
    };

    // Normal abort
    let non_clever_abort = IotaClientCommands::Call {
        package: package.reference.object_id,
        module: "clever_errors".to_string(),
        function: "aborter".to_string(),
        type_args: vec![],
        args: vec![],
        payment: PaymentArgs::default(),
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_PUBLISH),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await
    .unwrap_err();

    // Line-only abort
    let line_only_abort = IotaClientCommands::Call {
        package: package.reference.object_id,
        module: "clever_errors".to_string(),
        function: "aborter_line_no".to_string(),
        type_args: vec![],
        args: vec![],
        payment: PaymentArgs::default(),
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_PUBLISH),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await
    .unwrap_err();

    // Full clever error with utf-8 string
    let clever_error_utf8 = IotaClientCommands::Call {
        package: package.reference.object_id,
        module: "clever_errors".to_string(),
        function: "clever_aborter".to_string(),
        type_args: vec![],
        args: vec![],
        payment: PaymentArgs::default(),
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_PUBLISH),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await
    .unwrap_err();

    // Full clever error with non-utf-8 string
    let clever_error_non_utf8 = IotaClientCommands::Call {
        package: package.reference.object_id,
        module: "clever_errors".to_string(),
        function: "clever_aborter_not_a_string".to_string(),
        type_args: vec![],
        args: vec![],
        payment: PaymentArgs::default(),
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_PUBLISH),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await
    .unwrap_err();

    let error_string = format!(
        "Non-clever-abort\n---\n{}\n---\nLine-only-abort\n---\n{}\n---\nClever-error-utf8\n---\n{}\n---\nClever-error-non-utf8\n---\n{}\n---\n",
        elide_transaction_digest(non_clever_abort.to_string()),
        elide_transaction_digest(line_only_abort.to_string()),
        elide_transaction_digest(clever_error_utf8.to_string()),
        elide_transaction_digest(clever_error_non_utf8.to_string())
    );

    insta::assert_snapshot!(error_string);
    Ok(())
}

#[sim_test]
async fn test_balance() -> Result<(), anyhow::Error> {
    let mut test_cluster = TestClusterBuilder::new()
        .with_num_validators(2)
        .build()
        .await;

    let context = &mut test_cluster.wallet;

    let balance_result = IotaClientCommands::Balance {
        address: None,
        coin_type: None,
        with_coins: false,
    }
    .execute(context)
    .await?;

    if let IotaClientCommandResult::Balance(ordered_coins_iota_first, with_coins) = balance_result {
        // The address has by default one coin object
        assert_eq!(ordered_coins_iota_first.len(), 1);
        assert!(!with_coins, "response should be without coins");
    } else {
        unreachable!("Invalid response");
    }

    Ok(())
}

#[sim_test]
async fn test_faucet() -> Result<(), anyhow::Error> {
    let test_cluster = TestClusterBuilder::new()
        .with_fullnode_rpc_port(9000)
        .build()
        .await;

    let context = test_cluster.wallet;

    let tmp = tempfile::tempdir().unwrap();
    let prom_registry = prometheus::Registry::new();
    let config = iota_faucet::FaucetConfig::default();

    let prometheus_registry = prometheus::Registry::new();
    let app_state = std::sync::Arc::new(iota_faucet::AppState {
        faucet: iota_faucet::SimpleFaucet::new(
            context,
            &prometheus_registry,
            &tmp.path().join("faucet.wal"),
            config.clone(),
        )
        .await
        .unwrap(),
        config,
    });
    tokio::spawn(async move { iota_faucet::start_faucet(app_state, 10, &prom_registry).await });

    // Wait for the faucet to be up
    sleep(Duration::from_secs(1)).await;
    let wallet_config = test_cluster.swarm.dir().join(IOTA_CLIENT_CONFIG);
    let mut context = WalletContext::new(&wallet_config)?;

    let (address, _): (_, AccountKeyPair) = get_key_pair();

    let faucet_result = IotaClientCommands::Faucet {
        address: Some(KeyIdentity::Address(address)),
        url: Some("http://127.0.0.1:5003/gas".to_string()),
    }
    .execute(&mut context)
    .await?;

    if let IotaClientCommandResult::NoOutput = faucet_result {
    } else {
        unreachable!("Invalid response");
    };

    sleep(Duration::from_secs(5)).await;

    let gas_objects_after = context
        .get_gas_objects_owned_by_address(address, None)
        .await
        .unwrap()
        .len();
    assert_eq!(gas_objects_after, 1);

    Ok(())
}

#[sim_test]
async fn test_faucet_batch() -> Result<(), anyhow::Error> {
    let test_cluster = TestClusterBuilder::new()
        .with_fullnode_rpc_port(9000)
        .build()
        .await;

    let context = test_cluster.wallet;

    let tmp = tempfile::tempdir().unwrap();
    let prom_registry = prometheus::Registry::new();
    let config = iota_faucet::FaucetConfig {
        batch_enabled: true,
        ..Default::default()
    };

    let prometheus_registry = prometheus::Registry::new();
    let app_state = std::sync::Arc::new(iota_faucet::AppState {
        faucet: iota_faucet::SimpleFaucet::new(
            context,
            &prometheus_registry,
            &tmp.path().join("faucet.wal"),
            config.clone(),
        )
        .await
        .unwrap(),
        config,
    });
    tokio::spawn(async move { iota_faucet::start_faucet(app_state, 10, &prom_registry).await });

    // Wait for the faucet to be up
    sleep(Duration::from_secs(1)).await;
    let wallet_config = test_cluster.swarm.dir().join(IOTA_CLIENT_CONFIG);
    let mut context = WalletContext::new(&wallet_config)?;

    let (address_1, _): (_, AccountKeyPair) = get_key_pair();
    let (address_2, _): (_, AccountKeyPair) = get_key_pair();
    let (address_3, _): (_, AccountKeyPair) = get_key_pair();

    assert_ne!(address_1, address_2);
    assert_ne!(address_1, address_3);
    assert_ne!(address_2, address_3);

    for address in [address_1, address_2, address_3].iter() {
        let gas_objects_after = context
            .get_gas_objects_owned_by_address(*address, None)
            .await
            .unwrap()
            .len();
        assert_eq!(gas_objects_after, 0);
    }

    for address in [address_1, address_2, address_3].iter() {
        let faucet_result = IotaClientCommands::Faucet {
            address: Some(KeyIdentity::Address(*address)),
            url: Some("http://127.0.0.1:5003/v1/gas".to_string()),
        }
        .execute(&mut context)
        .await?;

        if let IotaClientCommandResult::NoOutput = faucet_result {
        } else {
            unreachable!("Invalid response");
        };
    }

    // we need to wait a minimum of 10 seconds for gathering the batch + some time
    // for transaction to be sequenced
    sleep(Duration::from_secs(15)).await;

    for address in [address_1, address_2, address_3].iter() {
        let gas_objects_after = context
            .get_gas_objects_owned_by_address(*address, None)
            .await
            .unwrap()
            .len();
        assert_eq!(gas_objects_after, 1);
    }

    // try with a new batch
    let (address_4, _): (_, AccountKeyPair) = get_key_pair();
    let (address_5, _): (_, AccountKeyPair) = get_key_pair();
    let (address_6, _): (_, AccountKeyPair) = get_key_pair();

    assert_ne!(address_4, address_5);
    assert_ne!(address_4, address_6);
    assert_ne!(address_5, address_6);

    for address in [address_4, address_5, address_6].iter() {
        let gas_objects_after = context
            .get_gas_objects_owned_by_address(*address, None)
            .await
            .unwrap()
            .len();
        assert_eq!(gas_objects_after, 0);
    }

    for address in [address_4, address_5, address_6].iter() {
        let faucet_result = IotaClientCommands::Faucet {
            address: Some(KeyIdentity::Address(*address)),
            url: Some("http://127.0.0.1:5003/v1/gas".to_string()),
        }
        .execute(&mut context)
        .await?;

        if let IotaClientCommandResult::NoOutput = faucet_result {
        } else {
            unreachable!("Invalid response");
        };
    }

    // we need to wait a minimum of 10 seconds for gathering the batch + some time
    // for transaction to be sequenced
    sleep(Duration::from_secs(15)).await;

    for address in [address_4, address_5, address_6].iter() {
        let gas_objects_after = context
            .get_gas_objects_owned_by_address(*address, None)
            .await
            .unwrap()
            .len();
        assert_eq!(gas_objects_after, 1);
    }

    Ok(())
}

#[sim_test]
async fn test_faucet_batch_concurrent_requests() -> Result<(), anyhow::Error> {
    let test_cluster = TestClusterBuilder::new()
        .with_fullnode_rpc_port(9000)
        .build()
        .await;

    let context = test_cluster.wallet;

    let tmp = tempfile::tempdir().unwrap();
    let prom_registry = prometheus::Registry::new();
    let config = iota_faucet::FaucetConfig {
        batch_enabled: true,
        ..Default::default()
    };

    let prometheus_registry = prometheus::Registry::new();
    let app_state = std::sync::Arc::new(iota_faucet::AppState {
        faucet: iota_faucet::SimpleFaucet::new(
            context,
            &prometheus_registry,
            &tmp.path().join("faucet.wal"),
            config.clone(),
        )
        .await
        .unwrap(),
        config,
    });
    tokio::spawn(async move { iota_faucet::start_faucet(app_state, 10, &prom_registry).await });

    // Wait for the faucet to be up
    sleep(Duration::from_secs(1)).await;

    let wallet_config = test_cluster.swarm.dir().join(IOTA_CLIENT_CONFIG);
    let context = WalletContext::new(&wallet_config)?; // Use immutable context

    // Generate multiple addresses
    let addresses: Vec<_> = (0..6)
        .map(|_| get_key_pair::<AccountKeyPair>().0)
        .collect::<Vec<IotaAddress>>();

    // Ensure all addresses have zero gas objects initially
    for address in &addresses {
        assert_eq!(
            context
                .get_gas_objects_owned_by_address(*address, None)
                .await
                .unwrap()
                .len(),
            0
        );
    }

    // First batch: send faucet requests concurrently for all addresses
    let first_batch_results: Vec<_> = futures::future::join_all(addresses.iter().map(|address| {
        let wallet_config = wallet_config.clone();
        async move {
            let mut context = WalletContext::new(&wallet_config)?; // Use mutable context (for faucet requests)
            IotaClientCommands::Faucet {
                address: Some(KeyIdentity::Address(*address)),
                url: Some("http://127.0.0.1:5003/v1/gas".to_string()),
            }
            .execute(&mut context)
            .await
        }
    }))
    .await;

    // Ensure all results are `NoOutput` indicating requests were batched
    for result in first_batch_results {
        assert!(matches!(result, Ok(IotaClientCommandResult::NoOutput)));
    }

    // Wait for the first batch to complete
    sleep(Duration::from_secs(15)).await;

    // Validate gas objects after the first batch
    for address in &addresses {
        assert_eq!(
            context
                .get_gas_objects_owned_by_address(*address, None)
                .await
                .unwrap()
                .len(),
            1
        );
    }

    // Second batch: send faucet requests again for all addresses
    let second_batch_results: Vec<_> = futures::future::join_all(addresses.iter().map(|address| {
        let wallet_config = wallet_config.clone();
        async move {
            let mut context = WalletContext::new(&wallet_config)?; // Use mutable context
            IotaClientCommands::Faucet {
                address: Some(KeyIdentity::Address(*address)),
                url: Some("http://127.0.0.1:5003/v1/gas".to_string()),
            }
            .execute(&mut context)
            .await
        }
    }))
    .await;

    // Ensure all results are `NoOutput` for the second batch
    for result in second_batch_results {
        assert!(matches!(result, Ok(IotaClientCommandResult::NoOutput)));
    }

    // Wait for the second batch to complete
    sleep(Duration::from_secs(15)).await;

    // Validate gas objects after the second batch
    for address in &addresses {
        assert_eq!(
            context
                .get_gas_objects_owned_by_address(*address, None)
                .await
                .unwrap()
                .len(),
            2
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_move_new() -> Result<(), anyhow::Error> {
    let current_dir = std::env::current_dir()?;
    let package_name = "test_move_new";
    IotaCommand::Move {
        package_path: None,
        config: IotaEnvConfig {
            config: None,
            env: None,
        },
        build_config: move_package::BuildConfig::default(),
        cmd: iota_move::Command::New(iota_move::new::New {
            new: move_cli::base::new::New {
                name: package_name.to_string(),
            },
        }),
    }
    .execute()
    .await?;

    // Get all the new file names
    let files = read_dir(package_name)?
        .flat_map(|r| r.map(|file| file.file_name().to_str().unwrap().to_owned()))
        .collect::<Vec<_>>();

    assert_eq!(4, files.len());
    for name in ["sources", "tests", "Move.toml", ".gitignore"] {
        assert!(files.contains(&name.to_string()));
    }
    assert!(std::path::Path::new(&format!("{package_name}/sources/{package_name}.move")).exists());
    assert!(
        std::path::Path::new(&format!("{package_name}/tests/{package_name}_tests.move")).exists()
    );

    // Test if the generated files are valid to build a package
    IotaCommand::Move {
        package_path: Some(package_name.parse()?),
        config: IotaEnvConfig {
            config: None,
            env: None,
        },
        build_config: move_package::BuildConfig::default(),
        cmd: iota_move::Command::Build(iota_move::build::Build {
            chain_id: None,
            ignore_chain: false,
            dump_bytecode_as_base64: false,
            generate_struct_layouts: false,
            with_unpublished_dependencies: false,
        }),
    }
    .execute()
    .await?;

    // iota_move::Command::Build changes the current dir, so we have to switch back
    // here
    std::env::set_current_dir(&current_dir)?;

    IotaCommand::Move {
        package_path: Some(package_name.parse()?),
        config: IotaEnvConfig {
            config: None,
            env: None,
        },
        build_config: move_package::BuildConfig::default(),
        cmd: iota_move::Command::Test(iota_move::unit_test::Test {
            test: move_cli::base::test::Test {
                compute_coverage: false,
                filter: None,
                gas_limit: None,
                list: false,
                num_threads: 1,
                report_statistics: None,
                verbose_mode: false,
                seed: None,
                rand_num_iters: None,
                trace_execution: false,
            },
        }),
    }
    .execute()
    .await?;

    // iota_move::Command::Test changes the current dir, so we have to switch back
    // here
    std::env::set_current_dir(current_dir)?;
    std::fs::remove_dir_all(package_name)?;
    Ok(())
}

#[sim_test]
async fn test_call_command_display_args() -> Result<(), anyhow::Error> {
    // Publish the package
    move_package::package_hooks::register_package_hooks(Box::new(IotaPackageHooks));
    let mut test_cluster = TestClusterBuilder::new()
        .with_num_validators(2)
        .build()
        .await;
    let rgp = test_cluster.get_reference_gas_price().await;
    let address = test_cluster.get_address_0();
    let context = &mut test_cluster.wallet;
    let client = context.get_client().await?;
    let object_refs = client
        .read_api()
        .get_owned_objects(
            address,
            Some(IotaObjectResponseQuery::new_with_options(
                IotaObjectDataOptions::new()
                    .with_type()
                    .with_owner()
                    .with_previous_transaction(),
            )),
            None,
            None,
        )
        .await?
        .data;

    let gas_obj_id = object_refs.first().unwrap().object().unwrap().object_id;

    // Provide path to well formed package sources
    let mut package_path = PathBuf::from(TEST_DATA_DIR);
    package_path.push("dummy_modules_upgrade");
    let build_config = BuildConfig::new_for_testing().config;
    let resp = IotaClientCommands::Publish {
        package_path: package_path.clone(),
        build_config,
        skip_dependency_verification: false,
        verify_deps: false,
        with_unpublished_dependencies: false,
        payment: PaymentArgs {
            gas: vec![gas_obj_id],
        },
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_PUBLISH),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;

    let effects = match resp {
        IotaClientCommandResult::TransactionBlock(response) => response.effects.unwrap(),
        _ => panic!("Expected TransactionBlock response"),
    };

    let package = effects
        .created()
        .iter()
        .find(|refe| matches!(refe.owner, Owner::Immutable))
        .unwrap();

    let start_call_result = IotaClientCommands::Call {
        package: package.reference.object_id,
        module: "trusted_coin".to_string(),
        function: "f".to_string(),
        type_args: vec![],
        args: vec![],
        payment: PaymentArgs::default(),
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_PUBLISH),
            ..Default::default()
        },
        processing: TxProcessingArgs {
            display: HashSet::from([DisplayOption::BalanceChanges]),
            ..Default::default()
        },
    }
    .execute(context)
    .await?;

    if let Some(tx_block_response) = start_call_result.tx_block_response() {
        // Assert Balance Changes are present in the response
        assert!(tx_block_response.balance_changes.is_some());
        // effects are always in the response
        assert!(tx_block_response.effects.is_some());

        // Assert every other field is not present in the response
        assert!(tx_block_response.object_changes.is_none());
        assert!(tx_block_response.events.is_none());
        assert!(tx_block_response.transaction.is_none());
    } else {
        panic!("Transaction block response is None");
    }

    // Make another call, this time with multiple display args
    let start_call_result = IotaClientCommands::Call {
        package: package.reference.object_id,
        module: "trusted_coin".to_string(),
        function: "f".to_string(),
        type_args: vec![],
        args: vec![],
        payment: PaymentArgs {
            gas: vec![gas_obj_id],
        },
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_PUBLISH),
            ..Default::default()
        },
        processing: TxProcessingArgs {
            display: HashSet::from([
                DisplayOption::BalanceChanges,
                DisplayOption::Effects,
                DisplayOption::ObjectChanges,
            ]),
            ..Default::default()
        },
    }
    .execute(context)
    .await?;

    start_call_result.print(true);

    // Assert Balance Changes, effects and object changes are present in the
    // response
    if let Some(tx_block_response) = start_call_result.tx_block_response() {
        assert!(tx_block_response.balance_changes.is_some());
        assert!(tx_block_response.effects.is_some());
        assert!(tx_block_response.object_changes.is_some());
        assert!(tx_block_response.events.is_none());
        assert!(tx_block_response.transaction.is_none());
    } else {
        panic!("Transaction block response is None");
    }

    // Make another call, this time without display args. This should return the
    // full response
    let start_call_result = IotaClientCommands::Call {
        package: package.reference.object_id,
        module: "trusted_coin".to_string(),
        function: "f".to_string(),
        type_args: vec![],
        args: vec![],
        payment: PaymentArgs {
            gas: vec![gas_obj_id],
        },
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_PUBLISH),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;

    // Assert all fields are present in the response
    if let Some(tx_block_response) = start_call_result.tx_block_response() {
        assert!(tx_block_response.balance_changes.is_some());
        assert!(tx_block_response.effects.is_some());
        assert!(tx_block_response.object_changes.is_some());
        assert!(tx_block_response.events.is_some());
        assert!(tx_block_response.transaction.is_some());
    } else {
        panic!("Transaction block response is None");
    }

    Ok(())
}

#[sim_test]
async fn test_ptb_dev_inspect() -> Result<(), anyhow::Error> {
    let mut test_cluster = TestClusterBuilder::new()
        .with_num_validators(2)
        .build()
        .await;
    let context = &mut test_cluster.wallet;

    let publish_ptb_string = r#"
        --assign hello_option "some('Hello')" \
        --move-call std::option::borrow "<std::string::String>" hello_option \
        --dev-inspect
        "#;
    let args = shlex::split(publish_ptb_string).unwrap();
    let PTBCommandResult::DevInspect(res) = iota::client_ptb::ptb::PTB {
        args,
        display: HashSet::new(),
    }
    .execute(context)
    .await?
    else {
        panic!("unexpected PTB result");
    };
    assert!(res.results.expect("missing results").iter().any(|res| {
        res.return_values.iter().any(|(bytes, tag)| {
            tag.as_ref() == "0x1::string::String" && bytes == &[5, 72, 101, 108, 108, 111]
        })
    }));
    Ok(())
}

#[sim_test]
async fn test_ptb_display_args() -> Result<(), anyhow::Error> {
    let mut test_cluster = TestClusterBuilder::new()
        .with_num_validators(2)
        .build()
        .await;
    let context = &mut test_cluster.wallet;

    let ptb_string = r#"
    --make-move-vec <u8> "[1]"
    "#;
    let args = shlex::split(ptb_string).unwrap();
    let PTBCommandResult::CommandResult(res) = iota::client_ptb::ptb::PTB {
        args,
        display: HashSet::from([DisplayOption::Input]),
    }
    .execute(context)
    .await?
    else {
        panic!("unexpected PTB result");
    };
    let IotaClientCommandResult::TransactionBlock(res) = *res else {
        panic!("unexpected PTB result");
    };

    assert!(res.transaction.is_some());
    assert!(res.effects.is_some());

    let ptb_string = r#"
        --make-move-vec <u8> "[1]"
        "#;
    let args = shlex::split(ptb_string).unwrap();
    let PTBCommandResult::CommandResult(res) = iota::client_ptb::ptb::PTB {
        args,
        display: HashSet::from([DisplayOption::Events]),
    }
    .execute(context)
    .await?
    else {
        panic!("unexpected PTB result");
    };
    let IotaClientCommandResult::TransactionBlock(res) = *res else {
        panic!("unexpected PTB result");
    };
    // `DisplayOption::Input` wasn't provided, so there is no `Transaction Data`
    assert!(res.transaction.is_none());
    assert!(res.effects.is_some());

    Ok(())
}

#[tokio::test]
async fn test_new_env() -> Result<(), anyhow::Error> {
    let mut test_cluster = TestClusterBuilder::new()
        .with_num_validators(1)
        .with_fullnode_rpc_port(9009)
        .build()
        .await;
    let context = &mut test_cluster.wallet;

    let alias = "network-alias".to_string();
    let rpc = "http://127.0.0.1:9009".to_string();
    let graphql = Some("http://127.0.0.1:8000".to_string());
    let ws = Some("ws://127.0.0.1:9000".to_string());
    let basic_auth = Some("username:password".to_string());
    let faucet = Some("http://127.0.0.1:9123/v1/gas".to_string());

    IotaClientCommands::NewEnv {
        alias: alias.clone(),
        rpc: rpc.clone(),
        graphql: graphql.clone(),
        ws: ws.clone(),
        basic_auth: basic_auth.clone(),
        faucet: faucet.clone(),
    }
    .execute(context)
    .await
    .unwrap();

    let res: IotaClientCommandResult = IotaClientCommands::Envs.execute(context).await?;

    let IotaClientCommandResult::Envs(envs, _active_env) = res else {
        unreachable!("Invalid response");
    };
    assert!(envs.len() == 2);
    let new_env = &envs[1];
    assert_eq!(*new_env.alias(), alias);
    assert_eq!(*new_env.rpc(), rpc);
    assert_eq!(*new_env.graphql(), graphql);
    assert_eq!(*new_env.ws(), ws);
    assert_eq!(*new_env.basic_auth(), basic_auth);
    assert_eq!(*new_env.faucet(), faucet);

    Ok(())
}

#[sim_test]
async fn test_ptb_sender() -> Result<(), anyhow::Error> {
    // Hardcoded multisig address (generated with `iota keytool multi-sig-address
    // --pks ADtqJ7zOtqQtYqOo0CpvDXNlMhV3HeJDpjrASKGLWdop --weights 1 --threshold
    // 1` where the pubKey is for the privKey with all zeros)
    let multisig_address =
        IotaAddress::from_str("0xdbcd4c41bd078067c1fed6382ce014771529f37087d02a48f927d678f96064fa")
            .unwrap();
    let mut test_cluster = TestClusterBuilder::new()
        .with_num_validators(2)
        .build()
        .await;
    let address = test_cluster.get_address_0();
    let rgp = test_cluster.get_reference_gas_price().await;
    let context = &mut test_cluster.wallet;
    let client = context.get_client().await?;
    let object_refs = client
        .read_api()
        .get_owned_objects(address, None, None, None)
        .await?
        .data;
    let obj_id = object_refs.get(1).unwrap().object().unwrap().object_id;
    // Send funds to the multisig address
    IotaClientCommands::Transfer {
        to: KeyIdentity::Address(multisig_address),
        object_id: obj_id,
        payment: PaymentArgs::default(),
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;
    // Now do a PTB with --sender
    let ptb_string = format!(
        r#"
        --split-coins gas [1]
        --assign s
        --transfer-objects [s.0] @{multisig_address}
        --sender @{multisig_address}
        --gas-budget 10000000
        --serialize-unsigned-transaction
        "#
    );
    let args = shlex::split(&ptb_string).unwrap();
    let ptb = PTB {
        args,
        display: HashSet::new(),
    };
    let ptb_res = ptb.execute(context).await?;
    let PTBCommandResult::CommandResult(res) = ptb_res else {
        unreachable!("Invalid result");
    };
    let IotaClientCommandResult::SerializedUnsignedTransaction(tx_data) = *res else {
        panic!("unexpected PTB result");
    };
    assert_eq!(tx_data.sender(), multisig_address);

    Ok(())
}

#[sim_test]
async fn test_tree_shaking_package_with_unpublished_deps() -> Result<(), anyhow::Error> {
    let mut test = TreeShakingTest::new().await.unwrap();
    // A package and with unpublished deps
    let (package_id, _) = test.publish_package("H", true).await.unwrap();

    // set with_unpublished_dependencies to true and publish package H
    let linkage_table_h = test.fetch_linkage_table(package_id).await;
    // H depends on G, which is unpublished, so the linkage table should be empty as
    // G will be included in H during publishing
    assert!(linkage_table_h.is_empty());

    // try publish package H but `with_unpublished_dependencies` is false. Should
    // error
    let resp = test.publish_package("H", false).await;
    assert!(resp.is_err());

    Ok(())
}

#[sim_test]
#[ignore] // TODO: DVX-786
async fn test_tree_shaking_package_with_bytecode_deps() -> Result<(), anyhow::Error> {
    let mut test = TreeShakingTest::new().await?;
    let with_unpublished_dependencies = false;

    // bytecode deps without source code
    let (package_a_id, _) = test
        .publish_package("A", with_unpublished_dependencies)
        .await?;

    // make pkg a to be a bytecode dep for package F
    // set published-at field to package id and addresses a to package id
    let package_path = test.package_path("A");
    add_ids_to_manifest(&package_path, &package_a_id, Some(package_a_id))?;

    // delete the sources folder from pkg A to setup A as bytecode dep for package F
    fs::remove_file(package_path.join("Move.lock"))?;
    let build_folder = package_path.join("build");
    if build_folder.exists() {
        fs::remove_dir_all(&build_folder)?;
    }
    move_package::package_hooks::register_package_hooks(Box::new(IotaPackageHooks));
    // now build the package which will create the build folder and a new Move.lock
    // file
    BuildConfig::new_for_testing().build(&package_path).unwrap();
    fs::remove_dir_all(package_path.join("sources"))?;

    let (package_f_id, _) = test
        .publish_package("F", with_unpublished_dependencies)
        .await?;
    let linkage_table_f = test.fetch_linkage_table(package_f_id).await;
    // F depends on A as a bytecode dep, so the linkage table should not be empty
    assert!(
        linkage_table_f.contains_key(&package_a_id),
        "Package F should depend on A"
    );

    Ok(())
}

#[sim_test]
async fn test_tree_shaking_package_without_dependencies() -> Result<(), anyhow::Error> {
    let mut test = TreeShakingTest::new().await?;

    // Publish package A and verify empty linkage table
    let (package_a_id, _) = test.publish_package("A", false).await?;
    let move_pkg_a = fetch_move_packages(&test.client, vec![package_a_id]).await;
    let linkage_table_a = move_pkg_a.first().unwrap().linkage_table();
    assert!(
        linkage_table_a.is_empty(),
        "Package A should have no dependencies"
    );

    Ok(())
}

#[sim_test]
async fn test_tree_shaking_package_with_direct_dependency() -> Result<(), anyhow::Error> {
    let mut test = TreeShakingTest::new().await?;

    // First publish package A
    let (package_a_id, _) = test.publish_package("A", false).await?;

    // Then publish B which depends on A
    let (package_b_id, _) = test.publish_package("B_A", false).await?;
    let linkage_table_b = test.fetch_linkage_table(package_b_id).await;
    assert!(
        linkage_table_b.contains_key(&package_a_id),
        "Package B should depend on A"
    );

    Ok(())
}

#[sim_test]
async fn test_tree_shaking_package_with_unused_dependency() -> Result<(), anyhow::Error> {
    let mut test = TreeShakingTest::new().await?;

    // First publish package A
    let (_, _) = test.publish_package("A", false).await?;

    // Then publish B which declares but doesn't use A
    let (package_b_id, _) = test.publish_package("B_A1", false).await?;
    let linkage_table_b = test.fetch_linkage_table(package_b_id).await;
    assert!(
        linkage_table_b.is_empty(),
        "Package B should have empty linkage table when not using A"
    );

    Ok(())
}

#[sim_test]
async fn test_tree_shaking_package_with_transitive_dependencies1() -> Result<(), anyhow::Error> {
    let mut test = TreeShakingTest::new().await?;

    // Publish packages A and B
    let (package_a_id, _) = test.publish_package("A", false).await?;
    let (package_b_id, _) = test.publish_package("B_A", false).await?;

    // Publish C which depends on B (which depends on A)
    let (package_c_id, _) = test.publish_package("C_B_A", false).await?;
    let linkage_table_c = test.fetch_linkage_table(package_c_id).await;

    assert!(
        linkage_table_c.contains_key(&package_a_id),
        "Package C should depend on A"
    );
    assert!(
        linkage_table_c.contains_key(&package_b_id),
        "Package C should depend on B"
    );
    assert_eq!(
        linkage_table_c.len(),
        2,
        "Package C should have exactly two dependencies"
    );

    Ok(())
}

#[sim_test]
async fn test_tree_shaking_package_with_transitive_dependencies_and_no_code_references()
-> Result<(), anyhow::Error> {
    // Publish package C_B and check the linkage
    // table we use here the package B published in TEST 3
    let mut test = TreeShakingTest::new().await?;

    // Publish packages A and B
    let (_, _) = test.publish_package("A", false).await?;
    let (_, _) = test.publish_package("B_A1", false).await?;

    // Publish C which depends on B
    let (package_c_id, _) = test.publish_package("C_B", false).await?;
    let linkage_table_c = test.fetch_linkage_table(package_c_id).await;

    assert!(
        linkage_table_c.is_empty(),
        "Package C should have no dependencies"
    );

    Ok(())
}

#[sim_test]
async fn test_tree_shaking_package_deps_on_pkg_upgrade() -> Result<(), anyhow::Error> {
    let mut test = TreeShakingTest::new().await?;

    // Publish package A and B
    let (package_a_id, cap) = test.publish_package("A", false).await?;
    let (_, _) = test.publish_package("B_A", false).await?;

    // Upgrade package A (named A_v1)
    std::fs::copy(
        test.package_path("A").join("Move.lock"),
        test.package_path("A_v1").join("Move.lock"),
    )?;
    let package_a_v1_id = test.upgrade_package("A_v1", cap).await?;

    // Publish D which depends on A_v1 but no code references A
    let (package_d_id, _) = test.publish_package("D_A", false).await?;
    let linkage_table_d = test.fetch_linkage_table(package_d_id).await;

    assert!(
        linkage_table_d.is_empty(),
        "Package D should have no dependencies"
    );

    // Publish D which depends on A_v1 and code references it
    let (package_d_id, _) = test.publish_package("D_A_v1", false).await?;
    let linkage_table_d = test.fetch_linkage_table(package_d_id).await;

    assert!(
        linkage_table_d.contains_key(&package_a_id),
        "Package D should depend on A"
    );
    assert!(
        linkage_table_d
            .get(&package_a_id)
            .is_some_and(|x| x.upgraded_id == package_a_v1_id),
        "Package D should depend on A_v1 after upgrade, and the UpgradeInfo should have matching ids"
    );

    let (package_e_id, _) = test.publish_package("E_A_v1", false).await?;

    let linkage_table_e = test.fetch_linkage_table(package_e_id).await;
    assert!(
        linkage_table_e.is_empty(),
        "Package E should have no dependencies"
    );

    let (package_e_id, _) = test.publish_package("E", false).await?;

    let linkage_table_e = test.fetch_linkage_table(package_e_id).await;
    assert!(
        linkage_table_e.contains_key(&package_a_id),
        "Package E should depend on A"
    );

    Ok(())
}

#[sim_test]
async fn test_tree_shaking_package_deps_on_pkg_upgrade_1() -> Result<(), anyhow::Error> {
    let mut test = TreeShakingTest::new().await?;

    // Publish package A and D_A
    let (package_a_id, cap) = test.publish_package("A", false).await?;
    let package_path = test.package_path("A");
    add_ids_to_manifest(&package_path, &package_a_id, None)?;
    // Upgrade package A (named A_v1)
    std::fs::copy(
        test.package_path("A").join("Move.lock"),
        test.package_path("A_v1").join("Move.lock"),
    )?;
    let package_a_v1_id = test.upgrade_package("A_v1", cap).await?;

    let package_path = test.package_path("A_v1");
    add_ids_to_manifest(&package_path, &package_a_v1_id, None)?;

    let package_d_id = test.publish_package_without_tree_shaking("D_A").await;
    let linkage_table_d = test.fetch_linkage_table(package_d_id).await;
    assert!(
        linkage_table_d.contains_key(&package_a_id),
        "Package D should depend on A"
    );

    // published package D with the old stuff that isn't aware of automated address
    // mgmt, so need to update the published-at field in the manifest
    add_ids_to_manifest(&test.package_path("D_A"), &package_d_id, None)?;

    // Upgrade package A (named A_v2)
    std::fs::copy(
        test.package_path("A_v1").join("Move.lock"),
        test.package_path("A_v2").join("Move.lock"),
    )?;
    let package_a_v2_id = test.upgrade_package("A_v2", cap).await?;

    // the old code for publishing a package from iota-test-transaction-builder does
    // not know about move.lock and so on, so we need to add manually the
    // published-at address.
    let package_path = test.package_path("A_v2");
    add_ids_to_manifest(&package_path, &package_a_v2_id, None)?;

    let (package_i_id, _) = test.publish_package("I", false).await?;
    let linkage_table_i = test.fetch_linkage_table(package_i_id).await;
    assert!(
        linkage_table_i.contains_key(&package_a_id),
        "Package I linkage table should have A"
    );
    assert!(
        linkage_table_i
            .get(&package_a_id)
            .is_some_and(|x| x.upgraded_id == package_a_v2_id),
        "Package I should depend on A_v2 after upgrade, and the UpgradeInfo should have matching ids"
    );

    Ok(())
}

#[sim_test]
async fn test_tree_shaking_package_deps_on_pkg_upgrade_2() -> Result<(), anyhow::Error> {
    let mut test = TreeShakingTest::new().await?;

    // Publish package K
    let (package_k_id, cap) = test.publish_package("K", false).await?;
    let package_path = test.package_path("K");
    add_ids_to_manifest(&package_path, &package_k_id, None)?;
    // Upgrade package K (named K_v2)
    std::fs::copy(
        test.package_path("K").join("Move.lock"),
        test.package_path("K_v2").join("Move.lock"),
    )?;
    let package_k_v2_id = test.upgrade_package("K_v2", cap).await?;

    let package_path = test.package_path("K_v2");
    add_ids_to_manifest(&package_path, &package_k_v2_id, None)?;

    let (package_l_id, _) = test.publish_package("L", false).await?;
    let linkage_table_l = test.fetch_linkage_table(package_l_id).await;
    assert!(
        linkage_table_l.contains_key(&package_k_id),
        "Package L should depend on K"
    );

    add_ids_to_manifest(&test.package_path("L"), &package_l_id, None)?;

    let (package_m_id, _) = test.publish_package("M", false).await?;
    let linkage_table_m = test.fetch_linkage_table(package_m_id).await;
    assert!(
        linkage_table_m.contains_key(&package_k_id),
        "Package M should depend on K"
    );

    assert!(
        linkage_table_m
            .get(&package_k_id)
            .is_some_and(|x| x.upgraded_id == package_k_v2_id),
        "Package I should depend on A_v2 after upgrade, and the UpgradeInfo should have matching ids"
    );

    // publish everything again but without automated address mgmt.

    Ok(())
}

#[sim_test]
async fn test_tree_shaking_package_deps_on_pkg_upgrade_3() -> Result<(), anyhow::Error> {
    let mut test = TreeShakingTest::new().await?;

    // This test is identical to #2, except it uses the old test-transaction-builder
    // infrastructure to publish a package without tree shaking. It is also
    // unaware of automated address mgmt, so this test sets up the published-at
    // fields and addresses sections accordingly.

    // Publish package K
    let (package_k_id, cap) = test.publish_package("K", false).await?;
    let package_path = test.package_path("K");
    add_ids_to_manifest(&package_path, &package_k_id, Some(package_k_id))?;
    // Upgrade package K (named K_v2)
    std::fs::copy(
        test.package_path("K").join("Move.lock"),
        test.package_path("K_v2").join("Move.lock"),
    )?;
    let package_k_v2_id = test.upgrade_package("K_v2", cap).await?;
    let package_path = test.package_path("K_v2");
    add_ids_to_manifest(&package_path, &package_k_v2_id, Some(package_k_id))?;

    let package_l_id = test.publish_package_without_tree_shaking("L").await;
    let linkage_table_l = test.fetch_linkage_table(package_l_id).await;
    assert!(
        linkage_table_l.contains_key(&package_k_id),
        "Package L should depend on K"
    );

    add_ids_to_manifest(&test.package_path("L"), &package_l_id, Some(package_l_id))?;

    let (package_m_id, _) = test.publish_package("M", false).await?;
    let linkage_table_m = test.fetch_linkage_table(package_m_id).await;
    assert!(
        linkage_table_m.contains_key(&package_k_id),
        "Package M should depend on K"
    );

    assert!(
        linkage_table_m
            .get(&package_k_id)
            .is_some_and(|x| x.upgraded_id == package_k_v2_id),
        "Package I should depend on A_v2 after upgrade, and the UpgradeInfo should have matching ids"
    );

    Ok(())
}

#[sim_test]
async fn test_tree_shaking_package_system_deps() -> Result<(), anyhow::Error> {
    let mut test = TreeShakingTest::new().await?;

    // Publish package J and verify empty linkage table
    let (package_j_id, _) = test.publish_package("J", false).await?;
    let move_pkg_j = fetch_move_packages(&test.client, vec![package_j_id]).await;
    let linkage_table_j = move_pkg_j.first().unwrap().linkage_table();
    assert!(
        linkage_table_j.is_empty(),
        "Package J should have no dependencies"
    );

    // iota move build --dump-bytecode-as-base64 should also yield a json with no
    // dependencies
    let package_path = test.package_path("J");
    let binary_path = env!("CARGO_BIN_EXE_iota");
    let cmd = std::process::Command::new(binary_path)
        .arg("move")
        .arg("build")
        .arg("--dump-bytecode-as-base64")
        .arg(package_path)
        .output()
        .expect("Failed to execute command");

    let output = String::from_utf8_lossy(&cmd.stdout);
    assert!(!output.contains("dependencies: []"));

    Ok(())
}

#[sim_test]
async fn test_ptb_gas_coin_smashing() -> Result<(), anyhow::Error> {
    let mut test_cluster = TestClusterBuilder::new()
        .with_num_validators(1)
        .build()
        .await;
    let address = test_cluster.get_address_0();
    let context = &mut test_cluster.wallet;
    let client = context.get_client().await?;
    let object_refs = client
        .read_api()
        .get_owned_objects(address, None, None, None)
        .await?
        .data;

    let gas_coin_ids: Vec<String> = object_refs
        .iter()
        .flat_map(|obj_ref| obj_ref.object().map(|o| format!("@{}", o.object_id)))
        .collect();
    assert!(
        gas_coin_ids.len() > 1,
        "Not enough gas coins to test gas smashing"
    );

    let ptb_string = format!(
        r#"
        --split-coins gas [100]
        --assign c
        --transfer-objects [c.0] @{address}
        --gas-coin {}
        "#,
        gas_coin_ids.join(" ")
    );
    let args = shlex::split(&ptb_string).unwrap();
    let ptb = PTB {
        args,
        display: HashSet::new(),
    };
    let ptb_res = ptb.execute(context).await?;
    let PTBCommandResult::CommandResult(res) = ptb_res else {
        panic!("Command failed, expected CommandResult");
    };

    let IotaClientCommandResult::TransactionBlock(tx_block) = *res else {
        panic!("Expected TransactionBlock result");
    };
    let payment_len = tx_block
        .transaction
        .as_ref()
        .expect("Missing transaction")
        .data
        .gas_data()
        .payment
        .len();
    assert!(
        payment_len > 1,
        "Expected more than one gas payment, got {payment_len}"
    );

    Ok(())
}

// Same as above, just with `gas-coins` instead of `gas-coin`
#[sim_test]
async fn test_ptb_gas_coins_smashing() -> Result<(), anyhow::Error> {
    let mut test_cluster = TestClusterBuilder::new()
        .with_num_validators(1)
        .build()
        .await;
    let address = test_cluster.get_address_0();
    let context = &mut test_cluster.wallet;
    let client = context.get_client().await?;
    let object_refs = client
        .read_api()
        .get_owned_objects(address, None, None, None)
        .await?
        .data;

    let gas_coin_ids: Vec<String> = object_refs
        .iter()
        .flat_map(|obj_ref| obj_ref.object().map(|o| format!("@{}", o.object_id)))
        .collect();
    assert!(
        gas_coin_ids.len() > 1,
        "Not enough gas coins to test gas smashing"
    );

    let ptb_string = format!(
        r#"
        --split-coins gas [100]
        --assign c
        --transfer-objects [c.0] @{address}
        --gas-coins {}
        "#,
        gas_coin_ids.join(" ")
    );
    let args = shlex::split(&ptb_string).unwrap();
    let ptb = PTB {
        args,
        display: HashSet::new(),
    };
    let ptb_res = ptb.execute(context).await?;
    let PTBCommandResult::CommandResult(res) = ptb_res else {
        panic!("Command failed, expected CommandResult");
    };

    let IotaClientCommandResult::TransactionBlock(tx_block) = *res else {
        panic!("Expected TransactionBlock result");
    };
    let payment_len = tx_block
        .transaction
        .as_ref()
        .expect("Missing transaction")
        .data
        .gas_data()
        .payment
        .len();
    assert!(
        payment_len > 1,
        "Expected more than one gas payment, got {payment_len}"
    );

    Ok(())
}

#[sim_test]
async fn test_move_authenticator() -> Result<(), anyhow::Error> {
    let mut test_cluster = TestClusterBuilder::new()
        .with_num_validators(1)
        .build()
        .await;
    let rgp = test_cluster.get_reference_gas_price().await;
    let sender_address = test_cluster.get_address_0();
    let context = &mut test_cluster.wallet;

    let client = context.get_client().await?;
    let gas_obj_id = client
        .read_api()
        .get_owned_objects(sender_address, None, None, None)
        .await?
        .data
        .first()
        .unwrap()
        .object()
        .unwrap()
        .object_id;

    // Publish the account package
    let package_path = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap())
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples/move/account");
    let mut build_config = BuildConfig::new_for_testing().config;
    build_config.lock_file = Some(package_path.join("Move.lock"));
    let resp = IotaClientCommands::Publish {
        package_path,
        build_config,
        skip_dependency_verification: false,
        with_unpublished_dependencies: false,
        verify_deps: true,
        payment: PaymentArgs {
            gas: vec![gas_obj_id],
        },
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_PUBLISH),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;

    // Extract IDs from publish response
    let IotaClientCommandResult::TransactionBlock(response) = resp else {
        panic!("Expected TransactionBlock");
    };
    let object_changes = response.object_changes.as_ref().unwrap();
    let account_address = object_changes
        .iter()
        .find_map(|oc| match oc {
            ObjectChange::Created {
                object_type,
                object_id,
                ..
            } if object_type.to_string().ends_with("::account::Account") => Some(*object_id),
            _ => None,
        })
        .unwrap();
    let package_id = object_changes
        .iter()
        .find_map(|oc| match oc {
            ObjectChange::Published { package_id, .. } => Some(*package_id),
            _ => None,
        })
        .unwrap();
    let metadata_id = object_changes
        .iter()
        .find_map(|oc| match oc {
            ObjectChange::Created {
                object_type,
                object_id,
                ..
            } if object_type.to_string() == "0x2::package_metadata::PackageMetadataV1" => {
                Some(*object_id)
            }
            _ => None,
        })
        .unwrap();

    // Link auth
    IotaClientCommands::Call {
        package: package_id,
        module: "account".to_string(),
        function: "link_auth".to_string(),
        type_args: vec![],
        args: vec![
            IotaJsonValue::from_str(&account_address.to_string()).unwrap(),
            IotaJsonValue::from_str(&metadata_id.to_string()).unwrap(),
            IotaJsonValue::from_str("\"account\"").unwrap(),
            IotaJsonValue::from_str("\"authenticate\"").unwrap(),
        ],
        payment: PaymentArgs::default(),
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;

    // Send funds to account
    let transfer_resp = IotaClientCommands::PTB(PTB {
        args: vec![
            "--split-coins".to_string(),
            "gas".to_string(),
            "[2000000000]".to_string(),
            "--assign".to_string(),
            "coin".to_string(),
            "--transfer-objects".to_string(),
            "[coin]".to_string(),
            format!("@{account_address}"),
        ],
        display: HashSet::new(),
    })
    .execute(context)
    .await?;
    assert!(matches!(
        transfer_resp,
        IotaClientCommandResult::TransactionBlock(ref tx) if tx.effects.as_ref().unwrap().status().is_ok()
    ));

    // Add and switch to account
    IotaClientCommands::AddAccount {
        alias: None,
        address: account_address.into(),
    }
    .execute(context)
    .await?;
    IotaClientCommands::Switch {
        address: Some(IotaAddress::from(account_address).into()),
        env: None,
    }
    .execute(context)
    .await?;

    // Perform auth transaction
    let ptb_resp = IotaClientCommands::PTB(PTB {
        args: vec![
            "--split-coins".to_string(),
            "gas".to_string(),
            "[1]".to_string(),
            "--assign".to_string(),
            "coin".to_string(),
            "--transfer-objects".to_string(),
            "[coin]".to_string(),
            format!("@{account_address}"),
            "--auth-call-args".to_string(),
            "hello".to_string(),
        ],
        display: HashSet::new(),
    })
    .execute(context)
    .await?;
    assert!(matches!(
        ptb_resp,
        IotaClientCommandResult::TransactionBlock(ref tx) if tx.effects.as_ref().unwrap().status().is_ok()
    ));

    // Perform auth transaction with client sign command
    let ptb_resp = IotaClientCommands::PTB(PTB {
        args: vec![
            "--split-coins".to_string(),
            "gas".to_string(),
            "[1]".to_string(),
            "--assign".to_string(),
            "coin".to_string(),
            "--transfer-objects".to_string(),
            "[coin]".to_string(),
            format!("@{account_address}"),
            "--sender".to_string(),
            format!("@{account_address}"),
            "--serialize-unsigned-transaction".to_string(),
        ],
        display: HashSet::new(),
    })
    .execute(context)
    .await?;
    let tx_data = if let IotaClientCommandResult::SerializedUnsignedTransaction(tx) = ptb_resp {
        tx
    } else {
        panic!("Expected SerializedUnsignedTransaction");
    };

    let sign_result = IotaClientCommands::Sign {
        address: KeyIdentity::Address(account_address.into()),
        data: Base64::encode(bcs::to_bytes(&tx_data).unwrap()),
        intent: None,
        auth_call_args: Some(vec!["hello".to_string()]),
        auth_type_args: None,
    }
    .execute(context)
    .await?;
    let sign_data = if let IotaClientCommandResult::Sign(data) = sign_result {
        data
    } else {
        panic!("Expected Sign result");
    };

    let execute_result = IotaClientCommands::ExecuteSignedTx {
        tx_bytes: sign_data.raw_tx_data,
        signatures: vec![sign_data.iota_signature],
    }
    .execute(context)
    .await?;
    assert!(matches!(
        execute_result,
        IotaClientCommandResult::TransactionBlock(ref tx) if tx.effects.as_ref().unwrap().status().is_ok()
    ));

    Ok(())
}

/// Tests that `--auth-call-args` correctly handles multiple argument types
/// including nested vectors (`vector<vector<u8>>`).
///
/// The authenticator function in `account_multi_auth` accepts:
/// - `magic_number: u64` (must be 42)
/// - `secret: vector<u8>` (must be 0xCAFE)
/// - `nested: vector<vector<u8>>` (must be [[0xAA], [0xBB, 0xCC]])
#[sim_test]
async fn test_move_authenticator_nested_vec() -> Result<(), anyhow::Error> {
    let mut test_cluster = TestClusterBuilder::new()
        .with_num_validators(1)
        .build()
        .await;
    let rgp = test_cluster.get_reference_gas_price().await;
    let sender_address = test_cluster.get_address_0();
    let context = &mut test_cluster.wallet;

    let client = context.get_client().await?;
    let gas_obj_id = client
        .read_api()
        .get_owned_objects(sender_address, None, None, None)
        .await?
        .data
        .first()
        .unwrap()
        .object()
        .unwrap()
        .object_id;

    // Publish the account_multi_auth package
    let package_path = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap())
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples/move/abstract_iota_accounts/account_multi_auth");
    let mut build_config = BuildConfig::new_for_testing().config;
    build_config.lock_file = Some(package_path.join("Move.lock"));
    let resp = IotaClientCommands::Publish {
        package_path,
        build_config,
        skip_dependency_verification: false,
        with_unpublished_dependencies: false,
        verify_deps: true,
        payment: PaymentArgs {
            gas: vec![gas_obj_id],
        },
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_PUBLISH),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;

    // Extract IDs from publish response
    let IotaClientCommandResult::TransactionBlock(response) = resp else {
        panic!("Expected TransactionBlock");
    };
    let object_changes = response.object_changes.as_ref().unwrap();
    let account_address = object_changes
        .iter()
        .find_map(|oc| match oc {
            ObjectChange::Created {
                object_type,
                object_id,
                ..
            } if object_type.to_string().ends_with("::account::Account") => Some(*object_id),
            _ => None,
        })
        .unwrap();
    let package_id = object_changes
        .iter()
        .find_map(|oc| match oc {
            ObjectChange::Published { package_id, .. } => Some(*package_id),
            _ => None,
        })
        .unwrap();
    let metadata_id = object_changes
        .iter()
        .find_map(|oc| match oc {
            ObjectChange::Created {
                object_type,
                object_id,
                ..
            } if object_type.to_string() == "0x2::package_metadata::PackageMetadataV1" => {
                Some(*object_id)
            }
            _ => None,
        })
        .unwrap();

    // Link auth
    IotaClientCommands::Call {
        package: package_id,
        module: "account".to_string(),
        function: "link_auth".to_string(),
        type_args: vec![],
        args: vec![
            IotaJsonValue::from_str(&account_address.to_string()).unwrap(),
            IotaJsonValue::from_str(&metadata_id.to_string()).unwrap(),
            IotaJsonValue::from_str("\"account\"").unwrap(),
            IotaJsonValue::from_str("\"authenticate\"").unwrap(),
        ],
        payment: PaymentArgs::default(),
        gas_data: GasDataArgs {
            gas_budget: Some(rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER),
            ..Default::default()
        },
        processing: TxProcessingArgs::default(),
    }
    .execute(context)
    .await?;

    // Send funds to account
    let transfer_resp = IotaClientCommands::PTB(PTB {
        args: vec![
            "--split-coins".to_string(),
            "gas".to_string(),
            "[2000000000]".to_string(),
            "--assign".to_string(),
            "coin".to_string(),
            "--transfer-objects".to_string(),
            "[coin]".to_string(),
            format!("@{account_address}"),
        ],
        display: HashSet::new(),
    })
    .execute(context)
    .await?;
    assert!(matches!(
        transfer_resp,
        IotaClientCommandResult::TransactionBlock(ref tx) if tx.effects.as_ref().unwrap().status().is_ok()
    ));

    // Add and switch to account
    IotaClientCommands::AddAccount {
        alias: None,
        address: account_address.into(),
    }
    .execute(context)
    .await?;
    IotaClientCommands::Switch {
        address: Some(IotaAddress::from(account_address).into()),
        env: None,
    }
    .execute(context)
    .await?;

    // Perform auth transaction with multiple arg types:
    // - "42" for u64
    // - "0xCAFE" for vector<u8>
    // - "[0xAA,0xBBCC]" for vector<vector<u8>>
    // - "test" for String
    // - "[0xDEAD]" for Option<vector<u8>> (Some([0xDE, 0xAD]))
    // - "0x6" for &Clock (immutable shared object reference)
    let ptb_resp = IotaClientCommands::PTB(PTB {
        args: vec![
            "--split-coins".to_string(),
            "gas".to_string(),
            "[1]".to_string(),
            "--assign".to_string(),
            "coin".to_string(),
            "--transfer-objects".to_string(),
            "[coin]".to_string(),
            format!("@{account_address}"),
            "--auth-call-args".to_string(),
            "42".to_string(),
            "0xCAFE".to_string(),
            "[0xAA,0xBBCC]".to_string(),
            "test".to_string(),
            "[0xDEAD]".to_string(),
            "0x6".to_string(),
        ],
        display: HashSet::new(),
    })
    .execute(context)
    .await?;
    assert!(
        matches!(
            ptb_resp,
            IotaClientCommandResult::TransactionBlock(ref tx) if tx.effects.as_ref().unwrap().status().is_ok()
        ),
        "Auth transaction with nested vector args should succeed"
    );

    Ok(())
}
