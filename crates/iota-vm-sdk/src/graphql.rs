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

    /// Recursively fetch the dynamic-field children of every object already in
    /// the store and insert them too. Mirrors
    /// [`GrpcStore::prefetch_dynamic_fields`](crate::grpc::GrpcStore::prefetch_dynamic_fields):
    /// Move calls that read tables/bags need these children present to execute
    /// offline — e.g. staking walks the validator set stored as a dynamic field
    /// inside `IotaSystemState`. Call after [`prefetch`](Self::prefetch);
    /// children are fetched at their latest version. Recursion is bounded only
    /// by the object graph (a `visited` set breaks cycles); intended for local
    /// development against trusted nodes.
    ///
    /// The GraphQL `dynamicFields` connection returns each field's name and
    /// value but not the `Field` wrapper object's id, so the wrapper id is
    /// derived from the field name (its type and BCS bytes) the same way the
    /// Move VM derives it on-chain.
    ///
    /// # Errors
    ///
    /// Returns [`VmSdkError::Store`] if a listing or fetch fails.
    pub async fn prefetch_dynamic_fields(&mut self) -> Result<(), VmSdkError> {
        let mut visited: std::collections::HashSet<ObjectId> = std::collections::HashSet::new();
        let mut queue: Vec<ObjectId> = self.inner.iter().map(|(id, _)| *id).collect();
        while let Some(parent) = queue.pop() {
            if !visited.insert(parent) {
                continue;
            }
            let ids = self.list_dynamic_field_object_ids(parent).await?;
            if ids.is_empty() {
                continue;
            }
            let mut aliases: Vec<String> = Vec::new();
            for id in &ids {
                aliases.push(format!(
                    r#"v{}: object(address: "{id}") {{ bcs }}"#,
                    aliases.len()
                ));
            }
            let query = format!("{{ {} }}", aliases.join("\n"));
            self.execute_and_insert(&query).await?;
            // Recurse into the newly fetched children to find their descendants.
            queue.extend(ids);
        }
        Ok(())
    }

    /// List the object ids that make up `parent`'s dynamic fields: the derived
    /// `Field` wrapper for every field, plus the separate child object of each
    /// dynamic *object* field.
    async fn list_dynamic_field_object_ids(
        &self,
        parent: ObjectId,
    ) -> Result<Vec<ObjectId>, VmSdkError> {
        let mut ids: Vec<ObjectId> = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let after = match &cursor {
                Some(c) => format!(r#", after: "{c}""#),
                None => String::new(),
            };
            let query = format!(
                r#"{{ object(address: "{parent}") {{ dynamicFields(first: 50{after}) {{
                    pageInfo {{ hasNextPage endCursor }}
                    nodes {{
                        name {{ type {{ repr }} bcs }}
                        value {{ __typename ... on MoveObject {{ address }} }}
                    }}
                }} }} }}"#
            );
            let json = self
                .client
                .execute(query, vec![])
                .await
                .map_err(|e| StoreError::new("list dynamic fields via GraphQL", e))?;
            let Some(fields) = json.pointer("/data/object/dynamicFields") else {
                break;
            };
            if let Some(nodes) = fields.get("nodes").and_then(|v| v.as_array()) {
                for node in nodes {
                    if let Some(field_id) = derive_field_wrapper_id(parent, node) {
                        ids.push(field_id);
                    }
                    // A dynamic *object* field keeps its value in a separate
                    // child object, addressable directly.
                    if node.pointer("/value/__typename").and_then(|v| v.as_str())
                        == Some("MoveObject")
                    {
                        if let Some(id) = node
                            .pointer("/value/address")
                            .and_then(|v| v.as_str())
                            .and_then(|s| s.parse::<ObjectId>().ok())
                        {
                            ids.push(id);
                        }
                    }
                }
            }
            let has_next = fields
                .pointer("/pageInfo/hasNextPage")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            cursor = fields
                .pointer("/pageInfo/endCursor")
                .and_then(|v| v.as_str())
                .map(String::from);
            if !has_next || cursor.is_none() {
                break;
            }
        }
        Ok(ids)
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

/// Derive the on-chain `Field` wrapper object id for a `dynamicFields` node
/// from its name (the field's type repr and BCS bytes), matching the Move VM's
/// derivation. Returns `None` if the node is missing a name or it can't be
/// parsed.
fn derive_field_wrapper_id(parent: ObjectId, node: &serde_json::Value) -> Option<ObjectId> {
    let repr = node.pointer("/name/type/repr").and_then(|v| v.as_str())?;
    let name_bcs = node.pointer("/name/bcs").and_then(|v| v.as_str())?;
    let tag = repr.parse::<iota_sdk_types::TypeTag>().ok()?;
    let name_bytes = BASE64.decode(name_bcs).ok()?;
    iota_types::dynamic_field::derive_dynamic_field_id(*parent.as_address(), &tag, &name_bytes).ok()
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
