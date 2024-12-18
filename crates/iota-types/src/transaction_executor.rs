// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;

use crate::{
    base_types::ObjectID,
    effects::{TransactionEffects, TransactionEvents},
    error::IotaError,
    object::Object,
    quorum_driver_types::{
        ExecuteTransactionRequestV3, ExecuteTransactionResponseV3, QuorumDriverError,
    },
    transaction::TransactionData,
};

/// Trait to define the interface for how the REST service interacts with a a
/// QuorumDriver or a simulated transaction executor.
#[async_trait::async_trait]
pub trait TransactionExecutor: Send + Sync {
    async fn execute_transaction(
        &self,
        request: ExecuteTransactionRequestV3,
        client_addr: Option<std::net::SocketAddr>,
    ) -> Result<ExecuteTransactionResponseV3, QuorumDriverError>;

    fn simulate_transaction(
        &self,
        transaction: TransactionData,
    ) -> Result<SimulateTransactionResult, IotaError>;
}

pub struct SimulateTransactionResult {
    pub effects: TransactionEffects,
    pub events: Option<TransactionEvents>,
    pub input_objects: BTreeMap<ObjectID, Object>,
    pub output_objects: BTreeMap<ObjectID, Object>,
    pub mock_gas_id: Option<ObjectID>,
}
