// Copyright 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! A builder to make constructing move calls simpler.

use core::marker::PhantomData;

use iota_types::base_types::ObjectID;

use crate::{
    client::response::TransactionResponse,
    types::{GasCoin, MoveParams, MoveTypes},
    Client, ClientError,
};

/// A builder for a move call.
#[derive(Debug)]
pub struct MoveCallBuilder<G: MoveTypes = (), P: MoveParams = ()> {
    package: ObjectID,
    module: String,
    function: String,
    params: Option<P>,
    generics: PhantomData<G>,
    gas_budget: Option<u64>,
    gas: Option<GasCoin>,
    wait_for_finalization: bool,
    client: Client,
}

impl<G: MoveTypes, P: MoveParams> MoveCallBuilder<G, P> {
    /// Instantiate a move call builder with a client.
    pub fn new(client: &Client, package_id: ObjectID, module: &str, function: &str) -> Self {
        Self {
            package: package_id,
            module: module.to_owned(),
            function: function.to_owned(),
            params: None,
            generics: PhantomData,
            gas_budget: None,
            gas: None,
            wait_for_finalization: true,
            client: client.clone(),
        }
    }

    /// Set the call params. Optional.
    pub fn params<U: MoveParams>(self, params: U) -> MoveCallBuilder<G, U> {
        MoveCallBuilder {
            package: self.package,
            module: self.module,
            function: self.function,
            params: Some(params),
            generics: PhantomData,
            gas_budget: self.gas_budget,
            gas: self.gas,
            wait_for_finalization: self.wait_for_finalization,
            client: self.client,
        }
    }

    /// Set the generic type arguments. Optional.
    pub fn generics<U: MoveTypes>(self) -> MoveCallBuilder<U, P> {
        MoveCallBuilder {
            package: self.package,
            module: self.module,
            function: self.function,
            params: self.params,
            generics: PhantomData,
            gas_budget: self.gas_budget,
            gas: self.gas,
            wait_for_finalization: self.wait_for_finalization,
            client: self.client,
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

    /// Execute the move call with the given data.
    pub async fn execute(self) -> Result<TransactionResponse, ClientError> {
        self.client
            .data_mut()
            .await
            .call_move(
                self.package,
                &self.module,
                &self.function,
                G::iota_type_args(self.package),
                self.params
                    .map(|p| p.iota_args())
                    .transpose()?
                    .unwrap_or_default(),
                self.gas_budget,
                self.gas,
                self.wait_for_finalization,
            )
            .await
    }
}
