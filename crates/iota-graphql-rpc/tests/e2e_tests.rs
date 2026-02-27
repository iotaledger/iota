// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[cfg(feature = "pg_integration")]
mod tests {
    use std::{sync::Arc, time::Duration};

    use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
    use fastcrypto::encoding::{Base58, Base64, Encoding};
    use iota_graphql_rpc::{
        client::{ClientError, simple_client::GraphqlQueryVariable},
        config::{ConnectionConfig, Limits, ServiceConfig},
        server::builder::tests::*,
        test_infra::cluster::{DEFAULT_INTERNAL_DATA_SOURCE_PORT, ExecutorCluster},
    };
    use iota_graphql_rpc_client::{response::GraphqlResponse, simple_client::SimpleClient};
    use iota_indexer::{
        run_query_async, schema::optimistic_transactions, spawn_read_only_blocking,
    };
    use iota_types::{
        IOTA_FRAMEWORK_ADDRESS, IOTA_FRAMEWORK_PACKAGE_ID, STARDUST_ADDRESS,
        digests::{ChainIdentifier, TransactionDigest},
        gas_coin::GAS,
        transaction::{CallArg, ObjectArg, Transaction, TransactionDataAPI},
    };
    use rand::{SeedableRng, rngs::StdRng};
    use serde_json::json;
    use serial_test::serial;
    use simulacrum::Simulacrum;
    use tempfile::tempdir;
    use tokio::time::sleep;

    async fn mutation_execute_transaction(
        client: &SimpleClient,
        signed_tx: &Transaction,
        response_fields: &str,
    ) -> GraphqlResponse {
        let (tx_bytes, sigs) = signed_tx.to_tx_bytes_and_signatures();
        let tx_bytes = tx_bytes.encoded();
        let sigs = sigs.iter().map(|sig| sig.encoded()).collect::<Vec<_>>();

        let mutation = format!(
            "{{ executeTransactionBlock(txBytes: $tx, signatures: $sigs) {{ {response_fields} }} }}"
        );

        let variables = vec![
            GraphqlQueryVariable {
                name: "tx".to_string(),
                ty: "String!".to_string(),
                value: json!(tx_bytes),
            },
            GraphqlQueryVariable {
                name: "sigs".to_string(),
                ty: "[String!]!".to_string(),
                value: json!(sigs),
            },
        ];
        client
            .execute_mutation_to_graphql(mutation, variables)
            .await
            .unwrap()
    }

    async fn query_is_transaction_indexed_on_node(client: &SimpleClient, digest: &str) -> bool {
        let query = "{ isTransactionIndexedOnNode(digest: $digest) }";
        let variables = vec![GraphqlQueryVariable {
            name: "digest".to_string(),
            ty: "String!".to_string(),
            value: json!(digest),
        }];
        let resp = client
            .execute_to_graphql(query.to_string(), false, variables.clone(), vec![])
            .await
            .unwrap()
            .response_body_json();
        resp["data"]["isTransactionIndexedOnNode"]
            .as_bool()
            .unwrap()
    }

    async fn prep_executor_cluster() -> (ConnectionConfig, ExecutorCluster) {
        let rng = StdRng::from_seed([12; 32]);
        let data_ingestion_path = tempdir().unwrap().keep();
        let sim = Simulacrum::new_with_rng(rng);
        sim.set_data_ingestion_path(data_ingestion_path.clone());

        sim.create_checkpoint();
        sim.create_checkpoint();

        let connection_config = ConnectionConfig::ci_integration_test_cfg();

        let cluster = iota_graphql_rpc::test_infra::cluster::serve_executor(
            connection_config.clone(),
            DEFAULT_INTERNAL_DATA_SOURCE_PORT,
            Arc::new(sim),
            None,
            None,
            data_ingestion_path,
        )
        .await;

        cluster
            .wait_for_checkpoint_catchup(1, Duration::from_secs(10))
            .await;

        (connection_config, cluster)
    }

    #[tokio::test]
    #[serial]
    async fn test_simple_client_validator_cluster() {
        let _guard = telemetry_subscribers::TelemetryConfig::new()
            .with_env()
            .init();

        let cluster = iota_graphql_rpc::test_infra::cluster::start_cluster(
            ConnectionConfig::default(),
            None,
            ServiceConfig::test_defaults(),
        )
        .await;

        cluster
            .wait_for_checkpoint_catchup(0, Duration::from_secs(10))
            .await;

        let query = r#"
            {
                chainIdentifier
            }
        "#;
        let res = cluster
            .graphql_client
            .execute(query.to_string(), vec![])
            .await
            .unwrap();
        let chain_id_actual = cluster
            .validator_fullnode_handle
            .fullnode_handle
            .iota_client
            .read_api()
            .get_chain_identifier()
            .await
            .unwrap();

        let exp = format!("{{\"data\":{{\"chainIdentifier\":\"{chain_id_actual}\"}}}}");
        assert_eq!(&format!("{res}"), &exp);
    }

    #[tokio::test]
    #[serial]
    async fn test_simple_client_simulator_cluster() {
        let rng = StdRng::from_seed([12; 32]);
        let sim = Simulacrum::new_with_rng(rng);
        let data_ingestion_path = tempdir().unwrap().keep();
        sim.set_data_ingestion_path(data_ingestion_path.clone());

        sim.create_checkpoint();
        sim.create_checkpoint();

        let genesis_checkpoint_digest1 = *sim
            .with_store(|store| store.get_checkpoint_by_sequence_number(0).cloned().unwrap())
            .digest();

        let chain_id_actual = format!("{}", ChainIdentifier::from(genesis_checkpoint_digest1));
        let exp = format!("{{\"data\":{{\"chainIdentifier\":\"{chain_id_actual}\"}}}}");
        let cluster = iota_graphql_rpc::test_infra::cluster::serve_executor(
            ConnectionConfig::default(),
            DEFAULT_INTERNAL_DATA_SOURCE_PORT,
            Arc::new(sim),
            None,
            None,
            data_ingestion_path,
        )
        .await;
        cluster
            .wait_for_checkpoint_catchup(1, Duration::from_secs(10))
            .await;

        let query = r#"
            {
                chainIdentifier
            }
        "#;
        let res = cluster
            .graphql_client
            .execute(query.to_string(), vec![])
            .await
            .unwrap();

        assert_eq!(&format!("{res}"), &exp);
    }

    #[tokio::test]
    #[serial]
    async fn test_graphql_client_response() {
        let (_, cluster) = prep_executor_cluster().await;

        let query = r#"
            {
                chainIdentifier
            }
        "#;
        let res = cluster
            .graphql_client
            .execute_to_graphql(query.to_string(), true, vec![], vec![])
            .await
            .unwrap();

        assert_eq!(res.http_status().as_u16(), 200);
        assert_eq!(res.http_version(), reqwest::Version::HTTP_11);
        assert!(res.graphql_version().unwrap().len() >= 5);
        assert!(res.errors().is_empty());

        let usage = res.usage().unwrap().unwrap();
        assert_eq!(*usage.get("inputNodes").unwrap(), 1);
        assert_eq!(*usage.get("outputNodes").unwrap(), 1);
        assert_eq!(*usage.get("depth").unwrap(), 1);
        assert_eq!(*usage.get("variables").unwrap(), 0);
        assert_eq!(*usage.get("fragments").unwrap(), 0);
    }

    #[tokio::test]
    #[serial]
    async fn test_graphql_client_variables() {
        let (_, cluster) = prep_executor_cluster().await;

        let query = r#"{obj1: object(address: $framework_addr) {address}
            obj2: object(address: $stardust_addr) {address}}"#;
        let variables = vec![
            GraphqlQueryVariable {
                name: "framework_addr".to_string(),
                ty: "IotaAddress!".to_string(),
                value: json!("0x2"),
            },
            GraphqlQueryVariable {
                name: "stardust_addr".to_string(),
                ty: "IotaAddress!".to_string(),
                value: json!("0x107a"),
            },
        ];
        let res = cluster
            .graphql_client
            .execute_to_graphql(query.to_string(), true, variables, vec![])
            .await
            .unwrap();

        assert!(res.errors().is_empty());
        let data = res.response_body().data.clone().into_json().unwrap();
        data.get("obj1").unwrap().get("address").unwrap();
        assert_eq!(
            data.get("obj1")
                .unwrap()
                .get("address")
                .unwrap()
                .as_str()
                .unwrap(),
            IOTA_FRAMEWORK_ADDRESS.to_canonical_string(true)
        );
        assert_eq!(
            data.get("obj2")
                .unwrap()
                .get("address")
                .unwrap()
                .as_str()
                .unwrap(),
            STARDUST_ADDRESS.to_canonical_string(true)
        );

        let bad_variables = vec![
            GraphqlQueryVariable {
                name: "framework_addr".to_string(),
                ty: "IotaAddress!".to_string(),
                value: json!("0x2"),
            },
            GraphqlQueryVariable {
                name: "stardust_addr".to_string(),
                ty: "IotaAddress!".to_string(),
                value: json!("0x107a"),
            },
            GraphqlQueryVariable {
                name: "stardust_addr".to_string(),
                ty: "IotaAddress!".to_string(),
                value: json!("0x0x107aaaaaaaa"),
            },
        ];
        let res = cluster
            .graphql_client
            .execute_to_graphql(query.to_string(), true, bad_variables, vec![])
            .await;

        assert!(res.is_err());

        let bad_variables = vec![
            GraphqlQueryVariable {
                name: "framework_addr".to_string(),
                ty: "IotaAddress!".to_string(),
                value: json!("0x2"),
            },
            GraphqlQueryVariable {
                name: "stardust_addr".to_string(),
                ty: "IotaAddress!".to_string(),
                value: json!("0x107a"),
            },
            GraphqlQueryVariable {
                name: "stardust_addr".to_string(),
                ty: "IotaAddressP!".to_string(),
                value: json!("0x107a"),
            },
        ];
        let res = cluster
            .graphql_client
            .execute_to_graphql(query.to_string(), true, bad_variables, vec![])
            .await;

        assert!(res.is_err());

        let bad_variables = vec![
            GraphqlQueryVariable {
                name: "framework addr".to_string(),
                ty: "IotaAddress!".to_string(),
                value: json!("0x2"),
            },
            GraphqlQueryVariable {
                name: " stardust_addr".to_string(),
                ty: "IotaAddress!".to_string(),
                value: json!("0x107a"),
            },
            GraphqlQueryVariable {
                name: "4stardust_addr".to_string(),
                ty: "IotaAddressP!".to_string(),
                value: json!("0x107a"),
            },
            GraphqlQueryVariable {
                name: "".to_string(),
                ty: "IotaAddress!".to_string(),
                value: json!("0x107a"),
            },
            GraphqlQueryVariable {
                name: " ".to_string(),
                ty: "IotaAddress!".to_string(),
                value: json!("0x107a"),
            },
        ];

        for var in bad_variables {
            let res = cluster
                .graphql_client
                .execute_to_graphql(query.to_string(), true, vec![var.clone()], vec![])
                .await;

            assert!(res.is_err());
            assert!(
                res.unwrap_err().to_string()
                    == ClientError::InvalidVariableName {
                        var_name: var.name.clone()
                    }
                    .to_string()
            );
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_transaction_is_indexed_on_node() {
        let cluster = iota_graphql_rpc::test_infra::cluster::start_cluster(
            ConnectionConfig::default(),
            None,
            ServiceConfig::test_defaults(),
        )
        .await;

        let tx = cluster.build_transfer_iota_for_test().await;
        let signed_tx = cluster.sign_transaction(&tx);
        let response_fields = "effects { transactionBlock { digest } } errors";
        let raw_response =
            mutation_execute_transaction(&cluster.graphql_client, &signed_tx, response_fields)
                .await
                .response_body_json();
        let response = &raw_response["data"]["executeTransactionBlock"];

        let digest = response["effects"]["transactionBlock"]["digest"]
            .as_str()
            .unwrap();

        // wait for the transaction to be indexed on the node
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if !query_is_transaction_indexed_on_node(&cluster.graphql_client, digest).await {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                } else {
                    break;
                }
            }
        })
        .await
        .unwrap();
    }

    #[tokio::test]
    #[serial]
    async fn test_transaction_not_indexed_on_node() {
        let cluster = iota_graphql_rpc::test_infra::cluster::start_cluster(
            ConnectionConfig::default(),
            None,
            ServiceConfig::test_defaults(),
        )
        .await;
        let digest = TransactionDigest::generate(StdRng::from_seed([12; 32])).to_string();

        assert!(
            !query_is_transaction_indexed_on_node(&cluster.graphql_client, digest.as_str()).await
        );
    }

    #[tokio::test]
    #[serial]
    async fn test_transaction_execution() {
        let cluster = iota_graphql_rpc::test_infra::cluster::start_cluster(
            ConnectionConfig::default(),
            None,
            ServiceConfig::test_defaults(),
        )
        .await;

        let addresses = cluster.validator_fullnode_handle.wallet.get_addresses();
        let sender = addresses[0];

        let tx_block_gql_fields = r#"
            digest
            sender {
              address
            }
            effects {
              status
              errors
              objectChanges {
                edges {
                  node {
                    inputState {
                        version
                        status
                        bcs
                    }
                    outputState {
                        version
                        status
                        bcs
                    }
                    idCreated
                    idDeleted
                  }
                }
              }
              balanceChanges {
                edges {
                  node {
                    coinType {
                      repr
                    }
                    amount
                  }
                }
              }
            }
            "#;

        let response_fields =
            format!("effects {{ transactionBlock {{ {tx_block_gql_fields} }} }} errors");

        let tx = cluster.build_transfer_iota_for_test().await;
        let signed_tx = cluster.sign_transaction(&tx);
        let original_digest = signed_tx.digest();
        let raw_response =
            mutation_execute_transaction(&cluster.graphql_client, &signed_tx, &response_fields)
                .await
                .response_body_json();
        let execute_transaction_block_res = &raw_response["data"]["executeTransactionBlock"];
        let mutation_tx_data = &execute_transaction_block_res["effects"]["transactionBlock"];
        let sender_read = mutation_tx_data["sender"]["address"].as_str().unwrap();
        let digest = mutation_tx_data["digest"].as_str().unwrap();
        assert!(execute_transaction_block_res["errors"].is_null());
        assert_eq!(digest, original_digest.to_string());
        assert_eq!(sender_read, sender.to_string());

        // Query the transaction immediately after execution (optimistic indexing)
        // Use the same fields as in the mutation
        let query = format!(
            r#"
                {{
                    transactionBlock(digest: $dig){{
                        {tx_block_gql_fields}
                    }}
                }}
            "#,
        );
        let variables = vec![GraphqlQueryVariable {
            name: "dig".to_string(),
            ty: "String!".to_string(),
            value: json!(digest),
        }];

        let immediate_res = cluster
            .graphql_client
            .execute_to_graphql(query.to_string(), true, variables.clone(), vec![])
            .await
            .unwrap()
            .response_body()
            .data
            .clone()
            .into_json()
            .unwrap();
        let immediate_tx_data = &immediate_res["transactionBlock"];

        // Wait 10 seconds for transaction to be checkpointed
        sleep(Duration::from_secs(10)).await;
        let checkpointed_res = cluster
            .graphql_client
            .execute_to_graphql(query.to_string(), true, variables.clone(), vec![])
            .await
            .unwrap()
            .response_body()
            .data
            .clone()
            .into_json()
            .unwrap();
        let checkpointed_tx_data = &checkpointed_res["transactionBlock"];

        // All 3 responses should be identical: mutation, optimistic and checkpointed
        assert_eq!(mutation_tx_data, immediate_tx_data);
        assert_eq!(immediate_tx_data, checkpointed_tx_data);

        // Check that optimistic indexing happened
        let digest_bytes = Base58::decode(digest).unwrap();
        let pool = cluster.indexer_store.blocking_cp();

        let count: i64 = run_query_async!(&pool, move |conn| {
            optimistic_transactions::table
                .filter(optimistic_transactions::transaction_digest.eq(&digest_bytes))
                .count()
                .get_result(conn)
        })
        .unwrap();

        assert_eq!(
            count, 1,
            "Transaction should be present in optimistic_transactions table"
        );
    }

    #[tokio::test]
    #[serial]
    async fn test_transaction_blocks_by_digests() {
        let cluster = iota_graphql_rpc::test_infra::cluster::start_cluster(
            ConnectionConfig::default(),
            None,
            ServiceConfig::test_defaults(),
        )
        .await;
        let addresses = cluster.validator_fullnode_handle.wallet.get_addresses();
        let sender1 = addresses[0];
        let sender2 = addresses[1];
        let recipient = addresses[2];

        let tx1 = cluster
            .validator_fullnode_handle
            .test_transaction_builder_with_sender(sender1)
            .await
            .transfer_iota(Some(1_000), recipient)
            .build();
        let signed_tx1 = cluster.sign_transaction(&tx1);
        let digest1 = signed_tx1.digest();

        let tx2 = cluster
            .validator_fullnode_handle
            .test_transaction_builder_with_sender(sender2)
            .await
            .transfer_iota(Some(2_000), recipient)
            .build();
        let signed_tx2 = cluster.sign_transaction(&tx2);
        let digest2 = signed_tx2.digest();

        let response_fields = "effects { transactionBlock { digest } } errors";
        mutation_execute_transaction(&cluster.graphql_client, &signed_tx1, response_fields).await;
        mutation_execute_transaction(&cluster.graphql_client, &signed_tx2, response_fields).await;

        let fake_digest = TransactionDigest::random().to_string();
        let query = format!(
            r#"
                {{
                    transactionBlocksByDigests(digests: ["{digest1}", "{digest2}", "{fake_digest}"]){{
                        digest
                        sender {{
                            address
                        }}
                    }}
                }}
            "#,
        );

        let response_body = cluster
            .graphql_client
            .execute_to_graphql(query.to_string(), true, vec![], vec![])
            .await
            .unwrap()
            .response_body_json();
        let transactions = response_body["data"]["transactionBlocksByDigests"]
            .as_array()
            .unwrap();

        assert_eq!(
            transactions.len(),
            3,
            "3 results should be present in the response (2 real transactions and 1 null for fake digest)"
        );

        assert_eq!(
            transactions[0]["digest"].as_str().unwrap(),
            digest1.to_string(),
            "First transaction should match digest1 (preserve input order)"
        );
        assert_eq!(
            transactions[1]["digest"].as_str().unwrap(),
            digest2.to_string(),
            "Second transaction should match digest2 (preserve input order)"
        );
        assert!(
            transactions[2].is_null(),
            "Third transaction should be null for the fake digest"
        );
    }

    #[tokio::test]
    #[serial]
    #[ignore = "https://github.com/iotaledger/iota/issues/1777"]
    async fn test_zklogin_sig_verify() {
        use iota_sdk_types::crypto::{Intent, IntentMessage};
        use iota_test_transaction_builder::TestTransactionBuilder;
        use iota_types::{
            base_types::IotaAddress, crypto::Signature, signature::GenericSignature,
            utils::load_test_vectors, zk_login_authenticator::ZkLoginAuthenticator,
        };

        let _guard = telemetry_subscribers::TelemetryConfig::new()
            .with_env()
            .init();

        let cluster = iota_graphql_rpc::test_infra::cluster::start_cluster(
            ConnectionConfig::default(),
            None,
            ServiceConfig::test_defaults(),
        )
        .await;

        let test_cluster = &cluster.validator_fullnode_handle;
        test_cluster.wait_for_epoch_all_nodes(1).await;
        test_cluster.wait_for_authenticator_state_update().await;

        // Construct a valid zkLogin transaction data, signature.
        let (kp, pk_zklogin, inputs) =
            &load_test_vectors("../iota-types/src/unit_tests/zklogin_test_vectors.json").unwrap()
                [1];

        let zklogin_addr = (pk_zklogin).into();
        let rgp = test_cluster.get_reference_gas_price().await;
        let gas = test_cluster
            .fund_address_and_return_gas(rgp, Some(20000000000), zklogin_addr)
            .await;
        let tx_data = TestTransactionBuilder::new(zklogin_addr, gas, rgp)
            .transfer_iota(None, IotaAddress::ZERO)
            .build();
        let msg = IntentMessage::new(Intent::iota_transaction(), tx_data.clone());
        let eph_sig = Signature::new_secure(&msg, kp);
        let generic_sig = GenericSignature::ZkLoginAuthenticator(ZkLoginAuthenticator::new(
            inputs.clone(),
            2,
            eph_sig.clone(),
        ));

        // construct all parameters for the query
        let bytes = Base64::encode(bcs::to_bytes(&tx_data).unwrap());
        let signature = Base64::encode(generic_sig.as_ref());
        let intent_scope = "TRANSACTION_DATA";
        let author = zklogin_addr.to_string();

        // now query the endpoint with a valid tx data bytes and a valid signature with
        // the correct proof for dev env.
        let query = r#"{ verifyZkloginSignature(bytes: $bytes, signature: $signature, intentScope: $intent_scope, author: $author ) { success, errors}}"#;
        let variables = vec![
            GraphqlQueryVariable {
                name: "bytes".to_string(),
                ty: "String!".to_string(),
                value: json!(bytes),
            },
            GraphqlQueryVariable {
                name: "signature".to_string(),
                ty: "String!".to_string(),
                value: json!(signature),
            },
            GraphqlQueryVariable {
                name: "intent_scope".to_string(),
                ty: "ZkLoginIntentScope!".to_string(),
                value: json!(intent_scope),
            },
            GraphqlQueryVariable {
                name: "author".to_string(),
                ty: "IotaAddress!".to_string(),
                value: json!(author),
            },
        ];
        let res = cluster
            .graphql_client
            .execute_to_graphql(query.to_string(), true, variables, vec![])
            .await
            .unwrap();

        // a valid signature with tx bytes returns success as true.
        let binding = res.response_body().data.clone().into_json().unwrap();
        tracing::info!("tktkbinding: {:?}", binding);
        let res = binding.get("verifyZkloginSignature").unwrap();
        assert_eq!(res.get("success").unwrap(), true);

        // set up an invalid intent scope.
        let incorrect_intent_scope = "PERSONAL_MESSAGE";
        let incorrect_variables = vec![
            GraphqlQueryVariable {
                name: "bytes".to_string(),
                ty: "String!".to_string(),
                value: json!(bytes),
            },
            GraphqlQueryVariable {
                name: "signature".to_string(),
                ty: "String!".to_string(),
                value: json!(signature),
            },
            GraphqlQueryVariable {
                name: "intent_scope".to_string(),
                ty: "ZkLoginIntentScope!".to_string(),
                value: json!(incorrect_intent_scope),
            },
            GraphqlQueryVariable {
                name: "author".to_string(),
                ty: "IotaAddress!".to_string(),
                value: json!(author),
            },
        ];
        //  returns a non-empty errors list in response
        let res = cluster
            .graphql_client
            .execute_to_graphql(query.to_string(), true, incorrect_variables, vec![])
            .await
            .unwrap();
        let binding = res.response_body().data.clone().into_json().unwrap();
        let res = binding.get("verifyZkloginSignature").unwrap();
        assert_eq!(res.get("success").unwrap(), false);
    }

    // TODO: add more test cases for transaction execution/dry run in transactional
    // test runner.
    #[tokio::test]
    #[serial]
    async fn test_transaction_dry_run() {
        let _guard = telemetry_subscribers::TelemetryConfig::new()
            .with_env()
            .init();

        let cluster = iota_graphql_rpc::test_infra::cluster::start_cluster(
            ConnectionConfig::default(),
            None,
            ServiceConfig::test_defaults(),
        )
        .await;

        let tx = cluster.build_transfer_iota_for_test().await;
        let tx_bytes = Base64::encode(bcs::to_bytes(&tx).unwrap());
        let sender = tx.sender();

        let query = r#"{ dryRunTransactionBlock(txBytes: $tx) {
                transaction {
                    digest
                    indexedOnNode
                    sender {
                        address
                    }
                    gasInput {
                        gasSponsor {
                            address
                        }
                        gasPrice
                    }
                }
                error
                results {
                    mutatedReferences {
                        input {
                            __typename
                            ... on Input {
                                ix
                            }
                            ... on Result {
                                cmd
                                ix
                            }
                        }
                        type {
                            repr
                        }
                    }
                    returnValues {
                        type {
                            repr
                        }
                        bcs
                    }
                }
            }
        }"#;
        let variables = vec![GraphqlQueryVariable {
            name: "tx".to_string(),
            ty: "String!".to_string(),
            value: json!(tx_bytes),
        }];
        let res = cluster
            .graphql_client
            .execute_to_graphql(query.to_string(), true, variables, vec![])
            .await
            .unwrap();
        let binding = res.response_body().data.clone().into_json().unwrap();
        let res = binding.get("dryRunTransactionBlock").unwrap();

        let tx = res.get("transaction").unwrap();
        let digest = tx.get("digest").unwrap();
        // Dry run txn does not have digest
        assert!(digest.is_null());
        assert!(res.get("error").unwrap().is_null());
        let sender_read = tx
            .get("sender")
            .unwrap()
            .get("address")
            .unwrap()
            .as_str()
            .unwrap();
        assert_eq!(sender_read, sender.to_string());
        let indexed_on_node = tx.get("indexedOnNode").unwrap().as_bool().unwrap();
        assert!(!indexed_on_node);
        assert!(res.get("results").unwrap().is_array());
    }

    // Test dry run where the transaction kind is provided instead of the full
    // transaction.
    #[tokio::test]
    #[serial]
    async fn test_transaction_dry_run_with_kind() {
        let _guard = telemetry_subscribers::TelemetryConfig::new()
            .with_env()
            .init();

        let cluster = iota_graphql_rpc::test_infra::cluster::start_cluster(
            ConnectionConfig::default(),
            None,
            ServiceConfig::test_defaults(),
        )
        .await;

        let addresses = cluster.validator_fullnode_handle.wallet.get_addresses();

        let recipient = addresses[1];
        let tx = cluster
            .validator_fullnode_handle
            .test_transaction_builder()
            .await
            .transfer_iota(Some(1_000), recipient)
            .build();
        let tx_kind_bytes = Base64::encode(bcs::to_bytes(&tx.into_kind()).unwrap());

        let query = r#"{ dryRunTransactionBlock(txBytes: $tx, txMeta: {}) {
                results {
                    mutatedReferences {
                        input {
                            __typename
                        }
                    }
                }
                transaction {
                    digest
                    sender {
                        address
                    }
                    gasInput {
                        gasSponsor {
                            address
                        }
                        gasPrice
                    }
                }
                error
            }
        }"#;
        let variables = vec![GraphqlQueryVariable {
            name: "tx".to_string(),
            ty: "String!".to_string(),
            value: json!(tx_kind_bytes),
        }];
        let res = cluster
            .graphql_client
            .execute_to_graphql(query.to_string(), true, variables, vec![])
            .await
            .unwrap();
        let binding = res.response_body().data.clone().into_json().unwrap();
        let res = binding.get("dryRunTransactionBlock").unwrap();

        let digest = res.get("transaction").unwrap().get("digest").unwrap();
        // Dry run txn does not have digest
        assert!(digest.is_null());
        assert!(res.get("error").unwrap().is_null());
        let sender_read = res.get("transaction").unwrap().get("sender").unwrap();
        // Since no transaction metadata is provided, we use 0x0 as the sender while dry
        // running the trasanction in which case the sender is null.
        assert!(sender_read.is_null());
        assert!(res.get("results").unwrap().is_array());
    }

    // Test that we can handle dry run with failures at execution stage too.
    #[tokio::test]
    #[serial]
    async fn test_dry_run_failed_execution() {
        let _guard = telemetry_subscribers::TelemetryConfig::new()
            .with_env()
            .init();

        let cluster = iota_graphql_rpc::test_infra::cluster::start_cluster(
            ConnectionConfig::default(),
            None,
            ServiceConfig::test_defaults(),
        )
        .await;

        let addresses = cluster.validator_fullnode_handle.wallet.get_addresses();

        let sender = addresses[0];
        let coin = *cluster
            .validator_fullnode_handle
            .wallet
            .get_gas_objects_owned_by_address(sender, None)
            .await
            .unwrap()
            .get(1)
            .unwrap();
        let tx = cluster
            .validator_fullnode_handle
            .test_transaction_builder()
            .await
            // A split coin that goes nowhere -> execution failure
            .move_call(
                IOTA_FRAMEWORK_PACKAGE_ID,
                "coin",
                "split",
                vec![
                    CallArg::Object(ObjectArg::ImmOrOwnedObject(coin)),
                    CallArg::Pure(bcs::to_bytes(&1000u64).unwrap()),
                ],
            )
            .with_type_args(vec![GAS::type_tag()])
            .build();
        let tx_bytes = Base64::encode(bcs::to_bytes(&tx).unwrap());

        let query = r#"{ dryRunTransactionBlock(txBytes: $tx) {
                results {
                    mutatedReferences {
                        input {
                            __typename
                        }
                    }
                }
                transaction {
                    digest
                    sender {
                        address
                    }
                    gasInput {
                        gasSponsor {
                            address
                        }
                        gasPrice
                    }
                }
                error
            }
        }"#;
        let variables = vec![GraphqlQueryVariable {
            name: "tx".to_string(),
            ty: "String!".to_string(),
            value: json!(tx_bytes),
        }];
        let res = cluster
            .graphql_client
            .execute_to_graphql(query.to_string(), true, variables, vec![])
            .await
            .unwrap();
        let binding = res.response_body().data.clone().into_json().unwrap();
        let res = binding.get("dryRunTransactionBlock").unwrap();

        // Execution failed so the results are null.
        assert!(res.get("results").unwrap().is_null());
        // Check that the error is not null and contains the error message.
        assert!(
            res.get("error")
                .unwrap()
                .as_str()
                .unwrap()
                .contains("UnusedValueWithoutDrop")
        );
    }

    #[tokio::test]
    #[serial]
    async fn test_epoch_data() {
        let _guard = telemetry_subscribers::TelemetryConfig::new()
            .with_env()
            .init();

        let cluster = iota_graphql_rpc::test_infra::cluster::start_cluster(
            ConnectionConfig::default(),
            None,
            ServiceConfig::test_defaults(),
        )
        .await;

        cluster.validator_fullnode_handle.force_new_epoch().await;

        // Wait for the epoch to be indexed
        sleep(Duration::from_secs(10)).await;

        // Query the epoch
        let query = "
            {
                epoch(id: 0){
                    liveObjectSetDigest
                }
            }
        ";

        let res = cluster
            .graphql_client
            .execute_to_graphql(query.to_string(), true, vec![], vec![])
            .await
            .unwrap();

        let binding = res.response_body().data.clone().into_json().unwrap();

        // Check that liveObjectSetDigest is not null
        assert!(
            !binding
                .get("epoch")
                .unwrap()
                .get("liveObjectSetDigest")
                .unwrap()
                .is_null()
        );
    }

    #[tokio::test]
    #[serial]
    async fn test_timeout() {
        let cluster = iota_graphql_rpc::test_infra::cluster::start_cluster(
            ConnectionConfig::default(),
            None,
            ServiceConfig::test_defaults(),
        )
        .await;
        cluster
            .wait_for_checkpoint_catchup(0, Duration::from_secs(10))
            .await;
        test_timeout_impl(&cluster).await;
    }

    #[tokio::test]
    #[serial]
    async fn test_query_depth_limit() {
        test_query_depth_limit_impl().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_query_node_limit() {
        test_query_node_limit_impl().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_query_default_page_limit() {
        let (connection_config, _) = prep_executor_cluster().await;
        test_query_default_page_limit_impl(connection_config).await;
    }

    #[tokio::test]
    #[serial]
    async fn test_query_max_page_limit() {
        test_query_max_page_limit_impl().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_query_complexity_metrics() {
        test_query_complexity_metrics_impl().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_health_check() {
        let _guard = telemetry_subscribers::TelemetryConfig::new()
            .with_env()
            .init();
        let connection_config = ConnectionConfig::ci_integration_test_cfg();
        let cluster = iota_graphql_rpc::test_infra::cluster::start_cluster(
            connection_config,
            None,
            ServiceConfig::test_defaults(),
        )
        .await;

        cluster
            .wait_for_checkpoint_catchup(0, Duration::from_secs(10))
            .await;
        test_health_check_impl().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_payload_total_exceeded() {
        test_payload_total_exceeded_impl().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_payload_read_exceeded() {
        test_payload_read_exceeded_impl().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_payload_mutation_exceeded() {
        test_payload_mutation_exceeded_impl().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_payload_dry_run_exceeded() {
        test_payload_dry_run_exceeded_impl().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_payload_using_vars_mutation_exceeded() {
        test_payload_using_vars_mutation_exceeded_impl().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_payload_using_vars_read_exceeded() {
        test_payload_using_vars_read_exceeded_impl().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_payload_using_vars_dry_run_read_exceeded() {
        test_payload_using_vars_dry_run_read_exceeded_impl().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_payload_using_vars_dry_run_exceeded() {
        test_payload_using_vars_dry_run_exceeded_impl().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_payload_multiple_execution_exceeded() {
        test_payload_multiple_execution_exceeded_impl().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_payload_multiple_dry_run_exceeded() {
        test_payload_multiple_dry_run_exceeded_impl().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_payload_execution_multiple_sigs_exceeded() {
        test_payload_execution_multiple_sigs_exceeded_impl().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_payload_sig_var_execution_exceeded() {
        test_payload_sig_var_execution_exceeded_impl().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_payload_reusing_vars_execution() {
        test_payload_reusing_vars_execution_impl().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_payload_reusing_vars_dry_run() {
        test_payload_reusing_vars_dry_run_impl().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_payload_named_fragment_execution_exceeded() {
        test_payload_named_fragment_execution_exceeded_impl().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_payload_inline_fragment_execution_exceeded() {
        test_payload_inline_fragment_execution_exceeded_impl().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_payload_named_fragment_dry_run_exceeded() {
        test_payload_named_fragment_dry_run_exceeded_impl().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_payload_inline_fragment_dry_run_exceeded() {
        test_payload_inline_fragment_dry_run_exceeded_impl().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_payload_using_vars_mutation_passes() {
        let _guard = telemetry_subscribers::TelemetryConfig::new()
            .with_env()
            .init();
        let cluster = iota_graphql_rpc::test_infra::cluster::start_cluster(
            ConnectionConfig::ci_integration_test_cfg(),
            None,
            ServiceConfig {
                limits: Limits {
                    max_query_payload_size: 5000,
                    max_tx_payload_size: 6000,
                    ..Default::default()
                },
                ..ServiceConfig::test_defaults()
            },
        )
        .await;
        let addresses = cluster.validator_fullnode_handle.wallet.get_addresses();

        let recipient = addresses[1];
        let tx = cluster
            .validator_fullnode_handle
            .test_transaction_builder()
            .await
            .transfer_iota(Some(1_000), recipient)
            .build();
        let signed_tx = cluster
            .validator_fullnode_handle
            .wallet
            .sign_transaction(&tx);
        let (tx_bytes, sigs) = signed_tx.to_tx_bytes_and_signatures();
        let tx_bytes = tx_bytes.encoded();
        let sigs = sigs.iter().map(|sig| sig.encoded()).collect::<Vec<_>>();

        let mutation = r#"{
            executeTransactionBlock(txBytes: $tx,  signatures: $sigs) {
                effects {
                    transactionBlock { digest }
                    status
                }
                errors
            }
        }"#;

        let variables = vec![
            GraphqlQueryVariable {
                name: "tx".to_string(),
                ty: "String!".to_string(),
                value: json!(tx_bytes),
            },
            GraphqlQueryVariable {
                name: "sigs".to_string(),
                ty: "[String!]!".to_string(),
                value: json!(sigs),
            },
        ];

        let res = cluster
            .graphql_client
            .execute_mutation_to_graphql(mutation.to_string(), variables)
            .await
            .unwrap();

        assert!(res.errors().is_empty());
    }
}
