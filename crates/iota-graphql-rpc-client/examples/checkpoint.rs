// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This example showcases how to use the GraphQL API by querying data about the
//! latest checkpoint.
//!
//! cargo run --example checkpoint

use iota_graphql_rpc_client::simple_client::SimpleClient;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let graphql_client = SimpleClient::new("http://127.0.0.1:8000");
    let query = r#"{
            checkpoint {
                digest
                sequenceNumber
                validatorSignatures
                previousCheckpointDigest
                networkTotalTransactions
                rollingGasSummary {
                    computationCost
                    storageCost
                    storageRebate
                    nonRefundableStorageFee
                    }
                epoch {
                    epochId
                    referenceGasPrice
                    startTimestamp
                    endTimestamp
                }
                transactionBlocks(first: 2) {
                    edges {
                        node {
                            kind {
                                __typename
                            }
                            digest
                            sender {
                                address
                            }
                        }
                    }
                }
            }
        }"#;
    let res = graphql_client
        .execute_to_graphql(query.to_string(), true, vec![], vec![])
        .await?;
    let resp_body = res.response_body().data.clone().into_json()?;
    // Access a nested field
    println!(
        "Selected data for checkpoint {}:",
        resp_body
            .get("checkpoint")
            .ok_or(anyhow::anyhow!("missing checkpoint"))?
            .get("sequenceNumber")
            .ok_or(anyhow::anyhow!("missing sequenceNumber"))?
    );
    // Full response
    println!("{:#?}", resp_body);

    Ok(())
}
