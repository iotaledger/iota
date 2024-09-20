// Copyright 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! A builder to make constructing merge coin calls simpler.

use iota_types::base_types::ObjectID;

use crate::{client::response::TransactionResponse, types::GasCoin, Client, ClientError};

/// A builder for a merge coin call.
#[derive(Debug)]
pub struct MergeCoinBuilder {
    primary_coin: ObjectID,
    consumed_coin: ObjectID,
    gas_budget: Option<u64>,
    gas: Option<GasCoin>,
    wait_for_finalization: bool,
    client: Client,
}

impl MergeCoinBuilder {
    /// Instantiate a merge coin builder with a client.
    pub fn new(client: &Client, primary_coin: ObjectID, consumed_coin: ObjectID) -> Self {
        Self {
            primary_coin,
            consumed_coin,
            gas_budget: None,
            gas: None,
            wait_for_finalization: true,
            client: client.clone(),
        }
    }

    /// Set the gas budget. Optional.
    pub fn gas_budget(mut self, gas_budget: u64) -> Self {
        self.gas_budget = Some(gas_budget);
        self
    }

    /// Set the gas coins that will be consumed. Optional.
    pub fn gas(mut self, gas: GasCoin) -> Self {
        self.gas = Some(gas);
        self
    }

    /// Set the flag that determines whether the execution will wait for
    /// finalization of this transaction. Default: true
    pub fn wait_for_finalization(mut self, wait_for_finalization: bool) -> Self {
        self.wait_for_finalization = wait_for_finalization;
        self
    }

    /// Execute the merge call with the given data.
    pub async fn execute(self) -> Result<TransactionResponse, ClientError> {
        self.client
            .data_mut()
            .await
            .merge_coins(
                self.primary_coin,
                self.consumed_coin,
                self.gas_budget,
                self.gas,
                self.wait_for_finalization,
            )
            .await
    }
}
