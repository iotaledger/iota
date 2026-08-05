// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// The protocol config override used to enable the feature flags is
// thread-local, so these tests only work under the simulator, where all nodes
// share one thread.
#[cfg(msim)]
mod simtests {
    use std::time::Duration;

    use iota_config::transaction_deny_config::TransactionDenyConfigBuilder;
    use iota_macros::sim_test;
    use iota_protocol_config::ProtocolConfig;
    use iota_swarm_config::genesis_config::{AccountConfig, DEFAULT_GAS_AMOUNT};
    use iota_test_transaction_builder::TestTransactionBuilder;
    use iota_types::crypto::get_account_key_pair;
    use test_cluster::TestClusterBuilder;

    /// Validators announce their local deny config through consensus on
    /// startup; the stake-weighted aggregate must become active on every
    /// validator, admit non-denied transactions, and reject denied senders.
    #[sim_test]
    async fn deny_rule_proposals_converge_across_validators() {
        telemetry_subscribers::init_for_testing();
        let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
            config.set_enable_pcool_flow_for_testing(true);
            config.set_deny_rule_governance_for_testing(true);
            config
        });

        // A funded account whose address every validator's local config
        // denies, and a funded non-denied account for the positive path.
        let (denied, denied_key) = get_account_key_pair();
        let deny_config = TransactionDenyConfigBuilder::new()
            .add_denied_address(denied)
            .build();

        let test_cluster = TestClusterBuilder::new()
            .with_transaction_deny_config(deny_config)
            .with_accounts(vec![
                AccountConfig {
                    address: Some(denied),
                    gas_amounts: vec![DEFAULT_GAS_AMOUNT],
                },
                AccountConfig {
                    address: None,
                    gas_amounts: vec![DEFAULT_GAS_AMOUNT; 2],
                },
            ])
            .build()
            .await;

        // Proposals are submitted on startup and aggregated at commit
        // boundaries; poll until the rule is active on every validator.
        let mut converged = false;
        for _ in 0..120 {
            converged = test_cluster
                .swarm
                .validator_node_handles()
                .iter()
                .all(|handle| {
                    handle.with(|node| {
                        node.state()
                            .epoch_store_for_testing()
                            .get_active_transaction_deny_rules()
                            .denied_addresses
                            .contains(&denied)
                    })
                });
            if converged {
                break;
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        assert!(
            converged,
            "governance deny rules did not become active on all validators"
        );

        let rgp = test_cluster.get_reference_gas_price().await;

        // A non-denied sender's transaction executes normally.
        let sender = test_cluster.get_address_0();
        let gas = test_cluster
            .wallet
            .get_one_gas_object_owned_by_address(sender)
            .await
            .unwrap()
            .expect("sender should own gas");
        let tx_data = TestTransactionBuilder::new(sender, gas, rgp)
            .transfer_iota(Some(1_000), sender)
            .build();
        test_cluster.sign_and_execute_transaction(&tx_data).await;

        // The denied sender is rejected at submission: admission checks the
        // local config and the governance rules (both deny here, since every
        // validator runs the same local config), so the transaction never
        // occupies a consensus slot. Governance propagation itself is proven
        // by the convergence poll above.
        let denied_gas = test_cluster
            .wallet
            .get_one_gas_object_owned_by_address(denied)
            .await
            .unwrap()
            .expect("denied account should own gas");
        let denied_tx = TestTransactionBuilder::new(denied, denied_gas, rgp)
            .transfer_iota(Some(1_000), sender)
            .build_and_sign(&denied_key);
        let result = test_cluster
            .wallet
            .execute_transaction_may_fail(denied_tx)
            .await;
        let error = result
            .expect_err("denied sender must be rejected")
            .to_string();
        assert!(
            error.contains("temporarily disabled"),
            "unexpected rejection reason: {error}"
        );
    }
}
