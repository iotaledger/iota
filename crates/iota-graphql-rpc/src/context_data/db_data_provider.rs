// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::time::Duration;

use iota_indexer::{
    apis::GovernanceReadApi, db::ConnectionPoolConfig, indexer_reader::IndexerReader,
};
use iota_json_rpc_types::Stake as RpcStakedIota;
use iota_types::{
    governance::StakedIota as NativeStakedIota,
    iota_system_state::iota_system_state_summary::IotaSystemStateSummary as NativeIotaSystemStateSummary,
};

use crate::{error::Error, types::system_state_summary::SystemStateSummaryView};

pub(crate) struct PgManager {
    pub inner: IndexerReader,
}

impl PgManager {
    pub(crate) fn new(inner: IndexerReader) -> Self {
        Self { inner }
    }

    /// Create a new underlying reader, which is used by this type as well as
    /// other data providers.
    pub(crate) fn reader_with_config(
        db_url: impl Into<String>,
        pool_size: u32,
        timeout_ms: u64,
    ) -> Result<IndexerReader, Error> {
        let mut config = ConnectionPoolConfig::default();
        config.set_pool_size(pool_size);
        config.set_statement_timeout(Duration::from_millis(timeout_ms));
        IndexerReader::new_with_config(db_url, config)
            .map_err(|e| Error::Internal(format!("Failed to create reader: {e}")))
    }
}

/// Implement methods to be used by graphql resolvers
impl PgManager {
    /// If no epoch was requested or if the epoch requested is in progress,
    /// returns the latest iota system state.
    pub(crate) async fn fetch_iota_system_state(
        &self,
        epoch_id: Option<u64>,
    ) -> Result<NativeIotaSystemStateSummary, Error> {
        let latest_iota_system_state = self
            .inner
            .spawn_blocking(move |this| this.get_latest_iota_system_state())
            .await?;

        if epoch_id.is_none() || epoch_id.is_some_and(|id| id == latest_iota_system_state.epoch()) {
            Ok(latest_iota_system_state)
        } else {
            Ok(self
                .inner
                .spawn_blocking(move |this| this.get_epoch_iota_system_state(epoch_id))
                .await?)
        }
    }

    /// Make a request to the RPC for its representations of the staked iota we
    /// parsed out of the object.  Used to implement fields that are
    /// implemented in JSON-RPC but not GraphQL (yet).
    pub(crate) async fn fetch_rpc_staked_iota(
        &self,
        stake: NativeStakedIota,
    ) -> Result<RpcStakedIota, Error> {
        let governance_api = GovernanceReadApi::new(self.inner.clone());

        let mut delegated_stakes = governance_api
            .get_delegated_stakes(vec![stake])
            .await
            .map_err(|e| Error::Internal(format!("Error fetching delegated stake. {e}")))?;

        let Some(mut delegated_stake) = delegated_stakes.pop() else {
            return Err(Error::Internal(
                "Error fetching delegated stake. No pools returned.".to_string(),
            ));
        };

        let Some(stake) = delegated_stake.stakes.pop() else {
            return Err(Error::Internal(
                "Error fetching delegated stake. No stake in pool.".to_string(),
            ));
        };

        Ok(stake)
    }
}
