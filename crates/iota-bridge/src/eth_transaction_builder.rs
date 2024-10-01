// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use ethers::{prelude::*, types::Address as EthAddress};

use crate::{
    abi::{
        EthBridgeCommittee, EthBridgeConfig, EthBridgeLimiter, EthCommitteeUpgradeableContract,
        EthIotaBridge, eth_bridge_committee, eth_bridge_config, eth_bridge_limiter,
        eth_committee_upgradeable_contract, eth_iota_bridge,
    },
    error::{BridgeError, BridgeResult},
    types::{
        AddTokensOnEvmAction, AssetPriceUpdateAction, BlocklistCommitteeAction, BridgeAction,
        BridgeCommitteeValiditySignInfo, EmergencyAction, EvmContractUpgradeAction,
        LimitUpdateAction, VerifiedCertifiedBridgeAction,
    },
    utils::EthSigner,
};

pub async fn build_eth_transaction(
    contract_address: EthAddress,
    signer: EthSigner,
    action: VerifiedCertifiedBridgeAction,
) -> BridgeResult<ContractCall<EthSigner, ()>> {
    if !action.is_governace_action() {
        return Err(BridgeError::ActionIsNotGovernanceAction(
            action.data().clone(),
        ));
    }
    // TODO: Check chain id?
    let sigs = action.auth_sig();
    match action.data() {
        BridgeAction::IotaToEthBridgeAction(_) => {
            unreachable!()
        }
        BridgeAction::EthToIotaBridgeAction(_) => {
            unreachable!()
        }
        BridgeAction::EmergencyAction(action) => {
            build_emergency_op_approve_transaction(contract_address, signer, action.clone(), sigs)
                .await
        }
        BridgeAction::BlocklistCommitteeAction(action) => {
            build_committee_blocklist_approve_transaction(
                contract_address,
                signer,
                action.clone(),
                sigs,
            )
            .await
        }
        BridgeAction::LimitUpdateAction(action) => {
            build_limit_update_approve_transaction(contract_address, signer, action.clone(), sigs)
                .await
        }
        BridgeAction::AssetPriceUpdateAction(action) => {
            build_asset_price_update_approve_transaction(
                contract_address,
                signer,
                action.clone(),
                sigs,
            )
            .await
        }
        BridgeAction::EvmContractUpgradeAction(action) => {
            build_evm_upgrade_transaction(signer, action.clone(), sigs).await
        }
        BridgeAction::AddTokensOnIotaAction(_) => {
            unreachable!();
        }
        BridgeAction::AddTokensOnEvmAction(action) => {
            build_add_tokens_on_evm_transaction(contract_address, signer, action.clone(), sigs)
                .await
        }
    }
}

pub async fn build_emergency_op_approve_transaction(
    contract_address: EthAddress,
    signer: EthSigner,
    action: EmergencyAction,
    sigs: &BridgeCommitteeValiditySignInfo,
) -> BridgeResult<ContractCall<EthSigner, ()>> {
    let contract = EthIotaBridge::new(contract_address, signer.into());

    let message: eth_iota_bridge::Message = action.clone().into();
    let signatures = sigs
        .signatures
        .values()
        .map(|sig| Bytes::from(sig.as_ref().to_vec()))
        .collect::<Vec<_>>();
    Ok(contract.execute_emergency_op_with_signatures(signatures, message))
}

pub async fn build_committee_blocklist_approve_transaction(
    contract_address: EthAddress,
    signer: EthSigner,
    action: BlocklistCommitteeAction,
    sigs: &BridgeCommitteeValiditySignInfo,
) -> BridgeResult<ContractCall<EthSigner, ()>> {
    let contract = EthBridgeCommittee::new(contract_address, signer.into());

    let message: eth_bridge_committee::Message = action.clone().into();
    let signatures = sigs
        .signatures
        .values()
        .map(|sig| Bytes::from(sig.as_ref().to_vec()))
        .collect::<Vec<_>>();
    Ok(contract.update_blocklist_with_signatures(signatures, message))
}

pub async fn build_limit_update_approve_transaction(
    contract_address: EthAddress,
    signer: EthSigner,
    action: LimitUpdateAction,
    sigs: &BridgeCommitteeValiditySignInfo,
) -> BridgeResult<ContractCall<EthSigner, ()>> {
    let contract = EthBridgeLimiter::new(contract_address, signer.into());

    let message: eth_bridge_limiter::Message = action.clone().into();
    let signatures = sigs
        .signatures
        .values()
        .map(|sig| Bytes::from(sig.as_ref().to_vec()))
        .collect::<Vec<_>>();
    Ok(contract.update_limit_with_signatures(signatures, message))
}

pub async fn build_asset_price_update_approve_transaction(
    contract_address: EthAddress,
    signer: EthSigner,
    action: AssetPriceUpdateAction,
    sigs: &BridgeCommitteeValiditySignInfo,
) -> BridgeResult<ContractCall<EthSigner, ()>> {
    let contract = EthBridgeConfig::new(contract_address, signer.into());
    let message: eth_bridge_config::Message = action.clone().into();
    let signatures = sigs
        .signatures
        .values()
        .map(|sig| Bytes::from(sig.as_ref().to_vec()))
        .collect::<Vec<_>>();
    Ok(contract.update_token_price_with_signatures(signatures, message))
}

pub async fn build_add_tokens_on_evm_transaction(
    contract_address: EthAddress,
    signer: EthSigner,
    action: AddTokensOnEvmAction,
    sigs: &BridgeCommitteeValiditySignInfo,
) -> BridgeResult<ContractCall<EthSigner, ()>> {
    let contract = EthBridgeConfig::new(contract_address, signer.into());
    let message: eth_bridge_config::Message = action.clone().into();
    let signatures = sigs
        .signatures
        .values()
        .map(|sig| Bytes::from(sig.as_ref().to_vec()))
        .collect::<Vec<_>>();
    Ok(contract.add_tokens_with_signatures(signatures, message))
}

pub async fn build_evm_upgrade_transaction(
    signer: EthSigner,
    action: EvmContractUpgradeAction,
    sigs: &BridgeCommitteeValiditySignInfo,
) -> BridgeResult<ContractCall<EthSigner, ()>> {
    let contract_address = action.proxy_address;
    let contract = EthCommitteeUpgradeableContract::new(contract_address, signer.into());
    let message: eth_committee_upgradeable_contract::Message = action.clone().into();
    let signatures = sigs
        .signatures
        .values()
        .map(|sig| Bytes::from(sig.as_ref().to_vec()))
        .collect::<Vec<_>>();
    Ok(contract.upgrade_with_signatures(signatures, message))
}

// TODO: add tests for eth transaction building
