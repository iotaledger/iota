// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! A tool to semi automate fire drills. It still requires some manual work
//! today. For example,
//! 1. update iptables for new tpc/udp ports
//! 2. restart the node in a new epoch when config file will be reloaded and
//!    take effects
//!
//! Example usage:
//! iota fire-drill metadata-rotation \
//! --iota-node-config-path validator.yaml \
//! --account-key-path account.key \
//! --fullnode-rpc-url http://fullnode-my-local-net:9000

use std::path::{Path, PathBuf};

use anyhow::bail;
use clap::*;
use fastcrypto::{
    ed25519::Ed25519KeyPair,
    traits::{KeyPair, ToFromBytes},
};
use iota_config::{
    Config, NodeConfig, PersistedConfig, local_ip_utils,
    node::{AuthorityKeyPairWithPath, KeyPairWithPath},
};
use iota_json_rpc_types::{IotaExecutionStatus, IotaTransactionBlockResponseOptions};
use iota_keys::keypair_file::read_keypair_from_file;
use iota_sdk::{IotaClient, IotaClientBuilder, rpc_types::IotaTransactionBlockEffectsAPI};
use iota_types::{
    IOTA_SYSTEM_PACKAGE_ID,
    base_types::{IotaAddress, ObjectRef},
    committee::EpochId,
    crypto::{IotaKeyPair, generate_proof_of_possession, get_authority_key_pair, get_key_pair},
    multiaddr::{Multiaddr, Protocol},
    transaction::{CallArg, TEST_ONLY_GAS_UNIT_FOR_GENERIC, Transaction, TransactionData},
};
use move_core_types::ident_str;
use tracing::info;

#[derive(Parser)]
pub enum FireDrill {
    MetadataRotation(MetadataRotation),
}

#[derive(Parser)]
pub struct MetadataRotation {
    /// Path to iota node config.
    #[arg(long)]
    iota_node_config_path: PathBuf,
    /// Path to account key file.
    #[arg(long)]
    account_key_path: PathBuf,
    /// Jsonrpc url for a reliable fullnode.
    #[arg(long)]
    fullnode_rpc_url: String,
}

pub async fn run_fire_drill(fire_drill: FireDrill) -> anyhow::Result<()> {
    match fire_drill {
        FireDrill::MetadataRotation(metadata_rotation) => {
            run_metadata_rotation(metadata_rotation).await?;
        }
    }
    Ok(())
}

async fn run_metadata_rotation(metadata_rotation: MetadataRotation) -> anyhow::Result<()> {
    let MetadataRotation {
        iota_node_config_path,
        account_key_path,
        fullnode_rpc_url,
    } = metadata_rotation;
    let account_key = read_keypair_from_file(&account_key_path)?;
    let config: NodeConfig = PersistedConfig::read(&iota_node_config_path).map_err(|err| {
        err.context(format!(
            "Cannot open IOTA Node Config file at {:?}",
            iota_node_config_path
        ))
    })?;

    let iota_client = IotaClientBuilder::default().build(fullnode_rpc_url).await?;
    let iota_address = IotaAddress::from(&account_key.public());
    let starting_epoch = current_epoch(&iota_client).await?;
    info!(
        "Running Metadata Rotation fire drill for validator address {iota_address} in epoch {starting_epoch}."
    );

    // Prepare new metadata for next epoch
    let new_config_path =
        update_next_epoch_metadata(&iota_node_config_path, &config, &iota_client, &account_key)
            .await?;

    let current_epoch = current_epoch(&iota_client).await?;
    if current_epoch > starting_epoch {
        bail!("Epoch already advanced to {current_epoch}");
    }
    let target_epoch = starting_epoch + 1;
    wait_for_next_epoch(&iota_client, target_epoch).await?;
    info!("Just advanced to epoch {target_epoch}");

    // Replace new config
    std::fs::rename(new_config_path, iota_node_config_path)?;
    info!("Updated IOTA Node config.");

    Ok(())
}

// TODO move this to a shared lib
pub async fn get_gas_obj_ref(
    iota_address: IotaAddress,
    iota_client: &IotaClient,
    minimal_gas_balance: u64,
) -> anyhow::Result<ObjectRef> {
    let coins = iota_client
        .coin_read_api()
        .get_coins(iota_address, Some("0x2::iota::IOTA".into()), None, None)
        .await?
        .data;
    let gas_obj = coins.iter().find(|c| c.balance >= minimal_gas_balance);
    if gas_obj.is_none() {
        bail!("Validator doesn't have enough IOTA coins to cover transaction fees.");
    }
    Ok(gas_obj.unwrap().object_ref())
}

async fn update_next_epoch_metadata(
    iota_node_config_path: &Path,
    config: &NodeConfig,
    iota_client: &IotaClient,
    account_key: &IotaKeyPair,
) -> anyhow::Result<PathBuf> {
    // Save backup config just in case
    let mut backup_config_path = iota_node_config_path.to_path_buf();
    backup_config_path.pop();
    backup_config_path.push("node_config_backup.yaml");
    let backup_config = config.clone();
    backup_config.persisted(&backup_config_path).save()?;

    let iota_address = IotaAddress::from(&account_key.public());

    let mut new_config = config.clone();

    // authority key
    let new_authority_key_pair = get_authority_key_pair().1;
    let new_authority_key_pair_copy = new_authority_key_pair.copy();
    let pop = generate_proof_of_possession(&new_authority_key_pair, iota_address);
    new_config.authority_key_pair = AuthorityKeyPairWithPath::new(new_authority_key_pair);

    // network key
    let new_network_key_pair: Ed25519KeyPair = get_key_pair().1;
    let new_network_key_pair_copy = new_network_key_pair.copy();
    new_config.network_key_pair = KeyPairWithPath::new(IotaKeyPair::Ed25519(new_network_key_pair));

    // protocol key
    let new_protocol_key_pair: Ed25519KeyPair = get_key_pair().1;
    let new_protocol_key_pair_copy = new_protocol_key_pair.copy();
    new_config.protocol_key_pair =
        KeyPairWithPath::new(IotaKeyPair::Ed25519(new_protocol_key_pair));

    let validators = iota_client
        .governance_api()
        .get_latest_iota_system_state()
        .await?
        .active_validators;
    let self_validator = validators
        .iter()
        .find(|v| v.iota_address == iota_address)
        .unwrap();

    // Network address
    let mut new_network_address = Multiaddr::try_from(self_validator.net_address.clone()).unwrap();
    info!("Current network address: {:?}", new_network_address);
    let http = new_network_address.pop().unwrap();
    // pop out tcp
    new_network_address.pop().unwrap();
    let localhost = local_ip_utils::localhost_for_testing();
    let new_port = local_ip_utils::get_available_port(&localhost);
    new_network_address.push(Protocol::Tcp(new_port));
    new_network_address.push(http);
    info!("New network address: {:?}", new_network_address);
    new_config.network_address = new_network_address.clone();

    // p2p address
    let mut new_external_address = config.p2p_config.external_address.clone().unwrap();
    info!("Current P2P external address: {:?}", new_external_address);
    // pop out udp
    new_external_address.pop().unwrap();
    let new_port = local_ip_utils::get_available_port(&localhost);
    new_external_address.push(Protocol::Udp(new_port));
    info!("New P2P external address: {:?}", new_external_address);
    new_config.p2p_config.external_address = Some(new_external_address.clone());

    let mut new_listen_address = config.p2p_config.listen_address;
    info!("Current P2P local listen address: {:?}", new_listen_address);
    new_listen_address.set_port(new_port);
    info!("New P2P local listen address: {:?}", new_listen_address);
    new_config.p2p_config.listen_address = new_listen_address;

    // primary address
    let mut new_primary_addresses =
        Multiaddr::try_from(self_validator.primary_address.clone()).unwrap();
    info!("Current primary address: {:?}", new_primary_addresses);
    // pop out udp
    new_primary_addresses.pop().unwrap();
    let new_port = local_ip_utils::get_available_port(&localhost);
    new_primary_addresses.push(Protocol::Udp(new_port));
    info!("New primary address: {:?}", new_primary_addresses);

    // Save new config
    let mut new_config_path = iota_node_config_path.to_path_buf();
    new_config_path.pop();
    new_config_path.push(
        String::from(iota_node_config_path.file_name().unwrap().to_str().unwrap()) + ".next_epoch",
    );
    new_config.persisted(&new_config_path).save()?;

    // update protocol authority pubkey on chain
    update_metadata_on_chain(
        account_key,
        "update_validator_next_epoch_authority_pubkey",
        vec![
            CallArg::Pure(
                bcs::to_bytes(&new_authority_key_pair_copy.public().as_bytes().to_vec()).unwrap(),
            ),
            CallArg::Pure(bcs::to_bytes(&pop.as_bytes().to_vec()).unwrap()),
        ],
        iota_client,
    )
    .await?;

    // update network pubkey on chain
    update_metadata_on_chain(
        account_key,
        "update_validator_next_epoch_network_pubkey",
        vec![CallArg::Pure(
            bcs::to_bytes(&new_network_key_pair_copy.public().as_bytes().to_vec()).unwrap(),
        )],
        iota_client,
    )
    .await?;

    // update protocol pubkey on chain
    update_metadata_on_chain(
        account_key,
        "update_validator_next_epoch_protocol_pubkey",
        vec![CallArg::Pure(
            bcs::to_bytes(&new_protocol_key_pair_copy.public().as_bytes().to_vec()).unwrap(),
        )],
        iota_client,
    )
    .await?;

    // update network address
    update_metadata_on_chain(
        account_key,
        "update_validator_next_epoch_network_address",
        vec![CallArg::Pure(bcs::to_bytes(&new_network_address).unwrap())],
        iota_client,
    )
    .await?;

    // update p2p address
    update_metadata_on_chain(
        account_key,
        "update_validator_next_epoch_p2p_address",
        vec![CallArg::Pure(bcs::to_bytes(&new_external_address).unwrap())],
        iota_client,
    )
    .await?;

    // update primary address
    update_metadata_on_chain(
        account_key,
        "update_validator_next_epoch_primary_address",
        vec![CallArg::Pure(
            bcs::to_bytes(&new_primary_addresses).unwrap(),
        )],
        iota_client,
    )
    .await?;

    Ok(new_config_path)
}

async fn update_metadata_on_chain(
    account_key: &IotaKeyPair,
    function: &'static str,
    call_args: Vec<CallArg>,
    iota_client: &IotaClient,
) -> anyhow::Result<()> {
    let iota_address = IotaAddress::from(&account_key.public());
    let gas_obj_ref = get_gas_obj_ref(iota_address, iota_client, 10000 * 100).await?;
    let rgp = iota_client
        .governance_api()
        .get_reference_gas_price()
        .await?;
    let mut args = vec![CallArg::IOTA_SYSTEM_MUT];
    args.extend(call_args);
    let tx_data = TransactionData::new_move_call(
        iota_address,
        IOTA_SYSTEM_PACKAGE_ID,
        ident_str!("iota_system").to_owned(),
        ident_str!(function).to_owned(),
        vec![],
        gas_obj_ref,
        args,
        rgp * TEST_ONLY_GAS_UNIT_FOR_GENERIC,
        rgp,
    )
    .unwrap();
    execute_tx(account_key, iota_client, tx_data, function).await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    Ok(())
}

async fn execute_tx(
    account_key: &IotaKeyPair,
    iota_client: &IotaClient,
    tx_data: TransactionData,
    action: &str,
) -> anyhow::Result<()> {
    let tx = Transaction::from_data_and_signer(tx_data, vec![account_key]);
    info!("Executing {:?}", tx.digest());
    let tx_digest = *tx.digest();
    let resp = iota_client
        .quorum_driver_api()
        .execute_transaction_block(
            tx,
            IotaTransactionBlockResponseOptions::full_content(),
            Some(iota_types::quorum_driver_types::ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await
        .unwrap();
    if *resp.effects.unwrap().status() != IotaExecutionStatus::Success {
        anyhow::bail!("Tx to update metadata {:?} failed", tx_digest);
    }
    info!("{action} succeeded");
    Ok(())
}

async fn wait_for_next_epoch(
    iota_client: &IotaClient,
    target_epoch: EpochId,
) -> anyhow::Result<()> {
    loop {
        let epoch_id = current_epoch(iota_client).await?;
        if epoch_id > target_epoch {
            bail!(
                "Current epoch ID {} is higher than target {}, likely something is off.",
                epoch_id,
                target_epoch
            );
        }
        if epoch_id == target_epoch {
            return Ok(());
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    }
}

async fn current_epoch(iota_client: &IotaClient) -> anyhow::Result<EpochId> {
    Ok(iota_client
        .governance_api()
        .get_committee_info(None)
        .await?
        .epoch)
}
