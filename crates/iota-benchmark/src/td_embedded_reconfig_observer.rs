// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! TD-side port of [`crate::embedded_reconfig_observer::EmbeddedReconfigObserver`].
//!
//! TransactionDriver uses its own `ReconfigObserver` trait
//! (`iota_core::transaction_driver::reconfig_observer::ReconfigObserver`) which
//! drives an `AuthorityAggregatorUpdatable` instead of a `QuorumDriver`. The
//! shipping observers (`OnsiteReconfigObserver`, `DummyReconfigObserver`) are
//! either validator-internal or no-op. For the stress client we need polling
//! parity with the QD-side observer, so this is a direct port — same 5s poll,
//! same `get_committee` priming helper, same logic.
use std::sync::Arc;

use anyhow::anyhow;
use async_trait::async_trait;
use iota_core::{
    authority_aggregator::AuthorityAggregator,
    authority_client::NetworkAuthorityClient,
    transaction_driver::{AuthorityAggregatorUpdatable, reconfig_observer::ReconfigObserver},
};
use iota_network::default_iota_network_config;
use iota_types::iota_system_state::IotaSystemStateTrait;
use tracing::{error, info, trace};

#[derive(Clone, Default)]
pub struct TdEmbeddedReconfigObserver {}

impl TdEmbeddedReconfigObserver {
    pub fn new() -> Self {
        Self {}
    }

    /// Prime the aggregator before starting the observer. Mirrors
    /// `EmbeddedReconfigObserver::get_committee` so call sites stay
    /// symmetrical with the QD path.
    pub async fn get_committee(
        &self,
        auth_agg: Arc<AuthorityAggregator<NetworkAuthorityClient>>,
    ) -> anyhow::Result<Arc<AuthorityAggregator<NetworkAuthorityClient>>> {
        let cur_epoch = auth_agg.committee.epoch();
        match auth_agg
            .get_latest_system_state_object_for_testing()
            .await
            .map(|state| state.get_current_epoch_committee())
        {
            Err(err) => Err(err),
            Ok(committee_info) => {
                let network_config = default_iota_network_config();
                let new_epoch = committee_info.epoch();
                if new_epoch <= cur_epoch {
                    trace!(
                        cur_epoch,
                        new_epoch, "Ignored Committee from a previous or current epoch",
                    );
                    return Ok(auth_agg);
                }
                info!(
                    cur_epoch,
                    new_epoch, "Observed a new epoch, attempting to reconfig: {committee_info}"
                );
                auth_agg
                    .recreate_with_net_addresses(committee_info, &network_config, false)
                    .map(Arc::new)
                    .map_err(|se| anyhow!("Failed to recreate due to: {:?}", se.to_string()))
            }
        }
    }
}

#[async_trait]
impl ReconfigObserver<NetworkAuthorityClient> for TdEmbeddedReconfigObserver {
    fn clone_boxed(&self) -> Box<dyn ReconfigObserver<NetworkAuthorityClient> + Send + Sync> {
        Box::new(self.clone())
    }

    async fn run(
        &mut self,
        epoch_updatable: Arc<dyn AuthorityAggregatorUpdatable<NetworkAuthorityClient>>,
    ) {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            let auth_agg = epoch_updatable.authority_aggregator();
            match self.get_committee(auth_agg.clone()).await {
                Ok(new_auth_agg) => epoch_updatable.update_authority_aggregator(new_auth_agg),
                Err(err) => {
                    error!(
                        "Failed to recreate authority aggregator with committee: {}",
                        err
                    );
                    continue;
                }
            }
        }
    }
}
