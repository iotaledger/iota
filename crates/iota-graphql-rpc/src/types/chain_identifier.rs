// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use async_graphql::*;
use diesel::QueryDsl;
use iota_indexer::schema::chain_identifier;
use iota_types::{
    digests::ChainIdentifier as NativeChainIdentifier, messages_checkpoint::CheckpointDigest,
};
use tokio::sync::OnceCell;
use tracing::error;

use crate::{
    data::{Db, DbConnection, QueryExecutor},
    error::Error,
    metrics::Metrics,
};

/// Cache the chain identifier with guaranteed one-time initialization. Once
/// set, typically from database, the value cannot be changed.
#[derive(Clone, Default)]
pub(crate) struct ChainIdentifierCache(Arc<OnceCell<ChainIdentifier>>);

impl ChainIdentifierCache {
    /// Read or initialize the stored chain identifier by querying the database
    /// once and cache the result to avoid subsequent queries.
    ///
    /// If the database query fails, this function will return an error, but the
    /// cached value remains unset. Subsequent calls will continue to attempt
    /// database queries until one succeeds, at which point the value will be
    /// cached for all future calls.
    ///
    /// This ensures the chain identifier is only queried once successfully,
    /// improving performance while maintaining proper error handling.
    pub(crate) async fn read(&self, db: &Db, metrics: &Metrics) -> Result<ChainIdentifier, Error> {
        self.0
            .get_or_try_init(|| async {
                match ChainIdentifier::query(db).await {
                    Ok(chain) => Ok(chain.into()),
                    Err(e) => {
                        error!("failed to fetch chain identifier: {e}");
                        metrics.inc_errors(&[ServerError::new(e.to_string(), None)]);
                        Err(e)
                    }
                }
            })
            .await
            .copied()
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ChainIdentifier(NativeChainIdentifier);

impl ChainIdentifier {
    /// Unwraps the inner
    /// [`NativeChainIdentifier`](iota_types::digests::ChainIdentifier).
    pub(crate) fn into_inner(self) -> NativeChainIdentifier {
        self.0
    }

    /// Query the Chain Identifier from the DB.
    pub(crate) async fn query(db: &Db) -> Result<NativeChainIdentifier, Error> {
        use chain_identifier::dsl;

        let digest_bytes = db
            .execute(move |conn| {
                conn.first(move || dsl::chain_identifier.select(dsl::checkpoint_digest))
            })
            .await
            .map_err(|e| Error::Internal(format!("Failed to fetch genesis digest: {e}")))?;

        Self::from_bytes(digest_bytes)
    }

    /// Treat `bytes` as a checkpoint digest and extract a chain identifier from
    /// it.
    pub(crate) fn from_bytes(bytes: Vec<u8>) -> Result<NativeChainIdentifier, Error> {
        let genesis_digest = CheckpointDigest::try_from(bytes)
            .map_err(|e| Error::Internal(format!("Failed to deserialize genesis digest: {e}")))?;
        Ok(NativeChainIdentifier::from(genesis_digest))
    }
}

impl From<NativeChainIdentifier> for ChainIdentifier {
    fn from(chain_identifier: NativeChainIdentifier) -> Self {
        Self(chain_identifier)
    }
}
