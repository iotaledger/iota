// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! GraphQL-backed store (`feature = "graphql"`, native only).
//!
//! [`GraphqlStore`] mirrors [`crate::grpc::GrpcStore`] but fetches objects over
//! GraphQL: it wraps a GraphQL client and an in-memory object cache, resolving
//! objects on demand during execution and caching them, so only the objects a
//! run actually touches are fetched.
//!
//! On-demand fetching blocks the executor thread on async I/O, so
//! [`LocalVm::execute`](crate::LocalVm::execute) must run inside a
//! multi-threaded Tokio runtime (e.g. `#[tokio::main]`).

use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use iota_sdk_graphql_client::Client;
use iota_sdk_types::{ObjectId, Version};
use iota_types::{digests::ChainIdentifier, object::Object};

use crate::{
    caching::{CachingStore, ObjectFetcher},
    error::{StoreError, VmSdkError},
    executor::ChainContext,
    store::{InMemoryStore, Store},
};

/// A [`Store`] backed by a remote node over GraphQL, resolving objects on
/// demand.
///
/// Clones share the same cache and client. Objects resolve at their latest
/// version; see [`Store::get_child_object`] for the dynamic-field version-bound
/// caveat.
///
/// # Panics
///
/// On-demand object resolution (via the synchronous [`Store`] impl) panics
/// unless called from within a multi-threaded Tokio runtime.
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
    /// anything fetched on demand).
    pub fn store(&self) -> InMemoryStore {
        self.cache.store()
    }

    /// Fetch the chain parameters a [`LocalVm`](crate::LocalVm) needs.
    ///
    /// The [`Chain`](crate::Chain) is resolved from the node's chain identifier
    /// so chain-gated protocol features match the real chain; an unrecognised
    /// identifier maps to [`Chain::Unknown`](crate::Chain).
    ///
    /// # Errors
    ///
    /// Returns [`VmSdkError::Store`] if a query fails or epoch fields are
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
        let chain_id = self
            .cache
            .fetcher()
            .client
            .chain_id()
            .await
            .map_err(|e| StoreError::new("fetch chain identifier", e))?;
        let chain = ChainIdentifier::from_chain_short_id(&chain_id)
            .map(|id| id.chain())
            .unwrap_or(iota_protocol_config::Chain::Unknown);
        Ok(ChainContext {
            protocol_version: iota_protocol_config::ProtocolVersion::new(protocol_version),
            reference_gas_price,
            epoch_id,
            epoch_timestamp_ms,
            chain,
        })
    }
}

impl Store for GraphqlStore {
    fn get_object(
        &self,
        id: &ObjectId,
        version: Option<Version>,
    ) -> Result<Option<Object>, StoreError> {
        self.cache.get_object(id, version)
    }

    fn get_child_object(
        &self,
        parent: &ObjectId,
        child: &ObjectId,
        version_upper_bound: Version,
    ) -> Result<Option<Object>, StoreError> {
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
    async fn query(&self, context: &str, query: String) -> Result<serde_json::Value, StoreError> {
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
            return Err(StoreError::new(context.to_owned(), message));
        }
        response
            .data
            .ok_or_else(|| StoreError::new(context.to_owned(), "empty response"))
    }
}

impl ObjectFetcher for GraphqlFetcher {
    async fn fetch_objects(
        &self,
        refs: &[(ObjectId, Option<Version>)],
    ) -> Result<Vec<Object>, StoreError> {
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
        // A missing object resolves to `null` (so its `bcs` is absent). The
        // `Store` contract treats absence as `Ok(None)` (the VM's child-object
        // resolver relies on this — a dynamic field that does not exist must
        // read as absent, not fault), so a single-object request that comes
        // back missing yields no objects rather than an error. A batched
        // request can't say which ref was missing, so it stays all-or-nothing;
        // the on-demand resolution path only ever fetches one object at a time.
        let mut objects = Vec::with_capacity(refs.len());
        for (index, (id, _)) in refs.iter().enumerate() {
            let alias = format!("v{index}");
            let bcs_b64 = match data
                .pointer(&format!("/{alias}/bcs"))
                .and_then(|v| v.as_str())
            {
                Some(bcs_b64) => bcs_b64,
                None if refs.len() == 1 => return Ok(Vec::new()),
                None => {
                    return Err(StoreError::new(
                        "GraphQL query",
                        format!("object {id} not found"),
                    ));
                }
            };
            let bytes = BASE64
                .decode(bcs_b64)
                .map_err(|e| StoreError::new(format!("decode {alias}"), e))?;
            let obj: Object =
                bcs::from_bytes(&bytes).map_err(|e| StoreError::new(format!("bcs {alias}"), e))?;
            objects.push(obj);
        }
        Ok(objects)
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
