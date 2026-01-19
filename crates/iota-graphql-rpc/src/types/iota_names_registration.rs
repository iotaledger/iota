// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::str::FromStr;

use async_graphql::{connection::Connection, *};
use iota_indexer::models::objects::StoredHistoryObject;
use iota_names::{
    IotaNamesNft, config::IotaNamesConfig, error::IotaNamesError, name::Name as NativeName,
    registry::NameRecord,
};
use iota_types::{base_types::IotaAddress as NativeIotaAddress, dynamic_field::Field, id::UID};
use move_core_types::language_storage::StructTag;
use serde::{Deserialize, Serialize};

use super::{
    available_range::AvailableRange,
    balance::{self, Balance},
    base64::Base64,
    big_int::BigInt,
    checkpoint::Checkpoint,
    coin::Coin,
    cursor::Page,
    display::DisplayEntry,
    dynamic_field::{DynamicField, DynamicFieldName},
    iota_address::IotaAddress,
    move_object::{MoveObject, MoveObjectImpl},
    move_value::MoveValue,
    object::{self, Object, ObjectFilter, ObjectImpl, ObjectOwner, ObjectStatus},
    owner::OwnerImpl,
    stake::StakedIota,
    string_input::impl_string_input,
    transaction_block::{self, TransactionBlock, TransactionBlockFilter},
    type_filter::ExactTypeFilter,
    uint53::UInt53,
};
use crate::{
    config::DEFAULT_PAGE_SIZE,
    connection::ScanConnection,
    consistency::{View, build_objects_query},
    data::{Db, DbConnection, QueryExecutor},
    error::Error,
};

/// Represents the "core" of the name service (e.g. the on-chain registry and
/// reverse registry). It doesn't contain any fields because we look them up
/// based on the `IotaNamesConfig`.
pub(crate) struct IotaNames;

/// Wrap IOTA-Names Name type to expose as a string scalar in GraphQL.
#[derive(Debug)]
pub(crate) struct Name(NativeName);

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
#[graphql(remote = "iota_names::name::NameFormat")]
pub enum NameFormat {
    At,
    Dot,
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct NativeNameRegistration {
    pub id: UID,
    pub name: NativeName,
    pub name_str: String,
    pub expiration_timestamp_ms: u64,
}

#[derive(Clone)]
pub(crate) struct NameRegistration {
    /// Representation of this NameRegistration as a generic Move object.
    pub super_: MoveObject,

    /// The deserialized representation of the Move object's contents.
    pub native: NativeNameRegistration,
}

/// Represents the results of a query for a name's `NameRecord` and its
/// parent's `NameRecord`. The `expiration_timestamp_ms` on the name records are
/// compared to the checkpoint's timestamp to check that the name is not
/// expired.
pub(crate) struct NameExpiration {
    /// The name's `NameRecord`.
    pub name_record: Option<NameRecord>,
    /// The parent's `NameRecord`, populated only if the name is a subname.
    pub parent_name_record: Option<NameRecord>,
    /// The timestamp of the checkpoint at which the query was made. This is
    /// used to check if the `expiration_timestamp_ms` on the name records
    /// are expired.
    pub checkpoint_timestamp_ms: u64,
}

pub(crate) enum NameRegistrationDowncastError {
    NotAnNameRegistration,
    Bcs(bcs::Error),
}

#[Object]
impl NameRegistration {
    pub(crate) async fn address(&self) -> IotaAddress {
        OwnerImpl::from(&self.super_.super_).address().await
    }

    /// Objects owned by this object, optionally `filter`-ed.
    pub(crate) async fn objects(
        &self,
        ctx: &Context<'_>,
        first: Option<u64>,
        after: Option<object::Cursor>,
        last: Option<u64>,
        before: Option<object::Cursor>,
        filter: Option<ObjectFilter>,
    ) -> Result<Connection<String, MoveObject>> {
        OwnerImpl::from(&self.super_.super_)
            .objects(ctx, first, after, last, before, filter)
            .await
    }

    /// Total balance of all coins with marker type owned by this object. If
    /// type is not supplied, it defaults to `0x2::iota::IOTA`.
    pub(crate) async fn balance(
        &self,
        ctx: &Context<'_>,
        type_: Option<ExactTypeFilter>,
    ) -> Result<Option<Balance>> {
        OwnerImpl::from(&self.super_.super_)
            .balance(ctx, type_)
            .await
    }

    /// The balances of all coin types owned by this object.
    pub(crate) async fn balances(
        &self,
        ctx: &Context<'_>,
        first: Option<u64>,
        after: Option<balance::Cursor>,
        last: Option<u64>,
        before: Option<balance::Cursor>,
    ) -> Result<Connection<String, Balance>> {
        OwnerImpl::from(&self.super_.super_)
            .balances(ctx, first, after, last, before)
            .await
    }

    /// The coin objects for this object.
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
        OwnerImpl::from(&self.super_.super_)
            .coins(ctx, first, after, last, before, type_)
            .await
    }

    /// The `0x3::staking_pool::StakedIota` objects owned by this object.
    pub(crate) async fn staked_iotas(
        &self,
        ctx: &Context<'_>,
        first: Option<u64>,
        after: Option<object::Cursor>,
        last: Option<u64>,
        before: Option<object::Cursor>,
    ) -> Result<Connection<String, StakedIota>> {
        OwnerImpl::from(&self.super_.super_)
            .staked_iotas(ctx, first, after, last, before)
            .await
    }

    /// The name explicitly configured as the default name pointing to this
    /// object.
    pub(crate) async fn iota_names_default_name(
        &self,
        ctx: &Context<'_>,
        format: Option<NameFormat>,
    ) -> Result<Option<String>> {
        OwnerImpl::from(&self.super_.super_)
            .iota_names_default_name(ctx, format)
            .await
    }

    /// The NameRegistration NFTs owned by this object. These grant the
    /// owner the capability to manage the associated name.
    pub(crate) async fn iota_names_registrations(
        &self,
        ctx: &Context<'_>,
        first: Option<u64>,
        after: Option<object::Cursor>,
        last: Option<u64>,
        before: Option<object::Cursor>,
    ) -> Result<Connection<String, NameRegistration>> {
        OwnerImpl::from(&self.super_.super_)
            .iota_names_registrations(ctx, first, after, last, before)
            .await
    }

    pub(crate) async fn version(&self) -> UInt53 {
        ObjectImpl(&self.super_.super_).version().await
    }

    /// The current status of the object as read from the off-chain store. The
    /// possible states are:
    /// - NOT_INDEXED: The object is loaded from serialized data, such as the
    ///   contents of a genesis or system package upgrade transaction.
    /// - INDEXED: The object is retrieved from the off-chain index and
    ///   represents the most recent or historical state of the object.
    /// - WRAPPED_OR_DELETED: The object is deleted or wrapped and only partial
    ///   information can be loaded.
    pub(crate) async fn status(&self) -> ObjectStatus {
        ObjectImpl(&self.super_.super_).status().await
    }

    /// 32-byte hash that identifies the object's contents, encoded as a Base58
    /// string.
    pub(crate) async fn digest(&self) -> Option<String> {
        ObjectImpl(&self.super_.super_).digest().await
    }

    /// The owner type of this object: Immutable, Shared, Parent, Address
    pub(crate) async fn owner(&self, ctx: &Context<'_>) -> Option<ObjectOwner> {
        ObjectImpl(&self.super_.super_).owner(ctx).await
    }

    /// The transaction block that created this version of the object.
    pub(crate) async fn previous_transaction_block(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Option<TransactionBlock>> {
        ObjectImpl(&self.super_.super_)
            .previous_transaction_block(ctx)
            .await
    }

    /// The amount of IOTA we would rebate if this object gets deleted or
    /// mutated. This number is recalculated based on the present storage
    /// gas price.
    pub(crate) async fn storage_rebate(&self) -> Option<BigInt> {
        ObjectImpl(&self.super_.super_).storage_rebate().await
    }

    /// The transaction blocks that sent objects to this object.
    ///
    /// `scanLimit` restricts the number of candidate transactions scanned when
    /// gathering a page of results. It is required for queries that apply
    /// more than two complex filters (on function, kind, sender, recipient,
    /// input object, changed object, or ids), and can be at most
    /// `serviceConfig.maxScanLimit`.
    ///
    /// When the scan limit is reached the page will be returned even if it has
    /// fewer than `first` results when paginating forward (`last` when
    /// paginating backwards). If there are more transactions to scan,
    /// `pageInfo.hasNextPage` (or `pageInfo.hasPreviousPage`) will be set to
    /// `true`, and `PageInfo.endCursor` (or `PageInfo.startCursor`) will be set
    /// to the last transaction that was scanned as opposed to the last (or
    /// first) transaction in the page.
    ///
    /// Requesting the next (or previous) page after this cursor will resume the
    /// search, scanning the next `scanLimit` many transactions in the
    /// direction of pagination, and so on until all transactions in the
    /// scanning range have been visited.
    ///
    /// By default, the scanning range includes all transactions known to
    /// GraphQL, but it can be restricted by the `after` and `before`
    /// cursors, and the `beforeCheckpoint`, `afterCheckpoint` and
    /// `atCheckpoint` filters.
    #[graphql(
        complexity = "first.or(last).unwrap_or(DEFAULT_PAGE_SIZE as u64) as usize * child_complexity"
    )]
    pub(crate) async fn received_transaction_blocks(
        &self,
        ctx: &Context<'_>,
        first: Option<u64>,
        after: Option<transaction_block::Cursor>,
        last: Option<u64>,
        before: Option<transaction_block::Cursor>,
        filter: Option<TransactionBlockFilter>,
        scan_limit: Option<u64>,
    ) -> Result<ScanConnection<String, TransactionBlock>> {
        ObjectImpl(&self.super_.super_)
            .received_transaction_blocks(ctx, first, after, last, before, filter, scan_limit)
            .await
    }

    /// The Base64-encoded BCS serialization of the object's content.
    pub(crate) async fn bcs(&self) -> Result<Option<Base64>> {
        ObjectImpl(&self.super_.super_).bcs().await
    }

    /// Displays the contents of the Move object in a JSON string and through
    /// GraphQL types. Also provides the flat representation of the type
    /// signature, and the BCS of the corresponding data.
    pub(crate) async fn contents(&self) -> Option<MoveValue> {
        MoveObjectImpl(&self.super_).contents().await
    }

    /// Determines whether a transaction can transfer this object, using the
    /// TransferObjects transaction command or
    /// `iota::transfer::public_transfer`, both of which require the object to
    /// have the `key` and `store` abilities.
    pub(crate) async fn has_public_transfer(&self, ctx: &Context<'_>) -> Result<bool> {
        MoveObjectImpl(&self.super_).has_public_transfer(ctx).await
    }

    /// The set of named templates defined on-chain for the type of this object,
    /// to be handled off-chain. The server substitutes data from the object
    /// into these templates to generate a display string per template.
    pub(crate) async fn display(&self, ctx: &Context<'_>) -> Result<Option<Vec<DisplayEntry>>> {
        ObjectImpl(&self.super_.super_).display(ctx).await
    }

    /// Access a dynamic field on an object using its name. Names are arbitrary
    /// Move values whose type have `copy`, `drop`, and `store`, and are
    /// specified using their type, and their BCS contents, Base64 encoded.
    ///
    /// Dynamic fields on wrapped objects can be accessed by using the same API
    /// under the Owner type.
    pub(crate) async fn dynamic_field(
        &self,
        ctx: &Context<'_>,
        name: DynamicFieldName,
    ) -> Result<Option<DynamicField>> {
        OwnerImpl::from(&self.super_.super_)
            .dynamic_field(ctx, name, Some(self.super_.root_version()))
            .await
    }

    /// Access a dynamic object field on an object using its name. Names are
    /// arbitrary Move values whose type have `copy`, `drop`, and `store`,
    /// and are specified using their type, and their BCS contents, Base64
    /// encoded. The value of a dynamic object field can also be accessed
    /// off-chain directly via its address (e.g. using `Query.object`).
    ///
    /// Dynamic fields on wrapped objects can be accessed by using the same API
    /// under the Owner type.
    pub(crate) async fn dynamic_object_field(
        &self,
        ctx: &Context<'_>,
        name: DynamicFieldName,
    ) -> Result<Option<DynamicField>> {
        OwnerImpl::from(&self.super_.super_)
            .dynamic_object_field(ctx, name, Some(self.super_.root_version()))
            .await
    }

    /// The dynamic fields and dynamic object fields on an object.
    ///
    /// Dynamic fields on wrapped objects can be accessed by using the same API
    /// under the Owner type.
    pub(crate) async fn dynamic_fields(
        &self,
        ctx: &Context<'_>,
        first: Option<u64>,
        after: Option<object::Cursor>,
        last: Option<u64>,
        before: Option<object::Cursor>,
    ) -> Result<Connection<String, DynamicField>> {
        OwnerImpl::from(&self.super_.super_)
            .dynamic_fields(
                ctx,
                first,
                after,
                last,
                before,
                Some(self.super_.root_version()),
            )
            .await
    }

    /// Name of the NameRegistration object
    async fn name(&self) -> &str {
        &self.native.name_str
    }
}

impl IotaNames {
    /// Lookup the IOTA-Names NameRecord for the given `name`. `config`
    /// specifies where to find the name registry, and its type.
    ///
    /// `checkpoint_viewed_at` represents the checkpoint sequence number at
    /// which this was queried for.
    ///
    /// The `NameRecord` is returned only if it has not expired as of the
    /// `checkpoint_viewed_at` or latest checkpoint's timestamp.
    ///
    /// For leaf names, the `NameRecord` is returned only if its parent is
    /// valid and not expired.
    pub(crate) async fn resolve_to_record(
        ctx: &Context<'_>,
        name: &Name,
        checkpoint_viewed_at: u64,
    ) -> Result<Option<NameRecord>, Error> {
        // Query for the name's NameRecord and parent NameRecord if applicable. The
        // checkpoint's timestamp is also fetched. These values are used to
        // determine if the name is expired.
        let Some(name_expiration) =
            Self::query_name_expiration(ctx, name, checkpoint_viewed_at).await?
        else {
            return Ok(None);
        };

        // Get the name_record from the query. If we didn't find it, we return as it
        // means that the requested name is not registered.
        let Some(name_record) = name_expiration.name_record else {
            return Ok(None);
        };

        // If name record is SLN, or Node subname, we can check the expiration and
        // return the record if not expired.
        if !name_record.is_leaf_record() {
            return if !name_record.is_node_expired(name_expiration.checkpoint_timestamp_ms) {
                Ok(Some(name_record))
            } else {
                Err(Error::IotaNames(IotaNamesError::NameExpired))
            };
        }

        // If we cannot find the parent, then the name is expired.
        let Some(parent_name_record) = name_expiration.parent_name_record else {
            return Err(Error::IotaNames(IotaNamesError::NameExpired));
        };

        // If the parent is valid for this leaf, and not expired, then we can return the
        // name record. Otherwise, the name is expired.
        if parent_name_record.is_valid_leaf_parent(&name_record)
            && !parent_name_record.is_node_expired(name_expiration.checkpoint_timestamp_ms)
        {
            Ok(Some(name_record))
        } else {
            Err(Error::IotaNames(IotaNamesError::NameExpired))
        }
    }

    /// Lookup the IOTA-Names Name for the given `address`. `config` specifies
    /// where to find the name registry, and its type.
    ///
    /// `checkpoint_viewed_at` represents the checkpoint sequence number at
    /// which this was queried for.
    pub(crate) async fn reverse_resolve_to_name(
        ctx: &Context<'_>,
        address: IotaAddress,
        checkpoint_viewed_at: u64,
    ) -> Result<Option<NativeName>, Error> {
        let config: &IotaNamesConfig = ctx.data_unchecked();
        let native_address = NativeIotaAddress::from(address);
        let reverse_record_id = config.reverse_record_field_id(&native_address);

        let Some(object) = MoveObject::query(
            ctx,
            reverse_record_id.into(),
            Object::latest_at(checkpoint_viewed_at),
        )
        .await?
        else {
            return Ok(None);
        };

        let field: Field<NativeIotaAddress, NativeName> = object
            .native
            .to_rust()
            .ok_or_else(|| Error::Internal("Malformed IOTA-Names Name".to_string()))?;

        let name = Name(field.value);

        // We attempt to resolve the name to a record, and if it fails, we return
        // None. That way we can validate that the name has not expired and is
        // still valid.
        let Some(_) = Self::resolve_to_record(ctx, &name, checkpoint_viewed_at).await? else {
            return Ok(None);
        };

        Ok(Some(name.0))
    }

    /// Query for a name's NameRecord, its parent's NameRecord if supplied,
    /// and the timestamp of the checkpoint bound.
    async fn query_name_expiration(
        ctx: &Context<'_>,
        name: &Name,
        checkpoint_viewed_at: u64,
    ) -> Result<Option<NameExpiration>, Error> {
        let config: &IotaNamesConfig = ctx.data_unchecked();
        let db: &Db = ctx.data_unchecked();
        // Construct the list of `object_id`s to look up. The first element is the
        // name's `NameRecord`. If the name is a subname, there will be a
        // second element for the parent's `NameRecord`.
        let mut object_ids = vec![IotaAddress::from(config.record_field_id(&name.0))];
        if name.0.is_subname() {
            let parent = name.0.parent().ok_or(Error::Internal(
                "Subname does not have a parent name".to_string(),
            ))?;
            object_ids.push(IotaAddress::from(config.record_field_id(&parent)));
        }

        // Create a page with a bound of `object_ids` length to fetch the relevant
        // `NameRecord`s.
        let page: Page<object::Cursor> = Page::from_params(
            ctx.data_unchecked(),
            Some(object_ids.len() as u64),
            None,
            None,
            None,
        )
        .map_err(|_| {
            Error::Internal("Page size of 2 is incompatible with configured limits".to_string())
        })?;

        // prepare the filter for the query.
        let filter = ObjectFilter {
            object_ids: Some(object_ids.clone()),
            ..Default::default()
        };

        let Some((checkpoint_timestamp_ms, results)) = db
            .execute_repeatable(move |conn| {
                let Some(range) = AvailableRange::result(conn, checkpoint_viewed_at)? else {
                    return Ok::<_, diesel::result::Error>(None);
                };

                let timestamp_ms = Checkpoint::query_timestamp(conn, checkpoint_viewed_at)?;

                let sql = build_objects_query(
                    View::Consistent,
                    range,
                    &page,
                    move |query| filter.apply(query),
                    move |newer| newer,
                );

                let objects: Vec<StoredHistoryObject> =
                    conn.results(move || sql.clone().into_boxed())?;

                Ok(Some((timestamp_ms, objects)))
            })
            .await?
        else {
            return Err(Error::Client(
                "Requested data is outside the available range".to_string(),
            ));
        };

        let mut name_expiration = NameExpiration {
            parent_name_record: None,
            name_record: None,
            checkpoint_timestamp_ms,
        };

        // Max size of results is 2. We loop through them, convert to objects, and then
        // parse name_record. We then assign it to the correct field on
        // `name_expiration` based on the address.
        for result in results {
            let object =
                Object::try_from_stored_history_object(result, checkpoint_viewed_at, None)?;
            let move_object = MoveObject::try_from(&object).map_err(|_| {
                Error::Internal(format!(
                    "Expected {0} to be a NameRecord, but it's not a Move Object.",
                    object.address
                ))
            })?;

            let record = NameRecord::try_from(move_object.native)?;

            if object.address == object_ids[0] {
                name_expiration.name_record = Some(record);
            } else if Some(&object.address) == object_ids.get(1) {
                name_expiration.parent_name_record = Some(record);
            }
        }

        Ok(Some(name_expiration))
    }
}

impl NameRegistration {
    /// Query the database for a `page` of IOTA-Names registrations. The page
    /// uses the same cursor type as is used for `Object`, and is further
    /// filtered to a particular `owner`. `config` specifies where to find
    /// the name registry and its type.
    ///
    /// `checkpoint_viewed_at` represents the checkpoint sequence number at
    /// which this page was queried for. Each entity returned in the
    /// connection will inherit this checkpoint, so that when viewing that
    /// entity's state, it will be as if it was read at the same checkpoint.
    pub(crate) async fn paginate(
        db: &Db,
        config: &IotaNamesConfig,
        page: Page<object::Cursor>,
        owner: IotaAddress,
        checkpoint_viewed_at: u64,
    ) -> Result<Connection<String, NameRegistration>, Error> {
        let type_ = NameRegistration::type_(config.package_address.into());

        let filter = ObjectFilter {
            type_: Some(type_.clone().into()),
            owner: Some(owner),
            ..Default::default()
        };

        Object::paginate_subtype(db, page, filter, checkpoint_viewed_at, |object| {
            let address = object.address;
            let move_object = MoveObject::try_from(&object).map_err(|_| {
                Error::Internal(format!(
                    "Expected {address} to be a NameRegistration, but it's not a Move Object.",
                ))
            })?;

            NameRegistration::try_from(&move_object, &type_).map_err(|_| {
                Error::Internal(format!(
                    "Expected {address} to be a NameRegistration, but it is not."
                ))
            })
        })
        .await
    }

    /// Return the type representing a `NameRegistration` on chain. This
    /// can change from chain to chain (mainnet, testnet, devnet etc).
    pub(crate) fn type_(package: IotaAddress) -> StructTag {
        iota_names::NameRegistration::type_(package.into())
    }

    // Because the type of the NameRegistration object is not constant,
    // we need to take it in as a param.
    pub(crate) fn try_from(
        move_object: &MoveObject,
        tag: &StructTag,
    ) -> Result<Self, NameRegistrationDowncastError> {
        if !move_object.native.is_type(tag) {
            return Err(NameRegistrationDowncastError::NotAnNameRegistration);
        }

        Ok(Self {
            super_: move_object.clone(),
            native: bcs::from_bytes(move_object.native.contents())
                .map_err(NameRegistrationDowncastError::Bcs)?,
        })
    }
}

impl_string_input!(Name);

impl FromStr for Name {
    type Err = <NativeName as FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Name(NativeName::from_str(s)?))
    }
}
