// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::BTreeSet,
    fs,
    net::SocketAddr,
    path::{Path, PathBuf},
    time::Duration,
};

#[cfg(feature = "indexer")]
use clap::Parser;
use iota_config::{
    Config, IOTA_CLIENT_CONFIG, IOTA_FULLNODE_CONFIG, IOTA_GENESIS_FILENAME,
    IOTA_KEYSTORE_FILENAME, IOTA_NETWORK_CONFIG, NodeConfig, PersistedConfig,
};
use iota_keys::keystore::AccountKeystore;
#[cfg(feature = "indexer")]
use iota_localnet::commands::IndexerFeatureArgs;
use iota_localnet::commands::{LocalnetCommand, parse_host_port};
use iota_macros::sim_test;
use iota_sdk::iota_client_config::IotaClientConfig;
use iota_swarm_config::{
    genesis_config::DEFAULT_NUMBER_OF_AUTHORITIES, network_config::PersistedNetworkConfig,
};
use iota_types::traffic_control::PolicyConfig;

fn genesis_command(working_dir: &Path, committee_size: usize) -> LocalnetCommand {
    LocalnetCommand::Genesis {
        working_dir: Some(working_dir.to_path_buf()),
        write_config: None,
        force: false,
        from_config: None,
        epoch_duration_ms: None,
        benchmark_ips: None,
        with_faucet: false,
        committee_size,
        num_additional_gas_accounts: None,
        chain_start_timestamp_ms: None,
        admin_interface_address: None,
    }
}

/// The port `start_command` gives the fullnode's JSON-RPC endpoint.
const FULLNODE_RPC_PORT: u16 = 9000;

fn start_command(
    config_dir: &Path,
    write_config: Option<PathBuf>,
    node_config_override: Vec<String>,
) -> LocalnetCommand {
    LocalnetCommand::Start {
        #[cfg(feature = "indexer")]
        data_ingestion_dir: None,
        config_dir: Some(config_dir.to_path_buf()),
        no_full_node: false,
        disable_fullnode_pruning: false,
        force_regenesis: false,
        with_faucet: None,
        faucet_amount: None,
        faucet_coin_count: None,
        with_grpc: None,
        node_config_override,
        fullnode_rpc_port: FULLNODE_RPC_PORT,
        committee_size: None,
        epoch_duration_ms: None,
        #[cfg(feature = "indexer")]
        indexer_feature_args: Box::new(IndexerFeatureArgs::for_testing()),
        write_config,
    }
}

fn file_names(directory: &Path) -> Vec<String> {
    fs::read_dir(directory)
        .unwrap()
        .flat_map(|entry| entry.map(|entry| entry.file_name().to_str().unwrap().to_owned()))
        .collect()
}

#[sim_test]
async fn test_genesis() -> Result<(), anyhow::Error> {
    let tmp_dir = iota_common::tempdir();
    let working_dir = tmp_dir.path();

    // Genesis
    genesis_command(working_dir, DEFAULT_NUMBER_OF_AUTHORITIES)
        .execute()
        .await?;

    // Get all the new file names
    let files = file_names(working_dir);

    // Genesis writes the network's state, not the node configs derived from
    // it: those come from `iota-localnet start --write-config`.
    assert_eq!(4, files.len(), "{files:?}");
    assert!(files.contains(&IOTA_CLIENT_CONFIG.to_string()));
    assert!(files.contains(&IOTA_NETWORK_CONFIG.to_string()));
    assert!(files.contains(&IOTA_GENESIS_FILENAME.to_string()));
    assert!(files.contains(&IOTA_KEYSTORE_FILENAME.to_string()));
    assert!(!files.contains(&IOTA_FULLNODE_CONFIG.to_string()));

    // Check network config
    let network_config = PersistedNetworkConfig::read(working_dir)?;
    assert_eq!(
        DEFAULT_NUMBER_OF_AUTHORITIES,
        network_config
            .genesis_config
            .validator_config_info
            .as_ref()
            .unwrap()
            .len()
    );
    assert!(network_config.genesis_config.fullnode_config_info.is_some());

    // Check wallet config
    let wallet_conf =
        PersistedConfig::<IotaClientConfig>::read(&working_dir.join(IOTA_CLIENT_CONFIG))?;

    assert!(!wallet_conf.envs().is_empty());

    assert_eq!(5, wallet_conf.keystore().addresses().len());

    // Genesis 2nd time should fail
    let result = genesis_command(working_dir, DEFAULT_NUMBER_OF_AUTHORITIES)
        .execute()
        .await;
    assert!(matches!(result, Err(..)));

    tmp_dir.close()?;
    Ok(())
}

/// A genesis config that names its validators is a deployment's, and `genesis`
/// writes its validator config files. The private network's `bootstrap.sh`
/// mounts them, and its template names no state sync fullnodes.
#[tokio::test]
async fn genesis_from_config_writes_the_validator_configs() -> Result<(), anyhow::Error> {
    let source_dir = iota_common::tempdir();
    let tmp_dir = iota_common::tempdir();
    let working_dir = tmp_dir.path();

    genesis_command(source_dir.path(), 2).execute().await?;
    let genesis_config = PersistedNetworkConfig::read(source_dir.path())?.genesis_config;
    assert!(genesis_config.ssfn_config_info.is_none());
    let validator_configs: Vec<String> = genesis_config
        .validator_config_info
        .iter()
        .flatten()
        .enumerate()
        .map(|(index, validator)| {
            iota_config::validator_config_file(validator.network_address.clone(), index)
        })
        .collect();
    let config_path = source_dir.path().join("genesis-config.yaml");
    genesis_config.persisted(&config_path).save()?;

    let mut command = genesis_command(working_dir, 2);
    let LocalnetCommand::Genesis { from_config, .. } = &mut command else {
        unreachable!("genesis_command builds a genesis command")
    };
    *from_config = Some(config_path);
    command.execute().await?;

    let files = file_names(working_dir);
    for name in validator_configs {
        assert!(files.contains(&name), "{files:?}");
    }

    tmp_dir.close()?;
    Ok(())
}

/// Genesis does not pin the fullnode's address. The simulator gives every node
/// an address of its own, and routes loopback addresses back to the caller. A
/// fullnode entry on 127.0.0.1 could therefore never reach the validators.
#[sim_test]
async fn the_persisted_fullnode_entry_keeps_its_own_address() -> Result<(), anyhow::Error> {
    let tmp_dir = iota_common::tempdir();
    let working_dir = tmp_dir.path();
    genesis_command(working_dir, 2).execute().await?;

    let genesis_config = PersistedNetworkConfig::read(working_dir)?.genesis_config;
    let fullnode = genesis_config.fullnode_config_info.unwrap();
    let node_ip = fullnode.network_address.to_socket_addr().unwrap().ip();

    // Outside the simulator every node is on localhost, inside it none is.
    for validator in genesis_config.validator_config_info.unwrap() {
        assert_eq!(
            validator
                .network_address
                .to_socket_addr()
                .unwrap()
                .ip()
                .is_loopback(),
            node_ip.is_loopback(),
            "the fullnode and the validators are not both on localhost"
        );
    }

    tmp_dir.close()?;
    Ok(())
}

/// The persisted network config is what every later run derives its node
/// configs from. A read followed by a write must not change it.
#[tokio::test]
async fn the_persisted_network_config_round_trips() -> Result<(), anyhow::Error> {
    let tmp_dir = iota_common::tempdir();
    let working_dir = tmp_dir.path();
    genesis_command(working_dir, 2).execute().await?;

    let network_config_path = working_dir.join(IOTA_NETWORK_CONFIG);
    let written = fs::read_to_string(&network_config_path)?;
    assert!(
        written.contains("fullnode_config_info"),
        "the fullnode entry is not persisted"
    );

    let network_config = PersistedNetworkConfig::read(working_dir)?;
    let round_tripped_path = working_dir.join("round-tripped.yaml");
    network_config.save(&round_tripped_path)?;

    assert_eq!(written, fs::read_to_string(&round_tripped_path)?);

    tmp_dir.close()?;
    Ok(())
}

/// The port layout documented in
/// `docs/content/developer/references/cli/localnet.mdx`.
#[tokio::test]
async fn genesis_gives_every_node_fixed_ports() -> Result<(), anyhow::Error> {
    let tmp_dir = iota_common::tempdir();
    let working_dir = tmp_dir.path();

    genesis_command(working_dir, 4).execute().await?;

    let network_config = PersistedNetworkConfig::read(working_dir)?;
    let validators = network_config
        .genesis_config
        .validator_config_info
        .as_ref()
        .unwrap();
    let mut ports = Vec::new();

    for (i, validator) in validators.iter().enumerate() {
        let base = 9200 + 10 * i as u16;
        assert_eq!(
            validator.network_address.to_string(),
            format!("/ip4/127.0.0.1/tcp/{base}/http")
        );
        assert_eq!(
            validator.p2p_address.to_string(),
            format!("/ip4/127.0.0.1/udp/{}/http", base + 1)
        );
        assert_eq!(
            validator.metrics_address,
            SocketAddr::from(([127, 0, 0, 1], base + 2))
        );
        assert_eq!(
            validator.primary_address.to_string(),
            format!("/ip4/127.0.0.1/udp/{}/http", base + 3)
        );
        assert_eq!(
            validator.admin_interface_address,
            SocketAddr::from(([127, 0, 0, 1], base + 4))
        );
        ports.extend([base, base + 1, base + 2, base + 3, base + 4]);
    }

    let fullnode = network_config
        .genesis_config
        .fullnode_config_info
        .as_ref()
        .unwrap();
    assert_eq!(
        fullnode.metrics_address,
        SocketAddr::from(([127, 0, 0, 1], 9184))
    );
    assert_eq!(
        fullnode.admin_interface_address,
        SocketAddr::from(([127, 0, 0, 1], 9185))
    );
    assert_eq!(
        fullnode.p2p_address.to_string(),
        "/ip4/127.0.0.1/udp/9186/http"
    );
    ports.extend([9184, 9185, 9186]);

    assert_eq!(
        ports.iter().collect::<BTreeSet<_>>().len(),
        ports.len(),
        "two nodes share a port: {ports:?}"
    );

    tmp_dir.close()?;
    Ok(())
}

/// Waits for the fullnode to execute a checkpoint, which it can only get by
/// syncing from the validators.
#[cfg(not(msim))]
async fn wait_until_the_fullnode_syncs(rpc_port: u16) {
    use iota_sdk::IotaClientBuilder;

    let url = format!("http://127.0.0.1:{rpc_port}");
    let synced = tokio::time::timeout(Duration::from_secs(120), async {
        loop {
            if let Ok(client) = IotaClientBuilder::default().build(&url).await {
                if let Ok(checkpoint) = client
                    .read_api()
                    .get_latest_checkpoint_sequence_number()
                    .await
                {
                    // Checkpoint 0 comes from the genesis blob the fullnode
                    // already has, so only a later one is synced.
                    if checkpoint > 0 {
                        return;
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    })
    .await;

    assert!(synced.is_ok(), "the fullnode executed no checkpoint");
}

/// The simulator gives every node an address of its own and routes loopback
/// addresses back to the caller, so the fullnode's JSON-RPC endpoint, which
/// `start` binds on all interfaces without reporting the address, cannot be
/// reached from the test. Give the network time to come up instead.
#[cfg(msim)]
async fn wait_until_the_fullnode_syncs(_rpc_port: u16) {
    tokio::time::sleep(Duration::from_secs(10)).await;
}

/// A node config directory written before the format carried a version, and
/// one written in a version this build does not know, are both rejected.
#[tokio::test]
async fn a_network_config_this_build_cannot_read_is_rejected() -> Result<(), anyhow::Error> {
    let tmp_dir = iota_common::tempdir();
    let working_dir = tmp_dir.path();
    genesis_command(working_dir, 1).execute().await?;

    let network_config_path = working_dir.join(IOTA_NETWORK_CONFIG);
    let network_config = fs::read_to_string(&network_config_path)?;
    let version_line = format!("version: {}\n", PersistedNetworkConfig::VERSION);
    assert!(network_config.contains(&version_line), "{network_config}");

    // A file without a version predates the field and reads as older.
    fs::write(
        &network_config_path,
        network_config.replace(&version_line, ""),
    )?;
    let err = start_command(working_dir, None, vec![])
        .execute()
        .await
        .unwrap_err();
    let err = format!("{err:#}");
    assert!(
        err.contains("was created by an older version of iota-localnet"),
        "{err}"
    );
    assert!(err.contains("iota-localnet genesis --force"), "{err}");

    // A newer version must not advise `genesis --force`, which deletes the
    // newer network's state.
    fs::write(
        &network_config_path,
        network_config.replace(&version_line, "version: 4294967295\n"),
    )?;
    let err = start_command(working_dir, None, vec![])
        .execute()
        .await
        .unwrap_err();
    let err = format!("{err:#}");
    assert!(
        err.contains("was created by a newer version of iota-localnet"),
        "{err}"
    );
    assert!(err.contains("Update iota-localnet"), "{err}");
    assert!(!err.contains("genesis --force"), "{err}");

    tmp_dir.close()?;
    Ok(())
}

/// `--write-config` writes the configs the run would have started, and nothing
/// else: no network, no database, no data ingestion directory.
#[tokio::test]
async fn write_config_writes_runnable_configs_and_starts_nothing() -> Result<(), anyhow::Error> {
    let tmp_dir = iota_common::tempdir();
    let working_dir = tmp_dir.path();
    let config_tmp_dir = iota_common::tempdir();
    let config_dir = config_tmp_dir.path().join("node-configs");
    genesis_command(working_dir, 2).execute().await?;

    let files_before = file_names(working_dir);
    start_command(
        working_dir,
        Some(config_dir.clone()),
        vec!["fullnode:enable-index-processing=false".to_owned()],
    )
    .execute()
    .await?;

    let mut written = file_names(&config_dir);
    written.sort();
    assert_eq!(
        written,
        vec![
            "127.0.0.1-9200.yaml".to_owned(),
            "127.0.0.1-9210.yaml".to_owned(),
            IOTA_FULLNODE_CONFIG.to_owned(),
        ]
    );

    // The run left no state behind: no database, no data ingestion directory,
    // and no node config in the config directory either.
    let mut files_after = file_names(working_dir);
    files_after.sort();
    let mut files_before = files_before;
    files_before.sort();
    assert_eq!(files_before, files_after);

    for name in written {
        let path = config_dir.join(&name);
        let contents = fs::read_to_string(&path)?;
        assert!(
            contents.starts_with("# Generated by `iota-localnet start --write-config`"),
            "{name} has no header: {contents:.200}"
        );
        assert!(
            contents.contains("--node-config-override"),
            "{name} does not say how to change what the run uses"
        );
        assert!(
            contents.contains("an explicit `null`"),
            "{name} does not say how an absent key reads"
        );

        // Runnable: a node reads its config with `PersistedConfig` and
        // validates it before it starts.
        let config = PersistedConfig::<NodeConfig>::read(&path)?;
        config.validate()?;
        assert_eq!(config.genesis.genesis()?.epoch(), 0);
        assert!(config.db_path.starts_with(working_dir), "{name}");
    }

    // The override reached the fullnode, and only the fullnode.
    let fullnode =
        PersistedConfig::<NodeConfig>::read(&config_dir.join(IOTA_FULLNODE_CONFIG)).unwrap();
    assert!(!fullnode.enable_index_processing);
    let validator =
        PersistedConfig::<NodeConfig>::read(&config_dir.join("127.0.0.1-9200.yaml")).unwrap();
    assert!(validator.enable_index_processing);

    tmp_dir.close()?;
    config_tmp_dir.close()?;
    Ok(())
}

/// A `--force-regenesis` run keeps its state in a temporary directory that is
/// gone once the command exits. The configs would therefore name paths that no
/// longer exist.
#[tokio::test]
async fn write_config_is_rejected_under_force_regenesis() {
    let tmp_dir = iota_common::tempdir();
    let mut command = start_command(
        tmp_dir.path(),
        Some(tmp_dir.path().join("node-configs")),
        vec![],
    );
    let LocalnetCommand::Start {
        config_dir,
        force_regenesis,
        ..
    } = &mut command
    else {
        unreachable!("start_command builds a start command")
    };
    *config_dir = None;
    *force_regenesis = true;

    let err = format!("{:#}", command.execute().await.unwrap_err());
    assert!(
        err.contains("`--force-regenesis` and `--write-config`"),
        "{err}"
    );
}

/// The faucet is a service of a running network, and `--write-config` starts
/// no network to serve.
#[tokio::test]
async fn write_config_is_rejected_with_the_faucet() {
    let tmp_dir = iota_common::tempdir();
    let mut command = start_command(
        tmp_dir.path(),
        Some(tmp_dir.path().join("node-configs")),
        vec![],
    );
    let LocalnetCommand::Start { with_faucet, .. } = &mut command else {
        unreachable!("start_command builds a start command")
    };
    *with_faucet = Some("0.0.0.0:9123".to_owned());

    let err = format!("{:#}", command.execute().await.unwrap_err());
    assert!(
        err.contains("`--with-faucet` and `--write-config`"),
        "{err}"
    );
}

/// A `--with-indexer` run without `--data-ingestion-dir` keeps the fullnode's
/// data ingestion directory in a temporary directory that is gone once the
/// command exits.
#[cfg(feature = "indexer")]
#[tokio::test]
async fn write_config_is_rejected_with_the_indexer() {
    let tmp_dir = iota_common::tempdir();
    let config_dir = tmp_dir.path().to_str().unwrap();
    let node_configs = tmp_dir.path().join("node-configs");
    let command = LocalnetCommand::parse_from([
        "iota-localnet",
        "start",
        "--with-indexer",
        "--network.config",
        config_dir,
        "--write-config",
        node_configs.to_str().unwrap(),
    ]);

    let err = format!("{:#}", command.execute().await.unwrap_err());
    assert!(
        err.contains("`--with-indexer` or `--with-graphql`"),
        "{err}"
    );
}

/// A derived node is a real node, so it gets the default denial-of-service
/// protection rather than the killswitch the swarm builders leave behind.
#[tokio::test]
async fn derived_node_configs_carry_the_default_traffic_control_policy() -> Result<(), anyhow::Error>
{
    let tmp_dir = iota_common::tempdir();
    let working_dir = tmp_dir.path();
    let config_tmp_dir = iota_common::tempdir();
    let config_dir = config_tmp_dir.path().join("node-configs");
    genesis_command(working_dir, 2).execute().await?;

    start_command(working_dir, Some(config_dir.clone()), vec![])
        .execute()
        .await?;

    // `PolicyConfig` has no `PartialEq`, and the serialized forms are what a
    // node reads back anyway.
    let expected = serde_yaml::to_string(&PolicyConfig::default_dos_protection_policy())?;
    let names = file_names(&config_dir);
    assert!(!names.is_empty());
    for name in names {
        let config = PersistedConfig::<NodeConfig>::read(&config_dir.join(&name))?;
        let policy = config
            .policy_config
            .ok_or_else(|| anyhow::anyhow!("{name} has no traffic control policy"))?;
        assert_eq!(serde_yaml::to_string(&policy)?, expected, "{name}");
    }

    // An override still reaches the policy, and can still clear it.
    let cleared = config_tmp_dir.path().join("cleared");
    start_command(
        working_dir,
        Some(cleared.clone()),
        vec!["policy-config=".to_owned()],
    )
    .execute()
    .await?;
    for name in file_names(&cleared) {
        let config = PersistedConfig::<NodeConfig>::read(&cleared.join(&name))?;
        assert!(config.policy_config.is_none(), "{name}");
    }

    tmp_dir.close()?;
    config_tmp_dir.close()?;
    Ok(())
}

/// Two runs against one config directory derive the same node configs, which
/// is what lets a persisted network keep its databases.
#[tokio::test]
async fn two_runs_derive_the_same_node_configs() -> Result<(), anyhow::Error> {
    let tmp_dir = iota_common::tempdir();
    let working_dir = tmp_dir.path();
    let config_tmp_dir = iota_common::tempdir();
    genesis_command(working_dir, 2).execute().await?;

    let first = config_tmp_dir.path().join("first");
    let second = config_tmp_dir.path().join("second");
    for directory in [&first, &second] {
        start_command(working_dir, Some(directory.clone()), vec![])
            .execute()
            .await?;
    }

    let names = file_names(&first);
    assert!(!names.is_empty());
    for name in names {
        assert_eq!(
            fs::read_to_string(first.join(&name))?,
            fs::read_to_string(second.join(&name))?,
            "the second run derived a different {name}"
        );
    }

    tmp_dir.close()?;
    config_tmp_dir.close()?;
    Ok(())
}

#[sim_test]
async fn test_start() -> Result<(), anyhow::Error> {
    let tmp_dir = iota_common::tempdir();
    let working_dir = tmp_dir.path();

    // `start` runs until the network fails, so race it against the fullnode.
    tokio::select! {
        result = start_command(working_dir, None, vec![]).execute() => {
            result?;
            unreachable!("the local network stops on failure only");
        }
        () = wait_until_the_fullnode_syncs(FULLNODE_RPC_PORT) => {}
    }

    // Get all the new file names
    let files = file_names(working_dir);
    assert!(files.contains(&IOTA_CLIENT_CONFIG.to_string()));
    assert!(files.contains(&IOTA_NETWORK_CONFIG.to_string()));
    assert!(files.contains(&IOTA_GENESIS_FILENAME.to_string()));
    assert!(files.contains(&IOTA_KEYSTORE_FILENAME.to_string()));

    // Check network config
    let network_config = PersistedNetworkConfig::read(working_dir)?;
    assert_eq!(
        1,
        network_config
            .genesis_config
            .validator_config_info
            .as_ref()
            .unwrap()
            .len()
    );

    // Check wallet config
    let wallet_conf =
        PersistedConfig::<IotaClientConfig>::read(&working_dir.join(IOTA_CLIENT_CONFIG))?;

    assert!(!wallet_conf.envs().is_empty());

    assert_eq!(5, wallet_conf.keystore().addresses().len());

    tmp_dir.close()?;
    Ok(())
}

/// The simulator panics when two nodes share an IP, so a second validator
/// catches a layout that pins addresses instead of ports.
#[sim_test]
async fn start_works_with_a_committee_of_two() -> Result<(), anyhow::Error> {
    let tmp_dir = iota_common::tempdir();
    let working_dir = tmp_dir.path();

    let mut command = start_command(working_dir, None, vec![]);
    let LocalnetCommand::Start { committee_size, .. } = &mut command else {
        unreachable!("start_command builds a start command")
    };
    *committee_size = Some(2);

    if let Ok(res) = tokio::time::timeout(Duration::from_secs(10), command.execute()).await {
        res.unwrap();
    };

    let network_config = PersistedNetworkConfig::read(working_dir)?;
    assert_eq!(
        2,
        network_config
            .genesis_config
            .validator_config_info
            .as_ref()
            .unwrap()
            .len()
    );

    tmp_dir.close()?;
    Ok(())
}

#[tokio::test]
async fn test_parse_host_port() {
    let input = "127.0.0.0";
    let result = parse_host_port(input.to_string(), 9123).unwrap();
    assert_eq!(result, "127.0.0.0:9123".parse::<SocketAddr>().unwrap());

    let input = "127.0.0.5:9124";
    let result = parse_host_port(input.to_string(), 9123).unwrap();
    assert_eq!(result, "127.0.0.5:9124".parse::<SocketAddr>().unwrap());

    let input = "9090";
    let result = parse_host_port(input.to_string(), 9123).unwrap();
    assert_eq!(result, "0.0.0.0:9090".parse::<SocketAddr>().unwrap());

    let input = "";
    let result = parse_host_port(input.to_string(), 9123).unwrap();
    assert_eq!(result, "0.0.0.0:9123".parse::<SocketAddr>().unwrap());

    let result = parse_host_port("localhost".to_string(), 9899).unwrap();
    assert_eq!(result, "127.0.0.1:9899".parse::<SocketAddr>().unwrap());

    let input = "asg";
    assert!(parse_host_port(input.to_string(), 9123).is_err());
    let input = "127.0.0:900";
    assert!(parse_host_port(input.to_string(), 9123).is_err());
    let input = "127.0.0";
    assert!(parse_host_port(input.to_string(), 9123).is_err());
    let input = "127.";
    assert!(parse_host_port(input.to_string(), 9123).is_err());
    let input = "127.9.0.1:asb";
    assert!(parse_host_port(input.to_string(), 9123).is_err());
}
