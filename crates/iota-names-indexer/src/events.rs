// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_names::{config::IotaNamesConfig, domain::Domain, registry::NameRecord};
use iota_types::{base_types::IotaAddress, event::Event};
use serde::{Deserialize, Serialize};

pub(crate) enum IotaNamesEvent {
    IotaNamesRegistry(IotaNamesRegistryEvent),
    IotaNamesReverseRegistry(IotaNamesReverseRegistryEvent),
    AuctionStarted(AuctionStartedEvent),
    AuctionBid(AuctionBidEvent),
    AuctionExtended(AuctionExtendedEvent),
    AuctionFinalized(AuctionFinalizedEvent),
}

impl IotaNamesEvent {
    pub(crate) fn try_from_event(
        event: &Event,
        config: &IotaNamesConfig,
    ) -> anyhow::Result<Option<Self>> {
        // TODO allow more package IDs
        if event.package_id == config.package_address.into() {
            Ok(Some(match event.type_.name.as_str() {
                "IotaNamesRegistryEvent" => {
                    Self::IotaNamesRegistry(bcs::from_bytes(&event.contents)?)
                }
                "IotaNamesReverseRegistryEvent" => {
                    Self::IotaNamesReverseRegistry(bcs::from_bytes(&event.contents)?)
                }
                "AuctionStartedEvent" => Self::AuctionStarted(bcs::from_bytes(&event.contents)?),
                "BidEvent" => Self::AuctionBid(bcs::from_bytes(&event.contents)?),
                "AuctionExtendedEvent" => Self::AuctionExtended(bcs::from_bytes(&event.contents)?),
                "AuctionFinalizedEvent" => {
                    Self::AuctionFinalized(bcs::from_bytes(&event.contents)?)
                }
                _ => anyhow::bail!("Invalid event type: {}", event.type_.name),
            }))
        } else {
            Ok(None)
        }
    }
}

#[derive(Serialize, Deserialize)]
pub(crate) struct IotaNamesRegistryEvent {
    pub domain: String,
    pub name_record: NameRecord,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct IotaNamesReverseRegistryEvent {
    pub default_address: IotaAddress,
    pub domain: Domain,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct AuctionStartedEvent {
    pub domain: Domain,
    pub start_timestamp_ms: u64,
    pub end_timestamp_ms: u64,
    pub starting_bid: u64,
    pub bidder: IotaAddress,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct AuctionBidEvent {
    pub domain: Domain,
    pub bid: u64,
    pub bidder: IotaAddress,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct AuctionExtendedEvent {
    pub domain: Domain,
    pub end_timestamp_ms: u64,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct AuctionFinalizedEvent {
    pub domain: Domain,
    pub start_timestamp_ms: u64,
    pub end_timestamp_ms: u64,
    pub winning_bid: u64,
    pub winner: IotaAddress,
}
