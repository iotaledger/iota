// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use async_trait::async_trait;
use futures::future::try_join_all;
use iota_sdk::IotaClient;

use crate::payload::{GetReferenceGasPrice, ProcessPayload, RpcCommandProcessor, SignerInfo};

#[async_trait]
impl<'a> ProcessPayload<'a, &'a GetReferenceGasPrice> for RpcCommandProcessor {
    async fn process(
        &'a self,
        op: &'a GetReferenceGasPrice,
        _signer_info: &Option<SignerInfo>,
    ) -> Result<()> {
        let clients = self.get_clients().await?;

        let futures = (0..op.num_repeats).map(|_| {
            let clients = clients.clone();
            async move {
                let futures = clients.iter().map(get_reference_gas_price);
                try_join_all(futures).await
            }
        });

        try_join_all(futures).await.unwrap();
        Ok(())
    }
}

async fn get_reference_gas_price(client: &IotaClient) -> Result<u64> {
    let results = client
        .governance_api()
        .get_reference_gas_price()
        .await
        .unwrap();
    Ok(results)
}
