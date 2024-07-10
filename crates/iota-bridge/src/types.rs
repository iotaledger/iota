// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::{BTreeMap, BTreeSet};

pub use ethers::types::H256 as EthTransactionHash;
use ethers::types::{Address as EthAddress, Log, H256};
use fastcrypto::hash::{HashFunction, Keccak256};
use iota_types::{
    base_types::{IotaAddress, IOTA_ADDRESS_LENGTH},
    collection_types::{Bag, LinkedTable, LinkedTableNode, VecMap},
    committee::{CommitteeTrait, EpochId, StakeUnit},
    digests::{Digest, TransactionDigest},
    dynamic_field::Field,
    error::IotaResult,
    message_envelope::{Envelope, Message, VerifiedEnvelope},
};
use num_enum::TryFromPrimitive;
use rand::{seq::SliceRandom, Rng};
use serde::{Deserialize, Serialize};
use shared_crypto::intent::IntentScope;

use crate::{
    abi::EthToIotaTokenBridgeV1,
    crypto::{
        BridgeAuthorityPublicKey, BridgeAuthorityPublicKeyBytes,
        BridgeAuthorityRecoverableSignature, BridgeAuthoritySignInfo,
    },
    error::{BridgeError, BridgeResult},
    events::EmittedIotaToEthTokenBridgeV1,
};

pub const BRIDGE_AUTHORITY_TOTAL_VOTING_POWER: u64 = 10000;

pub const USD_MULTIPLIER: u64 = 10000; // decimal places = 4

pub type BridgeInnerDynamicField = Field<u64, MoveTypeBridgeInner>;
pub type BridgeRecordDyanmicField = Field<
    MoveTypeBridgeMessageKey,
    LinkedTableNode<MoveTypeBridgeMessageKey, MoveTypeBridgeRecord>,
>;

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct BridgeAuthority {
    pub pubkey: BridgeAuthorityPublicKey,
    pub voting_power: u64,
    pub base_url: String,
    pub is_blocklisted: bool,
}

impl BridgeAuthority {
    pub fn pubkey_bytes(&self) -> BridgeAuthorityPublicKeyBytes {
        BridgeAuthorityPublicKeyBytes::from(&self.pubkey)
    }
}

// A static Bridge committee implementation
#[derive(Debug, Clone)]
pub struct BridgeCommittee {
    members: BTreeMap<BridgeAuthorityPublicKeyBytes, BridgeAuthority>,
    total_blocklisted_stake: StakeUnit,
}

impl BridgeCommittee {
    pub fn new(members: Vec<BridgeAuthority>) -> BridgeResult<Self> {
        let mut members_map = BTreeMap::new();
        let mut total_stake = 0;
        let mut total_blocklisted_stake = 0;
        for member in members {
            let public_key = BridgeAuthorityPublicKeyBytes::from(&member.pubkey);
            if members_map.contains_key(&public_key) {
                return Err(BridgeError::InvalidBridgeCommittee(
                    "Duplicate BridgeAuthority Public key".into(),
                ));
            }
            // TODO: should we disallow identical network addresses?
            total_stake += member.voting_power;
            if member.is_blocklisted {
                total_blocklisted_stake += member.voting_power;
            }
            members_map.insert(public_key, member);
        }
        if total_stake != BRIDGE_AUTHORITY_TOTAL_VOTING_POWER {
            return Err(BridgeError::InvalidBridgeCommittee(
                "Total voting power does not equal to 10000".into(),
            ));
        }
        Ok(Self {
            members: members_map,
            total_blocklisted_stake,
        })
    }

    pub fn is_active_member(&self, member: &BridgeAuthorityPublicKeyBytes) -> bool {
        self.members.contains_key(member) && !self.members.get(member).unwrap().is_blocklisted
    }

    pub fn members(&self) -> &BTreeMap<BridgeAuthorityPublicKeyBytes, BridgeAuthority> {
        &self.members
    }

    pub fn member(&self, member: &BridgeAuthorityPublicKeyBytes) -> Option<&BridgeAuthority> {
        self.members.get(member)
    }

    pub fn total_blocklisted_stake(&self) -> StakeUnit {
        self.total_blocklisted_stake
    }
}

impl CommitteeTrait<BridgeAuthorityPublicKeyBytes> for BridgeCommittee {
    // Note:
    // 1. preference is not supported today.
    // 2. blocklisted members are always excluded.
    fn shuffle_by_stake_with_rng(
        &self,
        // preference is not supported today
        _preferences: Option<&BTreeSet<BridgeAuthorityPublicKeyBytes>>,
        // only attempt from these authorities.
        restrict_to: Option<&BTreeSet<BridgeAuthorityPublicKeyBytes>>,
        rng: &mut impl Rng,
    ) -> Vec<BridgeAuthorityPublicKeyBytes> {
        let candidates = self
            .members
            .iter()
            .filter_map(|(name, a)| {
                // Remove blocklisted members
                if a.is_blocklisted {
                    return None;
                }
                // exclude non-allowlisted members
                if let Some(restrict_to) = restrict_to {
                    match restrict_to.contains(name) {
                        true => Some((name.clone(), a.voting_power)),
                        false => None,
                    }
                } else {
                    Some((name.clone(), a.voting_power))
                }
            })
            .collect::<Vec<_>>();

        candidates
            .choose_multiple_weighted(rng, candidates.len(), |(_, weight)| *weight as f64)
            // Unwrap safe: it panics when the third parameter is larger than the size of the slice
            .unwrap()
            .map(|(name, _)| name)
            .cloned()
            .collect()
    }

    fn weight(&self, author: &BridgeAuthorityPublicKeyBytes) -> StakeUnit {
        self.members
            .get(author)
            .map(|a| a.voting_power)
            .unwrap_or(0)
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum BridgeActionType {
    TokenTransfer = 0,
    UpdateCommitteeBlocklist = 1,
    EmergencyButton = 2,
    LimitUpdate = 3,
    AssetPriceUpdate = 4,
    EvmContractUpgrade = 5,
}

pub const IOTA_TX_DIGEST_LENGTH: usize = 32;
pub const ETH_TX_HASH_LENGTH: usize = 32;

pub const BRIDGE_MESSAGE_PREFIX: &[u8] = b"IOTA_BRIDGE_MESSAGE";

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy, TryFromPrimitive, Hash)]
#[repr(u8)]
pub enum BridgeChainId {
    IotaMainnet = 0,
    IotaTestnet = 1,
    IotaDevnet = 2,
    IotaLocalTest = 3,

    EthMainnet = 10,
    EthSepolia = 11,
    EthLocalTest = 12,
}

#[derive(
    Copy,
    Clone,
    Debug,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    TryFromPrimitive,
    Hash,
    Ord,
    PartialOrd,
)]
#[repr(u8)]
pub enum TokenId {
    Iota = 0,
    BTC = 1,
    ETH = 2,
    USDC = 3,
    USDT = 4,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum BridgeActionStatus {
    RecordNotFound,
    Pending,
    Approved,
    Claimed,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct IotaToEthBridgeAction {
    // Digest of the transaction where the event was emitted
    pub iota_tx_digest: TransactionDigest,
    // The index of the event in the transaction
    pub iota_tx_event_index: u16,
    pub iota_bridge_event: EmittedIotaToEthTokenBridgeV1,
}

impl IotaToEthBridgeAction {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        let e = &self.iota_bridge_event;
        // Add message type
        bytes.push(BridgeActionType::TokenTransfer as u8);
        // Add message version
        bytes.push(TOKEN_TRANSFER_MESSAGE_VERSION);
        // Add nonce
        bytes.extend_from_slice(&e.nonce.to_be_bytes());
        // Add source chain id
        bytes.push(e.iota_chain_id as u8);

        // Add source address length
        bytes.push(IOTA_ADDRESS_LENGTH as u8);
        // Add source address
        bytes.extend_from_slice(&e.iota_address.to_vec());
        // Add dest chain id
        bytes.push(e.eth_chain_id as u8);
        // Add dest address length
        bytes.push(EthAddress::len_bytes() as u8);
        // Add dest address
        bytes.extend_from_slice(e.eth_address.as_bytes());

        // Add token id
        bytes.push(e.token_id as u8);

        // Add token amount
        bytes.extend_from_slice(&e.amount.to_be_bytes());

        bytes
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct EthToIotaBridgeAction {
    // Digest of the transaction where the event was emitted
    pub eth_tx_hash: EthTransactionHash,
    // The index of the event in the transaction
    pub eth_event_index: u16,
    pub eth_bridge_event: EthToIotaTokenBridgeV1,
}

impl EthToIotaBridgeAction {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        let e = &self.eth_bridge_event;
        // Add message type
        bytes.push(BridgeActionType::TokenTransfer as u8);
        // Add message version
        bytes.push(TOKEN_TRANSFER_MESSAGE_VERSION);
        // Add nonce
        bytes.extend_from_slice(&e.nonce.to_be_bytes());
        // Add source chain id
        bytes.push(e.eth_chain_id as u8);

        // Add source address length
        bytes.push(EthAddress::len_bytes() as u8);
        // Add source address
        bytes.extend_from_slice(e.eth_address.as_bytes());
        // Add dest chain id
        bytes.push(e.iota_chain_id as u8);
        // Add dest address length
        bytes.push(IOTA_ADDRESS_LENGTH as u8);
        // Add dest address
        bytes.extend_from_slice(&e.iota_address.to_vec());

        // Add token id
        bytes.push(e.token_id as u8);

        // Add token amount
        bytes.extend_from_slice(&e.amount.to_be_bytes());

        bytes
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy, TryFromPrimitive, Hash)]
#[repr(u8)]
pub enum BlocklistType {
    Blocklist = 0,
    Unblocklist = 1,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct BlocklistCommitteeAction {
    pub nonce: u64,
    pub chain_id: BridgeChainId,
    pub blocklist_type: BlocklistType,
    pub blocklisted_members: Vec<BridgeAuthorityPublicKeyBytes>,
}

impl BlocklistCommitteeAction {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        // Add message type
        bytes.push(BridgeActionType::UpdateCommitteeBlocklist as u8);
        // Add message version
        bytes.push(COMMITTEE_BLOCKLIST_MESSAGE_VERSION);
        // Add nonce
        bytes.extend_from_slice(&self.nonce.to_be_bytes());
        // Add chain id
        bytes.push(self.chain_id as u8);
        // Add blocklist type
        bytes.push(self.blocklist_type as u8);
        // Add length of updated members.
        // Unwrap: It should not overflow given what we have today.
        bytes.push(u8::try_from(self.blocklisted_members.len()).unwrap());

        // Add list of updated members
        // Members are represented as pubkey derived evm addresses (20 bytes)
        let members_bytes = self
            .blocklisted_members
            .iter()
            .map(|m| m.to_eth_address().to_fixed_bytes().to_vec())
            .collect::<Vec<_>>();
        for members_bytes in members_bytes {
            bytes.extend_from_slice(&members_bytes);
        }
        bytes
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy, TryFromPrimitive, Hash)]
#[repr(u8)]
pub enum EmergencyActionType {
    Pause = 0,
    Unpause = 1,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct EmergencyAction {
    pub nonce: u64,
    pub chain_id: BridgeChainId,
    pub action_type: EmergencyActionType,
}

impl EmergencyAction {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        // Add message type
        bytes.push(BridgeActionType::EmergencyButton as u8);
        // Add message version
        bytes.push(EMERGENCY_BUTTON_MESSAGE_VERSION);
        // Add nonce
        bytes.extend_from_slice(&self.nonce.to_be_bytes());
        // Add chain id
        bytes.push(self.chain_id as u8);
        // Add action type
        bytes.push(self.action_type as u8);
        bytes
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct LimitUpdateAction {
    pub nonce: u64,
    // The chain id that will receive this signed action. It's also the destination chain id
    // for the limit update. For example, if chain_id is EthMainnet and sending_chain_id is
    // IotaMainnet, it means we want to update the limit for the IotaMainnet to EthMainnet route.
    pub chain_id: BridgeChainId,
    // The sending chain id for the limit update.
    pub sending_chain_id: BridgeChainId,
    pub new_usd_limit: u64,
}

impl LimitUpdateAction {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        // Add message type
        bytes.push(BridgeActionType::LimitUpdate as u8);
        // Add message version
        bytes.push(LIMIT_UPDATE_MESSAGE_VERSION);
        // Add nonce
        bytes.extend_from_slice(&self.nonce.to_be_bytes());
        // Add chain id
        bytes.push(self.chain_id as u8);
        // Add sending chain id
        bytes.push(self.sending_chain_id as u8);
        // Add new usd limit
        bytes.extend_from_slice(&self.new_usd_limit.to_be_bytes());
        bytes
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct AssetPriceUpdateAction {
    pub nonce: u64,
    pub chain_id: BridgeChainId,
    pub token_id: TokenId,
    pub new_usd_price: u64,
}

impl AssetPriceUpdateAction {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        // Add message type
        bytes.push(BridgeActionType::AssetPriceUpdate as u8);
        // Add message version
        bytes.push(EMERGENCY_BUTTON_MESSAGE_VERSION);
        // Add nonce
        bytes.extend_from_slice(&self.nonce.to_be_bytes());
        // Add chain id
        bytes.push(self.chain_id as u8);
        // Add token id
        bytes.push(self.token_id as u8);
        // Add new usd limit
        bytes.extend_from_slice(&self.new_usd_price.to_be_bytes());
        bytes
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct EvmContractUpgradeAction {
    pub nonce: u64,
    pub chain_id: BridgeChainId,
    pub proxy_address: EthAddress,
    pub new_impl_address: EthAddress,
    pub call_data: Vec<u8>,
}

impl EvmContractUpgradeAction {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        // Add message type
        bytes.push(BridgeActionType::EvmContractUpgrade as u8);
        // Add message version
        bytes.push(EVM_CONTRACT_UPGRADE_MESSAGE_VERSION);
        // Add nonce
        bytes.extend_from_slice(&self.nonce.to_be_bytes());
        // Add chain id
        bytes.push(self.chain_id as u8);
        // Add payload
        let encoded = ethers::abi::encode(&[
            ethers::abi::Token::Address(self.proxy_address),
            ethers::abi::Token::Address(self.new_impl_address),
            ethers::abi::Token::Bytes(self.call_data.clone()),
        ]);
        bytes.extend_from_slice(&encoded);
        bytes
    }
}

/// The type of actions Bridge Committee verify and sign off to execution.
/// Its relationship with BridgeEvent is similar to the relationship between
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum BridgeAction {
    /// Iota to Eth bridge action
    IotaToEthBridgeAction(IotaToEthBridgeAction),
    /// Eth to iota bridge action
    EthToIotaBridgeAction(EthToIotaBridgeAction),
    BlocklistCommitteeAction(BlocklistCommitteeAction),
    EmergencyAction(EmergencyAction),
    LimitUpdateAction(LimitUpdateAction),
    AssetPriceUpdateAction(AssetPriceUpdateAction),
    EvmContractUpgradeAction(EvmContractUpgradeAction),
    // TODO: add other bridge actions such as blocklist & emergency button
}

pub const TOKEN_TRANSFER_MESSAGE_VERSION: u8 = 1;
pub const COMMITTEE_BLOCKLIST_MESSAGE_VERSION: u8 = 1;
pub const EMERGENCY_BUTTON_MESSAGE_VERSION: u8 = 1;
pub const LIMIT_UPDATE_MESSAGE_VERSION: u8 = 1;
pub const ASSET_PRICE_UPDATE_MESSAGE_VERSION: u8 = 1;
pub const EVM_CONTRACT_UPGRADE_MESSAGE_VERSION: u8 = 1;

impl BridgeAction {
    /// Convert to message bytes that are verified in Move and Solidity
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        // Add prefix
        bytes.extend_from_slice(BRIDGE_MESSAGE_PREFIX);
        match self {
            BridgeAction::IotaToEthBridgeAction(a) => {
                bytes.extend_from_slice(&a.to_bytes());
            }
            BridgeAction::EthToIotaBridgeAction(a) => {
                bytes.extend_from_slice(&a.to_bytes());
            }
            BridgeAction::BlocklistCommitteeAction(a) => {
                bytes.extend_from_slice(&a.to_bytes());
            }
            BridgeAction::EmergencyAction(a) => {
                bytes.extend_from_slice(&a.to_bytes());
            }
            BridgeAction::LimitUpdateAction(a) => {
                bytes.extend_from_slice(&a.to_bytes());
            }
            BridgeAction::AssetPriceUpdateAction(a) => {
                bytes.extend_from_slice(&a.to_bytes());
            }
            BridgeAction::EvmContractUpgradeAction(a) => {
                bytes.extend_from_slice(&a.to_bytes());
            } // TODO add formats for other events
        }
        bytes
    }

    // Digest of BridgeAction (with Keccak256 hasher)
    pub fn digest(&self) -> BridgeActionDigest {
        let mut hasher = Keccak256::default();
        hasher.update(&self.to_bytes());
        BridgeActionDigest::new(hasher.finalize().into())
    }

    pub fn chain_id(&self) -> BridgeChainId {
        match self {
            BridgeAction::IotaToEthBridgeAction(a) => a.iota_bridge_event.iota_chain_id,
            BridgeAction::EthToIotaBridgeAction(a) => a.eth_bridge_event.eth_chain_id,
            BridgeAction::BlocklistCommitteeAction(a) => a.chain_id,
            BridgeAction::EmergencyAction(a) => a.chain_id,
            BridgeAction::LimitUpdateAction(a) => a.chain_id,
            BridgeAction::AssetPriceUpdateAction(a) => a.chain_id,
            BridgeAction::EvmContractUpgradeAction(a) => a.chain_id,
        }
    }

    pub fn is_governace_action(&self) -> bool {
        match self.action_type() {
            BridgeActionType::TokenTransfer => false,
            BridgeActionType::UpdateCommitteeBlocklist => true,
            BridgeActionType::EmergencyButton => true,
            BridgeActionType::LimitUpdate => true,
            BridgeActionType::AssetPriceUpdate => true,
            BridgeActionType::EvmContractUpgrade => true,
        }
    }

    // Also called `message_type`
    pub fn action_type(&self) -> BridgeActionType {
        match self {
            BridgeAction::IotaToEthBridgeAction(_) => BridgeActionType::TokenTransfer,
            BridgeAction::EthToIotaBridgeAction(_) => BridgeActionType::TokenTransfer,
            BridgeAction::BlocklistCommitteeAction(_) => BridgeActionType::UpdateCommitteeBlocklist,
            BridgeAction::EmergencyAction(_) => BridgeActionType::EmergencyButton,
            BridgeAction::LimitUpdateAction(_) => BridgeActionType::LimitUpdate,
            BridgeAction::AssetPriceUpdateAction(_) => BridgeActionType::AssetPriceUpdate,
            BridgeAction::EvmContractUpgradeAction(_) => BridgeActionType::EvmContractUpgrade,
        }
    }

    // Also called `nonce`
    pub fn seq_number(&self) -> u64 {
        match self {
            BridgeAction::IotaToEthBridgeAction(a) => a.iota_bridge_event.nonce,
            BridgeAction::EthToIotaBridgeAction(a) => a.eth_bridge_event.nonce,
            BridgeAction::BlocklistCommitteeAction(a) => a.nonce,
            BridgeAction::EmergencyAction(a) => a.nonce,
            BridgeAction::LimitUpdateAction(a) => a.nonce,
            BridgeAction::AssetPriceUpdateAction(a) => a.nonce,
            BridgeAction::EvmContractUpgradeAction(a) => a.nonce,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct BridgeActionDigest(Digest);

impl BridgeActionDigest {
    pub const fn new(digest: [u8; 32]) -> Self {
        Self(Digest::new(digest))
    }
}

#[derive(Debug, Clone)]
pub struct BridgeCommitteeValiditySignInfo {
    pub signatures: BTreeMap<BridgeAuthorityPublicKeyBytes, BridgeAuthorityRecoverableSignature>,
}

pub type SignedBridgeAction = Envelope<BridgeAction, BridgeAuthoritySignInfo>;
pub type VerifiedSignedBridgeAction = VerifiedEnvelope<BridgeAction, BridgeAuthoritySignInfo>;
pub type CertifiedBridgeAction = Envelope<BridgeAction, BridgeCommitteeValiditySignInfo>;
pub type VerifiedCertifiedBridgeAction =
    VerifiedEnvelope<BridgeAction, BridgeCommitteeValiditySignInfo>;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BridgeEventDigest(Digest);

impl BridgeEventDigest {
    pub const fn new(digest: [u8; 32]) -> Self {
        Self(Digest::new(digest))
    }
}

impl Message for BridgeAction {
    type DigestType = BridgeEventDigest;

    // this is not encoded in message today
    const SCOPE: IntentScope = IntentScope::BridgeEventUnused;

    // this is not used today
    fn digest(&self) -> Self::DigestType {
        unreachable!("BridgeEventDigest is not used today")
    }

    fn verify_user_input(&self) -> IotaResult {
        Ok(())
    }

    fn verify_epoch(&self, _epoch: EpochId) -> IotaResult {
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EthLog {
    pub block_number: u64,
    pub tx_hash: H256,
    pub log_index_in_tx: u16,
    // TODO: pull necessary fields from `Log`.
    pub log: Log,
}

/////////////////////////// Move Types Start //////////////////////////

/// Rust version of the Move bridge::BridgeInner type.
#[derive(Debug, Serialize, Deserialize)]
pub struct MoveTypeBridgeInner {
    pub bridge_version: u64,
    pub chain_id: u8,
    pub sequence_nums: VecMap<u8, u64>,
    pub committee: MoveTypeBridgeCommittee,
    pub treasury: MoveTypeBridgeTreasury,
    pub bridge_records: LinkedTable<MoveTypeBridgeMessageKey>,
    pub frozen: bool,
}

/// Rust version of the Move treasury::BridgeTreasury type.
#[derive(Debug, Serialize, Deserialize)]
pub struct MoveTypeBridgeTreasury {
    pub treasuries: Bag,
}

/// Rust version of the Move committee::BridgeCommittee type.
#[derive(Debug, Serialize, Deserialize)]
pub struct MoveTypeBridgeCommittee {
    pub members: VecMap<Vec<u8>, MoveTypeCommitteeMember>,
    pub thresholds: VecMap<u8, u64>,
}

/// Rust version of the Move committee::CommitteeMember type.
#[derive(Debug, Serialize, Deserialize)]
pub struct MoveTypeCommitteeMember {
    pub iota_address: IotaAddress,
    pub bridge_pubkey_bytes: Vec<u8>,
    pub voting_power: u64,
    pub http_rest_url: Vec<u8>,
    pub blocklisted: bool,
}

/// Rust version of the Move message::BridgeMessageKey type.
#[derive(Debug, Serialize, Deserialize)]
pub struct MoveTypeBridgeMessageKey {
    pub source_chain: u8,
    pub message_type: u8,
    pub bridge_seq_num: u64,
}

/// Rust version of the Move message::BridgeMessage type.
#[derive(Debug, Serialize, Deserialize)]
pub struct MoveTypeBridgeMessage {
    pub message_type: u8,
    pub message_version: u8,
    pub seq_num: u64,
    pub source_chain: u8,
    pub payload: Vec<u8>,
}

/// Rust version of the Move message::BridgeMessage type.
#[derive(Debug, Serialize, Deserialize)]
pub struct MoveTypeBridgeRecord {
    pub message: MoveTypeBridgeMessage,
    pub verified_signatures: Option<Vec<Vec<u8>>>,
    pub claimed: bool,
}

/////////////////////////// Move Types End //////////////////////////

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, str::FromStr};

    use ethers::{
        abi::ParamType,
        types::{Address as EthAddress, TxHash},
    };
    use fastcrypto::{
        encoding::{Encoding, Hex},
        hash::HashFunction,
        traits::{KeyPair, ToFromBytes},
    };
    use iota_types::{
        base_types::{IotaAddress, TransactionDigest},
        crypto::get_key_pair,
    };
    use prometheus::Registry;

    use super::*;
    use crate::{test_utils::get_test_authority_and_key, types::TokenId};

    #[test]
    fn test_bridge_message_encoding() -> anyhow::Result<()> {
        telemetry_subscribers::init_for_testing();
        let registry = Registry::new();
        mysten_metrics::init_metrics(&registry);
        let nonce = 54321u64;
        let iota_tx_digest = TransactionDigest::random();
        let iota_chain_id = BridgeChainId::IotaTestnet;
        let iota_tx_event_index = 1u16;
        let eth_chain_id = BridgeChainId::EthSepolia;
        let iota_address = IotaAddress::random_for_testing_only();
        let eth_address = EthAddress::random();
        let token_id = TokenId::USDC;
        let amount = 1_000_000;

        let iota_bridge_event = EmittedIotaToEthTokenBridgeV1 {
            nonce,
            iota_chain_id,
            eth_chain_id,
            iota_address,
            eth_address,
            token_id,
            amount,
        };

        let encoded_bytes = BridgeAction::IotaToEthBridgeAction(IotaToEthBridgeAction {
            iota_tx_digest,
            iota_tx_event_index,
            iota_bridge_event,
        })
        .to_bytes();

        // Construct the expected bytes
        let prefix_bytes = BRIDGE_MESSAGE_PREFIX.to_vec(); // len: 19
        let message_type = vec![BridgeActionType::TokenTransfer as u8]; // len: 1
        let message_version = vec![TOKEN_TRANSFER_MESSAGE_VERSION]; // len: 1
        let nonce_bytes = nonce.to_be_bytes().to_vec(); // len: 8
        let source_chain_id_bytes = vec![iota_chain_id as u8]; // len: 1

        let iota_address_length_bytes = vec![IOTA_ADDRESS_LENGTH as u8]; // len: 1
        let iota_address_bytes = iota_address.to_vec(); // len: 32
        let dest_chain_id_bytes = vec![eth_chain_id as u8]; // len: 1
        let eth_address_length_bytes = vec![EthAddress::len_bytes() as u8]; // len: 1
        let eth_address_bytes = eth_address.as_bytes().to_vec(); // len: 20

        let token_id_bytes = vec![token_id as u8]; // len: 1
        let token_amount_bytes = amount.to_be_bytes().to_vec(); // len: 8

        let mut combined_bytes = Vec::new();
        combined_bytes.extend_from_slice(&prefix_bytes);
        combined_bytes.extend_from_slice(&message_type);
        combined_bytes.extend_from_slice(&message_version);
        combined_bytes.extend_from_slice(&nonce_bytes);
        combined_bytes.extend_from_slice(&source_chain_id_bytes);
        combined_bytes.extend_from_slice(&iota_address_length_bytes);
        combined_bytes.extend_from_slice(&iota_address_bytes);
        combined_bytes.extend_from_slice(&dest_chain_id_bytes);
        combined_bytes.extend_from_slice(&eth_address_length_bytes);
        combined_bytes.extend_from_slice(&eth_address_bytes);
        combined_bytes.extend_from_slice(&token_id_bytes);
        combined_bytes.extend_from_slice(&token_amount_bytes);

        assert_eq!(combined_bytes, encoded_bytes);

        // Assert fixed length
        // TODO: for each action type add a test to assert the length
        assert_eq!(
            combined_bytes.len(),
            19 + 1 + 1 + 8 + 1 + 1 + 32 + 1 + 20 + 1 + 1 + 8
        );
        Ok(())
    }

    #[test]
    fn test_bridge_message_encoding_regression_emitted_iota_to_eth_token_bridge_v1()
    -> anyhow::Result<()> {
        telemetry_subscribers::init_for_testing();
        let registry = Registry::new();
        mysten_metrics::init_metrics(&registry);
        let iota_tx_digest = TransactionDigest::random();
        let iota_tx_event_index = 1u16;

        let nonce = 10u64;
        let iota_chain_id = BridgeChainId::IotaTestnet;
        let eth_chain_id = BridgeChainId::EthSepolia;
        let iota_address = IotaAddress::from_str(
            "0x0000000000000000000000000000000000000000000000000000000000000064",
        )
        .unwrap();
        let eth_address =
            EthAddress::from_str("0x00000000000000000000000000000000000000c8").unwrap();
        let token_id = TokenId::USDC;
        let amount = 12345;

        let iota_bridge_event = EmittedIotaToEthTokenBridgeV1 {
            nonce,
            iota_chain_id,
            eth_chain_id,
            iota_address,
            eth_address,
            token_id,
            amount,
        };
        let encoded_bytes = BridgeAction::IotaToEthBridgeAction(IotaToEthBridgeAction {
            iota_tx_digest,
            iota_tx_event_index,
            iota_bridge_event,
        })
        .to_bytes();
        assert_eq!(
            Hex::encode(&encoded_bytes),
            format!(
                "{}0001000000000000000a012000000000000000000000000000000000000000000000000000000000000000640b1400000000000000000000000000000000000000c8030000000000003039",
                Hex::encode(BRIDGE_MESSAGE_PREFIX)
            )
        );

        let hash = Keccak256::digest(encoded_bytes).digest;
        assert_eq!(
            Hex::encode(hash),
            "0fc6f3e2a8c64915217ac56ee10c722c633ce4cabe8ef17f1ee67c403c76608e",
        );
        Ok(())
    }

    #[test]
    fn test_bridge_message_encoding_blocklist_update_v1() {
        telemetry_subscribers::init_for_testing();
        let registry = Registry::new();
        mysten_metrics::init_metrics(&registry);

        let pub_key_bytes = BridgeAuthorityPublicKeyBytes::from_bytes(
            &Hex::decode("02321ede33d2c2d7a8a152f275a1484edef2098f034121a602cb7d767d38680aa4")
                .unwrap(),
        )
        .unwrap();
        let blocklist_action = BridgeAction::BlocklistCommitteeAction(BlocklistCommitteeAction {
            nonce: 129,
            chain_id: BridgeChainId::IotaLocalTest,
            blocklist_type: BlocklistType::Blocklist,
            blocklisted_members: vec![pub_key_bytes.clone()],
        });
        let bytes = blocklist_action.to_bytes();
        // b"IOTA_BRIDGE_MESSAGE" prefix
        // 01: msg type
        // 01: msg version
        // 0000000000000081: nonce
        // 03: chain id
        // 00: blocklist type
        // 01: length of updated members
        // [
        // 68b43fd906c0b8f024a18c56e06744f7c6157c65
        // ]: blocklisted members abi-encoded
        assert_eq!(
            Hex::encode(bytes),
            format!(
                "{}\
                01\
                01\
                0000000000000081\
                03\
                00\
                01\
                68b43fd906c0b8f024a18c56e06744f7c6157c65",
                Hex::encode(BRIDGE_MESSAGE_PREFIX)
            )
        );

        let pub_key_bytes_2 = BridgeAuthorityPublicKeyBytes::from_bytes(
            &Hex::decode("027f1178ff417fc9f5b8290bd8876f0a157a505a6c52db100a8492203ddd1d4279")
                .unwrap(),
        )
        .unwrap();
        // its evm address: 0xacaef39832cb995c4e049437a3e2ec6a7bad1ab5
        let blocklist_action = BridgeAction::BlocklistCommitteeAction(BlocklistCommitteeAction {
            nonce: 68,
            chain_id: BridgeChainId::IotaDevnet,
            blocklist_type: BlocklistType::Unblocklist,
            blocklisted_members: vec![pub_key_bytes.clone(), pub_key_bytes_2.clone()],
        });
        let bytes = blocklist_action.to_bytes();
        // b"IOTA_BRIDGE_MESSAGE" prefix
        // 01: msg type
        // 01: msg version
        // 0000000000000044: nonce
        // 02: chain id
        // 01: blocklist type
        // 02: length of updated members
        // [
        // 68b43fd906c0b8f024a18c56e06744f7c6157c65
        // acaef39832cb995c4e049437a3e2ec6a7bad1ab5
        // ]: blocklisted members abi-encoded
        assert_eq!(
            Hex::encode(bytes),
            format!(
                "{}\
                01\
                01\
                0000000000000044\
                02\
                01\
                02\
                68b43fd906c0b8f024a18c56e06744f7c6157c65\
                acaef39832cb995c4e049437a3e2ec6a7bad1ab5",
                Hex::encode(BRIDGE_MESSAGE_PREFIX)
            )
        );

        let blocklist_action = BridgeAction::BlocklistCommitteeAction(BlocklistCommitteeAction {
            nonce: 49,
            chain_id: BridgeChainId::EthLocalTest,
            blocklist_type: BlocklistType::Blocklist,
            blocklisted_members: vec![pub_key_bytes.clone()],
        });
        let bytes = blocklist_action.to_bytes();
        // b"IOTA_BRIDGE_MESSAGE" prefix
        // 01: msg type
        // 01: msg version
        // 0000000000000031: nonce
        // 0c: chain id
        // 00: blocklist type
        // 01: length of updated members
        // [
        // 68b43fd906c0b8f024a18c56e06744f7c6157c65
        // ]: blocklisted members abi-encoded
        assert_eq!(
            Hex::encode(bytes),
            format!(
                "{}\
                01\
                01\
                0000000000000031\
                0c\
                00\
                01\
                68b43fd906c0b8f024a18c56e06744f7c6157c65",
                Hex::encode(BRIDGE_MESSAGE_PREFIX)
            )
        );

        let blocklist_action = BridgeAction::BlocklistCommitteeAction(BlocklistCommitteeAction {
            nonce: 94,
            chain_id: BridgeChainId::EthSepolia,
            blocklist_type: BlocklistType::Unblocklist,
            blocklisted_members: vec![pub_key_bytes.clone(), pub_key_bytes_2.clone()],
        });
        let bytes = blocklist_action.to_bytes();
        // b"IOTA_BRIDGE_MESSAGE" prefix
        // 01: msg type
        // 01: msg version
        // 000000000000005e: nonce
        // 0b: chain id
        // 01: blocklist type
        // 02: length of updated members
        // [
        // 68b43fd906c0b8f024a18c56e06744f7c6157c65
        // acaef39832cb995c4e049437a3e2ec6a7bad1ab5
        // ]: blocklisted members abi-encoded
        assert_eq!(
            Hex::encode(bytes),
            format!(
                "{}\
                01\
                01\
                000000000000005e\
                0b\
                01\
                02\
                68b43fd906c0b8f024a18c56e06744f7c6157c65\
                acaef39832cb995c4e049437a3e2ec6a7bad1ab5",
                Hex::encode(BRIDGE_MESSAGE_PREFIX)
            )
        );
    }

    #[test]
    fn test_emergency_action_encoding() {
        let action = BridgeAction::EmergencyAction(EmergencyAction {
            nonce: 55,
            chain_id: BridgeChainId::IotaLocalTest,
            action_type: EmergencyActionType::Pause,
        });
        let bytes = action.to_bytes();
        // b"IOTA_BRIDGE_MESSAGE" prefix
        // 02: msg type
        // 01: msg version
        // 0000000000000037: nonce
        // 03: chain id
        // 00: action type
        assert_eq!(
            Hex::encode(bytes),
            format!(
                "{}\
                02\
                01\
                0000000000000037\
                03\
                00",
                Hex::encode(BRIDGE_MESSAGE_PREFIX)
            )
        );

        let action = BridgeAction::EmergencyAction(EmergencyAction {
            nonce: 56,
            chain_id: BridgeChainId::EthSepolia,
            action_type: EmergencyActionType::Unpause,
        });
        let bytes = action.to_bytes();
        // b"IOTA_BRIDGE_MESSAGE" prefix
        // 02: msg type
        // 01: msg version
        // 0000000000000038: nonce
        // 0b: chain id
        // 01: action type
        assert_eq!(
            Hex::encode(bytes),
            format!(
                "{}\
                02\
                01\
                0000000000000038\
                0b\
                01",
                Hex::encode(BRIDGE_MESSAGE_PREFIX)
            )
        );
    }

    #[test]
    fn test_limit_update_action_encoding() {
        let action = BridgeAction::LimitUpdateAction(LimitUpdateAction {
            nonce: 15,
            chain_id: BridgeChainId::IotaLocalTest,
            sending_chain_id: BridgeChainId::EthLocalTest,
            new_usd_limit: 1_000_000 * USD_MULTIPLIER, // $1M USD
        });
        let bytes = action.to_bytes();
        // b"IOTA_BRIDGE_MESSAGE" prefix
        // 03: msg type
        // 01: msg version
        // 000000000000000f: nonce
        // 03: chain id
        // 0c: sending chain id
        // 00000002540be400: new usd limit
        assert_eq!(
            Hex::encode(bytes),
            format!(
                "{}\
                03\
                01\
                000000000000000f\
                03\
                0c\
                00000002540be400",
                Hex::encode(BRIDGE_MESSAGE_PREFIX)
            )
        );
    }

    #[test]
    fn test_asset_price_update_action_encoding() {
        let action = BridgeAction::AssetPriceUpdateAction(AssetPriceUpdateAction {
            nonce: 266,
            chain_id: BridgeChainId::IotaLocalTest,
            token_id: TokenId::BTC,
            new_usd_price: 100_000 * USD_MULTIPLIER, // $100k USD
        });
        let bytes = action.to_bytes();
        // b"IOTA_BRIDGE_MESSAGE" prefix
        // 04: msg type
        // 01: msg version
        // 000000000000010a: nonce
        // 03: chain id
        // 01: token id
        // 000000003b9aca00: new usd price
        assert_eq!(
            Hex::encode(bytes),
            format!(
                "{}\
                04\
                01\
                000000000000010a\
                03\
                01\
                000000003b9aca00",
                Hex::encode(BRIDGE_MESSAGE_PREFIX)
            )
        );
    }

    #[test]
    fn test_evm_contract_upgrade_action() {
        // Calldata with only the function selector and no parameters: `function
        // initializeV2()`
        let function_signature = "initializeV2()";
        let selector = &Keccak256::digest(function_signature).digest[0..4];
        let call_data = selector.to_vec();
        assert_eq!(Hex::encode(call_data.clone()), "5cd8a76b");

        let action = BridgeAction::EvmContractUpgradeAction(EvmContractUpgradeAction {
            nonce: 123,
            chain_id: BridgeChainId::EthLocalTest,
            proxy_address: EthAddress::repeat_byte(6),
            new_impl_address: EthAddress::repeat_byte(9),
            call_data,
        });
        // b"IOTA_BRIDGE_MESSAGE" prefix
        // 05: msg type
        // 01: msg version
        // 000000000000007b: nonce
        // 0c: chain id
        // 0000000000000000000000000606060606060606060606060606060606060606: proxy
        // address
        // 0000000000000000000000000909090909090909090909090909090909090909: new impl
        // address
        //
        // 0000000000000000000000000000000000000000000000000000000000000060
        // 0000000000000000000000000000000000000000000000000000000000000004
        // 5cd8a76b00000000000000000000000000000000000000000000000000000000: call data
        assert_eq!(
            Hex::encode(action.to_bytes()),
            format!(
                "{}\
                05\
                01\
                000000000000007b\
                0c\
                0000000000000000000000000606060606060606060606060606060606060606\
                0000000000000000000000000909090909090909090909090909090909090909\
                0000000000000000000000000000000000000000000000000000000000000060\
                0000000000000000000000000000000000000000000000000000000000000004\
                5cd8a76b00000000000000000000000000000000000000000000000000000000",
                Hex::encode(BRIDGE_MESSAGE_PREFIX)
            )
        );

        // Calldata with one parameter: `function newMockFunction(bool)`
        let function_signature = "newMockFunction(bool)";
        let selector = &Keccak256::digest(function_signature).digest[0..4];
        let mut call_data = selector.to_vec();
        call_data.extend(ethers::abi::encode(&[ethers::abi::Token::Bool(true)]));
        assert_eq!(
            Hex::encode(call_data.clone()),
            "417795ef0000000000000000000000000000000000000000000000000000000000000001"
        );
        let action = BridgeAction::EvmContractUpgradeAction(EvmContractUpgradeAction {
            nonce: 123,
            chain_id: BridgeChainId::EthLocalTest,
            proxy_address: EthAddress::repeat_byte(6),
            new_impl_address: EthAddress::repeat_byte(9),
            call_data,
        });
        // b"IOTA_BRIDGE_MESSAGE" prefix
        // 05: msg type
        // 01: msg version
        // 000000000000007b: nonce
        // 0c: chain id
        // 0000000000000000000000000606060606060606060606060606060606060606: proxy
        // address
        // 0000000000000000000000000909090909090909090909090909090909090909: new impl
        // address
        //
        // 0000000000000000000000000000000000000000000000000000000000000060
        // 0000000000000000000000000000000000000000000000000000000000000024
        // 417795ef00000000000000000000000000000000000000000000000000000000
        // 0000000100000000000000000000000000000000000000000000000000000000: call data
        assert_eq!(
            Hex::encode(action.to_bytes()),
            format!(
                "{}\
                05\
                01\
                000000000000007b\
                0c\
                0000000000000000000000000606060606060606060606060606060606060606\
                0000000000000000000000000909090909090909090909090909090909090909\
                0000000000000000000000000000000000000000000000000000000000000060\
                0000000000000000000000000000000000000000000000000000000000000024\
                417795ef00000000000000000000000000000000000000000000000000000000\
                0000000100000000000000000000000000000000000000000000000000000000",
                Hex::encode(BRIDGE_MESSAGE_PREFIX)
            )
        );

        // Calldata with two parameters: `function newerMockFunction(bool, uint8)`
        let function_signature = "newMockFunction(bool,uint8)";
        let selector = &Keccak256::digest(function_signature).digest[0..4];
        let mut call_data = selector.to_vec();
        call_data.extend(ethers::abi::encode(&[
            ethers::abi::Token::Bool(true),
            ethers::abi::Token::Uint(42u8.into()),
        ]));
        assert_eq!(
            Hex::encode(call_data.clone()),
            "be8fc25d0000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000002a"
        );
        let action = BridgeAction::EvmContractUpgradeAction(EvmContractUpgradeAction {
            nonce: 123,
            chain_id: BridgeChainId::EthLocalTest,
            proxy_address: EthAddress::repeat_byte(6),
            new_impl_address: EthAddress::repeat_byte(9),
            call_data,
        });
        // b"IOTA_BRIDGE_MESSAGE" prefix
        // 05: msg type
        // 01: msg version
        // 000000000000007b: nonce
        // 0c: chain id
        // 0000000000000000000000000606060606060606060606060606060606060606: proxy
        // address
        // 0000000000000000000000000909090909090909090909090909090909090909: new impl
        // address
        //
        // 0000000000000000000000000000000000000000000000000000000000000060
        // 0000000000000000000000000000000000000000000000000000000000000044
        // be8fc25d00000000000000000000000000000000000000000000000000000000
        // 0000000100000000000000000000000000000000000000000000000000000000
        // 0000002a00000000000000000000000000000000000000000000000000000000: call data
        assert_eq!(
            Hex::encode(action.to_bytes()),
            format!(
                "{}\
                05\
                01\
                000000000000007b\
                0c\
                0000000000000000000000000606060606060606060606060606060606060606\
                0000000000000000000000000909090909090909090909090909090909090909\
                0000000000000000000000000000000000000000000000000000000000000060\
                0000000000000000000000000000000000000000000000000000000000000044\
                be8fc25d00000000000000000000000000000000000000000000000000000000\
                0000000100000000000000000000000000000000000000000000000000000000\
                0000002a00000000000000000000000000000000000000000000000000000000",
                Hex::encode(BRIDGE_MESSAGE_PREFIX)
            )
        );

        // Empty calldate
        let action = BridgeAction::EvmContractUpgradeAction(EvmContractUpgradeAction {
            nonce: 123,
            chain_id: BridgeChainId::EthLocalTest,
            proxy_address: EthAddress::repeat_byte(6),
            new_impl_address: EthAddress::repeat_byte(9),
            call_data: vec![],
        });
        // b"IOTA_BRIDGE_MESSAGE" prefix
        // 05: msg type
        // 01: msg version
        // 000000000000007b: nonce
        // 0c: chain id
        // 0000000000000000000000000606060606060606060606060606060606060606: proxy
        // address
        // 0000000000000000000000000909090909090909090909090909090909090909: new impl
        // address
        //
        // 0000000000000000000000000000000000000000000000000000000000000060
        // 0000000000000000000000000000000000000000000000000000000000000000: call data
        let data = action.to_bytes();
        assert_eq!(
            Hex::encode(&data),
            format!(
                "{}\
                05\
                01\
                000000000000007b\
                0c\
                0000000000000000000000000606060606060606060606060606060606060606\
                0000000000000000000000000909090909090909090909090909090909090909\
                0000000000000000000000000000000000000000000000000000000000000060\
                0000000000000000000000000000000000000000000000000000000000000000",
                Hex::encode(BRIDGE_MESSAGE_PREFIX)
            )
        );
        let types = vec![ParamType::Address, ParamType::Address, ParamType::Bytes];
        // Ensure that the call data (start from byte 30) can be decoded
        ethers::abi::decode(&types, &data[30..]).unwrap();
    }

    #[test]
    fn test_bridge_message_encoding_regression_eth_to_iota_token_bridge_v1() -> anyhow::Result<()> {
        telemetry_subscribers::init_for_testing();
        let registry = Registry::new();
        mysten_metrics::init_metrics(&registry);
        let eth_tx_hash = TxHash::random();
        let eth_event_index = 1u16;

        let nonce = 10u64;
        let iota_chain_id = BridgeChainId::IotaTestnet;
        let eth_chain_id = BridgeChainId::EthSepolia;
        let iota_address = IotaAddress::from_str(
            "0x0000000000000000000000000000000000000000000000000000000000000064",
        )
        .unwrap();
        let eth_address =
            EthAddress::from_str("0x00000000000000000000000000000000000000c8").unwrap();
        let token_id = TokenId::USDC;
        let amount = 12345;

        let eth_bridge_event = EthToIotaTokenBridgeV1 {
            nonce,
            iota_chain_id,
            eth_chain_id,
            iota_address,
            eth_address,
            token_id,
            amount,
        };
        let encoded_bytes = BridgeAction::EthToIotaBridgeAction(EthToIotaBridgeAction {
            eth_tx_hash,
            eth_event_index,
            eth_bridge_event,
        })
        .to_bytes();

        assert_eq!(
            Hex::encode(&encoded_bytes),
            format!(
                "{}0001000000000000000a0b1400000000000000000000000000000000000000c801200000000000000000000000000000000000000000000000000000000000000064030000000000003039",
                Hex::encode(BRIDGE_MESSAGE_PREFIX)
            )
        );

        let hash = Keccak256::digest(encoded_bytes).digest;
        assert_eq!(
            Hex::encode(hash),
            "cffec5fb6bf31c8fae7441a49bbf17127eadfb96efe14da8f8f81c1cdd538597",
        );
        Ok(())
    }

    #[test]
    fn test_bridge_committee_construction() -> anyhow::Result<()> {
        let (mut authority, _, _) = get_test_authority_and_key(10000, 9999);
        // This is ok
        let _ = BridgeCommittee::new(vec![authority.clone()]).unwrap();

        // This is not ok - total voting power != 10000
        authority.voting_power = 9999;
        let _ = BridgeCommittee::new(vec![authority.clone()]).unwrap_err();

        // This is not ok - total voting power != 10000
        authority.voting_power = 10001;
        let _ = BridgeCommittee::new(vec![authority.clone()]).unwrap_err();

        // This is ok
        authority.voting_power = 5000;
        let mut authority_2 = authority.clone();
        let (_, kp): (_, fastcrypto::secp256k1::Secp256k1KeyPair) = get_key_pair();
        let pubkey = kp.public().clone();
        authority_2.pubkey = pubkey.clone();
        let _ = BridgeCommittee::new(vec![authority.clone(), authority_2.clone()]).unwrap();

        // This is not ok - duplicate pub key
        authority_2.pubkey = authority.pubkey.clone();
        let _ = BridgeCommittee::new(vec![authority.clone(), authority.clone()]).unwrap_err();
        Ok(())
    }

    #[test]
    fn test_bridge_committee_total_blocklisted_stake() -> anyhow::Result<()> {
        let (mut authority1, _, _) = get_test_authority_and_key(10000, 9999);
        assert_eq!(
            BridgeCommittee::new(vec![authority1.clone()])
                .unwrap()
                .total_blocklisted_stake(),
            0
        );
        authority1.voting_power = 6000;

        let (mut authority2, _, _) = get_test_authority_and_key(4000, 9999);
        authority2.is_blocklisted = true;
        assert_eq!(
            BridgeCommittee::new(vec![authority1.clone(), authority2.clone()])
                .unwrap()
                .total_blocklisted_stake(),
            4000
        );

        authority1.voting_power = 7000;
        authority2.voting_power = 2000;
        let (mut authority3, _, _) = get_test_authority_and_key(1000, 9999);
        authority3.is_blocklisted = true;
        assert_eq!(
            BridgeCommittee::new(vec![authority1, authority2, authority3])
                .unwrap()
                .total_blocklisted_stake(),
            3000
        );

        Ok(())
    }

    #[test]
    fn test_bridge_committee_filter_blocklisted_authorities() -> anyhow::Result<()> {
        // Note: today BridgeCommitte does not shuffle authorities
        let (authority1, _, _) = get_test_authority_and_key(5000, 9999);
        let (mut authority2, _, _) = get_test_authority_and_key(3000, 9999);
        authority2.is_blocklisted = true;
        let (authority3, _, _) = get_test_authority_and_key(2000, 9999);
        let committee = BridgeCommittee::new(vec![
            authority1.clone(),
            authority2.clone(),
            authority3.clone(),
        ])
        .unwrap();

        // exclude authority2
        let result = committee
            .shuffle_by_stake(None, None)
            .into_iter()
            .collect::<HashSet<_>>();
        assert_eq!(
            HashSet::from_iter(vec![authority1.pubkey_bytes(), authority3.pubkey_bytes()]),
            result
        );

        // exclude authority2 and authority3
        let result = committee
            .shuffle_by_stake(
                None,
                Some(
                    &[authority1.pubkey_bytes(), authority2.pubkey_bytes()]
                        .iter()
                        .cloned()
                        .collect(),
                ),
            )
            .into_iter()
            .collect::<HashSet<_>>();
        assert_eq!(HashSet::from_iter(vec![authority1.pubkey_bytes()]), result);

        Ok(())
    }
}
