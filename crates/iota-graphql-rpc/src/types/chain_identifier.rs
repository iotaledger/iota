// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use async_graphql::*;
use diesel::{OptionalExtension, QueryDsl};
use iota_indexer::schema::chain_identifier;
use iota_types::{
    digests::ChainIdentifier as NativeChainIdentifier, messages_checkpoint::CheckpointDigest,
};

use crate::{
    data::{Db, DbConnection, QueryExecutor},
    error::Error,
};

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ChainIdentifier(Option<NativeChainIdentifier>);

impl ChainIdentifier {
    /// Unwraps the inner `Option<NativeChainIdentifier>`.
    pub(crate) fn into_inner(self) -> Option<NativeChainIdentifier> {
        self.0
    }

    /// Query the Chain Identifier from the DB.
    pub(crate) async fn query(db: &Db) -> Result<Option<NativeChainIdentifier>, Error> {
        use chain_identifier::dsl;

        db.execute(move |conn| {
            conn.first(move || dsl::chain_identifier.select(dsl::checkpoint_digest))
                .optional()
        })
        .await
        .map_err(|e| Error::Internal(format!("Failed to fetch genesis digest: {e}")))?
        .map(Self::from_bytes)
        .transpose()
    }

    /// Treat `bytes` as a checkpoint digest and extract a chain identifier from
    /// it.
    pub(crate) fn from_bytes(bytes: Vec<u8>) -> Result<NativeChainIdentifier, Error> {
        let genesis_digest = CheckpointDigest::try_from(bytes)
            .map_err(|e| Error::Internal(format!("Failed to deserialize genesis digest: {e}")))?;
        Ok(NativeChainIdentifier::from(genesis_digest))
    }
}

impl From<Option<NativeChainIdentifier>> for ChainIdentifier {
    fn from(chain_identifier: Option<NativeChainIdentifier>) -> Self {
        Self(chain_identifier)
    }
}

impl From<NativeChainIdentifier> for ChainIdentifier {
    fn from(chain_identifier: NativeChainIdentifier) -> Self {
        Self(Some(chain_identifier))
    }
}
