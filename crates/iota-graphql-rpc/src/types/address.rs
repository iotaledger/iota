// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use async_graphql::{connection::Connection, *};

use crate::{
    config::DEFAULT_PAGE_SIZE,
    types::{
        balance::{self, Balance},
        coin::Coin,
        cursor::Page,
        iota_address::IotaAddress,
        iota_names_registration::{NameFormat, NameRegistration},
        move_object::MoveObject,
        object::{self, ObjectFilter},
        owner::OwnerImpl,
        stake::StakedIota,
        transaction_block::{
            self, TransactionBlock, TransactionBlockConnection, TransactionBlockFilter,
        },
        type_filter::ExactTypeFilter,
    },
};

#[derive(Clone, Debug, PartialEq, Eq, Copy)]
pub(crate) struct Address {
    pub address: IotaAddress,
    /// The checkpoint sequence number at which this was viewed at.
    pub checkpoint_viewed_at: u64,
}

/// The possible relationship types for a transaction block: sent, received,
/// or affected.
#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub(crate) enum AddressTransactionBlockRelationship {
    /// Transactions this address has sent.
    Sent,
    /// Transactions that sent objects to this address.
    Recv,
    /// Transactions that affected this address (the address is the sender,
    /// a recipient, or the owner of the gas payment).
    Affected,
}

/// The 32-byte address that is an account address (corresponding to a public
/// key).
#[Object]
impl Address {
    pub(crate) async fn address(&self) -> IotaAddress {
        OwnerImpl::from(self).address().await
    }

    /// Objects owned by this address, optionally `filter`-ed.
    pub(crate) async fn objects(
        &self,
        ctx: &Context<'_>,
        first: Option<u64>,
        after: Option<object::Cursor>,
        last: Option<u64>,
        before: Option<object::Cursor>,
        filter: Option<ObjectFilter>,
    ) -> Result<Connection<String, MoveObject>> {
        OwnerImpl::from(self)
            .objects(ctx, first, after, last, before, filter)
            .await
    }

    /// Total balance of all coins with marker type owned by this address. If
    /// type is not supplied, it defaults to `0x2::iota::IOTA`.
    pub(crate) async fn balance(
        &self,
        ctx: &Context<'_>,
        type_: Option<ExactTypeFilter>,
    ) -> Result<Option<Balance>> {
        OwnerImpl::from(self).balance(ctx, type_).await
    }

    /// The balances of all coin types owned by this address.
    pub(crate) async fn balances(
        &self,
        ctx: &Context<'_>,
        first: Option<u64>,
        after: Option<balance::Cursor>,
        last: Option<u64>,
        before: Option<balance::Cursor>,
    ) -> Result<Connection<String, Balance>> {
        OwnerImpl::from(self)
            .balances(ctx, first, after, last, before)
            .await
    }

    /// The coin objects for this address.
    ///
    /// `type` is a filter on the coin's type parameter, defaulting to
    /// `0x2::iota::IOTA`.
    pub(crate) async fn coins(
        &self,
        ctx: &Context<'_>,
        first: Option<u64>,
        after: Option<object::Cursor>,
        last: Option<u64>,
        before: Option<object::Cursor>,
        type_: Option<ExactTypeFilter>,
    ) -> Result<Connection<String, Coin>> {
        OwnerImpl::from(self)
            .coins(ctx, first, after, last, before, type_)
            .await
    }

    /// The `0x3::staking_pool::StakedIota` objects owned by this address.
    pub(crate) async fn staked_iotas(
        &self,
        ctx: &Context<'_>,
        first: Option<u64>,
        after: Option<object::Cursor>,
        last: Option<u64>,
        before: Option<object::Cursor>,
    ) -> Result<Connection<String, StakedIota>> {
        OwnerImpl::from(self)
            .staked_iotas(ctx, first, after, last, before)
            .await
    }

    /// The name explicitly configured as the default name pointing to this
    /// address.
    pub(crate) async fn iota_names_default_name(
        &self,
        ctx: &Context<'_>,
        format: Option<NameFormat>,
    ) -> Result<Option<String>> {
        OwnerImpl::from(self)
            .iota_names_default_name(ctx, format)
            .await
    }

    /// The NameRegistration NFTs owned by this address. These grant the
    /// owner the capability to manage the associated name.
    pub(crate) async fn iota_names_registrations(
        &self,
        ctx: &Context<'_>,
        first: Option<u64>,
        after: Option<object::Cursor>,
        last: Option<u64>,
        before: Option<object::Cursor>,
    ) -> Result<Connection<String, NameRegistration>> {
        OwnerImpl::from(self)
            .iota_names_registrations(ctx, first, after, last, before)
            .await
    }

    /// Similar behavior to the `transactionBlocks` in Query but supporting the
    /// additional `AddressTransactionBlockRelationship` filter, which
    /// defaults to `SENT`.
    #[graphql(
        complexity = "first.or(last).unwrap_or(DEFAULT_PAGE_SIZE as u64) as usize * child_complexity"
    )]
    async fn transaction_blocks(
        &self,
        ctx: &Context<'_>,
        first: Option<u64>,
        after: Option<transaction_block::Cursor>,
        last: Option<u64>,
        before: Option<transaction_block::Cursor>,
        relation: Option<AddressTransactionBlockRelationship>,
        filter: Option<TransactionBlockFilter>,
    ) -> Result<TransactionBlockConnection> {
        use AddressTransactionBlockRelationship as R;
        let page = Page::from_params(ctx.data_unchecked(), first, after, last, before)?;

        let Some(filter) = filter.unwrap_or_default().intersect(match relation {
            // Relationship defaults to "signer" if none is supplied.
            Some(R::Sent) | None => TransactionBlockFilter {
                sent_address: Some(self.address),
                ..Default::default()
            },

            Some(R::Recv) => TransactionBlockFilter {
                recv_address: Some(self.address),
                ..Default::default()
            },

            Some(R::Affected) => TransactionBlockFilter {
                affected_address: Some(self.address),
                ..Default::default()
            },
        }) else {
            return Ok(Connection::with_additional_fields(
                false,
                false,
                Default::default(),
            ));
        };

        TransactionBlock::paginate(ctx, page, filter, self.checkpoint_viewed_at)
            .await
            .extend()
    }
}

impl From<&Address> for OwnerImpl {
    fn from(address: &Address) -> Self {
        OwnerImpl {
            address: address.address,
            checkpoint_viewed_at: address.checkpoint_viewed_at,
        }
    }
}
