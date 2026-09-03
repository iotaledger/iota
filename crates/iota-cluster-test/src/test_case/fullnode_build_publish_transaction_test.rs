// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use async_trait::async_trait;
use iota_json_rpc_types::IotaTransactionBlockEffectsAPI;
use iota_move_build::test_utils::compile_basics_package;
use iota_sdk_types::{MovePackageData, Owner};
use iota_test_transaction_builder::select_gas_coin;

use crate::{TestCaseImpl, TestContext};

pub struct FullNodeBuildPublishTransactionTest;

#[async_trait]
impl TestCaseImpl for FullNodeBuildPublishTransactionTest {
    fn name(&self) -> &'static str {
        "FullNodeBuildPublishTransaction"
    }

    fn description(&self) -> &'static str {
        "Test building publish transaction via full node"
    }

    async fn run(&self, ctx: &mut TestContext) -> Result<(), anyhow::Error> {
        let compiled_package = compile_basics_package();
        let package_data = MovePackageData::new(
            compiled_package.get_package_bytes(/* with_unpublished_deps */ false),
            compiled_package.get_dependency_storage_package_ids(),
        );

        let sender = ctx.get_wallet_address();
        let grpc_client = ctx.get_fullnode_grpc_client();
        let mut builder = grpc_client.transaction_builder(sender);
        let upgrade_cap = builder.publish_package(package_data).result();
        builder.transfer_objects(sender, [upgrade_cap]);
        builder.gas([select_gas_coin(&grpc_client, sender).await]);
        let data = builder.finish().await?;
        let response = ctx.sign_and_execute(data, "publish basics package").await;
        response
            .effects
            .as_ref()
            .unwrap()
            .created()
            .iter()
            .find(|obj_ref| obj_ref.owner == Owner::Immutable)
            .unwrap();

        Ok(())
    }
}
