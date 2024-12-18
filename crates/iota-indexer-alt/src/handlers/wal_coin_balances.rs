// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use anyhow::Result;
use diesel::{ExpressionMethods, QueryDsl};
use diesel_async::RunQueryDsl;
use iota_indexer_alt_framework::{
    db,
    pipeline::{Processor, concurrent::Handler},
};
use iota_types::full_checkpoint_content::CheckpointData;

use super::sum_coin_balances::SumCoinBalances;
use crate::{
    models::objects::{StoredObjectUpdate, StoredSumCoinBalance, StoredWalCoinBalance},
    schema::wal_coin_balances,
};

pub(crate) struct WalCoinBalances;

impl Processor for WalCoinBalances {
    const NAME: &'static str = "wal_coin_balances";

    type Value = StoredObjectUpdate<StoredSumCoinBalance>;

    fn process(&self, checkpoint: &Arc<CheckpointData>) -> Result<Vec<Self::Value>> {
        SumCoinBalances.process(checkpoint)
    }
}

#[async_trait::async_trait]
impl Handler for WalCoinBalances {
    const MIN_EAGER_ROWS: usize = 100;
    const MAX_PENDING_ROWS: usize = 10000;

    async fn commit(values: &[Self::Value], conn: &mut db::Connection<'_>) -> Result<usize> {
        let values: Vec<_> = values
            .iter()
            .map(|value| StoredWalCoinBalance {
                object_id: value.object_id.to_vec(),
                object_version: value.object_version as i64,

                owner_id: value.update.as_ref().map(|o| o.owner_id.clone()),

                coin_type: value.update.as_ref().map(|o| o.coin_type.clone()),
                coin_balance: value.update.as_ref().map(|o| o.coin_balance),

                cp_sequence_number: value.cp_sequence_number as i64,

                coin_owner_kind: value.update.as_ref().map(|o| o.coin_owner_kind),
            })
            .collect();

        Ok(diesel::insert_into(wal_coin_balances::table)
            .values(&values)
            .on_conflict_do_nothing()
            .execute(conn)
            .await?)
    }

    async fn prune(from: u64, to: u64, conn: &mut db::Connection<'_>) -> Result<usize> {
        let filter = wal_coin_balances::table
            .filter(wal_coin_balances::cp_sequence_number.between(from as i64, to as i64 - 1));

        Ok(diesel::delete(filter).execute(conn).await?)
    }
}
