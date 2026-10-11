// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The reader context and the driver. Holds the horizon and the store
//! handles, walks each input machine from its first lookup to a verdict, and
//! loads a transaction's inputs at a commit for the post-consensus validation
//! entry point.

use std::sync::Arc;

use iota_sdk_types::ObjectReference;
use iota_types::error::{IotaError, IotaResult};

use super::{
    KeptObject, OwnedVerdict,
    owned::{NeedBytes, OwnedReader, TablesLookup, TablesRecheck},
};
use crate::{
    authority::authority_per_epoch_store::{
        AuthorityPerEpochStore, handler_object_state::CommitIndex,
    },
    execution_cache::ObjectCacheRead,
};

/// Distance between the commit being validated and the highest commit whose
/// rows a verdict may trust. A future protocol parameter.
pub const K: CommitIndex = 2;

/// Reads inputs as of one consensus commit. Built once per commit and passed
/// to every transition that touches a store.
pub struct CommitIndexedReader {
    pub(super) cache: Arc<dyn ObjectCacheRead>,
    pub(super) epoch_store: Arc<AuthorityPerEpochStore>,
    /// `C - K`.
    horizon: CommitIndex,
}

impl CommitIndexedReader {
    pub fn new(
        cache: Arc<dyn ObjectCacheRead>,
        epoch_store: Arc<AuthorityPerEpochStore>,
        commit_index: CommitIndex,
    ) -> Self {
        Self {
            cache,
            epoch_store,
            horizon: commit_index.saturating_sub(K),
        }
    }

    /// Rules 1 and 3 for one owned input. Storage errors propagate. They are
    /// never a verdict.
    pub fn read_owned(&self, input: ObjectReference) -> IotaResult<OwnedVerdict> {
        // Rule 1: the row at the named version, deciding.
        let no_tables_entry =
            match OwnedReader::start(input, self.horizon).read_handler_row(self)? {
                TablesLookup::NeedBytes(need_bytes) => {
                    return Ok(OwnedVerdict::Keep(self.load_bytes(need_bytes)?));
                }
                TablesLookup::Missing(reason) => return Ok(OwnedVerdict::Missing(reason)),
                TablesLookup::Drop(reason) => return Ok(OwnedVerdict::Drop(reason)),
                TablesLookup::NoEntry(no_tables_entry) => no_tables_entry,
            };

        // Rule 3: ask the store, hold the answer, read the row again.
        match no_tables_entry.read_store(self)?.reread_handler_row(self)? {
            TablesRecheck::NeedBytes(need_bytes) => {
                Ok(OwnedVerdict::Keep(self.load_bytes(need_bytes)?))
            }
            TablesRecheck::Missing(reason) => Ok(OwnedVerdict::Missing(reason)),
            TablesRecheck::Drop(reason) => Ok(OwnedVerdict::Drop(reason)),
        }
    }

    /// The bytes at exactly `(id, V)` from the store. Reachable only with the
    /// machine's keep token. No bytes break the design's invariant, so that
    /// is a storage error and halts commit processing, never a verdict.
    fn load_bytes(&self, need_bytes: OwnedReader<NeedBytes>) -> IotaResult<KeptObject> {
        let key = need_bytes.into_key();
        if let Some(object) = self.cache.try_get_object_by_key(&key.0, key.1)? {
            return Ok(KeptObject(object));
        }
        Err(IotaError::Storage(format!(
            "kept input {key:?} has no bytes in the object store"
        )))
    }
}
