// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use async_trait::async_trait;
use futures::future::join_all;
use iota_types::{
    base_types::IotaAddress,
    crypto::{EncodeDecodeBase64, IotaKeyPair},
    quorum_driver_types::ExecuteTransactionRequestType,
    transaction::TransactionData,
};
use tracing::debug;

use crate::payload::{
    PayIota, ProcessPayload, RpcCommandProcessor, SignerInfo,
    rpc_command_processor::DEFAULT_GAS_BUDGET,
};

#[async_trait]
impl<'a> ProcessPayload<'a, &'a PayIota> for RpcCommandProcessor {
    async fn process(
        &'a self,
        _op: &'a PayIota,
        signer_info: &Option<SignerInfo>,
    ) -> anyhow::Result<()> {
        let clients = self.get_clients().await?;
        let SignerInfo {
            encoded_keypair,
            gas_budget,
            gas_payment,
        } = signer_info.clone().unwrap();
        let recipient = IotaAddress::random_for_testing_only();
        let amount = 1;
        let gas_budget = gas_budget.unwrap_or(DEFAULT_GAS_BUDGET);
        let gas_payments = gas_payment.unwrap();

        let keypair =
            IotaKeyPair::decode_base64(&encoded_keypair).expect("Decoding keypair should not fail");

        debug!(
            "Transfer IOTA {} time to {recipient} with {amount} NANOS with {gas_payments:?}",
            gas_payments.len()
        );

        let sender = IotaAddress::from(&keypair.public());
        // TODO: For write operations, we usually just want to submit the transaction to
        // fullnode Let's figure out what's the best way to support other mode
        // later
        let client = clients.first().unwrap();
        let gas_price = client
            .governance_api()
            .get_reference_gas_price()
            .await
            .expect("Unable to fetch gas price");
        join_all(gas_payments.iter().map(|gas| async {
            let tx = TransactionData::new_transfer_iota(
                recipient,
                sender,
                Some(amount),
                self.get_object_ref(client, gas).await,
                gas_budget,
                gas_price,
            );
            self.sign_and_execute(
                client,
                &keypair,
                tx,
                ExecuteTransactionRequestType::WaitForEffectsCert,
            )
            .await
        }))
        .await;

        Ok(())
    }
}
