// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use async_trait::async_trait;
use iota_sdk::wallet_context::WalletContext;
use iota_sdk_types::Owner;
use iota_test_transaction_builder::{increment_counter, publish_basics_package_and_make_counter};
use iota_types::effects::TransactionEffectsAPI;
use tracing::info;

use crate::{TestCaseImpl, TestContext, helper::ObjectChecker};

pub struct SharedCounterTest;

#[async_trait]
impl TestCaseImpl for SharedCounterTest {
    fn name(&self) -> &'static str {
        "SharedCounter"
    }

    fn description(&self) -> &'static str {
        "Test publishing basics packages and incrementing Counter (shared object)"
    }

    async fn run(&self, ctx: &mut TestContext) -> Result<(), anyhow::Error> {
        info!("Testing shared object transactions.");

        let iota_objs = ctx.get_iota_from_faucet(Some(1)).await;
        assert!(!iota_objs.is_empty());

        let wallet_context: &WalletContext = ctx.get_wallet();
        let address = ctx.get_wallet_address();
        let (package_ref, counter_ref) =
            publish_basics_package_and_make_counter(wallet_context).await;
        let response = increment_counter(
            wallet_context,
            address,
            None,
            package_ref.object_id,
            counter_ref.object_id,
            counter_ref.version,
        )
        .await;
        let effects = response.effects().unwrap().effects().unwrap();
        assert!(
            effects.status().is_success(),
            "increment counter txn failed: {:?}",
            effects.status()
        );

        effects
            .input_shared_objects()
            .iter()
            .find(|shared| shared.id_and_version().0 == counter_ref.object_id)
            .unwrap_or_else(|| panic!("expect obj {} in shared_objects", counter_ref.object_id));

        let counter_version = effects
            .mutated()
            .iter()
            .find_map(|(object_ref, owner)| {
                let initial_shared_version = owner.as_opt_shared()?;
                (object_ref.object_id == counter_ref.object_id
                    && *initial_shared_version == counter_ref.version)
                    .then_some(object_ref.version)
            })
            .unwrap_or_else(|| panic!("expect obj {} in mutated", counter_ref.object_id));

        // Verify fullnode observes the txn
        ctx.let_fullnode_sync(vec![response.transaction().unwrap().digest().unwrap()], 5)
            .await;

        let counter_object = ObjectChecker::new(counter_ref.object_id)
            .owner(Owner::Shared(counter_ref.version))
            .check_into_object(ctx.get_fullnode_client())
            .await;

        assert_eq!(
            counter_object.version, counter_version,
            "expect sequence number to be 2"
        );

        Ok(())
    }
}
