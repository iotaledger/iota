// Copyright 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! A builder to make constructing transfer object calls simpler.

use iota_types::base_types::{ObjectID, IotaAddress};

use crate::{client::response::TransactionResponse, types::GasCoin, Client, ClientError};

/// A builder for a transfer object call.
#[derive(Debug)]
pub struct TransferObjectBuilder {
    object: ObjectID,
    recipient: IotaAddress,
    gas_budget: Option<u64>,
    gas: Option<GasCoin>,
    wait_for_finalization: bool,
    client: Client,
}

impl TransferObjectBuilder {
    /// Instantiate a transfer object builder with a client.
    pub fn new(client: &Client, object: ObjectID, recipient: IotaAddress) -> Self {
        Self {
            object,
            recipient,
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

    /// Execute the transfer call with the given data.
    pub async fn execute(self) -> Result<TransactionResponse, ClientError> {
        self.client
            .data_mut()
            .await
            .transfer_object(
                self.object,
                self.recipient,
                self.gas_budget,
                self.gas,
                self.wait_for_finalization,
            )
            .await
    }
}
