// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use iota_core::{
    authority_aggregator::{AuthAggMetrics, AuthorityAggregator},
    authority_client::NetworkAuthorityClient,
    epoch::committee_store::CommitteeStore,
    quorum_driver::{QuorumDriver, reconfig_observer::ReconfigObserver},
    safe_client::SafeClientMetricsBase,
};
use iota_sdk::{IotaClient, IotaClientBuilder};
use tracing::{debug, error, trace};

/// A ReconfigObserver that polls FullNode periodically
/// to get new epoch information.
/// Caveat: it does not guarantee to insert every committee
/// into committee store. This is fine in scenarios such
/// as stress, but may not be suitable in some other cases.
#[derive(Clone)]
pub struct FullNodeReconfigObserver {
    pub fullnode_client: IotaClient,
    committee_store: Arc<CommitteeStore>,
    safe_client_metrics_base: SafeClientMetricsBase,
    auth_agg_metrics: Arc<AuthAggMetrics>,
}

impl FullNodeReconfigObserver {
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
                    panic!(
                        "Can't create IotaClient with rpc url {fullnode_rpc_url}: {:?}",
                        e
                    )
                }),
            committee_store,
            safe_client_metrics_base,
            auth_agg_metrics,
        }
    }
}

#[async_trait]
impl ReconfigObserver<NetworkAuthorityClient> for FullNodeReconfigObserver {
    fn clone_boxed(&self) -> Box<dyn ReconfigObserver<NetworkAuthorityClient> + Send + Sync> {
        Box::new(self.clone())
    }

    async fn run(&mut self, quorum_driver: Arc<QuorumDriver<NetworkAuthorityClient>>) {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
            match self
                .fullnode_client
                .governance_api()
                .get_latest_iota_system_state()
                .await
            {
                Ok(iota_system_state) => {
                    let epoch_id = iota_system_state.epoch;
                    if epoch_id > quorum_driver.current_epoch() {
                        debug!(epoch_id, "Got IotaSystemState in newer epoch");
                        let new_committee = iota_system_state.get_iota_committee_for_benchmarking();
                        let _ = self
                            .committee_store
                            .insert_new_committee(new_committee.committee());
                        let auth_agg = AuthorityAggregator::new_from_committee(
                            iota_system_state.get_iota_committee_for_benchmarking(),
                            &self.committee_store,
                            self.safe_client_metrics_base.clone(),
                            self.auth_agg_metrics.clone(),
                            Arc::new(HashMap::new()),
                        );
                        quorum_driver.update_validators(Arc::new(auth_agg)).await
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
