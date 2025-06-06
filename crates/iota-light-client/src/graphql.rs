// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use anyhow::{Result, anyhow};
use iota_graphql_rpc_client::simple_client::{GraphqlQueryVariable, SimpleClient};
use serde_json::json;

use crate::config::Config;

pub async fn query_last_checkpoint_of_epoch(config: &Config, epoch_id: u64) -> Result<u64> {
    // GraphQL query to get the last checkpoint of an epoch
    let query = r#"
        {
            epoch(id: $epochID) { 
                checkpoints(last: 1) { 
                    nodes { 
                        sequenceNumber
                    }
                }
            } 
        }
    "#;
    let variables = vec![GraphqlQueryVariable {
        name: "epochID".to_string(),
        ty: "Int".to_string(),
        value: json!(epoch_id),
    }];
    let client = SimpleClient::new(
        config
            .graphql_url
            .as_ref()
            .cloned()
            .ok_or_else(|| anyhow!("missing graphql url"))?,
    );
    // Submit the query by POSTing to the GraphQL endpoint
    let resp = client
        .execute_to_graphql(query.to_string(), true, variables, vec![])
        .await?;
    anyhow::ensure!(resp.errors().is_empty(), "{:?}", resp.errors());

    let data = resp.response_body().data.clone().into_json()?;

    // Parse the JSON response to get the last checkpoint of the epoch
    let checkpoint_number = data["epoch"]["checkpoints"]["nodes"][0]["sequenceNumber"]
        .as_u64()
        .expect("invalid sequence number");

    Ok(checkpoint_number)
}
