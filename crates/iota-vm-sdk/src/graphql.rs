// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! GraphQL-backed store population (`feature = "graphql"`, native only).
//!
//! [`GraphqlStore`] mirrors [`crate::grpc::GrpcStore`] but fetches objects over
//! GraphQL. Object fetching is async and confined to this module; the populated
//! store is a plain synchronous [`Store`].

use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use iota_graphql_rpc_client::simple_client::SimpleClient;
use iota_sdk_types::{ObjectId, Version};
use iota_types::{
    object::Object,
    transaction::{TransactionData, TransactionDataAPI},
};

use crate::{
    error::{ValidationError, VmSdkError},
    executor::ChainContext,
    store::{InMemoryStore, Store},
};

/// A [`Store`] populated from a remote node over GraphQL.
#[derive(Clone)]
pub struct GraphqlStore {
    inner: InMemoryStore,
    client: SimpleClient,
}

impl GraphqlStore {
    /// Create a store backed by a GraphQL endpoint, seeded with the built-in
    /// framework packages.
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            inner: InMemoryStore::with_framework(),
            client: SimpleClient::new(url),
        }
    }

    /// Fetch the chain parameters a [`LocalVm`](crate::LocalVm) needs.
    pub async fn fetch_chain_context(&self) -> Result<ChainContext, VmSdkError> {
        let query = r#"{
            epoch {
                epochId
                referenceGasPrice
                startTimestamp
                protocolConfigs { protocolVersion }
            }
        }"#;
        let json = self
            .client
            .execute(query.to_string(), vec![])
            .await
            .map_err(|e| ValidationError::new("fetch epoch via GraphQL", e))?;
        let epoch = json
            .pointer("/data/epoch")
            .ok_or_else(|| ValidationError::new("GraphQL epoch", "missing epoch data"))?;
        let epoch_id = epoch
            .get("epochId")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| ValidationError::new("GraphQL epoch", "missing epochId"))?;
        let reference_gas_price = epoch
            .get("referenceGasPrice")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<u64>().ok())
            .ok_or_else(|| ValidationError::new("GraphQL epoch", "missing referenceGasPrice"))?;
        let epoch_timestamp_ms = epoch
            .get("startTimestamp")
            .and_then(|v| v.as_str())
            .and_then(parse_start_timestamp_millis)
            .ok_or_else(|| ValidationError::new("GraphQL epoch", "missing startTimestamp"))?;
        let protocol_version = epoch
            .pointer("/protocolConfigs/protocolVersion")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| ValidationError::new("GraphQL epoch", "missing protocolVersion"))?;
        Ok(ChainContext {
            protocol_version: iota_protocol_config::ProtocolVersion::new(protocol_version),
            reference_gas_price,
            epoch_id,
            epoch_timestamp_ms,
            chain: iota_protocol_config::Chain::Unknown,
        })
    }

    /// Fetch every object the transaction references and insert it into the
    /// store. Owned/immutable objects are fetched at their transaction
    /// versions; shared objects and packages at the latest version.
    pub async fn prefetch(&mut self, transaction: &TransactionData) -> Result<(), VmSdkError> {
        use iota_types::transaction::InputObjectKind;
        let input_object_kinds = transaction
            .input_objects()
            .map_err(|e| ValidationError::new("collect input objects", e))?;

        let mut aliases: Vec<String> = Vec::new();
        let push_versioned = |id: &ObjectId, version: Version, aliases: &mut Vec<String>| {
            aliases.push(format!(
                r#"v{}: object(address: "{id}", version: {}) {{ bcs }}"#,
                aliases.len(),
                version.as_u64()
            ));
        };
        for kind in &input_object_kinds {
            match kind {
                InputObjectKind::ImmOrOwnedMoveObject(objref) => {
                    push_versioned(&objref.object_id, objref.version, &mut aliases)
                }
                InputObjectKind::SharedMoveObject { id, .. } => {
                    aliases.push(format!(
                        r#"v{}: object(address: "{id}") {{ bcs }}"#,
                        aliases.len()
                    ));
                }
                InputObjectKind::MovePackage(id) => {
                    aliases.push(format!(
                        r#"v{}: object(address: "{id}") {{ bcs }}"#,
                        aliases.len()
                    ));
                }
            }
        }
        for gas_ref in transaction.gas() {
            push_versioned(&gas_ref.object_id, gas_ref.version, &mut aliases);
        }
        for objref in transaction.receiving_objects() {
            push_versioned(&objref.object_id, objref.version, &mut aliases);
        }

        if aliases.is_empty() {
            return Ok(());
        }
        let query = format!("{{ {} }}", aliases.join("\n"));
        self.execute_and_insert(&query).await
    }

    async fn execute_and_insert(&mut self, query: &str) -> Result<(), VmSdkError> {
        let json = self
            .client
            .execute(query.to_string(), vec![])
            .await
            .map_err(|e| ValidationError::new("GraphQL query", e))?;
        let data = json
            .get("data")
            .ok_or_else(|| ValidationError::new("GraphQL response", "missing data"))?;
        if let Some(obj_map) = data.as_object() {
            for (alias, value) in obj_map {
                if let Some(bcs_b64) = value.pointer("/bcs").and_then(|v| v.as_str()) {
                    let bytes = BASE64
                        .decode(bcs_b64)
                        .map_err(|e| ValidationError::new(format!("decode {alias}"), e))?;
                    let obj: Object = bcs::from_bytes(&bytes)
                        .map_err(|e| ValidationError::new(format!("bcs {alias}"), e))?;
                    self.inner.insert(obj);
                }
            }
        }
        Ok(())
    }
}

/// Parse the GraphQL `startTimestamp` (integer ms, or float seconds) to
/// milliseconds since epoch.
fn parse_start_timestamp_millis(s: &str) -> Option<u64> {
    if let Ok(ms) = s.parse::<u64>() {
        return Some(ms);
    }
    if let Ok(secs) = s.parse::<f64>() {
        return Some((secs * 1000.0) as u64);
    }
    None
}

impl Store for GraphqlStore {
    fn get_object(&self, id: &ObjectId, version: Option<Version>) -> Option<Object> {
        self.inner.get_object(id, version)
    }

    fn get_child_object(
        &self,
        parent: &ObjectId,
        child: &ObjectId,
        version_upper_bound: Version,
    ) -> Option<Object> {
        self.inner
            .get_child_object(parent, child, version_upper_bound)
    }

    fn insert(&mut self, object: Object) {
        self.inner.insert(object);
    }

    fn remove(&mut self, id: &ObjectId) {
        self.inner.remove(id);
    }
}
