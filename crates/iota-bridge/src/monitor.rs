// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! `BridgeMonitor` receives all `IotaBridgeEvent` and `EthBridgeEvent`
//! and handles them accordingly.

use std::{collections::HashMap, sync::Arc};

use arc_swap::ArcSwap;
use iota_types::TypeTag;
use tokio::time::Duration;
use tracing::{error, info, warn};

use crate::{
    abi::{
        EthBridgeCommitteeEvents, EthBridgeConfigEvents, EthBridgeEvent, EthBridgeLimiterEvents,
        EthCommitteeUpgradeableContractEvents, EthIotaBridgeEvents,
    },
    client::bridge_authority_aggregator::BridgeAuthorityAggregator,
    crypto::BridgeAuthorityPublicKeyBytes,
    events::{
        BlocklistValidatorEvent, CommitteeMemberUrlUpdateEvent, EmergencyOpEvent, IotaBridgeEvent,
    },
    iota_client::{IotaClient, IotaClientInner},
    metrics::BridgeMetrics,
    retry_with_max_elapsed_time,
    types::{BridgeCommittee, IsBridgePaused},
};

const REFRESH_BRIDGE_RETRY_TIMES: u64 = 3;

pub struct BridgeMonitor<C> {
    iota_client: Arc<IotaClient<C>>,
    iota_monitor_rx: iota_metrics::metered_channel::Receiver<IotaBridgeEvent>,
    eth_monitor_rx: iota_metrics::metered_channel::Receiver<EthBridgeEvent>,
    bridge_auth_agg: Arc<ArcSwap<BridgeAuthorityAggregator>>,
    bridge_paused_watch_tx: tokio::sync::watch::Sender<IsBridgePaused>,
    iota_token_type_tags: Arc<ArcSwap<HashMap<u8, TypeTag>>>,
    bridge_metrics: Arc<BridgeMetrics>,
}

impl<C> BridgeMonitor<C>
where
    C: IotaClientInner + 'static,
{
    pub fn new(
        iota_client: Arc<IotaClient<C>>,
        iota_monitor_rx: iota_metrics::metered_channel::Receiver<IotaBridgeEvent>,
        eth_monitor_rx: iota_metrics::metered_channel::Receiver<EthBridgeEvent>,
        bridge_auth_agg: Arc<ArcSwap<BridgeAuthorityAggregator>>,
        bridge_paused_watch_tx: tokio::sync::watch::Sender<IsBridgePaused>,
        iota_token_type_tags: Arc<ArcSwap<HashMap<u8, TypeTag>>>,
        bridge_metrics: Arc<BridgeMetrics>,
    ) -> Self {
        Self {
            iota_client,
            iota_monitor_rx,
            eth_monitor_rx,
            bridge_auth_agg,
            bridge_paused_watch_tx,
            iota_token_type_tags,
            bridge_metrics,
        }
    }

    pub async fn run(self) {
        tracing::info!("Starting BridgeMonitor");
        let Self {
            iota_client,
            mut iota_monitor_rx,
            mut eth_monitor_rx,
            bridge_auth_agg,
            bridge_paused_watch_tx,
            iota_token_type_tags,
            bridge_metrics,
        } = self;
        let mut latest_token_config = (*iota_token_type_tags.load().clone()).clone();

        loop {
            tokio::select! {
                iota_event = iota_monitor_rx.recv() => {
                    if let Some(iota_event) = iota_event {
                        Self::handle_iota_events(
                            iota_event,
                            &iota_client,
                            &bridge_auth_agg,
                            &bridge_paused_watch_tx,
                            &iota_token_type_tags,
                            &bridge_metrics,
                            &mut latest_token_config,
                        )
                        .await;
                    } else {
                        panic!("BridgeMonitor iota events channel was closed unexpectedly");
                    }
                }
                eth_event = eth_monitor_rx.recv() => {
                    if let Some(eth_event) = eth_event {
                        Self::handle_eth_events(eth_event, &bridge_metrics);
                    } else {
                        panic!("BridgeMonitor eth events channel was closed unexpectedly");
                    }
                }
            }
        }
    }

    async fn handle_iota_events(
        event: IotaBridgeEvent,
        iota_client: &Arc<IotaClient<C>>,
        bridge_auth_agg: &Arc<ArcSwap<BridgeAuthorityAggregator>>,
        bridge_paused_watch_tx: &tokio::sync::watch::Sender<IsBridgePaused>,
        iota_token_type_tags: &Arc<ArcSwap<HashMap<u8, TypeTag>>>,
        bridge_metrics: &Arc<BridgeMetrics>,
        latest_token_config: &mut HashMap<u8, TypeTag>,
    ) {
        info!("Received IotaBridgeEvent: {:?}", event);
        macro_rules! bump_iota_counter {
            ($action:expr) => {
                bridge_metrics
                    .observed_governance_actions
                    .with_label_values(&[$action, "iota"])
                    .inc();
            };
        }

        match event {
            IotaBridgeEvent::IotaToEthTokenBridgeV1(_) => (),
            IotaBridgeEvent::TokenTransferApproved(_) => (),
            IotaBridgeEvent::TokenTransferClaimed(_) => (),
            IotaBridgeEvent::TokenTransferAlreadyApproved(_) => (),
            IotaBridgeEvent::TokenTransferAlreadyClaimed(_) => (),
            IotaBridgeEvent::TokenTransferLimitExceed(_) => {
                // TODO do we want to do anything here?
            }

            IotaBridgeEvent::EmergencyOpEvent(event) => {
                bump_iota_counter!(if event.frozen {
                    "bridge_paused"
                } else {
                    "bridge_unpaused"
                });
                let is_paused = get_latest_bridge_pause_status_with_emergency_event(
                    iota_client.clone(),
                    event,
                    Duration::from_secs(10),
                )
                .await;
                bridge_paused_watch_tx
                    .send(is_paused)
                    .expect("Bridge pause status watch channel should not be closed");
            }

            IotaBridgeEvent::CommitteeMemberRegistration(_) => {
                bump_iota_counter!("committee_member_registered");
            }
            IotaBridgeEvent::CommitteeUpdateEvent(_) => {
                bump_iota_counter!("committee_updated");
            }

            IotaBridgeEvent::CommitteeMemberUrlUpdateEvent(event) => {
                bump_iota_counter!("committee_member_url_updated");
                let new_committee = get_latest_bridge_committee_with_url_update_event(
                    iota_client.clone(),
                    event,
                    Duration::from_secs(10),
                )
                .await;
                let committee_names = bridge_auth_agg.load().committee_keys_to_names.clone();
                bridge_auth_agg.store(Arc::new(BridgeAuthorityAggregator::new(
                    Arc::new(new_committee),
                    bridge_metrics.clone(),
                    committee_names,
                )));
                info!("Committee updated with CommitteeMemberUrlUpdateEvent");
            }

            IotaBridgeEvent::BlocklistValidatorEvent(event) => {
                bump_iota_counter!(if event.blocklisted {
                    "validator_blocklisted"
                } else {
                    "validator_unblocklisted"
                });
                let new_committee = get_latest_bridge_committee_with_blocklist_event(
                    iota_client.clone(),
                    event,
                    Duration::from_secs(10),
                )
                .await;
                let committee_names = bridge_auth_agg.load().committee_keys_to_names.clone();
                bridge_auth_agg.store(Arc::new(BridgeAuthorityAggregator::new(
                    Arc::new(new_committee),
                    bridge_metrics.clone(),
                    committee_names,
                )));
                info!("Committee updated with BlocklistValidatorEvent");
            }

            IotaBridgeEvent::TokenRegistrationEvent(_) => {
                bump_iota_counter!("new_token_registered");
            }

            IotaBridgeEvent::NewTokenEvent(event) => {
                bump_iota_counter!("new_token_added");
                if let std::collections::hash_map::Entry::Vacant(entry) =
                    // We only add new tokens but not remove so it's ok to just insert
                    latest_token_config.entry(event.token_id)
                {
                    entry.insert(event.type_name.clone());
                    iota_token_type_tags.store(Arc::new(latest_token_config.clone()));
                } else {
                    // invariant
                    assert_eq!(event.type_name, latest_token_config[&event.token_id]);
                }
            }

            IotaBridgeEvent::UpdateTokenPriceEvent(_event) => {
                bump_iota_counter!("token_price_updated");
            }

            IotaBridgeEvent::UpdateRouteLimitEvent(_event) => {
                bump_iota_counter!("limit_updated");
            }
        }
    }

    fn handle_eth_events(event: EthBridgeEvent, bridge_metrics: &Arc<BridgeMetrics>) {
        info!("Received EthBridgeEvent: {:?}", event);

        macro_rules! bump_eth_counter {
            ($action:expr) => {
                bridge_metrics
                    .observed_governance_actions
                    .with_label_values(&[$action, "eth"])
                    .inc()
            };
        }

        match event {
            EthBridgeEvent::EthBridgeCommitteeEvents(event) => match event {
                EthBridgeCommitteeEvents::BlocklistUpdatedFilter(event) => {
                    bump_eth_counter!(if event.is_blocklisted {
                        "validator_blocklisted"
                    } else {
                        "validator_unblocklisted"
                    });
                }
                EthBridgeCommitteeEvents::InitializedFilter(_) => {
                    bump_eth_counter!("committee_contract_initialized");
                }
                EthBridgeCommitteeEvents::UpgradedFilter(_) => {
                    bump_eth_counter!("committee_contract_upgraded");
                }
                EthBridgeCommitteeEvents::BlocklistUpdatedV2Filter(e) => {
                    bump_eth_counter!(if e.is_blocklisted {
                        "validator_blocklisted"
                    } else {
                        "validator_unblocklisted"
                    });
                }
                EthBridgeCommitteeEvents::ContractUpgradedFilter(_) => {
                    bump_eth_counter!("committee_contract_upgraded");
                }
            },
            EthBridgeEvent::EthBridgeLimiterEvents(event) => match event {
                EthBridgeLimiterEvents::InitializedFilter(_) => {
                    bump_eth_counter!("limiter_contract_initialized");
                }
                EthBridgeLimiterEvents::UpgradedFilter(_) => {
                    bump_eth_counter!("limiter_contract_upgraded");
                }
                EthBridgeLimiterEvents::OwnershipTransferredFilter(_) => {
                    bump_eth_counter!("limiter_contract_ownership_transferred");
                }
                EthBridgeLimiterEvents::LimitUpdatedFilter(_) => {
                    bump_eth_counter!("limit_updated");
                }
                // This event is deprecated but we keep it for ABI compatibility
                // TODO: We can safely update abi and remove it once the testnet bridge contract is
                // upgraded
                EthBridgeLimiterEvents::HourlyTransferAmountUpdatedFilter(_) => (),
                EthBridgeLimiterEvents::ContractUpgradedFilter(_) => {
                    bump_eth_counter!("limiter_contract_upgraded");
                }
                EthBridgeLimiterEvents::LimitUpdatedV2Filter(_) => {
                    bump_eth_counter!("limit_updated");
                }
            },
            EthBridgeEvent::EthBridgeConfigEvents(event) => match event {
                EthBridgeConfigEvents::InitializedFilter(_) => {
                    bump_eth_counter!("config_contract_initialized");
                }
                EthBridgeConfigEvents::UpgradedFilter(_) => {
                    bump_eth_counter!("config_contract_upgraded");
                }
                EthBridgeConfigEvents::TokenAddedFilter(_) => {
                    bump_eth_counter!("new_token_added");
                }
                EthBridgeConfigEvents::TokenPriceUpdatedFilter(_) => {
                    bump_eth_counter!("update_token_price");
                }
                EthBridgeConfigEvents::ContractUpgradedFilter(_) => {
                    bump_eth_counter!("config_contract_upgraded");
                }
                EthBridgeConfigEvents::TokenPriceUpdatedV2Filter(_) => {
                    bump_eth_counter!("update_token_price");
                }
                EthBridgeConfigEvents::TokensAddedV2Filter(_) => {
                    bump_eth_counter!("new_token_added");
                }
            },
            EthBridgeEvent::EthCommitteeUpgradeableContractEvents(event) => match event {
                EthCommitteeUpgradeableContractEvents::InitializedFilter(_) => {
                    bump_eth_counter!("upgradeable_contract_initialized");
                }
                EthCommitteeUpgradeableContractEvents::UpgradedFilter(_) => {
                    bump_eth_counter!("upgradeable_contract_upgraded");
                }
            },
            EthBridgeEvent::EthIotaBridgeEvents(event) => match event {
                EthIotaBridgeEvents::TokensClaimedFilter(_) => (),
                EthIotaBridgeEvents::TokensDepositedFilter(_) => (),
                EthIotaBridgeEvents::PausedFilter(_) => bump_eth_counter!("bridge_paused"),
                EthIotaBridgeEvents::UnpausedFilter(_) => bump_eth_counter!("bridge_unpaused"),
                EthIotaBridgeEvents::UpgradedFilter(_) => {
                    bump_eth_counter!("bridge_contract_upgraded")
                }
                EthIotaBridgeEvents::InitializedFilter(_) => {
                    bump_eth_counter!("bridge_contract_initialized")
                }
                EthIotaBridgeEvents::ContractUpgradedFilter(_) => {
                    bump_eth_counter!("bridge_contract_upgraded")
                }
                EthIotaBridgeEvents::EmergencyOperationFilter(e) => {
                    if e.paused {
                        bump_eth_counter!("bridge_paused")
                    } else {
                        bump_eth_counter!("bridge_unpaused")
                    }
                }
            },
        }
    }
}

async fn get_latest_bridge_committee_with_url_update_event<C: IotaClientInner>(
    iota_client: Arc<IotaClient<C>>,
    event: CommitteeMemberUrlUpdateEvent,
    staleness_retry_interval: Duration,
) -> BridgeCommittee {
    let mut remaining_retry_times = REFRESH_BRIDGE_RETRY_TIMES;
    loop {
        let Ok(Ok(committee)) = retry_with_max_elapsed_time!(
            iota_client.get_bridge_committee(),
            Duration::from_secs(600)
        ) else {
            error!("Failed to get bridge committee after retry");
            continue;
        };
        let member = committee.member(&BridgeAuthorityPublicKeyBytes::from(&event.member));
        let Some(member) = member else {
            // This is possible when a node is processing an older event while the member
            // quit at a later point, which is fine. Or fullnode returns a stale
            // committee that the member hasn't joined, which is rare and tricy to handle so
            // we just log it.
            warn!(
                "Committee member not found in the committee: {:?}",
                event.member
            );
            return committee;
        };
        if member.base_url == event.new_url {
            return committee;
        }
        // If url does not match, it could be:
        // 1. the query is sent to a stale fullnode that does not have the latest data
        //    yet
        // 2. the node is processing an older message, and the latest url has changed
        //    again
        // In either case, we retry a few times. If it still fails to match, we assume
        // it's the latter case.
        tokio::time::sleep(staleness_retry_interval).await;
        remaining_retry_times -= 1;
        if remaining_retry_times == 0 {
            warn!(
                "Committee member url {:?} does not match onchain record {:?} after retry",
                event.member, member
            );
            return committee;
        }
    }
}

async fn get_latest_bridge_committee_with_blocklist_event<C: IotaClientInner>(
    iota_client: Arc<IotaClient<C>>,
    event: BlocklistValidatorEvent,
    staleness_retry_interval: Duration,
) -> BridgeCommittee {
    let mut remaining_retry_times = REFRESH_BRIDGE_RETRY_TIMES;
    loop {
        let Ok(Ok(committee)) = retry_with_max_elapsed_time!(
            iota_client.get_bridge_committee(),
            Duration::from_secs(600)
        ) else {
            error!("Failed to get bridge committee after retry");
            continue;
        };
        let mut any_mismatch = false;
        for pk in &event.public_keys {
            let member = committee.member(&BridgeAuthorityPublicKeyBytes::from(pk));
            let Some(member) = member else {
                // This is possible when a node is processing an older event while the member
                // quit at a later point. Or fullnode returns a stale committee that
                // the member hasn't joined.
                warn!("Committee member not found in the committee: {:?}", pk);
                any_mismatch = true;
                break;
            };
            if member.is_blocklisted != event.blocklisted {
                warn!(
                    "Committee member blocklist status does not match onchain record: {:?}",
                    member
                );
                any_mismatch = true;
                break;
            }
        }
        if !any_mismatch {
            return committee;
        }
        // If there is any match, it could be:
        // 1. the query is sent to a stale fullnode that does not have the latest data
        //    yet
        // 2. the node is processing an older message, and the latest blocklist status
        //    has changed again
        // In either case, we retry a few times. If it still fails to match, we assume
        // it's the latter case.
        tokio::time::sleep(staleness_retry_interval).await;
        remaining_retry_times -= 1;
        if remaining_retry_times == 0 {
            warn!(
                "Committee member blocklist status {:?} does not match onchain record after retry",
                event
            );
            return committee;
        }
    }
}

async fn get_latest_bridge_pause_status_with_emergency_event<C: IotaClientInner>(
    iota_client: Arc<IotaClient<C>>,
    event: EmergencyOpEvent,
    staleness_retry_interval: Duration,
) -> IsBridgePaused {
    let mut remaining_retry_times = REFRESH_BRIDGE_RETRY_TIMES;
    loop {
        let Ok(Ok(summary)) = retry_with_max_elapsed_time!(
            iota_client.get_bridge_summary(),
            Duration::from_secs(600)
        ) else {
            error!("Failed to get bridge summary after retry");
            continue;
        };
        if summary.is_frozen == event.frozen {
            return summary.is_frozen;
        }
        // If the onchain status does not match, it could be:
        // 1. the query is sent to a stale fullnode that does not have the latest data
        //    yet
        // 2. the node is processing an older message, and the latest status has changed
        //    again
        // In either case, we retry a few times. If it still fails to match, we assume
        // it's the latter case.
        tokio::time::sleep(staleness_retry_interval).await;
        remaining_retry_times -= 1;
        if remaining_retry_times == 0 {
            warn!(
                "Bridge pause status {:?} does not match onchain record {:?} after retry",
                event, summary.is_frozen
            );
            return summary.is_frozen;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use fastcrypto::traits::KeyPair;
    use iota_types::{
        base_types::IotaAddress,
        bridge::{BridgeCommitteeSummary, MoveTypeCommitteeMember},
        crypto::{ToFromBytes, get_key_pair},
    };
    use prometheus::Registry;

    use super::*;
    use crate::{
        events::{NewTokenEvent, init_all_struct_tags},
        iota_mock_client::IotaMockClient,
        test_utils::{bridge_committee_to_bridge_committee_summary, get_test_authority_and_key},
        types::{BRIDGE_PAUSED, BRIDGE_UNPAUSED, BridgeAuthority, BridgeCommittee},
    };

    #[tokio::test]
    async fn test_get_latest_bridge_committee_with_url_update_event() {
        telemetry_subscribers::init_for_testing();
        let iota_client_mock = IotaMockClient::default();
        let iota_client = Arc::new(IotaClient::new_for_testing(iota_client_mock.clone()));
        let (_, kp): (_, fastcrypto::secp256k1::Secp256k1KeyPair) = get_key_pair();
        let pk = kp.public().clone();
        let pk_as_bytes = BridgeAuthorityPublicKeyBytes::from(&pk);
        let pk_bytes = pk_as_bytes.as_bytes().to_vec();
        let event = CommitteeMemberUrlUpdateEvent {
            member: pk,
            new_url: "http://new.url".to_string(),
        };
        let summary = BridgeCommitteeSummary {
            members: vec![(pk_bytes.clone(), MoveTypeCommitteeMember {
                iota_address: IotaAddress::random_for_testing_only(),
                bridge_pubkey_bytes: pk_bytes.clone(),
                voting_power: 10000,
                http_rest_url: "http://new.url".to_string().as_bytes().to_vec(),
                blocklisted: false,
            })],
            member_registration: vec![],
            last_committee_update_epoch: 0,
        };

        // Test the regular case, the onchain url matches
        iota_client_mock.set_bridge_committee(summary.clone());
        let timer = std::time::Instant::now();
        let committee = get_latest_bridge_committee_with_url_update_event(
            iota_client.clone(),
            event.clone(),
            Duration::from_secs(2),
        )
        .await;
        assert_eq!(
            committee.member(&pk_as_bytes).unwrap().base_url,
            "http://new.url"
        );
        assert!(timer.elapsed().as_millis() < 500);

        // Test the case where the onchain url is older. Then update onchain url in 1
        // second. Since the retry interval is 2 seconds, it should return the
        // next retry.
        let old_summary = BridgeCommitteeSummary {
            members: vec![(pk_bytes.clone(), MoveTypeCommitteeMember {
                iota_address: IotaAddress::random_for_testing_only(),
                bridge_pubkey_bytes: pk_bytes.clone(),
                voting_power: 10000,
                http_rest_url: "http://old.url".to_string().as_bytes().to_vec(),
                blocklisted: false,
            })],
            member_registration: vec![],
            last_committee_update_epoch: 0,
        };
        iota_client_mock.set_bridge_committee(old_summary.clone());
        let timer = std::time::Instant::now();
        // update the url to "http://new.url" in 1 second
        let iota_client_mock_clone = iota_client_mock.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(1)).await;
            iota_client_mock_clone.set_bridge_committee(summary.clone());
        });
        let committee = get_latest_bridge_committee_with_url_update_event(
            iota_client.clone(),
            event.clone(),
            Duration::from_secs(2),
        )
        .await;
        assert_eq!(
            committee.member(&pk_as_bytes).unwrap().base_url,
            "http://new.url"
        );
        let elapsed = timer.elapsed().as_millis();
        assert!(elapsed > 1000 && elapsed < 3000);

        // Test the case where the onchain url is newer. It should retry up to
        // REFRESH_BRIDGE_RETRY_TIMES time then return the onchain record.
        let newer_summary = BridgeCommitteeSummary {
            members: vec![(pk_bytes.clone(), MoveTypeCommitteeMember {
                iota_address: IotaAddress::random_for_testing_only(),
                bridge_pubkey_bytes: pk_bytes.clone(),
                voting_power: 10000,
                http_rest_url: "http://newer.url".to_string().as_bytes().to_vec(),
                blocklisted: false,
            })],
            member_registration: vec![],
            last_committee_update_epoch: 0,
        };
        iota_client_mock.set_bridge_committee(newer_summary.clone());
        let timer = std::time::Instant::now();
        let committee = get_latest_bridge_committee_with_url_update_event(
            iota_client.clone(),
            event.clone(),
            Duration::from_millis(500),
        )
        .await;
        assert_eq!(
            committee.member(&pk_as_bytes).unwrap().base_url,
            "http://newer.url"
        );
        let elapsed = timer.elapsed().as_millis();
        assert!(elapsed > 500 * REFRESH_BRIDGE_RETRY_TIMES as u128);

        // Test the case where the member is not found in the committee
        // It should return the onchain record.
        let (_, kp2): (_, fastcrypto::secp256k1::Secp256k1KeyPair) = get_key_pair();
        let pk2 = kp2.public().clone();
        let pk_as_bytes2 = BridgeAuthorityPublicKeyBytes::from(&pk2);
        let pk_bytes2 = pk_as_bytes2.as_bytes().to_vec();
        let newer_summary = BridgeCommitteeSummary {
            members: vec![(pk_bytes2.clone(), MoveTypeCommitteeMember {
                iota_address: IotaAddress::random_for_testing_only(),
                bridge_pubkey_bytes: pk_bytes2.clone(),
                voting_power: 10000,
                http_rest_url: "http://newer.url".to_string().as_bytes().to_vec(),
                blocklisted: false,
            })],
            member_registration: vec![],
            last_committee_update_epoch: 0,
        };
        iota_client_mock.set_bridge_committee(newer_summary.clone());
        let timer = std::time::Instant::now();
        let committee = get_latest_bridge_committee_with_url_update_event(
            iota_client.clone(),
            event.clone(),
            Duration::from_secs(1),
        )
        .await;
        assert_eq!(
            committee.member(&pk_as_bytes2).unwrap().base_url,
            "http://newer.url"
        );
        assert!(committee.member(&pk_as_bytes).is_none());
        let elapsed = timer.elapsed().as_millis();
        assert!(elapsed < 1000);
    }

    #[tokio::test]
    async fn test_get_latest_bridge_committee_with_blocklist_event() {
        telemetry_subscribers::init_for_testing();
        let iota_client_mock = IotaMockClient::default();
        let iota_client = Arc::new(IotaClient::new_for_testing(iota_client_mock.clone()));
        let (_, kp): (_, fastcrypto::secp256k1::Secp256k1KeyPair) = get_key_pair();
        let pk = kp.public().clone();
        let pk_as_bytes = BridgeAuthorityPublicKeyBytes::from(&pk);
        let pk_bytes = pk_as_bytes.as_bytes().to_vec();

        // Test the case where the onchain status is the same as the event (blocklisted)
        let event = BlocklistValidatorEvent {
            blocklisted: true,
            public_keys: vec![pk.clone()],
        };
        let summary = BridgeCommitteeSummary {
            members: vec![(pk_bytes.clone(), MoveTypeCommitteeMember {
                iota_address: IotaAddress::random_for_testing_only(),
                bridge_pubkey_bytes: pk_bytes.clone(),
                voting_power: 10000,
                http_rest_url: "http://new.url".to_string().as_bytes().to_vec(),
                blocklisted: true,
            })],
            member_registration: vec![],
            last_committee_update_epoch: 0,
        };
        iota_client_mock.set_bridge_committee(summary.clone());
        let timer = std::time::Instant::now();
        let committee = get_latest_bridge_committee_with_blocklist_event(
            iota_client.clone(),
            event.clone(),
            Duration::from_secs(2),
        )
        .await;
        assert!(committee.member(&pk_as_bytes).unwrap().is_blocklisted);
        assert!(timer.elapsed().as_millis() < 500);

        // Test the case where the onchain status is the same as the event
        // (unblocklisted)
        let event = BlocklistValidatorEvent {
            blocklisted: false,
            public_keys: vec![pk.clone()],
        };
        let summary = BridgeCommitteeSummary {
            members: vec![(pk_bytes.clone(), MoveTypeCommitteeMember {
                iota_address: IotaAddress::random_for_testing_only(),
                bridge_pubkey_bytes: pk_bytes.clone(),
                voting_power: 10000,
                http_rest_url: "http://new.url".to_string().as_bytes().to_vec(),
                blocklisted: false,
            })],
            member_registration: vec![],
            last_committee_update_epoch: 0,
        };
        iota_client_mock.set_bridge_committee(summary.clone());
        let timer = std::time::Instant::now();
        let committee = get_latest_bridge_committee_with_blocklist_event(
            iota_client.clone(),
            event.clone(),
            Duration::from_secs(2),
        )
        .await;
        assert!(!committee.member(&pk_as_bytes).unwrap().is_blocklisted);
        assert!(timer.elapsed().as_millis() < 500);

        // Test the case where the onchain status is older. Then update onchain status
        // in 1 second. Since the retry interval is 2 seconds, it should return
        // the next retry.
        let old_summary = BridgeCommitteeSummary {
            members: vec![(pk_bytes.clone(), MoveTypeCommitteeMember {
                iota_address: IotaAddress::random_for_testing_only(),
                bridge_pubkey_bytes: pk_bytes.clone(),
                voting_power: 10000,
                http_rest_url: "http://new.url".to_string().as_bytes().to_vec(),
                blocklisted: true,
            })],
            member_registration: vec![],
            last_committee_update_epoch: 0,
        };
        iota_client_mock.set_bridge_committee(old_summary.clone());
        let timer = std::time::Instant::now();
        // update unblocklisted in 1 second
        let iota_client_mock_clone = iota_client_mock.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(1)).await;
            iota_client_mock_clone.set_bridge_committee(summary.clone());
        });
        let committee = get_latest_bridge_committee_with_blocklist_event(
            iota_client.clone(),
            event.clone(),
            Duration::from_secs(2),
        )
        .await;
        assert!(!committee.member(&pk_as_bytes).unwrap().is_blocklisted);
        let elapsed = timer.elapsed().as_millis();
        assert!(elapsed > 1000 && elapsed < 3000);

        // Test the case where the onchain url is newer. It should retry up to
        // REFRESH_BRIDGE_RETRY_TIMES time then return the onchain record.
        let newer_summary = BridgeCommitteeSummary {
            members: vec![(pk_bytes.clone(), MoveTypeCommitteeMember {
                iota_address: IotaAddress::random_for_testing_only(),
                bridge_pubkey_bytes: pk_bytes.clone(),
                voting_power: 10000,
                http_rest_url: "http://new.url".to_string().as_bytes().to_vec(),
                blocklisted: true,
            })],
            member_registration: vec![],
            last_committee_update_epoch: 0,
        };
        iota_client_mock.set_bridge_committee(newer_summary.clone());
        let timer = std::time::Instant::now();
        let committee = get_latest_bridge_committee_with_blocklist_event(
            iota_client.clone(),
            event.clone(),
            Duration::from_millis(500),
        )
        .await;
        assert!(committee.member(&pk_as_bytes).unwrap().is_blocklisted);
        let elapsed = timer.elapsed().as_millis();
        assert!(elapsed > 500 * REFRESH_BRIDGE_RETRY_TIMES as u128);

        // Test the case where the member onchain url is not found in the committee
        // It should return the onchain record after retrying a few times.
        let (_, kp2): (_, fastcrypto::secp256k1::Secp256k1KeyPair) = get_key_pair();
        let pk2 = kp2.public().clone();
        let pk_as_bytes2 = BridgeAuthorityPublicKeyBytes::from(&pk2);
        let pk_bytes2 = pk_as_bytes2.as_bytes().to_vec();
        let summary = BridgeCommitteeSummary {
            members: vec![(pk_bytes2.clone(), MoveTypeCommitteeMember {
                iota_address: IotaAddress::random_for_testing_only(),
                bridge_pubkey_bytes: pk_bytes2.clone(),
                voting_power: 10000,
                http_rest_url: "http://newer.url".to_string().as_bytes().to_vec(),
                blocklisted: false,
            })],
            member_registration: vec![],
            last_committee_update_epoch: 0,
        };
        iota_client_mock.set_bridge_committee(summary.clone());
        let timer = std::time::Instant::now();
        let committee = get_latest_bridge_committee_with_blocklist_event(
            iota_client.clone(),
            event.clone(),
            Duration::from_secs(1),
        )
        .await;
        assert_eq!(
            committee.member(&pk_as_bytes2).unwrap().base_url,
            "http://newer.url"
        );
        assert!(committee.member(&pk_as_bytes).is_none());
        let elapsed = timer.elapsed().as_millis();
        assert!(elapsed > 500 * REFRESH_BRIDGE_RETRY_TIMES as u128);

        // Test any mismtach in the blocklist status should retry a few times
        let event = BlocklistValidatorEvent {
            blocklisted: true,
            public_keys: vec![pk, pk2],
        };
        let summary = BridgeCommitteeSummary {
            members: vec![
                (pk_bytes.clone(), MoveTypeCommitteeMember {
                    iota_address: IotaAddress::random_for_testing_only(),
                    bridge_pubkey_bytes: pk_bytes.clone(),
                    voting_power: 5000,
                    http_rest_url: "http://pk.url".to_string().as_bytes().to_vec(),
                    blocklisted: true,
                }),
                (pk_bytes2.clone(), MoveTypeCommitteeMember {
                    iota_address: IotaAddress::random_for_testing_only(),
                    bridge_pubkey_bytes: pk_bytes2.clone(),
                    voting_power: 5000,
                    http_rest_url: "http://pk2.url".to_string().as_bytes().to_vec(),
                    blocklisted: false,
                }),
            ],
            member_registration: vec![],
            last_committee_update_epoch: 0,
        };
        iota_client_mock.set_bridge_committee(summary.clone());
        let timer = std::time::Instant::now();
        let committee = get_latest_bridge_committee_with_blocklist_event(
            iota_client.clone(),
            event.clone(),
            Duration::from_millis(500),
        )
        .await;
        assert!(committee.member(&pk_as_bytes).unwrap().is_blocklisted);
        assert!(!committee.member(&pk_as_bytes2).unwrap().is_blocklisted);
        let elapsed = timer.elapsed().as_millis();
        assert!(elapsed > 500 * REFRESH_BRIDGE_RETRY_TIMES as u128);
    }

    #[tokio::test]
    async fn test_get_bridge_pause_status_with_emergency_event() {
        telemetry_subscribers::init_for_testing();
        let iota_client_mock = IotaMockClient::default();
        let iota_client = Arc::new(IotaClient::new_for_testing(iota_client_mock.clone()));

        // Test event and onchain status match
        let event = EmergencyOpEvent { frozen: true };
        iota_client_mock.set_is_bridge_paused(BRIDGE_PAUSED);
        let timer = std::time::Instant::now();
        assert!(
            get_latest_bridge_pause_status_with_emergency_event(
                iota_client.clone(),
                event.clone(),
                Duration::from_secs(2),
            )
            .await
        );
        assert!(timer.elapsed().as_millis() < 500);

        let event = EmergencyOpEvent { frozen: false };
        iota_client_mock.set_is_bridge_paused(BRIDGE_UNPAUSED);
        let timer = std::time::Instant::now();
        assert!(
            !get_latest_bridge_pause_status_with_emergency_event(
                iota_client.clone(),
                event.clone(),
                Duration::from_secs(2),
            )
            .await
        );
        assert!(timer.elapsed().as_millis() < 500);

        // Test the case where the onchain status (paused) is older. Then update onchain
        // status in 1 second. Since the retry interval is 2 seconds, it should
        // return the next retry.
        iota_client_mock.set_is_bridge_paused(BRIDGE_PAUSED);
        let timer = std::time::Instant::now();
        // update the bridge to unpaused in 1 second
        let iota_client_mock_clone = iota_client_mock.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(1)).await;
            iota_client_mock_clone.set_is_bridge_paused(BRIDGE_UNPAUSED);
        });
        assert!(
            !get_latest_bridge_pause_status_with_emergency_event(
                iota_client.clone(),
                event.clone(),
                Duration::from_secs(2),
            )
            .await
        );
        let elapsed = timer.elapsed().as_millis();
        assert!(elapsed > 1000 && elapsed < 3000, "{}", elapsed);

        // Test the case where the onchain status (paused) is newer. It should retry up
        // to REFRESH_BRIDGE_RETRY_TIMES time then return the onchain record.
        iota_client_mock.set_is_bridge_paused(BRIDGE_PAUSED);
        let timer = std::time::Instant::now();
        assert!(
            get_latest_bridge_pause_status_with_emergency_event(
                iota_client.clone(),
                event.clone(),
                Duration::from_secs(2),
            )
            .await
        );
        let elapsed = timer.elapsed().as_millis();
        assert!(elapsed > 500 * REFRESH_BRIDGE_RETRY_TIMES as u128);
    }

    #[tokio::test]
    async fn test_update_bridge_authority_aggregation_with_url_change_event() {
        let (
            iota_monitor_tx,
            iota_monitor_rx,
            _eth_monitor_tx,
            eth_monitor_rx,
            iota_client_mock,
            iota_client,
            bridge_pause_tx,
            _bridge_pause_rx,
            mut authorities,
            bridge_metrics,
        ) = setup();
        let old_committee = BridgeCommittee::new(authorities.clone()).unwrap();
        let agg = Arc::new(ArcSwap::new(Arc::new(
            BridgeAuthorityAggregator::new_for_testing(Arc::new(old_committee)),
        )));
        let iota_token_type_tags = Arc::new(ArcSwap::from(Arc::new(HashMap::new())));
        let _handle = tokio::task::spawn(
            BridgeMonitor::new(
                iota_client.clone(),
                iota_monitor_rx,
                eth_monitor_rx,
                agg.clone(),
                bridge_pause_tx,
                iota_token_type_tags,
                bridge_metrics,
            )
            .run(),
        );
        let new_url = "http://new.url".to_string();
        authorities[0].base_url = new_url.clone();
        let new_committee = BridgeCommittee::new(authorities.clone()).unwrap();
        let new_committee_summary =
            bridge_committee_to_bridge_committee_summary(new_committee.clone());
        iota_client_mock.set_bridge_committee(new_committee_summary.clone());
        iota_monitor_tx
            .send(IotaBridgeEvent::CommitteeMemberUrlUpdateEvent(
                CommitteeMemberUrlUpdateEvent {
                    member: authorities[0].pubkey.clone(),
                    new_url: new_url.clone(),
                },
            ))
            .await
            .unwrap();
        // Wait for the monitor to process the event
        tokio::time::sleep(Duration::from_secs(1)).await;
        // Now expect the committee to be updated
        assert_eq!(
            agg.load()
                .committee
                .member(&BridgeAuthorityPublicKeyBytes::from(&authorities[0].pubkey))
                .unwrap()
                .base_url,
            new_url
        );
    }

    #[tokio::test]
    async fn test_update_bridge_authority_aggregation_with_blocklist_event() {
        let (
            iota_monitor_tx,
            iota_monitor_rx,
            _eth_monitor_tx,
            eth_monitor_rx,
            iota_client_mock,
            iota_client,
            bridge_pause_tx,
            _bridge_pause_rx,
            mut authorities,
            bridge_metrics,
        ) = setup();
        let old_committee = BridgeCommittee::new(authorities.clone()).unwrap();
        let agg = Arc::new(ArcSwap::new(Arc::new(
            BridgeAuthorityAggregator::new_for_testing(Arc::new(old_committee)),
        )));
        let iota_token_type_tags = Arc::new(ArcSwap::from(Arc::new(HashMap::new())));
        let _handle = tokio::task::spawn(
            BridgeMonitor::new(
                iota_client.clone(),
                iota_monitor_rx,
                eth_monitor_rx,
                agg.clone(),
                bridge_pause_tx,
                iota_token_type_tags,
                bridge_metrics,
            )
            .run(),
        );
        authorities[0].is_blocklisted = true;
        let to_blocklist = &authorities[0];
        let new_committee = BridgeCommittee::new(authorities.clone()).unwrap();
        let new_committee_summary =
            bridge_committee_to_bridge_committee_summary(new_committee.clone());
        iota_client_mock.set_bridge_committee(new_committee_summary.clone());
        iota_monitor_tx
            .send(IotaBridgeEvent::BlocklistValidatorEvent(
                BlocklistValidatorEvent {
                    public_keys: vec![to_blocklist.pubkey.clone()],
                    blocklisted: true,
                },
            ))
            .await
            .unwrap();
        // Wait for the monitor to process the event
        tokio::time::sleep(Duration::from_secs(1)).await;
        assert!(
            agg.load()
                .committee
                .member(&BridgeAuthorityPublicKeyBytes::from(&to_blocklist.pubkey))
                .unwrap()
                .is_blocklisted,
        );
    }

    #[tokio::test]
    async fn test_update_bridge_pause_status_with_emergency_event() {
        let (
            iota_monitor_tx,
            iota_monitor_rx,
            _eth_monitor_tx,
            eth_monitor_rx,
            iota_client_mock,
            iota_client,
            bridge_pause_tx,
            bridge_pause_rx,
            authorities,
            bridge_metrics,
        ) = setup();
        let event = EmergencyOpEvent {
            frozen: !*bridge_pause_tx.borrow(), // toggle the bridge pause status
        };
        let committee = BridgeCommittee::new(authorities.clone()).unwrap();
        let agg = Arc::new(ArcSwap::new(Arc::new(
            BridgeAuthorityAggregator::new_for_testing(Arc::new(committee)),
        )));
        let iota_token_type_tags = Arc::new(ArcSwap::from(Arc::new(HashMap::new())));
        let _handle = tokio::task::spawn(
            BridgeMonitor::new(
                iota_client.clone(),
                iota_monitor_rx,
                eth_monitor_rx,
                agg.clone(),
                bridge_pause_tx,
                iota_token_type_tags,
                bridge_metrics,
            )
            .run(),
        );

        iota_client_mock.set_is_bridge_paused(event.frozen);
        iota_monitor_tx
            .send(IotaBridgeEvent::EmergencyOpEvent(event.clone()))
            .await
            .unwrap();
        // Wait for the monitor to process the event
        tokio::time::sleep(Duration::from_secs(1)).await;
        // Now expect the committee to be updated
        assert!(*bridge_pause_rx.borrow() == event.frozen);
    }

    #[tokio::test]
    async fn test_update_iota_token_type_tags() {
        let (
            iota_monitor_tx,
            iota_monitor_rx,
            _eth_monitor_tx,
            eth_monitor_rx,
            _iota_client_mock,
            iota_client,
            bridge_pause_tx,
            _bridge_pause_rx,
            authorities,
            bridge_metrics,
        ) = setup();
        let event = NewTokenEvent {
            token_id: 255,
            type_name: TypeTag::from_str("0xbeef::beef::BEEF").unwrap(),
            native_token: false,
            decimal_multiplier: 1000000,
            notional_value: 100000000,
        };
        let committee = BridgeCommittee::new(authorities.clone()).unwrap();
        let agg = Arc::new(ArcSwap::new(Arc::new(
            BridgeAuthorityAggregator::new_for_testing(Arc::new(committee)),
        )));
        let iota_token_type_tags = Arc::new(ArcSwap::from(Arc::new(HashMap::new())));
        let iota_token_type_tags_clone = iota_token_type_tags.clone();
        let _handle = tokio::task::spawn(
            BridgeMonitor::new(
                iota_client.clone(),
                iota_monitor_rx,
                eth_monitor_rx,
                agg.clone(),
                bridge_pause_tx,
                iota_token_type_tags_clone,
                bridge_metrics,
            )
            .run(),
        );

        iota_monitor_tx
            .send(IotaBridgeEvent::NewTokenEvent(event.clone()))
            .await
            .unwrap();
        // Wait for the monitor to process the event
        tokio::time::sleep(Duration::from_secs(1)).await;
        // Now expect new token type tags to appear in iota_token_type_tags
        iota_token_type_tags
            .load()
            .clone()
            .get(&event.token_id)
            .unwrap();
    }

    #[allow(clippy::type_complexity)]
    fn setup() -> (
        iota_metrics::metered_channel::Sender<IotaBridgeEvent>,
        iota_metrics::metered_channel::Receiver<IotaBridgeEvent>,
        iota_metrics::metered_channel::Sender<EthBridgeEvent>,
        iota_metrics::metered_channel::Receiver<EthBridgeEvent>,
        IotaMockClient,
        Arc<IotaClient<IotaMockClient>>,
        tokio::sync::watch::Sender<IsBridgePaused>,
        tokio::sync::watch::Receiver<IsBridgePaused>,
        Vec<BridgeAuthority>,
        Arc<BridgeMetrics>,
    ) {
        telemetry_subscribers::init_for_testing();
        let registry = Registry::new();
        iota_metrics::init_metrics(&registry);
        init_all_struct_tags();
        let bridge_metrics = Arc::new(BridgeMetrics::new_for_testing());

        let iota_client_mock = IotaMockClient::default();
        let iota_client = Arc::new(IotaClient::new_for_testing(iota_client_mock.clone()));
        let (iota_monitor_tx, iota_monitor_rx) = iota_metrics::metered_channel::channel(
            10000,
            &iota_metrics::get_metrics()
                .unwrap()
                .channel_inflight
                .with_label_values(&["iota_monitor_queue"]),
        );
        let (eth_monitor_tx, eth_monitor_rx) = iota_metrics::metered_channel::channel(
            10000,
            &iota_metrics::get_metrics()
                .unwrap()
                .channel_inflight
                .with_label_values(&["eth_monitor_queue"]),
        );

        let (bridge_pause_tx, bridge_pause_rx) = tokio::sync::watch::channel(false);
        let authorities = vec![
            get_test_authority_and_key(2500, 0 /* port, dummy value */).0,
            get_test_authority_and_key(2500, 0 /* port, dummy value */).0,
            get_test_authority_and_key(2500, 0 /* port, dummy value */).0,
            get_test_authority_and_key(2500, 0 /* port, dummy value */).0,
        ];
        (
            iota_monitor_tx,
            iota_monitor_rx,
            eth_monitor_tx,
            eth_monitor_rx,
            iota_client_mock,
            iota_client,
            bridge_pause_tx,
            bridge_pause_rx,
            authorities,
            bridge_metrics,
        )
    }
}
