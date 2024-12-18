// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use crate::abi::EthToIotaTokenBridgeV1;
use crate::eth_mock_provider::EthMockProvider;
use crate::events::IotaBridgeEvent;
use crate::server::mock_handler::run_mock_server;
use crate::iota_transaction_builder::build_iota_transaction;
use crate::types::{
    BridgeCommittee, BridgeCommitteeValiditySignInfo, CertifiedBridgeAction,
    VerifiedCertifiedBridgeAction,
};
use crate::{
    crypto::{BridgeAuthorityKeyPair, BridgeAuthorityPublicKey, BridgeAuthoritySignInfo},
    events::EmittedIotaToEthTokenBridgeV1,
    server::mock_handler::BridgeRequestMockHandler,
    types::{
        BridgeAction, BridgeAuthority, EthToIotaBridgeAction, SignedBridgeAction,
        IotaToEthBridgeAction,
    },
};
use ethers::abi::{long_signature, ParamType};
use ethers::types::Address as EthAddress;
use ethers::types::{
    Block, BlockNumber, Filter, FilterBlockOption, Log, TransactionReceipt, TxHash, ValueOrArray,
    U64,
};
use fastcrypto::encoding::{Encoding, Hex};
use fastcrypto::traits::KeyPair;
use hex_literal::hex;
use move_core_types::language_storage::TypeTag;
use std::collections::{BTreeMap, HashMap};
use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use iota_config::local_ip_utils;
use iota_json_rpc_types::IotaTransactionBlockEffectsAPI;
use iota_sdk::wallet_context::WalletContext;
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::base_types::ObjectRef;
use iota_types::base_types::SequenceNumber;
use iota_types::bridge::MoveTypeCommitteeMember;
use iota_types::bridge::{BridgeChainId, BridgeCommitteeSummary, TOKEN_ID_USDC};
use iota_types::crypto::ToFromBytes;
use iota_types::object::Owner;
use iota_types::transaction::{CallArg, ObjectArg};
use iota_types::{base_types::IotaAddress, crypto::get_key_pair, digests::TransactionDigest};
use iota_types::{BRIDGE_PACKAGE_ID, IOTA_BRIDGE_OBJECT_ID};
use tokio::task::JoinHandle;

pub const DUMMY_MUTALBE_BRIDGE_OBJECT_ARG: ObjectArg = ObjectArg::SharedObject {
    id: IOTA_BRIDGE_OBJECT_ID,
    initial_shared_version: SequenceNumber::from_u64(1),
    mutable: true,
};

pub fn get_test_authority_and_key(
    voting_power: u64,
    port: u16,
) -> (
    BridgeAuthority,
    BridgeAuthorityPublicKey,
    BridgeAuthorityKeyPair,
) {
    let (_, kp): (_, fastcrypto::secp256k1::Secp256k1KeyPair) = get_key_pair();
    let pubkey = kp.public().clone();
    let authority = BridgeAuthority {
        iota_address: IotaAddress::random_for_testing_only(),
        pubkey: pubkey.clone(),
        voting_power,
        base_url: format!("http://127.0.0.1:{}", port),
        is_blocklisted: false,
    };

    (authority, pubkey, kp)
}

// TODO: make a builder for this
pub fn get_test_iota_to_eth_bridge_action(
    iota_tx_digest: Option<TransactionDigest>,
    iota_tx_event_index: Option<u16>,
    nonce: Option<u64>,
    amount_iota_adjusted: Option<u64>,
    sender_address: Option<IotaAddress>,
    recipient_address: Option<EthAddress>,
    token_id: Option<u8>,
) -> BridgeAction {
    BridgeAction::IotaToEthBridgeAction(IotaToEthBridgeAction {
        iota_tx_digest: iota_tx_digest.unwrap_or_else(TransactionDigest::random),
        iota_tx_event_index: iota_tx_event_index.unwrap_or(0),
        iota_bridge_event: EmittedIotaToEthTokenBridgeV1 {
            nonce: nonce.unwrap_or_default(),
            iota_chain_id: BridgeChainId::IotaCustom,
            iota_address: sender_address.unwrap_or_else(IotaAddress::random_for_testing_only),
            eth_chain_id: BridgeChainId::EthCustom,
            eth_address: recipient_address.unwrap_or_else(EthAddress::random),
            token_id: token_id.unwrap_or(TOKEN_ID_USDC),
            amount_iota_adjusted: amount_iota_adjusted.unwrap_or(100_000),
        },
    })
}

pub fn get_test_eth_to_iota_bridge_action(
    nonce: Option<u64>,
    amount: Option<u64>,
    iota_address: Option<IotaAddress>,
    token_id: Option<u8>,
) -> BridgeAction {
    BridgeAction::EthToIotaBridgeAction(EthToIotaBridgeAction {
        eth_tx_hash: TxHash::random(),
        eth_event_index: 0,
        eth_bridge_event: EthToIotaTokenBridgeV1 {
            eth_chain_id: BridgeChainId::EthCustom,
            nonce: nonce.unwrap_or_default(),
            iota_chain_id: BridgeChainId::IotaCustom,
            token_id: token_id.unwrap_or(TOKEN_ID_USDC),
            iota_adjusted_amount: amount.unwrap_or(100_000),
            iota_address: iota_address.unwrap_or_else(IotaAddress::random_for_testing_only),
            eth_address: EthAddress::random(),
        },
    })
}

pub fn run_mock_bridge_server(
    mock_handlers: Vec<BridgeRequestMockHandler>,
) -> (Vec<JoinHandle<()>>, Vec<u16>) {
    let mut handles = vec![];
    let mut ports = vec![];
    for mock_handler in mock_handlers {
        let localhost = local_ip_utils::localhost_for_testing();
        let port = local_ip_utils::get_available_port(&localhost);
        // start server
        let server_handle = run_mock_server(
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), port),
            mock_handler.clone(),
        );
        ports.push(port);
        handles.push(server_handle);
    }
    (handles, ports)
}

pub fn get_test_authorities_and_run_mock_bridge_server(
    voting_power: Vec<u64>,
    mock_handlers: Vec<BridgeRequestMockHandler>,
) -> (
    Vec<JoinHandle<()>>,
    Vec<BridgeAuthority>,
    Vec<BridgeAuthorityKeyPair>,
) {
    assert_eq!(voting_power.len(), mock_handlers.len());
    let (handles, ports) = run_mock_bridge_server(mock_handlers);
    let mut authorites = vec![];
    let mut secrets = vec![];
    for (port, vp) in ports.iter().zip(voting_power) {
        let (authority, _, secret) = get_test_authority_and_key(vp, *port);
        authorites.push(authority);
        secrets.push(secret);
    }

    (handles, authorites, secrets)
}

pub fn sign_action_with_key(
    action: &BridgeAction,
    secret: &BridgeAuthorityKeyPair,
) -> SignedBridgeAction {
    let sig = BridgeAuthoritySignInfo::new(action, secret);
    SignedBridgeAction::new_from_data_and_sig(action.clone(), sig)
}

pub fn mock_last_finalized_block(mock_provider: &EthMockProvider, block_number: u64) {
    let block = Block::<ethers::types::TxHash> {
        number: Some(U64::from(block_number)),
        ..Default::default()
    };
    mock_provider
        .add_response("eth_getBlockByNumber", ("finalized", false), block)
        .unwrap();
}

// Mocks eth_getLogs and eth_getTransactionReceipt for the given address and block range.
// The input log needs to have transaction_hash set.
pub fn mock_get_logs(
    mock_provider: &EthMockProvider,
    address: EthAddress,
    from_block: u64,
    to_block: u64,
    logs: Vec<Log>,
) {
    mock_provider.add_response::<[ethers::types::Filter; 1], Vec<ethers::types::Log>, Vec<ethers::types::Log>>(
        "eth_getLogs",
        [
            Filter {
                block_option: FilterBlockOption::Range {
                    from_block: Some(BlockNumber::Number(U64::from(from_block))),
                    to_block: Some(BlockNumber::Number(U64::from(to_block))),
                },
                address: Some(ValueOrArray::Value(address)),
                topics: [None, None, None, None],
            }
        ],
        logs.clone(),
    ).unwrap();

    for log in logs {
        mock_provider
            .add_response::<[TxHash; 1], TransactionReceipt, TransactionReceipt>(
                "eth_getTransactionReceipt",
                [log.transaction_hash.unwrap()],
                TransactionReceipt {
                    block_number: log.block_number,
                    logs: vec![log],
                    ..Default::default()
                },
            )
            .unwrap();
    }
}

/// Returns a test Log and corresponding BridgeAction
// Refernece: https://github.com/rust-ethereum/ethabi/blob/master/ethabi/src/event.rs#L192
pub fn get_test_log_and_action(
    contract_address: EthAddress,
    tx_hash: TxHash,
    event_index: u16,
) -> (Log, BridgeAction) {
    let token_id = 3u8;
    let iota_adjusted_amount = 10000000u64;
    let source_address = EthAddress::random();
    let iota_address: IotaAddress = IotaAddress::random_for_testing_only();
    let target_address = Hex::decode(&iota_address.to_string()).unwrap();
    // Note: must use `encode` rather than `encode_packaged`
    let encoded = ethers::abi::encode(&[
        // u8/u64 is encoded as u256 in abi standard
        ethers::abi::Token::Uint(ethers::types::U256::from(token_id)),
        ethers::abi::Token::Uint(ethers::types::U256::from(iota_adjusted_amount)),
        ethers::abi::Token::Address(source_address),
        ethers::abi::Token::Bytes(target_address.clone()),
    ]);
    let log = Log {
        address: contract_address,
        topics: vec![
            long_signature(
                "TokensDeposited",
                &[
                    ParamType::Uint(8),
                    ParamType::Uint(64),
                    ParamType::Uint(8),
                    ParamType::Uint(8),
                    ParamType::Uint(64),
                    ParamType::Address,
                    ParamType::Bytes,
                ],
            ),
            hex!("0000000000000000000000000000000000000000000000000000000000000001").into(), // chain id: iota testnet
            hex!("0000000000000000000000000000000000000000000000000000000000000010").into(), // nonce: 16
            hex!("000000000000000000000000000000000000000000000000000000000000000b").into(), // chain id: sepolia
        ],
        data: encoded.into(),
        block_hash: Some(TxHash::random()),
        block_number: Some(1.into()),
        transaction_hash: Some(tx_hash),
        log_index: Some(0.into()),
        ..Default::default()
    };
    let topic_1: [u8; 32] = log.topics[1].into();
    let topic_3: [u8; 32] = log.topics[3].into();

    let bridge_action = BridgeAction::EthToIotaBridgeAction(EthToIotaBridgeAction {
        eth_tx_hash: tx_hash,
        eth_event_index: event_index,
        eth_bridge_event: EthToIotaTokenBridgeV1 {
            eth_chain_id: BridgeChainId::try_from(topic_1[topic_1.len() - 1]).unwrap(),
            nonce: u64::from_be_bytes(log.topics[2].as_ref()[24..32].try_into().unwrap()),
            iota_chain_id: BridgeChainId::try_from(topic_3[topic_3.len() - 1]).unwrap(),
            token_id,
            iota_adjusted_amount,
            iota_address,
            eth_address: source_address,
        },
    });
    (log, bridge_action)
}

pub async fn bridge_token(
    context: &mut WalletContext,
    recv_address: EthAddress,
    token_ref: ObjectRef,
    token_type: TypeTag,
    bridge_object_arg: ObjectArg,
) -> EmittedIotaToEthTokenBridgeV1 {
    let rgp = context.get_reference_gas_price().await.unwrap();
    let sender = context.active_address().unwrap();
    let gas_object = context.get_one_gas_object().await.unwrap().unwrap().1;
    let tx = TestTransactionBuilder::new(sender, gas_object, rgp)
        .move_call(
            BRIDGE_PACKAGE_ID,
            "bridge",
            "send_token",
            vec![
                CallArg::Object(bridge_object_arg),
                CallArg::Pure(bcs::to_bytes(&(BridgeChainId::EthCustom as u8)).unwrap()),
                CallArg::Pure(bcs::to_bytes(&recv_address.as_bytes()).unwrap()),
                CallArg::Object(ObjectArg::ImmOrOwnedObject(token_ref)),
            ],
        )
        .with_type_args(vec![token_type])
        .build();
    let signed_tn = context.sign_transaction(&tx);
    let resp = context.execute_transaction_must_succeed(signed_tn).await;
    let events = resp.events.unwrap();
    let bridge_events = events
        .data
        .iter()
        .filter_map(|event| IotaBridgeEvent::try_from_iota_event(event).unwrap())
        .collect::<Vec<_>>();
    bridge_events
        .iter()
        .find_map(|e| match e {
            IotaBridgeEvent::IotaToEthTokenBridgeV1(event) => Some(event.clone()),
            _ => None,
        })
        .unwrap()
}

/// Returns a VerifiedCertifiedBridgeAction with signatures from the given
/// BridgeAction and BridgeAuthorityKeyPair
pub fn get_certified_action_with_validator_secrets(
    action: BridgeAction,
    secrets: &Vec<BridgeAuthorityKeyPair>,
) -> VerifiedCertifiedBridgeAction {
    let mut sigs = BTreeMap::new();
    for secret in secrets {
        let signed_action = sign_action_with_key(&action, secret);
        sigs.insert(secret.public().into(), signed_action.into_sig().signature);
    }
    let certified_action = CertifiedBridgeAction::new_from_data_and_sig(
        action,
        BridgeCommitteeValiditySignInfo { signatures: sigs },
    );
    VerifiedCertifiedBridgeAction::new_from_verified(certified_action)
}

/// Approve a bridge action with the given validator secrets. Return the
/// newly created token object reference if `expected_token_receiver` is Some
/// (only relevant when the action is eth -> Iota transfer),
/// Otherwise return None.
/// Note: for iota -> eth transfers, the actual deposit needs to be recorded.
/// Use `bridge_token` to do it.
// TODO(bridge): It appears this function is very slow (particularly, `execute_transaction_must_succeed`).
// Investigate why.
pub async fn approve_action_with_validator_secrets(
    wallet_context: &mut WalletContext,
    bridge_obj_org: ObjectArg,
    // TODO: add `token_recipient()` for `BridgeAction` so we don't need `expected_token_receiver`
    action: BridgeAction,
    validator_secrets: &Vec<BridgeAuthorityKeyPair>,
    // Only relevant for eth -> iota transfers when token will be dropped to the recipient
    expected_token_receiver: Option<IotaAddress>,
    id_token_map: &HashMap<u8, TypeTag>,
) -> Option<ObjectRef> {
    let action_certificate = get_certified_action_with_validator_secrets(action, validator_secrets);
    let rgp = wallet_context.get_reference_gas_price().await.unwrap();
    let iota_address = wallet_context.active_address().unwrap();
    let gas_obj_ref = wallet_context
        .get_one_gas_object()
        .await
        .unwrap()
        .unwrap()
        .1;
    let tx_data = build_iota_transaction(
        iota_address,
        &gas_obj_ref,
        action_certificate,
        bridge_obj_org,
        id_token_map,
        rgp,
    )
    .unwrap();
    let signed_tx = wallet_context.sign_transaction(&tx_data);
    let resp = wallet_context
        .execute_transaction_must_succeed(signed_tx)
        .await;

    // If `expected_token_receiver` is None, return
    expected_token_receiver?;

    let expected_token_receiver = expected_token_receiver.unwrap();
    for created in resp.effects.unwrap().created() {
        if created.owner == Owner::AddressOwner(expected_token_receiver) {
            return Some(created.reference.to_object_ref());
        }
    }
    panic!(
        "Didn't find the created object owned by {}",
        expected_token_receiver
    );
}

pub fn bridge_committee_to_bridge_committee_summary(
    committee: BridgeCommittee,
) -> BridgeCommitteeSummary {
    BridgeCommitteeSummary {
        members: committee
            .members()
            .iter()
            .map(|(k, v)| {
                let bytes = k.as_bytes().to_vec();
                (
                    bytes.clone(),
                    MoveTypeCommitteeMember {
                        iota_address: IotaAddress::random_for_testing_only(),
                        bridge_pubkey_bytes: bytes,
                        voting_power: v.voting_power,
                        http_rest_url: v.base_url.as_bytes().to_vec(),
                        blocklisted: v.is_blocklisted,
                    },
                )
            })
            .collect(),
        member_registration: vec![],
        last_committee_update_epoch: 0,
    }
}
