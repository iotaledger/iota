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
    transaction::{InputObjectKind, TransactionData, TransactionDataAPI},
};

use crate::{
    error::{StoreError, VmSdkError},
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
    /// Wrap an existing client. The store starts with the built-in framework
    /// packages already loaded so Move calls resolve.
    pub fn new(client: SimpleClient) -> Self {
        Self {
            inner: InMemoryStore::with_framework(),
            client,
        }
    }

    /// Connect to a GraphQL endpoint (by URL) and create a store containing
    /// only the built-in framework packages.
    ///
    /// Returns a `Result` to mirror
    /// [`GrpcStore::connect`](crate::grpc::GrpcStore::connect); building the
    /// client is currently infallible.
    pub fn connect(url: impl Into<String>) -> Result<Self, VmSdkError> {
        Ok(Self::new(SimpleClient::new(url)))
    }

    /// Read-only access to the wrapped in-memory store, e.g. to snapshot the
    /// objects fetched so far.
    pub fn store(&self) -> &InMemoryStore {
        &self.inner
    }

    /// Fetch the chain parameters a [`LocalVm`](crate::LocalVm) needs.
    ///
    /// Reports [`Chain::Unknown`](crate::Chain) — the chain identity is not
    /// resolved here; this only affects chain-specific protocol behaviour.
    ///
    /// # Errors
    ///
    /// Returns [`VmSdkError::Store`] if the query fails or epoch fields are
    /// missing.
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
            .map_err(|e| StoreError::new("fetch epoch via GraphQL", e))?;
        let epoch = json
            .pointer("/data/epoch")
            .ok_or_else(|| StoreError::new("GraphQL epoch", "missing epoch data"))?;
        let epoch_id = epoch
            .get("epochId")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| StoreError::new("GraphQL epoch", "missing epochId"))?;
        let reference_gas_price = epoch
            .get("referenceGasPrice")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<u64>().ok())
            .ok_or_else(|| StoreError::new("GraphQL epoch", "missing referenceGasPrice"))?;
        let start_timestamp = epoch
            .get("startTimestamp")
            .and_then(|v| v.as_str())
            .ok_or_else(|| StoreError::new("GraphQL epoch", "missing startTimestamp"))?;
        let epoch_timestamp_ms =
            parse_start_timestamp_millis(start_timestamp).ok_or_else(|| {
                StoreError::new(
                    "GraphQL epoch",
                    format!("invalid startTimestamp {start_timestamp:?}"),
                )
            })?;
        let protocol_version = epoch
            .pointer("/protocolConfigs/protocolVersion")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| StoreError::new("GraphQL epoch", "missing protocolVersion"))?;
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
    ///
    /// This covers the transaction body only. A `MoveAuthenticator`-signed run
    /// also needs the authenticator's input objects and each account's
    /// `AuthenticatorFunctionRefV1` field present in the store; insert them
    /// before executing such a transaction.
    pub async fn prefetch(&mut self, transaction: &TransactionData) -> Result<(), VmSdkError> {
        let input_object_kinds = transaction
            .input_objects()
            .map_err(|e| StoreError::new("collect input objects", e))?;

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
            .map_err(|e| StoreError::new("GraphQL query", e))?;
        let data = json
            .get("data")
            .ok_or_else(|| StoreError::new("GraphQL response", "missing data"))?;
        if let Some(obj_map) = data.as_object() {
            for (alias, value) in obj_map {
                if let Some(bcs_b64) = value.pointer("/bcs").and_then(|v| v.as_str()) {
                    let bytes = BASE64
                        .decode(bcs_b64)
                        .map_err(|e| StoreError::new(format!("decode {alias}"), e))?;
                    let obj: Object = bcs::from_bytes(&bytes)
                        .map_err(|e| StoreError::new(format!("bcs {alias}"), e))?;
                    self.inner.insert(obj);
                }
            }
        }
        Ok(())
    }
}

/// Parse the GraphQL `startTimestamp` scalar — an RFC 3339 datetime string
/// (`YYYY-MM-DDTHH:MM:SS.mmmZ`), or plain integer milliseconds — to
/// milliseconds since the Unix epoch.
fn parse_start_timestamp_millis(s: &str) -> Option<u64> {
    if let Ok(ms) = s.parse::<u64>() {
        return Some(ms);
    }
    let datetime = chrono::DateTime::parse_from_rfc3339(s).ok()?;
    u64::try_from(datetime.timestamp_millis()).ok()
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

#[cfg(test)]
mod tests {
    use super::parse_start_timestamp_millis;

    #[test]
    fn start_timestamp_parses_rfc3339_and_integer_millis() {
        assert_eq!(
            parse_start_timestamp_millis("2023-08-19T15:37:24.761Z"),
            Some(1_692_459_444_761)
        );
        assert_eq!(
            parse_start_timestamp_millis("2023-08-19T15:37:24Z"),
            Some(1_692_459_444_000)
        );
        assert_eq!(
            parse_start_timestamp_millis("1692459444761"),
            Some(1_692_459_444_761)
        );
        assert_eq!(parse_start_timestamp_millis("not a date"), None);
    }
}
