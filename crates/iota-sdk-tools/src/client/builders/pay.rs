// Copyright 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! A builder to make constructing payment calls simpler.

use iota_types::base_types::{ObjectID, IotaAddress};

use crate::{client::response::TransactionResponse, types::GasCoin, Client, ClientError};

/// A builder for a payment call.
#[derive(Debug)]
pub struct PaymentBuilder {
    coins: Vec<ObjectID>,
    address_amounts: Vec<(IotaAddress, u64)>,
    gas_budget: Option<u64>,
    gas: Option<GasCoin>,
    wait_for_finalization: bool,
    client: Client,
}

impl PaymentBuilder {
    /// Instantiate a payment builder with a client.
    pub fn new(client: &Client, coins: impl IntoIterator<Item = ObjectID>) -> Self {
        Self {
            coins: coins.into_iter().collect(),
            address_amounts: Vec::new(),
            gas_budget: None,
            gas: None,
            wait_for_finalization: true,
            client: client.clone(),
        }
    }

    /// Add an address and the amount that will be sent to it.
    pub fn to(mut self, address: IotaAddress, amount: u64) -> Self {
        self.address_amounts.push((address, amount));
        self
    }

    /// Set the recipient list of addresses and amounts.
    pub fn recipients(
        mut self,
        address_amounts: impl IntoIterator<Item = (IotaAddress, u64)>,
    ) -> Self {
        self.address_amounts = address_amounts.into_iter().collect();
        self
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

    /// Execute the transfer call with the given data.
    pub async fn execute(self) -> Result<TransactionResponse, ClientError> {
        let (addresses, amounts) = self.address_amounts.into_iter().unzip();
        self.client
            .data_mut()
            .await
            .pay(
                self.coins,
                addresses,
                amounts,
                self.gas_budget,
                self.gas,
                self.wait_for_finalization,
            )
            .await
    }
}
