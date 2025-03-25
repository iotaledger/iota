// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    fs,
    io::Read,
    os::unix::fs::FileExt,
    path::PathBuf,
    sync::{Arc, RwLock},
};

use expect_test::expect;
use iota::client_commands::{IotaClientCommandResult, IotaClientCommands, OptsWithGas};
use iota_json_rpc_types::{IotaTransactionBlockEffects, IotaTransactionBlockEffectsAPI};
use iota_move_build::{BuildConfig, IotaPackageHooks};
use iota_sdk::{
    rpc_types::{
        IotaObjectDataOptions, IotaObjectResponseQuery, IotaTransactionBlockEffectsV1,
        OwnedObjectRef,
    },
    types::{base_types::ObjectID, object::Owner, transaction::TEST_ONLY_GAS_UNIT_FOR_PUBLISH},
    wallet_context::WalletContext,
};
use iota_source_validation_service::{
    AddressLookup, AppState, Branch, CloneCommand, Config, DirectorySource, ErrorResponse,
    IOTA_SOURCE_VALIDATION_VERSION_HEADER, METRICS_HOST_PORT, Network, NetworkLookup, Package,
    PackageSource, RepositorySource, SourceInfo, SourceLookup, SourceResponse,
    SourceServiceMetrics, host_port, initialize, serve, start_prometheus_server, verify_packages,
    watch_for_upgrades,
};
use move_core_types::account_address::AccountAddress;
use move_symbol_pool::Symbol;
use reqwest::Client;
use test_cluster::TestClusterBuilder;
use tokio::sync::oneshot;

const LOCALNET_PORT: u16 = 9000;
const TEST_FIXTURES_DIR: &str = "tests/fixture";

#[expect(clippy::await_holding_lock)]
#[tokio::test]
#[ignore]
async fn test_end_to_end() -> anyhow::Result<()> {
    move_package::package_hooks::register_package_hooks(Box::new(IotaPackageHooks));
    let mut test_cluster = TestClusterBuilder::new()
        .with_fullnode_rpc_port(LOCALNET_PORT)
        .build()
        .await;

    ///////////////////////////
    // Test watch_for_upgrades
    //////////////////////////
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
    let mut package_path = PathBuf::from(TEST_FIXTURES_DIR);
    package_path.push("custom");

    // Publish and get upgrade capability to monitor.
    let effects = run_publish(package_path.clone(), context, gas_obj_id, rgp).await?;
    let cap = effects
        .created()
        .iter()
        .find(|refe| matches!(refe.owner, Owner::AddressOwner(_)))
        .unwrap();

    // Set up source service config to watch the upgrade cap.
    let config = Config {
        packages: vec![PackageSource::Directory(DirectorySource {
            paths: vec![Package {
                path: "unused".into(),
                watch: Some(cap.reference.object_id), // watch the upgrade cap
            }],
            network: Some(Network::Localnet),
        })],
    };
    // Start watching for upgrades.
    let mut sources = NetworkLookup::new();
    sources.insert(Network::Localnet, AddressLookup::new());

    let mut sources_list = NetworkLookup::new();
    sources_list.insert(Network::Localnet, AddressLookup::new());
    let app_state = Arc::new(RwLock::new(AppState {
        sources,
        metrics: None,
        sources_list,
    }));
    let app_state_ref = app_state.clone();
    let (tx, rx) = oneshot::channel();
    tokio::spawn(async move {
        watch_for_upgrades(config.packages, app_state, Network::Localnet, Some(tx)).await
    });

    // Set up to upgrade package.
    let package = effects
        .created()
        .iter()
        .find(|refe| matches!(refe.owner, Owner::Immutable))
        .unwrap();
    let package_id = package.reference.object_id;
    let tmp_dir = tempfile::tempdir().unwrap();
    let upgrade_pkg_path =
        copy_with_published_at_manifest(&package_path, &tmp_dir.path().to_path_buf(), package_id);
    // Run the upgrade.
    run_upgrade(upgrade_pkg_path, cap, context, gas_obj_id, rgp).await?;

    // Test expects to observe an upgrade transaction.
    let Ok(IotaTransactionBlockEffects::V1(effects)) = rx.await else {
        panic!("No upgrade transaction observed")
    };
    assert!(effects.status.is_ok());
    // Test expects `sources` of server state to be empty / cleared on upgrade.
    let app_state_ref = app_state_ref.read().unwrap();
    assert!(app_state_ref.sources.is_empty());

    ///////////////////////////
    // Test verify_packages
    //////////////////////////
    let config = Config {
        packages: vec![PackageSource::Repository(RepositorySource {
            repository: "https://github.com/iotaledger/iota".into(),
            branches: vec![Branch {
                branch: "main".into(),
                paths: vec![Package {
                    path: "move-stdlib".into(),
                    watch: None,
                }],
            }],
            network: Some(Network::Localnet),
        })],
    };

    let fixtures = tempfile::tempdir()?;
    fs::create_dir(fixtures.path().join("localnet"))?;
    fs_extra::dir::copy(
        PathBuf::from(TEST_FIXTURES_DIR).join("iota__main"),
        fixtures.path().join("localnet"),
        &fs_extra::dir::CopyOptions::default(),
    )?;
    let result = verify_packages(&config, fixtures.path()).await;
    let truncated_error_message = &result
        .unwrap_err()
        .to_string()
        .lines()
        .take(3)
        .map(|s| s.into())
        .collect::<Vec<String>>()
        .join("\n");
    let expected = expect![
        r#"
Network localnet: Multiple source verification errors found:

- Local dependency did not match its on-chain version at 0000000000000000000000000000000000000000000000000000000000000001::MoveStdlib::address"#
    ];
    expected.assert_eq(truncated_error_message);
    Ok(())
}

async fn run_publish(
    package_path: PathBuf,
    context: &mut WalletContext,
    gas_obj_id: ObjectID,
    rgp: u64,
) -> anyhow::Result<IotaTransactionBlockEffectsV1> {
    let build_config = BuildConfig::new_for_testing().config;
    let resp = IotaClientCommands::Publish {
        package_path: package_path.clone(),
        build_config,
        skip_dependency_verification: false,
        verify_deps: true,
        with_unpublished_dependencies: false,
        opts: OptsWithGas::for_testing(Some(gas_obj_id), rgp * TEST_ONLY_GAS_UNIT_FOR_PUBLISH),
    }
    .execute(context)
    .await?;

    let IotaClientCommandResult::TransactionBlock(response) = resp else {
        unreachable!("Invalid response");
    };
    let IotaTransactionBlockEffects::V1(effects) = response.effects.unwrap();
    assert!(effects.status.is_ok());
    Ok(effects)
}

async fn run_upgrade(
    upgrade_pkg_path: PathBuf,
    cap: &OwnedObjectRef,
    context: &mut WalletContext,
    gas_obj_id: ObjectID,
    rgp: u64,
) -> anyhow::Result<()> {
    let build_config = BuildConfig::new_for_testing().config;
    let resp = IotaClientCommands::Upgrade {
        package_path: upgrade_pkg_path,
        upgrade_capability: cap.reference.object_id,
        build_config,
        skip_dependency_verification: false,
        verify_deps: true,
        with_unpublished_dependencies: false,
        opts: OptsWithGas::for_testing(Some(gas_obj_id), rgp * TEST_ONLY_GAS_UNIT_FOR_PUBLISH),
    }
    .execute(context)
    .await?;

    let IotaClientCommandResult::TransactionBlock(response) = resp else {
        unreachable!("Invalid upgrade response");
    };
    let IotaTransactionBlockEffects::V1(effects) = response.effects.unwrap();
    assert!(effects.status.is_ok());
    Ok(())
}

/// Copy the package and set `published-at` in the Move toml file. The need for
/// this will be subsumed by automated address management.
fn copy_with_published_at_manifest(
    source_path: &PathBuf,
    dest_path: &PathBuf,
    package_id: ObjectID,
) -> PathBuf {
    fs_extra::dir::copy(
        source_path,
        dest_path,
        &fs_extra::dir::CopyOptions::default(),
    )
    .unwrap();
    let mut upgrade_pkg_path = dest_path.clone();
    upgrade_pkg_path.extend(["custom", "Move.toml"]);
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
        format!("published-at = \"{}\"", package_id.to_hex_uncompressed()),
    );
    let new = lines.join("\n");

    #[cfg(target_os = "windows")]
    move_toml.seek_write(new.as_bytes(), 0).unwrap();

    #[cfg(not(target_os = "windows"))]
    move_toml.write_at(new.as_bytes(), 0).unwrap();

    upgrade_pkg_path
}

#[tokio::test]
async fn test_api_route() -> anyhow::Result<()> {
    let config = Config { packages: vec![] };
    let tmp_dir = tempfile::tempdir()?;
    initialize(&config, tmp_dir.path()).await?;

    // set up sample lookup to serve
    let fixtures = tempfile::tempdir()?;
    fs_extra::dir::copy(
        PathBuf::from(TEST_FIXTURES_DIR).join("iota__main"),
        fixtures.path(),
        &fs_extra::dir::CopyOptions::default(),
    )?;

    let address = "0x2";
    let module = "address";
    let source_path = fixtures
        .into_path()
        .join("iota/move-stdlib/sources/address.move");

    let mut source_lookup = SourceLookup::new();
    source_lookup.insert(
        Symbol::from(module),
        SourceInfo {
            path: source_path,
            source: Some("module address {...}".to_owned()),
        },
    );
    let mut address_lookup = AddressLookup::new();
    let account_address = AccountAddress::from_hex_literal(address).unwrap();
    address_lookup.insert(account_address, source_lookup);
    let mut sources = NetworkLookup::new();
    sources.insert(Network::Localnet, address_lookup);
    let mut sources_list = NetworkLookup::new();
    sources_list.insert(Network::Localnet, AddressLookup::new());
    let app_state = Arc::new(RwLock::new(AppState {
        sources,
        metrics: None,
        sources_list,
    }));

    tokio::spawn(async move { serve(app_state).await.expect("Cannot start service.") });
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    let client = Client::new();

    // check that serve returns expected sample code
    let json = client
        .get(format!(
            "http://{}/api?address={address}&module={module}&network=localnet",
            host_port()
        ))
        .send()
        .await
        .expect("Request failed")
        .json::<SourceResponse>()
        .await?;

    let expected = expect!["module address {...}"];
    expected.assert_eq(&json.source);

    // check /list route
    let response = client
        .get(format!("http://{}/api/list", host_port()))
        .send()
        .await?
        .text()
        .await?;

    let expected = expect![[r#"{"localnet":{}}"#]];
    expected.assert_eq(response.as_str());

    // check server rejects bad version header
    let json = client
        .get(format!(
            "http://{}/api?address={address}&module={module}&network=localnet",
            host_port()
        ))
        .header(IOTA_SOURCE_VALIDATION_VERSION_HEADER, "bogus")
        .send()
        .await
        .expect("Request failed.")
        .json::<ErrorResponse>()
        .await?;

    let expected =
        expect!["Unsupported version 'bogus' specified in header x-iota-source-validation-version"];
    expected.assert_eq(&json.error);

    Ok(())
}

#[tokio::test]
async fn test_metrics_route() -> anyhow::Result<()> {
    // Start metrics server
    let metrics_listener = std::net::TcpListener::bind(METRICS_HOST_PORT)?;
    let registry_service = start_prometheus_server(metrics_listener);
    let prometheus_registry = registry_service.default_registry();
    SourceServiceMetrics::new(&prometheus_registry);

    let client = Client::new();
    let response = client
        .get(format!("http://{METRICS_HOST_PORT}/metrics"))
        .send()
        .await
        .expect("Request failed.")
        .text()
        .await?;

    let expected = expect![[r#"
        # HELP total_requests Total number of requests received by Source Service
        # TYPE total_requests counter
        total_requests 0
    "#]];
    expected.assert_eq(response.as_str());
    Ok(())
}

#[test]
fn test_parse_package_config() -> anyhow::Result<()> {
    let config = r#"
[[packages]]
source = "Repository"
[packages.values]
repository = "https://github.com/iotaledger/iota"
network = "mainnet"
[[packages.values.branches]]
branch = "framework/mainnet"
paths = [
  { path = "crates/iota-framework/packages/move-stdlib", watch = "0x1" },
  { path = "crates/iota-framework/packages/iota-framework", watch = "0x2" },
  { path = "crates/iota-framework/packages/iota-system", watch = "0x3" }
]

    [[packages]]
    source = "Directory"
    [packages.values]
    paths = [
        { path = "home/user/some/upgradeable-package", watch = "0x1234" },
        { path = "home/user/some/immutable-package" },
    ]
"#;

    let config: Config = toml::from_str(config).unwrap();
    let expect = expect![[r#"
        Config {
            packages: [
                Repository(
                    RepositorySource {
                        repository: "https://github.com/iotaledger/iota",
                        network: Some(
                            Mainnet,
                        ),
                        branches: [
                            Branch {
                                branch: "framework/mainnet",
                                paths: [
                                    Package {
                                        path: "crates/iota-framework/packages/move-stdlib",
                                        watch: Some(
                                            0x0000000000000000000000000000000000000000000000000000000000000001,
                                        ),
                                    },
                                    Package {
                                        path: "crates/iota-framework/packages/iota-framework",
                                        watch: Some(
                                            0x0000000000000000000000000000000000000000000000000000000000000002,
                                        ),
                                    },
                                    Package {
                                        path: "crates/iota-framework/packages/iota-system",
                                        watch: Some(
                                            0x0000000000000000000000000000000000000000000000000000000000000003,
                                        ),
                                    },
                                ],
                            },
                        ],
                    },
                ),
                Directory(
                    DirectorySource {
                        paths: [
                            Package {
                                path: "home/user/some/upgradeable-package",
                                watch: Some(
                                    0x0000000000000000000000000000000000000000000000000000000000001234,
                                ),
                            },
                            Package {
                                path: "home/user/some/immutable-package",
                                watch: None,
                            },
                        ],
                        network: None,
                    },
                ),
            ],
        }"#]];
    expect.assert_eq(&format!("{:#?}", config));
    Ok(())
}

#[test]
fn test_clone_command() -> anyhow::Result<()> {
    let source = RepositorySource {
        repository: "https://github.com/user/repo".into(),
        branches: vec![Branch {
            branch: "main".into(),
            paths: vec![
                Package {
                    path: "a".into(),
                    watch: None,
                },
                Package {
                    path: "b".into(),
                    watch: None,
                },
            ],
        }],
        network: Some(Network::Localnet),
    };

    let command = CloneCommand::new(
        &source,
        &source.branches[0],
        PathBuf::from("/foo").as_path(),
    )?;
    let expect = expect![
        r#"CloneCommand {
    args: [
        [
            "clone",
            "--no-checkout",
            "--depth=1",
            "--filter=tree:0",
            "--branch=main",
            "https://github.com/user/repo",
            "/foo/localnet/repo__main",
        ],
        [
            "-C",
            "/foo/localnet/repo__main",
            "sparse-checkout",
            "set",
            "--no-cone",
            "a",
            "b",
        ],
        [
            "-C",
            "/foo/localnet/repo__main",
            "checkout",
        ],
    ],
    repo_url: "https://github.com/user/repo",
}"#
    ];
    expect.assert_eq(&format!("{:#?}", command));
    Ok(())
}
