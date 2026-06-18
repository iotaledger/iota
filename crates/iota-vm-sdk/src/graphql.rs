// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! GraphQL-backed store (`feature = "graphql"`, native only).
//!
//! [`GraphqlStore`] mirrors [`crate::grpc::GrpcStore`] but fetches objects over
//! GraphQL: it wraps a GraphQL client and an in-memory object cache, resolving
//! objects on demand during execution and caching them, so only the objects a
//! run actually touches are fetched. [`prefetch`](GraphqlStore::prefetch) is an
//! optional warm-up.
//!
//! On-demand fetching blocks the executor thread on async I/O, so
//! [`LocalVm::execute`](crate::LocalVm::execute) must run inside a
//! multi-threaded Tokio runtime (e.g. `#[tokio::main]`).

use std::collections::HashSet;

use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use iota_sdk_graphql_client::Client;
use iota_sdk_types::{ObjectId, Version};
use iota_types::{object::Object, transaction::TransactionData};

use crate::{
    caching::{CachingStore, ObjectFetcher},
    error::{StoreError, VmSdkError},
    executor::ChainContext,
    store::{InMemoryStore, Store},
};

/// A [`Store`] backed by a remote node over GraphQL, resolving objects on
/// demand.
///
/// Clones share the same cache and client.
#[derive(Clone)]
pub struct GraphqlStore {
    cache: CachingStore<GraphqlFetcher>,
}

impl GraphqlStore {
    /// Wrap an existing client. The store starts with the built-in framework
    /// packages already loaded so Move calls resolve.
    pub fn new(client: Client) -> Self {
        Self {
            cache: CachingStore::new(GraphqlFetcher { client }),
        }
    }

    /// Connect to a GraphQL endpoint (by URL) and create a store containing
    /// only the built-in framework packages.
    ///
    /// # Errors
    ///
    /// Returns [`VmSdkError::Store`] if `url` is not a valid server address.
    pub fn connect(url: impl Into<String>) -> Result<Self, VmSdkError> {
        let client =
            Client::new(&url.into()).map_err(|e| StoreError::new("connect GraphQL client", e))?;
        Ok(Self::new(client))
    }

    /// A snapshot clone of the objects cached so far (framework packages plus
    /// anything fetched on demand or pre-fetched).
    pub fn store(&self) -> InMemoryStore {
        self.cache.store()
    }

    /// The most recent on-demand fetch failure, if any.
    ///
    /// A failed cache-miss fetch collapses to "object absent" — surfacing later
    /// as [`VmSdkError::MissingObject`] — so check this to tell a transient
    /// transport or decode failure apart from a genuinely missing object.
    /// Cleared by the next successful fetch.
    pub fn last_fetch_error(&self) -> Option<String> {
        self.cache.last_fetch_error()
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
        let data = self
            .cache
            .fetcher()
            .query("fetch epoch via GraphQL", query.to_string())
            .await?;
        let epoch = data
            .pointer("/epoch")
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

    /// Fetch every object the transaction references and cache them in one
    /// batched request.
    ///
    /// Optional: the store also resolves these objects on demand during
    /// execution. Pre-fetching only saves the per-object round-trips the
    /// executor would otherwise make for the transaction body.
    pub async fn prefetch(&mut self, transaction: &TransactionData) -> Result<(), VmSdkError> {
        self.cache.prefetch(transaction).await
    }

    /// Eagerly fetch the dynamic-field children of every object already cached,
    /// recursively, and insert them too. Mirrors
    /// [`GrpcStore::prefetch_dynamic_fields`](crate::grpc::GrpcStore::prefetch_dynamic_fields).
    ///
    /// The GraphQL `dynamicFields` connection returns each field's name and
    /// value but not the `Field` wrapper object's id, so the wrapper id is
    /// derived from the field name the same way the Move VM derives it
    /// on-chain.
    ///
    /// # Errors
    ///
    /// Returns [`VmSdkError::Store`] if a listing or fetch fails.
    pub async fn prefetch_dynamic_fields(&mut self) -> Result<(), VmSdkError> {
        let mut visited: HashSet<ObjectId> = HashSet::new();
        let mut queue: Vec<ObjectId> = self.cache.cached_ids();
        while let Some(parent) = queue.pop() {
            if !visited.insert(parent) {
                continue;
            }
            let ids = self.list_dynamic_field_object_ids(parent).await?;
            if ids.is_empty() {
                continue;
            }
            let refs: Vec<(ObjectId, Option<Version>)> = ids.iter().map(|id| (*id, None)).collect();
            self.cache.fetch_and_insert(&refs).await?;
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
            let data = self
                .cache
                .fetcher()
                .query("list dynamic fields via GraphQL", query)
                .await?;
            let Some(fields) = data.pointer("/object/dynamicFields") else {
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
}

impl Store for GraphqlStore {
    fn get_object(&self, id: &ObjectId, version: Option<Version>) -> Option<Object> {
        self.cache.get_object(id, version)
    }

    fn get_child_object(
        &self,
        parent: &ObjectId,
        child: &ObjectId,
        version_upper_bound: Version,
    ) -> Option<Object> {
        self.cache
            .get_child_object(parent, child, version_upper_bound)
    }

    fn insert(&mut self, object: Object) {
        self.cache.insert(object);
    }

    fn remove(&mut self, id: &ObjectId) {
        self.cache.remove(id);
    }
}

/// GraphQL transport for [`CachingStore`].
#[derive(Clone)]
struct GraphqlFetcher {
    client: Client,
}

impl GraphqlFetcher {
    /// Run a raw GraphQL query and return its `data` payload, surfacing any
    /// GraphQL `errors` as a [`StoreError`] tagged with `context`.
    async fn query(&self, context: &str, query: String) -> Result<serde_json::Value, VmSdkError> {
        let request =
            serde_json::Map::from_iter([("query".to_owned(), serde_json::Value::String(query))]);
        let response = self
            .client
            .run_query_from_json(request)
            .await
            .map_err(|e| StoreError::new(context.to_owned(), e))?;
        if let Some(errors) = response.errors.filter(|errors| !errors.is_empty()) {
            let message = errors
                .iter()
                .map(|e| e.message.as_str())
                .collect::<Vec<_>>()
                .join("; ");
            return Err(StoreError::new(context.to_owned(), message).into());
        }
        response
            .data
            .ok_or_else(|| StoreError::new(context.to_owned(), "empty response").into())
    }
}

impl ObjectFetcher for GraphqlFetcher {
    async fn fetch_objects(
        &self,
        refs: &[(ObjectId, Option<Version>)],
    ) -> Result<Vec<Object>, VmSdkError> {
        let mut aliases: Vec<String> = Vec::with_capacity(refs.len());
        for (id, version) in refs {
            match version {
                Some(v) => aliases.push(format!(
                    r#"v{}: object(address: "{id}", version: {}) {{ bcs }}"#,
                    aliases.len(),
                    v.as_u64()
                )),
                None => aliases.push(format!(
                    r#"v{}: object(address: "{id}") {{ bcs }}"#,
                    aliases.len()
                )),
            }
        }
        let query = format!("{{ {} }}", aliases.join("\n"));
        let data = self.query("GraphQL query", query).await?;
        let mut objects = Vec::new();
        if let Some(obj_map) = data.as_object() {
            for (alias, value) in obj_map {
                if let Some(bcs_b64) = value.pointer("/bcs").and_then(|v| v.as_str()) {
                    let bytes = BASE64
                        .decode(bcs_b64)
                        .map_err(|e| StoreError::new(format!("decode {alias}"), e))?;
                    let obj: Object = bcs::from_bytes(&bytes)
                        .map_err(|e| StoreError::new(format!("bcs {alias}"), e))?;
                    objects.push(obj);
                }
            }
        }
        Ok(objects)
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
