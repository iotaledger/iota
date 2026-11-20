// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{borrow::Cow, marker::PhantomData};

use async_graphql::{
    Object, ObjectType, OutputType, SimpleObject, TypeName,
    connection::{
        ConnectionNameType, CursorType, DefaultConnectionName, DefaultEdgeName, Edge, EdgeNameType,
        EmptyFields, EnableNodesField, NodesFieldSwitcherSealed, PageInfo,
    },
};
use iota_indexer::pruning::oldest_available_checkpoint::OldestAvailableCheckpoint;

use crate::types::uint53::UInt53;

/// The `oldestAvailableCheckpoint` field, to be added to a connection as its
/// additional fields.
#[derive(SimpleObject, Default)]
pub(crate) struct OldestAvailableCheckpointField {
    /// The oldest checkpoint this page can contain data from. Anything before
    /// it has been pruned and is not listed. Zero when nothing has been
    /// pruned, or when the historical fallback serves the whole history. Null
    /// when no bound applies to the page, such as a lookup by digest, or a
    /// page whose filters contradict each other.
    pub oldest_available_checkpoint: Option<UInt53>,
}

impl From<OldestAvailableCheckpoint> for OldestAvailableCheckpointField {
    fn from(oldest_available_checkpoint: OldestAvailableCheckpoint) -> Self {
        Self {
            oldest_available_checkpoint: Some(UInt53::new_unchecked(u64::from(
                oldest_available_checkpoint,
            ))),
        }
    }
}

/// Mirrors the `Connection` type from async-graphql, with the exception that if
/// `start_cursor` and/ or `end_cursor` is set on the struct, then when
/// `page_info` is called, it will use those values before deferring to `edges`.
/// The default implementation derives these cursors from the first and
/// last element of `edges`, so if `edges` is empty, both are set to null. This
/// is undesirable for queries that make use of `scan_limit`; when the scan
/// limit is reached, a caller can continue to paginate forwards or backwards
/// until all candidates in the scanning range have been visited, even if the
/// current page yields no results.
pub(crate) struct ScanConnection<
    Cursor,
    Node,
    ConnectionFields = EmptyFields,
    EdgeFields = EmptyFields,
    Name = DefaultConnectionName,
    EdgeName = DefaultEdgeName,
    NodesField = EnableNodesField,
> where
    Cursor: CursorType + Send + Sync,
    Node: OutputType,
    ConnectionFields: ObjectType,
    EdgeFields: ObjectType,
    Name: ConnectionNameType,
    EdgeName: EdgeNameType,
    NodesField: NodesFieldSwitcherSealed,
{
    _mark1: PhantomData<Name>,
    _mark2: PhantomData<EdgeName>,
    _mark3: PhantomData<NodesField>,
    pub edges: Vec<Edge<Cursor, Node, EdgeFields, EdgeName>>,
    pub has_previous_page: bool,
    pub has_next_page: bool,
    pub start_cursor: Option<String>,
    pub end_cursor: Option<String>,
    /// Additional fields, flattened into the connection.
    pub additional_fields: ConnectionFields,
}

#[Object(name_type)]
impl<Cursor, Node, ConnectionFields, EdgeFields, Name, EdgeName>
    ScanConnection<Cursor, Node, ConnectionFields, EdgeFields, Name, EdgeName, EnableNodesField>
where
    Cursor: CursorType + Send + Sync,
    Node: OutputType,
    ConnectionFields: ObjectType,
    EdgeFields: ObjectType,
    Name: ConnectionNameType,
    EdgeName: EdgeNameType,
{
    /// Information to aid in pagination.
    async fn page_info(&self) -> PageInfo {
        // Unlike the default implementation, this Connection will use `start_cursor`
        // and `end_cursor` if they are `Some`.
        PageInfo {
            has_previous_page: self.has_previous_page,
            has_next_page: self.has_next_page,
            start_cursor: self
                .start_cursor
                .clone()
                .or_else(|| self.edges.first().map(|edge| edge.cursor.encode_cursor())),
            end_cursor: self
                .end_cursor
                .clone()
                .or_else(|| self.edges.last().map(|edge| edge.cursor.encode_cursor())),
        }
    }

    /// A list of edges.
    #[inline]
    async fn edges(&self) -> &[Edge<Cursor, Node, EdgeFields, EdgeName>] {
        &self.edges
    }

    /// A list of nodes.
    async fn nodes(&self) -> Vec<&Node> {
        self.edges.iter().map(|e| &e.node).collect()
    }

    #[graphql(flatten)]
    #[inline]
    async fn additional_fields(&self) -> &ConnectionFields {
        &self.additional_fields
    }
}

impl<Cursor, Node, NodesField, ConnectionFields, EdgeFields, Name, EdgeName>
    ScanConnection<Cursor, Node, ConnectionFields, EdgeFields, Name, EdgeName, NodesField>
where
    Cursor: CursorType + Send + Sync,
    Node: OutputType,
    ConnectionFields: ObjectType + Default,
    EdgeFields: ObjectType,
    Name: ConnectionNameType,
    EdgeName: EdgeNameType,
    NodesField: NodesFieldSwitcherSealed,
{
    /// Create a new connection, with default additional fields.
    #[inline]
    pub fn new(has_previous_page: bool, has_next_page: bool) -> Self {
        ScanConnection {
            _mark1: PhantomData,
            _mark2: PhantomData,
            _mark3: PhantomData,
            edges: Vec::new(),
            has_previous_page,
            has_next_page,
            start_cursor: None,
            end_cursor: None,
            additional_fields: ConnectionFields::default(),
        }
    }
}

impl<Cursor, Node, ConnectionFields, EdgeFields, Name, EdgeName, NodesField> TypeName
    for ScanConnection<Cursor, Node, ConnectionFields, EdgeFields, Name, EdgeName, NodesField>
where
    Cursor: CursorType + Send + Sync,
    Node: OutputType,
    ConnectionFields: ObjectType,
    EdgeFields: ObjectType,
    Name: ConnectionNameType,
    EdgeName: EdgeNameType,
    NodesField: NodesFieldSwitcherSealed,
{
    #[inline]
    fn type_name() -> Cow<'static, str> {
        Name::type_name::<Node>().into()
    }
}
