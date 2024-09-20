// Copyright 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! A builder to make constructing split coin calls simpler.

use iota_types::base_types::ObjectID;

use crate::{client::response::TransactionResponse, types::GasCoin, Client, ClientError};

/// A builder for a split coin call.
#[derive(Debug)]
pub struct SplitCoinBuilder {
    coin: ObjectID,
    split_amounts: Vec<u64>,
    gas_budget: Option<u64>,
    gas: Option<GasCoin>,
    wait_for_finalization: bool,
    client: Client,
}

impl SplitCoinBuilder {
    /// Instantiate a split coin builder with a client.
    pub fn new(
        client: &Client,
        coin: ObjectID,
        split_amounts: impl IntoIterator<Item = u64>,
    ) -> Self {
        Self {
            coin,
            split_amounts: split_amounts.into_iter().collect(),
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

    /// Execute the split call with the given data.
    pub async fn execute(self) -> Result<TransactionResponse, ClientError> {
        self.client
            .data_mut()
            .await
            .split_coin(
                self.coin,
                self.split_amounts,
                self.gas_budget,
                self.gas,
                self.wait_for_finalization,
            )
            .await
    }
}
