// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! TD-side port of [`crate::fullnode_reconfig_observer::FullNodeReconfigObserver`].
//!
//! Same poll-the-fullnode-every-3s logic; just adapted to the TD trait that
//! drives an `AuthorityAggregatorUpdatable` rather than a `QuorumDriver`.
use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use iota_core::{
    authority_aggregator::{AuthAggMetrics, AuthorityAggregator},
    authority_client::NetworkAuthorityClient,
    epoch::committee_store::CommitteeStore,
    safe_client::SafeClientMetricsBase,
    transaction_driver::{AuthorityAggregatorUpdatable, reconfig_observer::ReconfigObserver},
};
use iota_sdk::{IotaClient, IotaClientBuilder};
use iota_types::iota_system_state::iota_system_state_summary::IotaSystemStateSummary;
use tracing::{debug, error, trace};

#[derive(Clone)]
pub struct TdFullNodeReconfigObserver {
    pub fullnode_client: IotaClient,
    committee_store: Arc<CommitteeStore>,
    safe_client_metrics_base: SafeClientMetricsBase,
    auth_agg_metrics: Arc<AuthAggMetrics>,
}

impl TdFullNodeReconfigObserver {
    pub async fn new(
        fullnode_rpc_url: &str,
        committee_store: Arc<CommitteeStore>,
        safe_client_metrics_base: SafeClientMetricsBase,
        auth_agg_metrics: Arc<AuthAggMetrics>,
    ) -> Self {
        Self {
            fullnode_client: IotaClientBuilder::default()
                .build(fullnode_rpc_url)
                .await
                .unwrap_or_else(|e| {
                    panic!("Can't create IotaClient with rpc url {fullnode_rpc_url}: {e:?}")
                }),
            committee_store,
            safe_client_metrics_base,
            auth_agg_metrics,
        }
    }
}

#[async_trait]
impl ReconfigObserver<NetworkAuthorityClient> for TdFullNodeReconfigObserver {
    fn clone_boxed(&self) -> Box<dyn ReconfigObserver<NetworkAuthorityClient> + Send + Sync> {
        Box::new(self.clone())
    }

    async fn run(
        &mut self,
        epoch_updatable: Arc<dyn AuthorityAggregatorUpdatable<NetworkAuthorityClient>>,
    ) {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
            match self
                .fullnode_client
                .governance_api()
                .get_latest_iota_system_state()
                .await
            {
                Ok(iota_system_state) => {
                    let epoch_id = match &iota_system_state {
                        IotaSystemStateSummary::V1(v1) => v1.epoch,
                        IotaSystemStateSummary::V2(v2) => v2.epoch,
                        _ => panic!("unsupported IotaSystemStateSummary"),
                    };

                    if epoch_id > epoch_updatable.epoch() {
                        debug!(epoch_id, "Got IotaSystemState in newer epoch");
                        let new_committee = match iota_system_state {
                            IotaSystemStateSummary::V1(v1) => {
                                v1.get_iota_committee_for_benchmarking()
                            }
                            IotaSystemStateSummary::V2(v2) => {
                                v2.get_iota_committee_for_benchmarking()
                            }
                            _ => panic!("unsupported IotaSystemStateSummary"),
                        };
                        let _ = self
                            .committee_store
                            .insert_new_committee(new_committee.committee());
                        let auth_agg = AuthorityAggregator::new_from_committee(
                            new_committee,
                            &self.committee_store,
                            self.safe_client_metrics_base.clone(),
                            self.auth_agg_metrics.clone(),
                            Arc::new(HashMap::new()),
                        );
                        epoch_updatable.update_authority_aggregator(Arc::new(auth_agg));
                    } else {
                        trace!(
                            epoch_id,
                            "Ignored SystemState from a previous or current epoch",
                        );
                    }
                }
                Err(err) => error!("Can't get IotaSystemState from Full Node: {:?}", err,),
            }
        }
    }
}
