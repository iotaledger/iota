// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This file contains the definition of the IotaBridgeEvent enum, of
//! which each variant is an emitted Event struct defined in the Move
//! Bridge module. We rely on structures in this file to decode
//! the bcs content of the emitted events.

#![allow(non_upper_case_globals)]

use std::str::FromStr;

use ethers::types::Address as EthAddress;
use fastcrypto::encoding::{Encoding, Hex};
use iota_json_rpc_types::IotaEvent;
use iota_types::{
    BRIDGE_PACKAGE_ID, TypeTag,
    base_types::IotaAddress,
    bridge::{
        BridgeChainId, MoveTypeBridgeMessageKey, MoveTypeCommitteeMember,
        MoveTypeCommitteeMemberRegistration,
    },
    collection_types::VecMap,
    crypto::ToFromBytes,
    digests::TransactionDigest,
    parse_iota_type_tag,
};
use move_core_types::language_storage::StructTag;
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};

use crate::{
    crypto::BridgeAuthorityPublicKey,
    error::{BridgeError, BridgeResult},
    types::{BridgeAction, IotaToEthBridgeAction},
};

// `TokendDepositedEvent` emitted in bridge.move
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct MoveTokenDepositedEvent {
    pub seq_num: u64,
    pub source_chain: u8,
    pub sender_address: Vec<u8>,
    pub target_chain: u8,
    pub target_address: Vec<u8>,
    pub token_type: u8,
    pub amount_iota_adjusted: u64,
}

macro_rules! new_move_event {
    ($struct_name:ident, $move_struct_name:ident) => {

        // `$move_struct_name` emitted in bridge.move
        #[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
        pub struct $move_struct_name {
            pub message_key: MoveTypeBridgeMessageKey,
        }

        // Sanitized version of the given `move_struct_name`
        #[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Hash)]
        pub struct $struct_name {
            pub nonce: u64,
            pub source_chain: BridgeChainId,
        }

        impl TryFrom<$move_struct_name> for $struct_name {
            type Error = BridgeError;

            fn try_from(event: $move_struct_name) -> BridgeResult<Self> {
                let source_chain = BridgeChainId::try_from(event.message_key.source_chain).map_err(|_e| {
                    BridgeError::Generic(format!(
                        "Failed to convert {} to {}. Failed to convert source chain {} to BridgeChainId",
                        stringify!($move_struct_name),
                        stringify!($struct_name),
                        event.message_key.source_chain,
                    ))
                })?;
                Ok(Self {
                    nonce: event.message_key.bridge_seq_num,
                    source_chain,
                })
            }
        }
    };
}

new_move_event!(TokenTransferClaimed, MoveTokenTransferClaimed);
new_move_event!(TokenTransferApproved, MoveTokenTransferApproved);
new_move_event!(
    TokenTransferAlreadyApproved,
    MoveTokenTransferAlreadyApproved
);
new_move_event!(TokenTransferAlreadyClaimed, MoveTokenTransferAlreadyClaimed);
new_move_event!(TokenTransferLimitExceed, MoveTokenTransferLimitExceed);

// `EmergencyOpEvent` emitted in bridge.move
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct EmergencyOpEvent {
    pub frozen: bool,
}

// `CommitteeUpdateEvent` emitted in committee.move
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MoveCommitteeUpdateEvent {
    pub members: VecMap<Vec<u8>, MoveTypeCommitteeMember>,
    pub stake_participation_percentage: u64,
}

// `CommitteeMemberUrlUpdateEvent` emitted in committee.move
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MoveCommitteeMemberUrlUpdateEvent {
    pub member: Vec<u8>,
    pub new_url: Vec<u8>,
}

// `BlocklistValidatorEvent` emitted in committee.move
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MoveBlocklistValidatorEvent {
    pub blocklisted: bool,
    pub public_keys: Vec<Vec<u8>>,
}

// `UpdateRouteLimitEvent` emitted in limiter.move
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MoveUpdateRouteLimitEvent {
    pub sending_chain: u8,
    pub receiving_chain: u8,
    pub new_limit: u64,
}

// `TokenRegistrationEvent` emitted in treasury.move
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MoveTokenRegistrationEvent {
    pub type_name: String,
    pub decimal: u8,
    pub native_token: bool,
}

// Sanitized version of MoveTokenRegistrationEvent
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct TokenRegistrationEvent {
    pub type_name: TypeTag,
    pub decimal: u8,
    pub native_token: bool,
}

impl TryFrom<MoveTokenRegistrationEvent> for TokenRegistrationEvent {
    type Error = BridgeError;

    fn try_from(event: MoveTokenRegistrationEvent) -> BridgeResult<Self> {
        let type_name = parse_iota_type_tag(&format!("0x{}", event.type_name)).map_err(|e| {
            BridgeError::Internal(format!(
                "Failed to parse TypeTag: {e}, type name: {}",
                event.type_name
            ))
        })?;

        Ok(Self {
            type_name,
            decimal: event.decimal,
            native_token: event.native_token,
        })
    }
}

// `NewTokenEvent` emitted in treasury.move
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MoveNewTokenEvent {
    pub token_id: u8,
    pub type_name: String,
    pub native_token: bool,
    pub decimal_multiplier: u64,
    pub notional_value: u64,
}

// Sanitized version of MoveNewTokenEvent
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct NewTokenEvent {
    pub token_id: u8,
    pub type_name: TypeTag,
    pub native_token: bool,
    pub decimal_multiplier: u64,
    pub notional_value: u64,
}

impl TryFrom<MoveNewTokenEvent> for NewTokenEvent {
    type Error = BridgeError;

    fn try_from(event: MoveNewTokenEvent) -> BridgeResult<Self> {
        let type_name = parse_iota_type_tag(&format!("0x{}", event.type_name)).map_err(|e| {
            BridgeError::Internal(format!(
                "Failed to parse TypeTag: {e}, type name: {}",
                event.type_name
            ))
        })?;

        Ok(Self {
            token_id: event.token_id,
            type_name,
            native_token: event.native_token,
            decimal_multiplier: event.decimal_multiplier,
            notional_value: event.notional_value,
        })
    }
}

// `UpdateTokenPriceEvent` emitted in treasury.move
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct UpdateTokenPriceEvent {
    pub token_id: u8,
    pub new_price: u64,
}

// Sanitized version of MoveTokenDepositedEvent
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Hash)]
pub struct EmittedIotaToEthTokenBridgeV1 {
    pub nonce: u64,
    pub iota_chain_id: BridgeChainId,
    pub eth_chain_id: BridgeChainId,
    pub iota_address: IotaAddress,
    pub eth_address: EthAddress,
    pub token_id: u8,
    // The amount of tokens deposited with decimal points on IOTA side
    pub amount_iota_adjusted: u64,
}

// Sanitized version of MoveCommitteeUpdateEvent
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct CommitteeUpdate {
    pub members: Vec<MoveTypeCommitteeMember>,
    pub stake_participation_percentage: u64,
}

impl TryFrom<MoveCommitteeUpdateEvent> for CommitteeUpdate {
    type Error = BridgeError;

    fn try_from(event: MoveCommitteeUpdateEvent) -> BridgeResult<Self> {
        let members = event
            .members
            .contents
            .into_iter()
            .map(|v| v.value)
            .collect();
        Ok(Self {
            members,
            stake_participation_percentage: event.stake_participation_percentage,
        })
    }
}

// Sanitized version of MoveBlocklistValidatorEvent
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct BlocklistValidatorEvent {
    pub blocklisted: bool,
    pub public_keys: Vec<BridgeAuthorityPublicKey>,
}

impl TryFrom<MoveBlocklistValidatorEvent> for BlocklistValidatorEvent {
    type Error = BridgeError;

    fn try_from(event: MoveBlocklistValidatorEvent) -> BridgeResult<Self> {
        let public_keys = event.public_keys.into_iter().map(|bytes|
            BridgeAuthorityPublicKey::from_bytes(&bytes).map_err(|e|
                BridgeError::Generic(format!("Failed to convert MoveBlocklistValidatorEvent to BlocklistValidatorEvent. Failed to convert public key to BridgeAuthorityPublicKey: {:?}", e))
            )
        ).collect::<BridgeResult<Vec<_>>>()?;
        Ok(Self {
            blocklisted: event.blocklisted,
            public_keys,
        })
    }
}

// Sanitized version of MoveCommitteeMemberUrlUpdateEvent
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct CommitteeMemberUrlUpdateEvent {
    pub member: BridgeAuthorityPublicKey,
    pub new_url: String,
}

impl TryFrom<MoveCommitteeMemberUrlUpdateEvent> for CommitteeMemberUrlUpdateEvent {
    type Error = BridgeError;

    fn try_from(event: MoveCommitteeMemberUrlUpdateEvent) -> BridgeResult<Self> {
        let member = BridgeAuthorityPublicKey::from_bytes(&event.member).map_err(|e|
            BridgeError::Generic(format!("Failed to convert MoveBlocklistValidatorEvent to BlocklistValidatorEvent. Failed to convert public key to BridgeAuthorityPublicKey: {:?}", e))
        )?;
        let new_url = String::from_utf8(event.new_url).map_err(|e|
            BridgeError::Generic(format!("Failed to convert MoveBlocklistValidatorEvent to BlocklistValidatorEvent. Failed to convert new_url to String: {:?}", e))
        )?;
        Ok(Self { member, new_url })
    }
}

impl TryFrom<MoveTokenDepositedEvent> for EmittedIotaToEthTokenBridgeV1 {
    type Error = BridgeError;

    fn try_from(event: MoveTokenDepositedEvent) -> BridgeResult<Self> {
        if event.amount_iota_adjusted == 0 {
            return Err(BridgeError::ZeroValueBridgeTransfer(format!(
                "Failed to convert MoveTokenDepositedEvent to EmittedIotaToEthTokenBridgeV1. Manual intervention is required. 0 value transfer should not be allowed in Move: {:?}",
                event,
            )));
        }

        let token_id = event.token_type;
        let iota_chain_id = BridgeChainId::try_from(event.source_chain).map_err(|_e| {
            BridgeError::Generic(format!(
                "Failed to convert MoveTokenDepositedEvent to EmittedIotaToEthTokenBridgeV1. Failed to convert source chain {} to BridgeChainId",
                event.token_type,
            ))
        })?;
        let eth_chain_id = BridgeChainId::try_from(event.target_chain).map_err(|_e| {
            BridgeError::Generic(format!(
                "Failed to convert MoveTokenDepositedEvent to EmittedIotaToEthTokenBridgeV1. Failed to convert target chain {} to BridgeChainId",
                event.token_type,
            ))
        })?;
        if !iota_chain_id.is_iota_chain() {
            return Err(BridgeError::Generic(format!(
                "Failed to convert MoveTokenDepositedEvent to EmittedIotaToEthTokenBridgeV1. Invalid source chain {}",
                event.source_chain
            )));
        }
        if eth_chain_id.is_iota_chain() {
            return Err(BridgeError::Generic(format!(
                "Failed to convert MoveTokenDepositedEvent to EmittedIotaToEthTokenBridgeV1. Invalid target chain {}",
                event.target_chain
            )));
        }

        let iota_address = IotaAddress::from_bytes(event.sender_address)
            .map_err(|e| BridgeError::Generic(format!("Failed to convert MoveTokenDepositedEvent to EmittedIotaToEthTokenBridgeV1. Failed to convert sender_address to IotaAddress: {:?}", e)))?;
        let eth_address = EthAddress::from_str(&Hex::encode(&event.target_address))?;

        Ok(Self {
            nonce: event.seq_num,
            iota_chain_id,
            eth_chain_id,
            iota_address,
            eth_address,
            token_id,
            amount_iota_adjusted: event.amount_iota_adjusted,
        })
    }
}

crate::declare_events!(
    IotaToEthTokenBridgeV1(EmittedIotaToEthTokenBridgeV1) => ("bridge::TokenDepositedEvent", MoveTokenDepositedEvent),
    TokenTransferApproved(TokenTransferApproved) => ("bridge::TokenTransferApproved", MoveTokenTransferApproved),
    TokenTransferClaimed(TokenTransferClaimed) => ("bridge::TokenTransferClaimed", MoveTokenTransferClaimed),
    TokenTransferAlreadyApproved(TokenTransferAlreadyApproved) => ("bridge::TokenTransferAlreadyApproved", MoveTokenTransferAlreadyApproved),
    TokenTransferAlreadyClaimed(TokenTransferAlreadyClaimed) => ("bridge::TokenTransferAlreadyClaimed", MoveTokenTransferAlreadyClaimed),
    TokenTransferLimitExceed(TokenTransferLimitExceed) => ("bridge::TokenTransferLimitExceed", MoveTokenTransferLimitExceed),
    EmergencyOpEvent(EmergencyOpEvent) => ("bridge::EmergencyOpEvent", EmergencyOpEvent),
    // No need to define a sanitized event struct for MoveTypeCommitteeMemberRegistration
    // because the info provided by validators could be invalid
    CommitteeMemberRegistration(MoveTypeCommitteeMemberRegistration) => ("committee::CommitteeMemberRegistration", MoveTypeCommitteeMemberRegistration),
    CommitteeUpdateEvent(CommitteeUpdate) => ("committee::CommitteeUpdateEvent", MoveCommitteeUpdateEvent),
    CommitteeMemberUrlUpdateEvent(CommitteeMemberUrlUpdateEvent) => ("committee::CommitteeMemberUrlUpdateEvent", MoveCommitteeMemberUrlUpdateEvent),
    BlocklistValidatorEvent(BlocklistValidatorEvent) => ("committee::BlocklistValidatorEvent", MoveBlocklistValidatorEvent),
    TokenRegistrationEvent(TokenRegistrationEvent) => ("treasury::TokenRegistrationEvent", MoveTokenRegistrationEvent),
    NewTokenEvent(NewTokenEvent) => ("treasury::NewTokenEvent", MoveNewTokenEvent),
    UpdateTokenPriceEvent(UpdateTokenPriceEvent) => ("treasury::UpdateTokenPriceEvent", UpdateTokenPriceEvent),

    // Add new event types here. Format:
    // EnumVariantName(Struct) => ("{module}::{event_struct}", CorrespondingMoveStruct)
);

#[macro_export]
macro_rules! declare_events {
    ($($variant:ident($type:path) => ($event_tag:expr, $event_struct:path)),* $(,)?) => {

        #[derive(Debug, Eq, PartialEq, Clone, Serialize, Deserialize)]
        pub enum IotaBridgeEvent {
            $($variant($type),)*
        }

        $(pub static $variant: OnceCell<StructTag> = OnceCell::new();)*

        pub(crate) fn init_all_struct_tags() {
            $($variant.get_or_init(|| {
                StructTag::from_str(&format!("0x{}::{}", BRIDGE_PACKAGE_ID.to_hex(), $event_tag)).unwrap()
            });)*
        }

        // Try to convert a IotaEvent into IotaBridgeEvent
        impl IotaBridgeEvent {
            pub fn try_from_iota_event(event: &IotaEvent) -> BridgeResult<Option<IotaBridgeEvent>> {
                init_all_struct_tags(); // Ensure all tags are initialized

                // Unwrap safe: we inited above
                $(
                    if &event.type_ == $variant.get().unwrap() {
                        let event_struct: $event_struct = bcs::from_bytes(event.bcs.bytes()).map_err(|e| BridgeError::Internal(format!("Failed to deserialize event to {}: {:?}", stringify!($event_struct), e)))?;
                        return Ok(Some(IotaBridgeEvent::$variant(event_struct.try_into()?)));
                    }
                )*
                Ok(None)
            }
        }
    };
}

impl IotaBridgeEvent {
    pub fn try_into_bridge_action(
        self,
        iota_tx_digest: TransactionDigest,
        iota_tx_event_index: u16,
    ) -> Option<BridgeAction> {
        match self {
            IotaBridgeEvent::IotaToEthTokenBridgeV1(event) => {
                Some(BridgeAction::IotaToEthBridgeAction(IotaToEthBridgeAction {
                    iota_tx_digest,
                    iota_tx_event_index,
                    iota_bridge_event: event.clone(),
                }))
            }
            IotaBridgeEvent::TokenTransferApproved(_event) => None,
            IotaBridgeEvent::TokenTransferClaimed(_event) => None,
            IotaBridgeEvent::TokenTransferAlreadyApproved(_event) => None,
            IotaBridgeEvent::TokenTransferAlreadyClaimed(_event) => None,
            IotaBridgeEvent::TokenTransferLimitExceed(_event) => None,
            IotaBridgeEvent::EmergencyOpEvent(_event) => None,
            IotaBridgeEvent::CommitteeMemberRegistration(_event) => None,
            IotaBridgeEvent::CommitteeUpdateEvent(_event) => None,
            IotaBridgeEvent::CommitteeMemberUrlUpdateEvent(_event) => None,
            IotaBridgeEvent::BlocklistValidatorEvent(_event) => None,
            IotaBridgeEvent::TokenRegistrationEvent(_event) => None,
            IotaBridgeEvent::NewTokenEvent(_event) => None,
            IotaBridgeEvent::UpdateTokenPriceEvent(_event) => None,
        }
    }
}

#[cfg(test)]
pub mod tests {
    use std::collections::HashSet;

    use ethers::types::Address as EthAddress;
    use iota_json_rpc_types::{BcsEvent, IotaEvent};
    use iota_types::{
        Identifier,
        base_types::{IotaAddress, ObjectID},
        bridge::{BridgeChainId, TOKEN_ID_IOTA},
        crypto::get_key_pair,
        digests::TransactionDigest,
        event::EventID,
    };

    use super::*;
    use crate::{
        crypto::BridgeAuthorityKeyPair,
        e2e_tests::test_utils::BridgeTestClusterBuilder,
        types::{BridgeAction, IotaToEthBridgeAction},
    };

    /// Returns a test IotaEvent and corresponding BridgeAction
    pub fn get_test_iota_event_and_action(identifier: Identifier) -> (IotaEvent, BridgeAction) {
        init_all_struct_tags(); // Ensure all tags are initialized
        let sanitized_event = EmittedIotaToEthTokenBridgeV1 {
            nonce: 1,
            iota_chain_id: BridgeChainId::IotaTestnet,
            iota_address: IotaAddress::random_for_testing_only(),
            eth_chain_id: BridgeChainId::EthSepolia,
            eth_address: EthAddress::random(),
            token_id: TOKEN_ID_IOTA,
            amount_iota_adjusted: 100,
        };
        let emitted_event = MoveTokenDepositedEvent {
            seq_num: sanitized_event.nonce,
            source_chain: sanitized_event.iota_chain_id as u8,
            sender_address: sanitized_event.iota_address.to_vec(),
            target_chain: sanitized_event.eth_chain_id as u8,
            target_address: sanitized_event.eth_address.as_bytes().to_vec(),
            token_type: sanitized_event.token_id,
            amount_iota_adjusted: sanitized_event.amount_iota_adjusted,
        };

        let tx_digest = TransactionDigest::random();
        let event_idx = 10u16;
        let bridge_action = BridgeAction::IotaToEthBridgeAction(IotaToEthBridgeAction {
            iota_tx_digest: tx_digest,
            iota_tx_event_index: event_idx,
            iota_bridge_event: sanitized_event.clone(),
        });
        let event = IotaEvent {
            type_: IotaToEthTokenBridgeV1.get().unwrap().clone(),
            bcs: BcsEvent::new(bcs::to_bytes(&emitted_event).unwrap()),
            id: EventID {
                tx_digest,
                event_seq: event_idx as u64,
            },

            // The following fields do not matter as of writing,
            // but if tests start to fail, it's worth checking these fields.
            package_id: ObjectID::ZERO,
            transaction_module: identifier.clone(),
            sender: IotaAddress::random_for_testing_only(),
            parsed_json: serde_json::json!({"test": "test"}),
            timestamp_ms: None,
        };
        (event, bridge_action)
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 8)]
    #[ignore = "https://github.com/iotaledger/iota/issues/3224"]
    async fn test_bridge_events_when_init() {
        telemetry_subscribers::init_for_testing();
        init_all_struct_tags();
        let mut bridge_test_cluster = BridgeTestClusterBuilder::new()
            .with_eth_env(false)
            .with_bridge_cluster(false)
            .with_num_validators(2)
            .build()
            .await;

        let events = bridge_test_cluster
            .new_bridge_events(
                HashSet::from_iter([
                    CommitteeMemberRegistration.get().unwrap().clone(),
                    CommitteeUpdateEvent.get().unwrap().clone(),
                    TokenRegistrationEvent.get().unwrap().clone(),
                    NewTokenEvent.get().unwrap().clone(),
                ]),
                false,
            )
            .await;
        let mut mask = 0u8;
        for event in events.iter() {
            match IotaBridgeEvent::try_from_iota_event(event)
                .unwrap()
                .unwrap()
            {
                IotaBridgeEvent::CommitteeMemberRegistration(_event) => mask |= 0x1,
                IotaBridgeEvent::CommitteeUpdateEvent(_event) => mask |= 0x2,
                IotaBridgeEvent::TokenRegistrationEvent(_event) => mask |= 0x4,
                IotaBridgeEvent::NewTokenEvent(_event) => mask |= 0x8,
                _ => panic!("Got unexpected event: {:?}", event),
            }
        }
        // assert all the above events are emitted
        assert_eq!(mask, 0xF);

        // TODO: trigger other events and make sure they are converted correctly
    }

    #[test]
    #[ignore = "https://github.com/iotaledger/iota/issues/3224"]
    fn test_conversion_for_committee_member_url_update_event() {
        let (_, kp): (_, BridgeAuthorityKeyPair) = get_key_pair();
        let new_url = "https://example.com:443";
        let event: CommitteeMemberUrlUpdateEvent = MoveCommitteeMemberUrlUpdateEvent {
            member: kp.public.as_bytes().to_vec(),
            new_url: new_url.as_bytes().to_vec(),
        }
        .try_into()
        .unwrap();
        assert_eq!(event.member, kp.public);
        assert_eq!(event.new_url, new_url);

        CommitteeMemberUrlUpdateEvent::try_from(MoveCommitteeMemberUrlUpdateEvent {
            member: vec![1, 2, 3],
            new_url: new_url.as_bytes().to_vec(),
        })
        .unwrap_err();

        CommitteeMemberUrlUpdateEvent::try_from(MoveCommitteeMemberUrlUpdateEvent {
            member: kp.public.as_bytes().to_vec(),
            new_url: [240, 130, 130, 172].into(),
        })
        .unwrap_err();
    }

    // TODO: add conversion tests for other events

    #[test]
    #[ignore = "https://github.com/iotaledger/iota/issues/3224"]
    fn test_0_iota_amount_conversion_for_iota_event() {
        let emitted_event = MoveTokenDepositedEvent {
            seq_num: 1,
            source_chain: BridgeChainId::IotaTestnet as u8,
            sender_address: IotaAddress::random_for_testing_only().to_vec(),
            target_chain: BridgeChainId::EthSepolia as u8,
            target_address: EthAddress::random().as_bytes().to_vec(),
            token_type: TOKEN_ID_IOTA,
            amount_iota_adjusted: 0,
        };
        match EmittedIotaToEthTokenBridgeV1::try_from(emitted_event).unwrap_err() {
            BridgeError::ZeroValueBridgeTransfer(_) => (),
            other => panic!("Expected Generic error, got: {:?}", other),
        }
    }
}
