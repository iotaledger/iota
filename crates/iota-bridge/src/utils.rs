// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{path::PathBuf, str::FromStr, sync::Arc};

use anyhow::anyhow;
use ethers::{
    core::k256::ecdsa::SigningKey,
    middleware::SignerMiddleware,
    prelude::*,
    providers::{Http, Provider},
    signers::Wallet,
    types::Address as EthAddress,
};
use fastcrypto::{
    ed25519::Ed25519KeyPair,
    encoding::{Encoding, Hex},
    secp256k1::Secp256k1KeyPair,
    traits::{EncodeDecodeBase64, KeyPair},
};
use futures::future::join_all;
use iota_config::Config;
use iota_json_rpc_types::{
    IotaExecutionStatus, IotaTransactionBlockEffectsAPI, IotaTransactionBlockResponseOptions,
};
use iota_keys::keypair_file::read_key;
use iota_sdk::wallet_context::WalletContext;
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    BRIDGE_PACKAGE_ID,
    base_types::IotaAddress,
    bridge::{BRIDGE_MODULE_NAME, BRIDGE_REGISTER_FOREIGN_TOKEN_FUNCTION_NAME, BridgeChainId},
    crypto::{IotaKeyPair, ToFromBytes, get_key_pair},
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::{ObjectArg, TransactionData},
};

use crate::{
    abi::{EthBridgeCommittee, EthBridgeConfig, EthBridgeLimiter, EthBridgeVault, EthIotaBridge},
    config::{BridgeNodeConfig, EthConfig, IotaConfig},
    crypto::{BridgeAuthorityKeyPair, BridgeAuthorityPublicKeyBytes},
    server::APPLICATION_JSON,
    types::{AddTokensOnIotaAction, BridgeAction},
};

pub type EthSigner = SignerMiddleware<Provider<Http>, Wallet<SigningKey>>;

pub struct EthBridgeContracts<P> {
    pub bridge: EthIotaBridge<Provider<P>>,
    pub committee: EthBridgeCommittee<Provider<P>>,
    pub limiter: EthBridgeLimiter<Provider<P>>,
    pub vault: EthBridgeVault<Provider<P>>,
    pub config: EthBridgeConfig<Provider<P>>,
}

/// Generate Bridge Authority key (Secp256k1KeyPair) and write to a file as
/// base64 encoded `privkey`.
pub fn generate_bridge_authority_key_and_write_to_file(
    path: &PathBuf,
) -> Result<(), anyhow::Error> {
    let (_, kp): (_, BridgeAuthorityKeyPair) = get_key_pair();
    let eth_address = BridgeAuthorityPublicKeyBytes::from(&kp.public).to_eth_address();
    println!(
        "Corresponding Ethereum address by this ecdsa key: {:?}",
        eth_address
    );
    let iota_address = IotaAddress::from(&kp.public);
    println!(
        "Corresponding IOTA address by this ecdsa key: {:?}",
        iota_address
    );
    let base64_encoded = kp.encode_base64();
    std::fs::write(path, base64_encoded)
        .map_err(|err| anyhow!("Failed to write encoded key to path: {:?}", err))
}

/// Generate Bridge Client key (Secp256k1KeyPair or Ed25519KeyPair) and write to
/// a file as base64 encoded `flag || privkey`.
pub fn generate_bridge_client_key_and_write_to_file(
    path: &PathBuf,
    use_ecdsa: bool,
) -> Result<(), anyhow::Error> {
    let kp = if use_ecdsa {
        let (_, kp): (_, Secp256k1KeyPair) = get_key_pair();
        let eth_address = BridgeAuthorityPublicKeyBytes::from(&kp.public).to_eth_address();
        println!(
            "Corresponding Ethereum address by this ecdsa key: {:?}",
            eth_address
        );
        IotaKeyPair::from(kp)
    } else {
        let (_, kp): (_, Ed25519KeyPair) = get_key_pair();
        IotaKeyPair::from(kp)
    };
    let iota_address = IotaAddress::from(&kp.public());
    println!("Corresponding IOTA address by this key: {:?}", iota_address);

    let contents = kp.encode_base64();
    std::fs::write(path, contents)
        .map_err(|err| anyhow!("Failed to write encoded key to path: {:?}", err))
}

/// Given the address of IotaBridge Proxy, return the addresses of the
/// committee, limiter, vault, and config.
pub async fn get_eth_contract_addresses<P: ethers::providers::JsonRpcClient + 'static>(
    bridge_proxy_address: EthAddress,
    provider: &Arc<Provider<P>>,
) -> anyhow::Result<(EthAddress, EthAddress, EthAddress, EthAddress)> {
    let iota_bridge = EthIotaBridge::new(bridge_proxy_address, provider.clone());
    let committee_address: EthAddress = iota_bridge.committee().call().await?;
    let limiter_address: EthAddress = iota_bridge.limiter().call().await?;
    let vault_address: EthAddress = iota_bridge.vault().call().await?;
    let committee = EthBridgeCommittee::new(committee_address, provider.clone());
    let config_address: EthAddress = committee.config().call().await?;

    Ok((
        committee_address,
        limiter_address,
        vault_address,
        config_address,
    ))
}

/// Given the address of IotaBridge Proxy, return the contracts of the
/// committee, limiter, vault, and config.
pub async fn get_eth_contracts<P: ethers::providers::JsonRpcClient + 'static>(
    bridge_proxy_address: EthAddress,
    provider: &Arc<Provider<P>>,
) -> anyhow::Result<EthBridgeContracts<P>> {
    let iota_bridge = EthIotaBridge::new(bridge_proxy_address, provider.clone());
    let committee_address: EthAddress = iota_bridge.committee().call().await?;
    let limiter_address: EthAddress = iota_bridge.limiter().call().await?;
    let vault_address: EthAddress = iota_bridge.vault().call().await?;
    let committee = EthBridgeCommittee::new(committee_address, provider.clone());
    let config_address: EthAddress = committee.config().call().await?;

    let limiter = EthBridgeLimiter::new(limiter_address, provider.clone());
    let vault = EthBridgeVault::new(vault_address, provider.clone());
    let config = EthBridgeConfig::new(config_address, provider.clone());
    Ok(EthBridgeContracts {
        bridge: iota_bridge,
        committee,
        limiter,
        vault,
        config,
    })
}

/// Read bridge key from a file and print the corresponding information.
/// If `is_validator_key` is true, the key must be a Secp256k1 key.
pub fn examine_key(path: &PathBuf, is_validator_key: bool) -> Result<(), anyhow::Error> {
    let key = read_key(path, is_validator_key)?;
    let iota_address = IotaAddress::from(&key.public());
    let pubkey = match key {
        IotaKeyPair::Secp256k1(kp) => {
            println!("Secp256k1 key:");
            let eth_address = BridgeAuthorityPublicKeyBytes::from(&kp.public).to_eth_address();
            println!("Corresponding Ethereum address: {:x}", eth_address);
            kp.public.as_bytes().to_vec()
        }
        IotaKeyPair::Ed25519(kp) => {
            println!("Ed25519 key:");
            kp.public().as_bytes().to_vec()
        }
        IotaKeyPair::Secp256r1(kp) => {
            println!("Secp256r1 key:");
            kp.public().as_bytes().to_vec()
        }
    };
    println!("Corresponding IOTA address: {:?}", iota_address);
    println!("Corresponding PublicKey: {:?}", Hex::encode(pubkey));
    Ok(())
}

/// Generate Bridge Node Config template and write to a file.
pub fn generate_bridge_node_config_and_write_to_file(
    path: &PathBuf,
    run_client: bool,
) -> Result<(), anyhow::Error> {
    let mut config = BridgeNodeConfig {
        server_listen_port: 9191,
        metrics_port: 9184,
        bridge_authority_key_path: PathBuf::from("/path/to/your/bridge_authority_key"),
        iota: IotaConfig {
            iota_rpc_url: "your_iota_rpc_url".to_string(),
            iota_bridge_chain_id: BridgeChainId::IotaTestnet as u8,
            bridge_client_key_path: None,
            bridge_client_gas_object: None,
            iota_bridge_module_last_processed_event_id_override: None,
        },
        eth: EthConfig {
            eth_rpc_url: "your_eth_rpc_url".to_string(),
            eth_bridge_proxy_address: "0x0000000000000000000000000000000000000000".to_string(),
            eth_bridge_chain_id: BridgeChainId::EthSepolia as u8,
            eth_contracts_start_block_fallback: Some(0),
            eth_contracts_start_block_override: None,
        },
        approved_governance_actions: vec![],
        run_client,
        db_path: None,
    };
    if run_client {
        config.iota.bridge_client_key_path = Some(PathBuf::from("/path/to/your/bridge_client_key"));
        config.db_path = Some(PathBuf::from("/path/to/your/client_db"));
    }
    config.save(path)
}

pub async fn get_eth_signer_client(url: &str, private_key_hex: &str) -> anyhow::Result<EthSigner> {
    let provider = Provider::<Http>::try_from(url)
        .unwrap()
        .interval(std::time::Duration::from_millis(2000));
    let chain_id = provider.get_chainid().await?;
    let wallet = Wallet::from_str(private_key_hex)
        .unwrap()
        .with_chain_id(chain_id.as_u64());
    Ok(SignerMiddleware::new(provider, wallet))
}

pub async fn publish_and_register_coins_return_add_coins_on_iota_action(
    wallet_context: &WalletContext,
    bridge_arg: ObjectArg,
    token_packages_dir: Vec<PathBuf>,
    token_ids: Vec<u8>,
    token_prices: Vec<u64>,
    nonce: u64,
) -> BridgeAction {
    assert!(token_ids.len() == token_packages_dir.len());
    assert!(token_prices.len() == token_packages_dir.len());
    let iota_client = wallet_context.get_client().await.unwrap();
    let quorum_driver_api = Arc::new(iota_client.quorum_driver_api().clone());
    let rgp = iota_client
        .governance_api()
        .get_reference_gas_price()
        .await
        .unwrap();

    let senders = wallet_context.get_addresses();
    // We want each sender to deal with one coin
    assert!(senders.len() >= token_packages_dir.len());

    // publish coin packages
    let mut publish_tokens_tasks = vec![];

    for (token_package_dir, sender) in token_packages_dir.iter().zip(senders.clone()) {
        let gas = wallet_context
            .get_one_gas_object_owned_by_address(sender)
            .await
            .unwrap()
            .unwrap();
        let tx = TestTransactionBuilder::new(sender, gas, rgp)
            .publish(token_package_dir.to_path_buf())
            .build();
        let tx = wallet_context.sign_transaction(&tx);
        let api_clone = quorum_driver_api.clone();
        publish_tokens_tasks.push(tokio::spawn(async move {
            api_clone.execute_transaction_block(
                tx,
                IotaTransactionBlockResponseOptions::new()
                    .with_effects()
                    .with_input()
                    .with_events()
                    .with_object_changes()
                    .with_balance_changes(),
                Some(iota_types::quorum_driver_types::ExecuteTransactionRequestType::WaitForLocalExecution),
            ).await
        }));
    }
    let publish_coin_responses = join_all(publish_tokens_tasks).await;

    let mut token_type_names = vec![];
    let mut register_tasks = vec![];
    for (response, sender) in publish_coin_responses.into_iter().zip(senders.clone()) {
        let response = response.unwrap().unwrap();
        assert_eq!(
            response.effects.unwrap().status(),
            &IotaExecutionStatus::Success
        );
        let object_changes = response.object_changes.unwrap();
        let mut tc = None;
        let mut type_ = None;
        let mut uc = None;
        let mut metadata = None;
        for object_change in &object_changes {
            if let o @ iota_json_rpc_types::ObjectChange::Created { object_type, .. } =
                object_change
            {
                if object_type.name.as_str().starts_with("TreasuryCap") {
                    assert!(tc.is_none() && type_.is_none());
                    tc = Some(o.clone());
                    type_ = Some(object_type.type_params.first().unwrap().clone());
                } else if object_type.name.as_str().starts_with("UpgradeCap") {
                    assert!(uc.is_none());
                    uc = Some(o.clone());
                } else if object_type.name.as_str().starts_with("CoinMetadata") {
                    assert!(metadata.is_none());
                    metadata = Some(o.clone());
                }
            }
        }
        let (tc, type_, uc, metadata) =
            (tc.unwrap(), type_.unwrap(), uc.unwrap(), metadata.unwrap());

        // register with the bridge
        let mut builder = ProgrammableTransactionBuilder::new();
        let bridge_arg = builder.obj(bridge_arg).unwrap();
        let uc_arg = builder
            .obj(ObjectArg::ImmOrOwnedObject(uc.object_ref()))
            .unwrap();
        let tc_arg = builder
            .obj(ObjectArg::ImmOrOwnedObject(tc.object_ref()))
            .unwrap();
        let metadata_arg = builder
            .obj(ObjectArg::ImmOrOwnedObject(metadata.object_ref()))
            .unwrap();
        builder.programmable_move_call(
            BRIDGE_PACKAGE_ID,
            BRIDGE_MODULE_NAME.into(),
            BRIDGE_REGISTER_FOREIGN_TOKEN_FUNCTION_NAME.into(),
            vec![type_.clone()],
            vec![bridge_arg, tc_arg, uc_arg, metadata_arg],
        );
        let pt = builder.finish();
        let gas = wallet_context
            .get_one_gas_object_owned_by_address(sender)
            .await
            .unwrap()
            .unwrap();
        let tx = TransactionData::new_programmable(sender, vec![gas], pt, 1_000_000_000, rgp);
        let signed_tx = wallet_context.sign_transaction(&tx);
        let api_clone = quorum_driver_api.clone();
        register_tasks.push(async move {
            api_clone
                .execute_transaction_block(
                    signed_tx,
                    IotaTransactionBlockResponseOptions::new().with_effects(),
                    None,
                )
                .await
        });
        token_type_names.push(type_);
    }
    for response in join_all(register_tasks).await {
        assert_eq!(
            response.unwrap().effects.unwrap().status(),
            &IotaExecutionStatus::Success
        );
    }

    BridgeAction::AddTokensOnIotaAction(AddTokensOnIotaAction {
        nonce,
        chain_id: BridgeChainId::IotaCustom,
        native: false,
        token_ids,
        token_type_names,
        token_prices,
    })
}

pub async fn wait_for_server_to_be_up(server_url: String, timeout_sec: u64) -> anyhow::Result<()> {
    let now = std::time::Instant::now();
    loop {
        if let Ok(true) = reqwest::Client::new()
            .get(server_url.clone())
            .header(reqwest::header::ACCEPT, APPLICATION_JSON)
            .send()
            .await
            .map(|res| res.status().is_success())
        {
            break;
        }
        if now.elapsed().as_secs() > timeout_sec {
            anyhow::bail!("Server is not up and running after {} seconds", timeout_sec);
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
    Ok(())
}
