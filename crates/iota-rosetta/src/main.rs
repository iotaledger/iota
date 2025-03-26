// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::BTreeMap,
    fs,
    fs::File,
    io::BufReader,
    net::SocketAddr,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::anyhow;
use clap::Parser;
use fastcrypto::encoding::{Encoding, Hex};
use iota_config::{
    Config, IOTA_FULLNODE_CONFIG, IOTA_KEYSTORE_FILENAME, NodeConfig, iota_config_dir,
};
use iota_node::IotaNode;
use iota_rosetta::{
    IOTA, RosettaOfflineServer, RosettaOnlineServer,
    types::{CurveType, IotaEnv, PrefundedAccount},
};
use iota_sdk::{IotaClient, IotaClientBuilder};
use iota_types::{
    base_types::IotaAddress,
    crypto::{IotaKeyPair, KeypairTraits, ToFromBytes},
};
use serde_json::{Value, json};
use tracing::{info, log::warn};

#[derive(Parser)]
#[command(name = "iota-rosetta", author, version)]
pub enum RosettaServerCommand {
    GenerateRosettaCLIConfig {
        #[arg(long)]
        keystore_path: Option<PathBuf>,
        #[arg(long, default_value = "localnet")]
        env: IotaEnv,
        #[arg(long, default_value = "http://rosetta-online:9002")]
        online_url: String,
        #[arg(long, default_value = "http://rosetta-offline:9003")]
        offline_url: String,
    },
    StartOnlineRemoteServer {
        #[arg(long, default_value = "localnet")]
        env: IotaEnv,
        #[arg(long, default_value = "0.0.0.0:9002")]
        addr: SocketAddr,
        #[arg(long)]
        full_node_url: String,
        #[arg(long, default_value = "/data")]
        data_path: PathBuf,
    },
    StartOnlineServer {
        #[arg(long, default_value = "localnet")]
        env: IotaEnv,
        #[arg(long, default_value = "0.0.0.0:9002")]
        addr: SocketAddr,
        #[arg(long)]
        node_config: Option<PathBuf>,
        #[arg(long, default_value = "/data")]
        data_path: PathBuf,
    },
    StartOfflineServer {
        #[arg(long, default_value = "localnet")]
        env: IotaEnv,
        #[arg(long, default_value = "0.0.0.0:9003")]
        addr: SocketAddr,
    },
}

impl RosettaServerCommand {
    async fn execute(self) -> Result<(), anyhow::Error> {
        match self {
            RosettaServerCommand::GenerateRosettaCLIConfig {
                keystore_path,
                env,
                online_url,
                offline_url,
            } => {
                let path = keystore_path
                    .unwrap_or_else(|| iota_config_dir().unwrap().join(IOTA_KEYSTORE_FILENAME));

                let prefunded_accounts = read_prefunded_account(&path)?;

                info!(
                    "Retrieved {} IOTA address from keystore file {:?}",
                    prefunded_accounts.len(),
                    &path
                );

                let mut config: Value =
                    serde_json::from_str(include_str!("../resources/rosetta_cli.json"))?;

                config
                    .as_object_mut()
                    .unwrap()
                    .insert("online_url".into(), json!(online_url));

                // Set network.
                let network = config.pointer_mut("/network").ok_or_else(|| {
                    anyhow!("Cannot find construction config in default config file.")
                })?;
                network
                    .as_object_mut()
                    .unwrap()
                    .insert("network".into(), json!(env));

                // Add prefunded accounts.
                let construction = config.pointer_mut("/construction").ok_or_else(|| {
                    anyhow!("Cannot find construction config in default config file.")
                })?;

                let construction = construction.as_object_mut().unwrap();
                construction.insert("prefunded_accounts".into(), json!(prefunded_accounts));
                construction.insert("offline_url".into(), json!(offline_url));

                let config_path = PathBuf::from(".").join("rosetta_cli.json");
                fs::write(&config_path, serde_json::to_string_pretty(&config)?)?;
                info!(
                    "Rosetta CLI configuration file is stored in {:?}",
                    config_path
                );

                let dsl_path = PathBuf::from(".").join("iota.ros");
                let dsl = include_str!("../resources/iota.ros");
                fs::write(
                    &dsl_path,
                    dsl.replace("{{iota.env}}", json!(env).as_str().unwrap()),
                )?;
                info!("Rosetta DSL file is stored in {:?}", dsl_path);
            }
            RosettaServerCommand::StartOfflineServer { env, addr } => {
                info!("Starting Rosetta Offline Server.");
                let server = RosettaOfflineServer::new(env);
                server.serve(addr).await;
            }
            RosettaServerCommand::StartOnlineRemoteServer {
                env,
                addr,
                full_node_url,
                data_path,
            } => {
                info!(
                    "Starting Rosetta Online Server with remove IOTA full node [{full_node_url}]."
                );
                let iota_client = wait_for_iota_client(full_node_url).await;
                let rosetta_path = data_path.join("rosetta_db");
                info!("Rosetta db path : {rosetta_path:?}");
                let rosetta = RosettaOnlineServer::new(env, iota_client);
                rosetta.serve(addr).await;
            }

            RosettaServerCommand::StartOnlineServer {
                env,
                addr,
                node_config,
                data_path,
            } => {
                info!("Starting Rosetta Online Server with embedded IOTA full node.");
                info!("Data directory path: {data_path:?}");

                let node_config = node_config.unwrap_or_else(|| {
                    let path = iota_config_dir().unwrap().join(IOTA_FULLNODE_CONFIG);
                    info!("Using default node config from {path:?}");
                    path
                });

                let mut config = NodeConfig::load(&node_config)?;
                config.db_path = data_path.join("iota_db");
                info!("Overriding IOTA db path to : {:?}", config.db_path);

                let registry_service =
                    iota_metrics::start_prometheus_server(config.metrics_address);
                // Staring a full node for the rosetta server.
                let rpc_address = format!("http://127.0.0.1:{}", config.json_rpc_address.port());
                let _node = IotaNode::start(config, registry_service, None).await?;

                let iota_client = wait_for_iota_client(rpc_address).await;

                let rosetta_path = data_path.join("rosetta_db");
                info!("Rosetta db path : {rosetta_path:?}");
                let rosetta = RosettaOnlineServer::new(env, iota_client);
                rosetta.serve(addr).await;
            }
        };
        Ok(())
    }
}

async fn wait_for_iota_client(rpc_address: String) -> IotaClient {
    loop {
        match IotaClientBuilder::default().build(&rpc_address).await {
            Ok(client) => return client,
            Err(e) => {
                warn!(
                    "Error connecting to IOTA RPC server [{rpc_address}]: {e}, retrying in 5 seconds."
                );
                tokio::time::sleep(Duration::from_millis(5000)).await;
            }
        }
    }
}

/// This method reads the keypairs from the IOTA keystore to create the
/// PrefundedAccount objects, PrefundedAccount will be written to the
/// rosetta-cli config file for testing.
fn read_prefunded_account(path: &Path) -> Result<Vec<PrefundedAccount>, anyhow::Error> {
    let reader = BufReader::new(File::open(path).unwrap());
    let kp_strings: Vec<String> = serde_json::from_reader(reader).unwrap();
    let keys = kp_strings
        .iter()
        .map(|kpstr| {
            let key = IotaKeyPair::decode(kpstr);
            key.map(|k| (IotaAddress::from(&k.public()), k))
        })
        .collect::<Result<BTreeMap<_, _>, _>>()
        .unwrap();

    Ok(keys
        .into_iter()
        .map(|(address, key)| {
            let (privkey, curve_type) = match key {
                IotaKeyPair::Ed25519(k) => {
                    (Hex::encode(k.private().as_bytes()), CurveType::Edwards25519)
                }
                IotaKeyPair::Secp256k1(k) => {
                    (Hex::encode(k.private().as_bytes()), CurveType::Secp256k1)
                }
                IotaKeyPair::Secp256r1(k) => {
                    (Hex::encode(k.private().as_bytes()), CurveType::Secp256r1)
                }
            };
            PrefundedAccount {
                privkey,
                account_identifier: address.into(),
                curve_type,
                currency: IOTA.clone(),
            }
        })
        .collect())
}

#[test]
fn test_read_keystore() {
    use iota_keys::keystore::{AccountKeystore, FileBasedKeystore, Keystore};
    use iota_types::crypto::SignatureScheme;

    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("iota.keystore");
    let mut ks = Keystore::from(FileBasedKeystore::new(&path).unwrap());
    let key1 = ks
        .generate_and_add_new_key(SignatureScheme::ED25519, None, None, None)
        .unwrap();
    let key2 = ks
        .generate_and_add_new_key(SignatureScheme::Secp256k1, None, None, None)
        .unwrap();

    let accounts = read_prefunded_account(&path).unwrap();
    let acc_map = accounts
        .into_iter()
        .map(|acc| (acc.account_identifier.address, acc))
        .collect::<BTreeMap<_, _>>();

    assert_eq!(2, acc_map.len());
    assert!(acc_map.contains_key(&key1.0));
    assert!(acc_map.contains_key(&key2.0));

    let acc1 = acc_map[&key1.0].clone();
    let acc2 = acc_map[&key2.0].clone();

    let schema1: SignatureScheme = acc1.curve_type.into();
    let schema2: SignatureScheme = acc2.curve_type.into();
    assert!(matches!(schema1, SignatureScheme::ED25519));
    assert!(matches!(schema2, SignatureScheme::Secp256k1));
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let cmd: RosettaServerCommand = RosettaServerCommand::parse();

    let (_guard, _) = telemetry_subscribers::TelemetryConfig::new()
        .with_env()
        .init();

    cmd.execute().await
}
