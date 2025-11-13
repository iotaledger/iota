// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use async_graphql::{
    connection::{Connection, CursorType, Edge},
    *,
};
use iota_types::{
    base_types::{ObjectID, SequenceNumber},
    committee::{EpochId, ProtocolVersion},
    digests::TransactionDigest,
    object::Object as NativeObject,
    transaction::{
        AuthenticatorStateExpire as NativeAuthenticatorStateExpireTransaction,
        ChangeEpoch as NativeChangeEpochTransaction,
        ChangeEpochV2 as NativeChangeEpochTransactionV2,
        ChangeEpochV3 as NativeChangeEpochTransactionV3,
        ChangeEpochV4 as NativeChangeEpochTransactionV4,
        EndOfEpochTransactionKind as NativeEndOfEpochTransactionKind,
    },
};
use move_binary_format::{CompiledModule, errors::PartialVMResult};

use crate::{
    consistency::ConsistentIndexCursor,
    error::Error,
    types::{
        big_int::BigInt,
        cursor::{JsonCursor, Page},
        date_time::DateTime,
        epoch::Epoch,
        iota_address::IotaAddress,
        move_package::MovePackage,
        object::Object,
        uint53::UInt53,
    },
};

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct EndOfEpochTransaction {
    pub native: Vec<NativeEndOfEpochTransactionKind>,
    /// The checkpoint sequence number this was viewed at.
    pub checkpoint_viewed_at: u64,
}

#[derive(Union, Clone, PartialEq, Eq)]
pub(crate) enum EndOfEpochTransactionKind {
    ChangeEpoch(ChangeEpochTransaction),
    ChangeEpochV2(ChangeEpochTransactionV2),
    AuthenticatorStateCreate(AuthenticatorStateCreateTransaction),
    AuthenticatorStateExpire(AuthenticatorStateExpireTransaction),
}

// System transaction for advancing the epoch.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct ChangeEpochTransaction {
    pub native: NativeChangeEpochTransaction,
    /// The checkpoint sequence number this was viewed at.
    pub checkpoint_viewed_at: u64,
}

/// System transaction for advancing the epoch.
///
/// This represents all native `ChangeEpochTransaction{V2,V3,V4}` versions, by
/// using optional fields to extend on existing fields in a backward compatible
/// manner.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct ChangeEpochTransactionV2 {
    /// The next (to become) epoch ID.
    pub epoch: EpochId,
    /// The protocol version in effect in the new epoch.
    pub protocol_version: ProtocolVersion,
    /// The total amount of gas charged for storage during the epoch.
    pub storage_charge: u64,
    /// The total amount of gas charged for computation during the epoch.
    pub computation_charge: u64,
    /// The burned component of the total computation/execution costs.
    pub computation_charge_burned: u64,
    /// The amount of storage rebate refunded to the txn senders.
    pub storage_rebate: u64,
    /// The amount of storage rebate that is burnt due to the
    /// gas_price. It's given that storage_rebate + non_refundable_storage_fee
    /// is always equal to the storage_charge of the tx.
    pub non_refundable_storage_fee: u64,
    /// Unix timestamp from the start of the epoch as milliseconds
    pub epoch_start_timestamp_ms: u64,
    /// System packages (specifically framework and move stdlib) that are
    /// written by the execution of this transaction. Validators must write
    /// out the modules below.  Modules are provided with the version they
    /// will be upgraded to, their modules in serialized form (which include
    /// their package ID), and a list of their transitive dependencies.
    pub system_packages: Vec<(SequenceNumber, Vec<Vec<u8>>, Vec<ObjectID>)>,
    /// Vector of active validator indices eligible to take part in committee
    /// selection because they support the new, target protocol version.
    pub eligible_active_validators: Option<Vec<u64>>,
    /// Scores for the epoch being finalized. Each value corresponds to
    /// an authority, ordered by the ending epoch's AuthorityIndex.
    pub scores: Option<Vec<u64>>,
    /// The checkpoint sequence number this was viewed at.
    pub checkpoint_viewed_at: u64,
}

impl ChangeEpochTransactionV2 {
    pub fn new_with_native_v2(
        native: NativeChangeEpochTransactionV2,
        checkpoint_viewed_at: u64,
    ) -> Self {
        Self {
            epoch: native.epoch,
            protocol_version: native.protocol_version,
            storage_charge: native.storage_charge,
            computation_charge: native.computation_charge,
            computation_charge_burned: native.computation_charge_burned,
            storage_rebate: native.storage_rebate,
            non_refundable_storage_fee: native.non_refundable_storage_fee,
            epoch_start_timestamp_ms: native.epoch_start_timestamp_ms,
            system_packages: native.system_packages,
            eligible_active_validators: None,
            scores: None,
            checkpoint_viewed_at,
        }
    }

    pub fn new_with_native_v3(
        native: NativeChangeEpochTransactionV3,
        checkpoint_viewed_at: u64,
    ) -> Self {
        Self {
            epoch: native.epoch,
            protocol_version: native.protocol_version,
            storage_charge: native.storage_charge,
            computation_charge: native.computation_charge,
            computation_charge_burned: native.computation_charge_burned,
            storage_rebate: native.storage_rebate,
            non_refundable_storage_fee: native.non_refundable_storage_fee,
            epoch_start_timestamp_ms: native.epoch_start_timestamp_ms,
            system_packages: native.system_packages,
            eligible_active_validators: Some(native.eligible_active_validators),
            scores: None,
            checkpoint_viewed_at,
        }
    }

    pub fn new_with_native_v4(
        native: NativeChangeEpochTransactionV4,
        checkpoint_viewed_at: u64,
    ) -> Self {
        Self {
            epoch: native.epoch,
            protocol_version: native.protocol_version,
            storage_charge: native.storage_charge,
            computation_charge: native.computation_charge,
            computation_charge_burned: native.computation_charge_burned,
            storage_rebate: native.storage_rebate,
            non_refundable_storage_fee: native.non_refundable_storage_fee,
            epoch_start_timestamp_ms: native.epoch_start_timestamp_ms,
            system_packages: native.system_packages,
            eligible_active_validators: Some(native.eligible_active_validators),
            scores: Some(native.scores),
            checkpoint_viewed_at,
        }
    }
}

/// System transaction for creating the on-chain state used by zkLogin.
#[derive(SimpleObject, Clone, PartialEq, Eq)]
pub(crate) struct AuthenticatorStateCreateTransaction {
    /// A workaround to define an empty variant of a GraphQL union.
    #[graphql(name = "_")]
    dummy: Option<bool>,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct AuthenticatorStateExpireTransaction {
    pub native: NativeAuthenticatorStateExpireTransaction,
    /// The checkpoint sequence number this was viewed at.
    pub checkpoint_viewed_at: u64,
}

pub(crate) type CTxn = JsonCursor<ConsistentIndexCursor>;
pub(crate) type CPackage = JsonCursor<ConsistentIndexCursor>;

/// System transaction that supersedes `ChangeEpochTransaction` as the new way
/// to run transactions at the end of an epoch. Behaves similarly to
/// `ChangeEpochTransaction` but can accommodate other optional transactions to
/// run at the end of the epoch.
#[Object]
impl EndOfEpochTransaction {
    /// The list of system transactions that are allowed to run at the end of
    /// the epoch.
    async fn transactions(
        &self,
        ctx: &Context<'_>,
        first: Option<u64>,
        before: Option<CTxn>,
        last: Option<u64>,
        after: Option<CTxn>,
    ) -> Result<Connection<String, EndOfEpochTransactionKind>> {
        let page = Page::from_params(ctx.data_unchecked(), first, after, last, before)?;

        let mut connection = Connection::new(false, false);
        let Some(consistent_page) =
            page.paginate_consistent_indices(self.native.len(), self.checkpoint_viewed_at)?
        else {
            return Ok(connection);
        };

        connection.has_previous_page = consistent_page.has_previous_page;
        connection.has_next_page = consistent_page.has_next_page;

        for c in consistent_page.cursors {
            let tx = EndOfEpochTransactionKind::from(self.native[c.ix].clone(), c.c);
            connection.edges.push(Edge::new(c.encode_cursor(), tx));
        }

        Ok(connection)
    }
}

/// A system transaction that updates epoch information on-chain (increments the
/// current epoch). Executed by the system once per epoch, without using gas.
/// Epoch change transactions cannot be submitted by users, because validators
/// will refuse to sign them.
#[Object]
impl ChangeEpochTransaction {
    /// The next (to become) epoch.
    async fn epoch(&self, ctx: &Context<'_>) -> Result<Option<Epoch>> {
        Epoch::query(ctx, Some(self.native.epoch), self.checkpoint_viewed_at)
            .await
            .extend()
    }

    /// The protocol version in effect in the new epoch.
    async fn protocol_version(&self) -> UInt53 {
        self.native.protocol_version.as_u64().into()
    }

    /// The total amount of gas charged for storage during the previous epoch
    /// (in NANOS).
    async fn storage_charge(&self) -> BigInt {
        BigInt::from(self.native.storage_charge)
    }

    /// The total amount of gas charged for computation during the previous
    /// epoch (in NANOS).
    async fn computation_charge(&self) -> BigInt {
        BigInt::from(self.native.computation_charge)
    }

    /// The IOTA returned to transaction senders for cleaning up objects (in
    /// NANOS).
    async fn storage_rebate(&self) -> BigInt {
        BigInt::from(self.native.storage_rebate)
    }

    /// The total gas retained from storage fees, that will not be returned by
    /// storage rebates when the relevant objects are cleaned up (in NANOS).
    async fn non_refundable_storage_fee(&self) -> BigInt {
        BigInt::from(self.native.non_refundable_storage_fee)
    }

    /// Time at which the next epoch will start.
    async fn start_timestamp(&self) -> Result<DateTime, Error> {
        DateTime::from_ms(self.native.epoch_start_timestamp_ms as i64)
    }

    /// System packages (specifically framework and move stdlib) that are
    /// written before the new epoch starts, to upgrade them on-chain.
    /// Validators write these packages out when running the transaction.
    async fn system_packages(
        &self,
        ctx: &Context<'_>,
        first: Option<u64>,
        after: Option<CPackage>,
        last: Option<u64>,
        before: Option<CPackage>,
    ) -> Result<Connection<String, MovePackage>> {
        let page = Page::from_params(ctx.data_unchecked(), first, after, last, before)?;

        let mut connection = Connection::new(false, false);
        let Some(consistent_page) = page.paginate_consistent_indices(
            self.native.system_packages.len(),
            self.checkpoint_viewed_at,
        )?
        else {
            return Ok(connection);
        };

        connection.has_previous_page = consistent_page.has_previous_page;
        connection.has_next_page = consistent_page.has_next_page;

        for c in consistent_page.cursors {
            let (version, modules, deps) = &self.native.system_packages[c.ix];
            let compiled_modules = modules
                .iter()
                .map(|bytes| CompiledModule::deserialize_with_defaults(bytes))
                .collect::<PartialVMResult<Vec<_>>>()
                .map_err(|e| Error::Internal(format!("Failed to deserialize system modules: {e}")))
                .extend()?;

            let native = NativeObject::new_system_package(
                &compiled_modules,
                *version,
                deps.clone(),
                TransactionDigest::ZERO,
            );

            let runtime_id = native.id();
            let object = Object::from_native(IotaAddress::from(runtime_id), native, c.c, None);
            let package = MovePackage::try_from(&object)
                .map_err(|_| Error::Internal("Failed to create system package".to_string()))
                .extend()?;

            connection.edges.push(Edge::new(c.encode_cursor(), package));
        }

        Ok(connection)
    }
}

/// A system transaction that updates epoch information on-chain (increments the
/// current epoch). Executed by the system once per epoch, without using gas.
/// Epoch change transactions cannot be submitted by users, because validators
/// will refuse to sign them.
#[Object]
impl ChangeEpochTransactionV2 {
    /// The next (to become) epoch.
    async fn epoch(&self, ctx: &Context<'_>) -> Result<Option<Epoch>> {
        Epoch::query(ctx, Some(self.epoch), self.checkpoint_viewed_at)
            .await
            .extend()
    }

    /// The protocol version in effect in the new epoch.
    async fn protocol_version(&self) -> UInt53 {
        self.protocol_version.as_u64().into()
    }

    /// The total amount of gas charged for storage during the previous epoch
    /// (in NANOS).
    async fn storage_charge(&self) -> BigInt {
        BigInt::from(self.storage_charge)
    }

    /// The total amount of gas charged for computation during the previous
    /// epoch (in NANOS).
    async fn computation_charge(&self) -> BigInt {
        BigInt::from(self.computation_charge)
    }

    /// The total amount of gas burned for computation during the previous
    /// epoch (in NANOS).
    async fn computation_charge_burned(&self) -> BigInt {
        BigInt::from(self.computation_charge_burned)
    }

    /// The IOTA returned to transaction senders for cleaning up objects (in
    /// NANOS).
    async fn storage_rebate(&self) -> BigInt {
        BigInt::from(self.storage_rebate)
    }

    /// The total gas retained from storage fees, that will not be returned by
    /// storage rebates when the relevant objects are cleaned up (in NANOS).
    async fn non_refundable_storage_fee(&self) -> BigInt {
        BigInt::from(self.non_refundable_storage_fee)
    }

    /// Time at which the next epoch will start.
    async fn start_timestamp(&self) -> Result<DateTime, Error> {
        DateTime::from_ms(self.epoch_start_timestamp_ms as i64)
    }

    /// System packages (specifically framework and move stdlib) that are
    /// written before the new epoch starts, to upgrade them on-chain.
    /// Validators write these packages out when running the transaction.
    async fn system_packages(
        &self,
        ctx: &Context<'_>,
        first: Option<u64>,
        after: Option<CPackage>,
        last: Option<u64>,
        before: Option<CPackage>,
    ) -> Result<Connection<String, MovePackage>> {
        let page = Page::from_params(ctx.data_unchecked(), first, after, last, before)?;

        let mut connection = Connection::new(false, false);
        let Some(consistent_page) = page
            .paginate_consistent_indices(self.system_packages.len(), self.checkpoint_viewed_at)?
        else {
            return Ok(connection);
        };

        connection.has_previous_page = consistent_page.has_previous_page;
        connection.has_next_page = consistent_page.has_next_page;

        for c in consistent_page.cursors {
            let (version, modules, deps) = &self.system_packages[c.ix];
            let compiled_modules = modules
                .iter()
                .map(|bytes| CompiledModule::deserialize_with_defaults(bytes))
                .collect::<PartialVMResult<Vec<_>>>()
                .map_err(|e| Error::Internal(format!("Failed to deserialize system modules: {e}")))
                .extend()?;

            let native = NativeObject::new_system_package(
                &compiled_modules,
                *version,
                deps.clone(),
                TransactionDigest::ZERO,
            );

            let runtime_id = native.id();
            let object = Object::from_native(IotaAddress::from(runtime_id), native, c.c, None);
            let package = MovePackage::try_from(&object)
                .map_err(|_| Error::Internal("Failed to create system package".to_string()))
                .extend()?;

            connection.edges.push(Edge::new(c.encode_cursor(), package));
        }

        Ok(connection)
    }

    /// The list of active validators eligible for committee selection for the
    /// next epoch.
    async fn eligible_active_validators(&self) -> Option<Vec<BigInt>> {
        self.eligible_active_validators
            .as_ref()
            .map(|v| v.iter().map(|id| BigInt::from(*id)).collect())
    }

    /// The validator scores at the end of the epoch.
    async fn scores(&self) -> Option<Vec<BigInt>> {
        self.scores
            .as_ref()
            .map(|v| v.iter().map(|s| BigInt::from(*s)).collect())
    }
}

#[Object]
impl AuthenticatorStateExpireTransaction {
    /// Expire JWKs that have a lower epoch than this.
    async fn min_epoch(&self, ctx: &Context<'_>) -> Result<Option<Epoch>> {
        Epoch::query(ctx, Some(self.native.min_epoch), self.checkpoint_viewed_at)
            .await
            .extend()
    }

    /// The initial version that the AuthenticatorStateUpdateV1 was shared at.
    async fn authenticator_obj_initial_shared_version(&self) -> UInt53 {
        self.native
            .authenticator_obj_initial_shared_version
            .value()
            .into()
    }
}

impl EndOfEpochTransactionKind {
    fn from(kind: NativeEndOfEpochTransactionKind, checkpoint_viewed_at: u64) -> Self {
        use EndOfEpochTransactionKind as K;
        use NativeEndOfEpochTransactionKind as N;

        match kind {
            N::ChangeEpoch(ce) => K::ChangeEpoch(ChangeEpochTransaction {
                native: ce,
                checkpoint_viewed_at,
            }),
            N::ChangeEpochV2(ce) => K::ChangeEpochV2(ChangeEpochTransactionV2::new_with_native_v2(
                ce,
                checkpoint_viewed_at,
            )),
            N::ChangeEpochV3(ce) => K::ChangeEpochV2(ChangeEpochTransactionV2::new_with_native_v3(
                ce,
                checkpoint_viewed_at,
            )),
            N::ChangeEpochV4(ce) => K::ChangeEpochV2(ChangeEpochTransactionV2::new_with_native_v4(
                ce,
                checkpoint_viewed_at,
            )),
            N::AuthenticatorStateCreate => {
                K::AuthenticatorStateCreate(AuthenticatorStateCreateTransaction { dummy: None })
            }
            N::AuthenticatorStateExpire(ase) => {
                K::AuthenticatorStateExpire(AuthenticatorStateExpireTransaction {
                    native: ase,
                    checkpoint_viewed_at,
                })
            }
        }
    }
}
