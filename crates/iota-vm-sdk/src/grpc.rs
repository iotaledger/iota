// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! gRPC-backed store (`feature = "grpc"`, native only); see [`GrpcStore`].

use iota_sdk_ext::{
    grpc_client::{
        Client,
        read_mask_fields::{EpochReadMask, ObjectReadMask, ServiceInfoReadMask},
    },
    types::{CheckpointDigest, Digest, ObjectId, Version},
};
use iota_types::{digests::ChainIdentifier, object::Object};

use crate::{
    caching::{CachingStore, ObjectFetcher},
    error::{StoreError, VmSdkError},
    executor::ChainContext,
    store::{InMemoryStore, Store},
};

/// A [`Store`] backed by a remote node over gRPC, resolving objects on demand.
///
/// Clones share the same cache and client. Objects resolve at their latest
/// version; see [`Store::get_child_object`] for the dynamic-field version-bound
/// caveat.
///
/// On-demand object resolution (via the synchronous [`Store`] impl) requires a
/// multi-threaded Tokio runtime (e.g. `#[tokio::main]`); outside one, a cache
/// miss fails with a [`StoreError`] instead of fetching.
#[derive(Clone)]
pub struct GrpcStore {
    cache: CachingStore<GrpcFetcher>,
}

impl GrpcStore {
    /// Wrap an existing client. The store starts with the built-in framework
    /// packages already loaded so Move calls resolve.
    pub fn new(client: Client) -> Self {
        Self {
            cache: CachingStore::new(GrpcFetcher { client }),
        }
    }

    /// Connect to a gRPC endpoint (by URL) and create a store containing only
    /// the built-in framework packages.
    ///
    /// # Errors
    ///
    /// Returns [`VmSdkError::Store`] if the client cannot be created for `url`.
    pub fn connect(url: impl Into<String>) -> Result<Self, VmSdkError> {
        let url: String = url.into();
        let client = Client::new(url).map_err(|e| StoreError::new("connect gRPC", e))?;
        Ok(Self::new(client))
    }

    /// A snapshot clone of the objects cached so far (framework packages plus
    /// anything fetched on demand).
    pub fn store(&self) -> InMemoryStore {
        self.cache.store()
    }

    /// Fetch the chain parameters a [`LocalVm`](crate::LocalVm) needs.
    ///
    /// The [`Chain`](crate::Chain) is resolved from the node's service info so
    /// chain-gated protocol features match the real chain; an unrecognised
    /// chain identifier maps to [`Chain::Unknown`](crate::Chain).
    ///
    /// # Errors
    ///
    /// Returns [`VmSdkError::Store`] if the epoch or service-info RPC fails or
    /// can't be decoded.
    pub async fn fetch_chain_context(&self) -> Result<ChainContext, VmSdkError> {
        let client = &self.cache.fetcher().client;
        let (epoch_response, service_info_response) = tokio::join!(
            client.get_epoch(None, EpochReadMask::default()),
            client.get_service_info(ServiceInfoReadMask::default())
        );
        let epoch = epoch_response
            .map_err(|e| StoreError::new("fetch epoch", e))?
            .into_inner();
        let epoch_id = epoch
            .epoch_id()
            .map_err(|e| StoreError::new("epoch id", e))?;
        let reference_gas_price = epoch
            .gas_price()
            .map_err(|e| StoreError::new("gas price", e))?;
        let epoch_timestamp_ms = epoch
            .start_ms()
            .map_err(|e| StoreError::new("epoch start", e))?;
        let protocol_version = epoch
            .protocol_config()
            .and_then(|pc| pc.version())
            .map_err(|e| StoreError::new("protocol version", e))?;
        let chain = service_info_response
            .map_err(|e| StoreError::new("fetch service info", e))?
            .body()
            .chain_id
            .as_ref()
            .and_then(|d| Digest::try_from(d).ok())
            .map(|digest| ChainIdentifier::from(CheckpointDigest::from(digest)).chain())
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

impl Store for GrpcStore {
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

/// gRPC transport for [`CachingStore`].
#[derive(Clone)]
struct GrpcFetcher {
    client: Client,
}

impl ObjectFetcher for GrpcFetcher {
    async fn fetch_objects(
        &self,
        refs: &[(ObjectId, Option<Version>)],
    ) -> Result<Vec<Object>, StoreError> {
        let results = self
            .client
            .get_objects_with_versions(refs.iter().copied(), ObjectReadMask::default())
            .await
            .map_err(|e| StoreError::new("fetch objects via gRPC", e))?
            .into_inner();

        // The proto helper yields the SDK `Object`, which the node's
        // `iota_types::object::Object` is a newtype over.
        skip_not_found(results)?
            .into_iter()
            .map(|proto_obj| {
                proto_obj
                    .object()
                    .map(Into::into)
                    .map_err(|e| StoreError::new("decode gRPC object", e))
            })
            .collect()
    }
}

/// Keep the items the node returned, dropping the ones it reported as
/// `NOT_FOUND`.
///
/// The `Store` contract treats absence as `Ok(None)` — the VM's child-object
/// resolver relies on this, since a dynamic field that does not exist must read
/// as absent rather than fault. The batched read reports a missing object per
/// requested ref, so the refs the node could serve survive a missing one.
fn skip_not_found<T>(
    results: Vec<Result<T, iota_sdk_ext::grpc_client::api::Error>>,
) -> Result<Vec<T>, StoreError> {
    let mut items = Vec::with_capacity(results.len());
    for result in results {
        match result {
            Ok(item) => items.push(item),
            Err(e) if e.is_not_found() => {}
            Err(e) => return Err(StoreError::new("fetch objects via gRPC", e)),
        }
    }
    Ok(items)
}

#[cfg(test)]
mod tests {
    use iota_sdk_ext::grpc_client::{RpcStatus, api::Error};

    use super::skip_not_found;

    fn server_error(code: tonic::Code) -> Error {
        Error::Server(RpcStatus {
            code: code.into(),
            message: String::new(),
            details: Vec::new(),
        })
    }

    #[test]
    fn a_missing_ref_is_skipped_without_dropping_the_rest() {
        let results = vec![Ok(1), Err(server_error(tonic::Code::NotFound)), Ok(3)];

        assert_eq!(skip_not_found(results).unwrap(), vec![1, 3]);
    }

    #[test]
    fn every_ref_missing_yields_no_objects() {
        let results: Vec<Result<u32, Error>> = vec![
            Err(server_error(tonic::Code::NotFound)),
            Err(server_error(tonic::Code::NotFound)),
        ];

        assert!(skip_not_found(results).unwrap().is_empty());
    }

    #[test]
    fn an_error_other_than_not_found_fails_the_fetch() {
        let results: Vec<Result<u32, Error>> =
            vec![Ok(1), Err(server_error(tonic::Code::Internal))];

        assert!(skip_not_found(results).is_err());
    }
}
