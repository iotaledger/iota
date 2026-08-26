// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use async_trait::async_trait;
use iota_json_rpc_types::IotaTransactionBlockResponse;
use iota_sdk_transaction_builder::unresolved;
use iota_sdk_types::{Address, ObjectId, Owner};
use tracing::info;

use crate::{
    TestCaseImpl, TestContext,
    helper::{BalanceChangeChecker, ObjectChecker},
};

pub struct NativeTransferTest;

#[async_trait]
impl TestCaseImpl for NativeTransferTest {
    fn name(&self) -> &'static str {
        "NativeTransfer"
    }

    fn description(&self) -> &'static str {
        "Test transferring IOTA coins natively"
    }

    async fn run(&self, ctx: &mut TestContext) -> Result<(), anyhow::Error> {
        info!("Testing gas coin transfer");
        let mut iota_objs = ctx.get_iota_from_faucet(Some(1)).await;
        let gas_obj = ctx.get_iota_from_faucet(Some(1)).await.swap_remove(0);

        let signer = ctx.get_wallet_address();
        let recipient_addr = Address::random();
        let grpc_client = ctx.get_fullnode_grpc_client();
        // Test transfer object
        let obj_to_transfer: ObjectId = *iota_objs.swap_remove(0).id();
        let mut builder = grpc_client.transaction_builder(signer);
        builder.transfer_objects(recipient_addr, [obj_to_transfer]);
        builder.gas([*gas_obj.id()]);
        builder.gas_budget(2_000_000);
        let data = builder.finish().await?;
        let mut response = ctx.sign_and_execute(data, "coin transfer").await;

        Self::examine_response(ctx, &mut response, signer, recipient_addr, obj_to_transfer).await;

        let mut iota_objs_2 = ctx.get_iota_from_faucet(Some(1)).await;
        // Test transfer iota: the transferred coin doubles as the gas coin, so
        // the recipient receives it whole, minus the gas fee.
        let obj_to_transfer_2 = *iota_objs_2.swap_remove(0).id();
        let mut builder = grpc_client.transaction_builder(signer);
        builder.transfer_objects(recipient_addr, [unresolved::Argument::Gas]);
        builder.gas([obj_to_transfer_2]);
        builder.gas_budget(2_000_000);
        let data = builder.finish().await?;
        let mut response = ctx.sign_and_execute(data, "coin transfer").await;

        Self::examine_response(ctx, &mut response, signer, recipient_addr, obj_to_transfer).await;
        Ok(())
    }
}

impl NativeTransferTest {
    async fn examine_response(
        ctx: &TestContext,
        response: &mut IotaTransactionBlockResponse,
        signer: Address,
        recipient: Address,
        obj_to_transfer_id: ObjectId,
    ) {
        let balance_changes = &mut response.balance_changes.as_mut().unwrap();
        // for transfer we only expect 2 balance changes, one for sender and one for
        // recipient.
        assert_eq!(
            balance_changes.len(),
            2,
            "expect 2 balance changes emitted, but got {}",
            balance_changes.len()
        );
        // Order of balance change is not fixed so need to check who's balance come
        // first. this make sure recipient always come first
        if *balance_changes[0].owner.address_or_object().unwrap() == signer {
            balance_changes.reverse()
        }
        BalanceChangeChecker::new()
            .owner(Owner::Address(recipient))
            .coin_type("0x2::iota::IOTA")
            .check(&balance_changes.remove(0));
        BalanceChangeChecker::new()
            .owner(Owner::Address(signer))
            .coin_type("0x2::iota::IOTA")
            .check(&balance_changes.remove(0));
        // Verify fullnode observes the txn
        ctx.let_fullnode_sync(vec![response.digest], 5).await;

        let _ = ObjectChecker::new(obj_to_transfer_id)
            .owner(Owner::Address(recipient))
            .check(ctx.get_fullnode_client())
            .await;
    }
}
