// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use async_graphql::{
    connection::{Connection, CursorType, Edge},
    *,
};
use iota_types::{
    digests::TransactionDigest,
    object::Object as NativeObject,
    transaction::{GenesisObject, GenesisTransaction as NativeGenesisTransaction},
};

use crate::{
    consistency::ConsistentIndexCursor,
    types::{
        cursor::{JsonCursor, Page},
        event::Event,
        iota_address::IotaAddress,
        object::Object,
    },
};

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct GenesisTransaction {
    pub native: NativeGenesisTransaction,
    /// The checkpoint sequence number this was viewed at.
    pub checkpoint_viewed_at: u64,
}

pub(crate) type CObject = JsonCursor<ConsistentIndexCursor>;
pub(crate) type CEvent = JsonCursor<ConsistentIndexCursor>;

/// System transaction that initializes the network and writes the initial set
/// of objects on-chain.
#[Object]
impl GenesisTransaction {
    /// Objects to be created during genesis.
    async fn objects(
        &self,
        ctx: &Context<'_>,
        first: Option<u64>,
        after: Option<CObject>,
        last: Option<u64>,
        before: Option<CObject>,
    ) -> Result<Connection<String, Object>> {
        let page = Page::from_params(ctx.data_unchecked(), first, after, last, before)?;

        let mut connection = Connection::new(false, false);
        let Some((prev, next, _, cursors)) =
            page.paginate_consistent_indices(self.native.objects.len(), self.checkpoint_viewed_at)?
        else {
            return Ok(connection);
        };

        connection.has_previous_page = prev;
        connection.has_next_page = next;

        for cursor in cursors {
            let GenesisObject::RawObject { data, owner } = self.native.objects[cursor.ix].clone();
            let native =
                NativeObject::new_from_genesis(data, owner, TransactionDigest::genesis_marker());

            let object =
                Object::from_native(IotaAddress::from(native.id()), native, cursor.c, None);
            connection
                .edges
                .push(Edge::new(cursor.encode_cursor(), object));
        }

        Ok(connection)
    }

    /// Events emitted during genesis.
    async fn events(
        &self,
        ctx: &Context<'_>,
        first: Option<u64>,
        after: Option<CEvent>,
        last: Option<u64>,
        before: Option<CEvent>,
    ) -> Result<Connection<String, Event>> {
        let page = Page::from_params(ctx.data_unchecked(), first, after, last, before)?;

        let mut connection = Connection::new(false, false);
        let Some((prev, next, _, cursors)) =
            page.paginate_consistent_indices(self.native.events.len(), self.checkpoint_viewed_at)?
        else {
            return Ok(connection);
        };

        connection.has_previous_page = prev;
        connection.has_next_page = next;

        for cursor in cursors {
            let native_event = self.native.events[cursor.ix].clone();

            let event = Event {
                stored: None,
                native: native_event,
                checkpoint_viewed_at: cursor.c,
            };

            connection
                .edges
                .push(Edge::new(cursor.encode_cursor(), event));
        }

        Ok(connection)
    }
}
