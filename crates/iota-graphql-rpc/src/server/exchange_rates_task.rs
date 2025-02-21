// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_indexer::apis::GovernanceReadApi;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use crate::data::pg::PgExecutor;

/// Background task for kicking on epoch change the exchange rates function on
/// the indexer, which caches the ValidatorExchangeRates that are needed for
/// computing APYs.
pub(crate) struct TriggerExchangeRatesTask {
    cancel: CancellationToken,
    db: PgExecutor,
    epoch_rx: watch::Receiver<u64>,
}

impl TriggerExchangeRatesTask {
    pub(crate) fn new(
        db: PgExecutor,
        epoch_rx: watch::Receiver<u64>,
        cancel: CancellationToken,
    ) -> Self {
        Self {
            db,
            epoch_rx,
            cancel,
        }
    }

    pub(crate) async fn run(&mut self) {
        loop {
            tokio::select! {
                _ = self.cancel.cancelled() => {
                    info!("Shutdown signal received, terminating trigger exchange rates task");
                    return;
                }

                _ = self.epoch_rx.changed() => {
                    info!("Detected epoch boundary, triggering call to exchange rates");
                    let latest_iota_system_state = self.db.inner.spawn_blocking(move |this|
                        this.get_latest_iota_system_state()
                    ).await.map_err(|_| error!("Failed to fetch latest IOTA system state"));

                    if let Ok(latest_iota_system_state) = latest_iota_system_state {
                        let db = self.db.clone();
                        let governance_api = GovernanceReadApi::new(db.inner) ;
                        governance_api.exchange_rates( &latest_iota_system_state)
                            .await
                            .map_err(|e| error!("Failed to fetch exchange rates: {:?}", e))
                            .ok();
                        info!("Finished fetching exchange rates");
                    }
                }
            }
        }
    }
}
