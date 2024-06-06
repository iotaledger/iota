// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use async_trait::async_trait;
use iota_types::{
    base_types::{TransactionDigest, TransactionEffectsDigest},
    effects::TransactionEffects,
    error::IotaResult,
};

#[async_trait]
pub trait EffectsNotifyRead: Send + Sync + 'static {
    /// This method reads executed transaction effects from database.
    /// If effects are not available immediately (i.e. haven't been executed
    /// yet), the method blocks until they are persisted in the database.
    ///
    /// This method **does not** schedule transactions for execution - it is
    /// responsibility of the caller to schedule transactions for execution
    /// before calling this method.
    async fn notify_read_executed_effects(
        &self,
        digests: Vec<TransactionDigest>,
    ) -> IotaResult<Vec<TransactionEffects>>;

    async fn notify_read_executed_effects_digests(
        &self,
        digests: Vec<TransactionDigest>,
    ) -> IotaResult<Vec<TransactionEffectsDigest>>;

    fn multi_get_executed_effects(
        &self,
        digests: &[TransactionDigest],
    ) -> IotaResult<Vec<Option<TransactionEffects>>>;
}
