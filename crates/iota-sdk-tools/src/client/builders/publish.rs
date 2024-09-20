// Copyright 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! A builder to make publishing modules simpler.

use crate::{
    client::{
        publish_type::PublishType,
        response::{Published, TransactionResponse},
    },
    types::GasCoin,
    Client, ClientError,
};

/// A builder for a publish call.
#[derive(Debug)]
pub struct PublishBuilder {
    kind: PublishType,
    gas_budget: Option<u64>,
    gas: Option<GasCoin>,
    wait_for_finalization: bool,
    client: Client,
}

impl PublishBuilder {
    /// Instantiate a publish builder with a client.
    pub fn new(client: &Client, kind: impl Into<PublishType> + Send) -> Self {
        Self {
            kind: kind.into(),
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

    /// Execute the publish with the given data.
    pub async fn execute(self) -> Result<(Published, TransactionResponse), ClientError> {
        self.client
            .data_mut()
            .await
            .publish(
                self.kind,
                self.gas_budget,
                self.gas,
                self.wait_for_finalization,
            )
            .await
    }
}
