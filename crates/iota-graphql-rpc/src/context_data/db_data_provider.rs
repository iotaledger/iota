// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeMap, time::Duration};

use iota_indexer::{
    apis::GovernanceReadApi, db::ConnectionPoolConfig, indexer_reader::IndexerReader,
};
use iota_json_rpc_types::Stake as RpcStakedIota;
use iota_types::{
    governance::StakedIota as NativeStakedIota,
    iota_system_state::iota_system_state_summary::IotaSystemStateSummary as NativeIotaSystemStateSummary,
};

use crate::{
    error::Error,
    types::{address::Address, iota_address::IotaAddress, validator::Validator},
};

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

        if epoch_id.is_none() || epoch_id.is_some_and(|id| id == latest_iota_system_state.epoch) {
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

/// `checkpoint_viewed_at` represents the checkpoint sequence number at which
/// the set of `IotaValidatorSummary` was queried for. Each `Validator` will
/// inherit this checkpoint, so that when viewing the `Validator`'s state, it
/// will be as if it was read at the same checkpoint.
pub(crate) fn convert_to_validators(
    system_state_at_requested_epoch: NativeIotaSystemStateSummary,
    checkpoint_viewed_at: u64,
    requested_for_epoch: u64,
) -> Vec<Validator> {
    let at_risk = BTreeMap::from_iter(system_state_at_requested_epoch.at_risk_validators);
    let reports = BTreeMap::from_iter(system_state_at_requested_epoch.validator_report_records);

    system_state_at_requested_epoch
        .active_validators
        .into_iter()
        .map(move |validator_summary| {
            let at_risk = at_risk.get(&validator_summary.iota_address).copied();
            let report_records = reports.get(&validator_summary.iota_address).map(|addrs| {
                addrs
                    .iter()
                    .cloned()
                    .map(|a| Address {
                        address: IotaAddress::from(a),
                        checkpoint_viewed_at,
                    })
                    .collect()
            });

            Validator {
                validator_summary,
                at_risk,
                report_records,
                checkpoint_viewed_at,
                requested_for_epoch,
            }
        })
        .collect()
}
