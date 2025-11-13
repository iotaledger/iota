// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeSet;

use async_graphql::InputObject;
use iota_types::base_types::IotaAddress as NativeIotaAddress;

use crate::types::{
    digest::Digest, intersect, iota_address::IotaAddress,
    transaction_block::TransactionBlockKindInput, type_filter::FqNameFilter, uint53::UInt53,
};

/// Represents optional available filters for transaction blocks.
#[derive(InputObject, Debug, Default, Clone)]
pub(crate) struct TransactionBlockFilter {
    /// Filter transactions by move function called.
    ///
    /// Calls can be filtered by the `package`, `package::module`, or the
    /// `package::module::name` of their function.
    pub function: Option<FqNameFilter>,

    /// An input filter selecting for either system or programmable
    /// transactions.
    pub kind: Option<TransactionBlockKindInput>,
    /// Limit to transactions that occurred strictly after the given checkpoint.
    pub after_checkpoint: Option<UInt53>,
    /// Limit to transactions in the given checkpoint.
    pub at_checkpoint: Option<UInt53>,
    /// Limit to transaction that occurred strictly before the given checkpoint.
    pub before_checkpoint: Option<UInt53>,
    /// Limit to transactions that were sent by the given address. NOTE: this
    /// input filter has been deprecated in favor of `sentAddress` which has
    /// clearer semantics. Both filters restrict transactions by their sender,
    /// only, not signers in general.
    ///
    /// This filter will be removed after 6 months with the 1.24.0 release.
    #[graphql(
        deprecation = "Misleading semantics. Use `sentAddress` instead. This will be removed with the 1.24.0 release."
    )]
    pub sign_address: Option<IotaAddress>,
    /// Limit to transactions that were sent by the given address.
    pub sent_address: Option<IotaAddress>,
    /// Limit to transactions that sent an object to the given address.
    pub recv_address: Option<IotaAddress>,
    /// Limit to transactions that accepted the given object as an input.
    pub input_object: Option<IotaAddress>,
    /// Limit to transactions that output a version of this object.
    pub changed_object: Option<IotaAddress>,
    /// Limit to transactions that wrapped or deleted the given object.
    pub wrapped_or_deleted_object: Option<IotaAddress>,
    /// Select transactions by their digest.
    pub transaction_ids: Option<Vec<Digest>>,
}

impl TransactionBlockFilter {
    /// Try to create a filter whose results are the intersection of transaction
    /// blocks in `self`'s results and transaction blocks in `other`'s
    /// results. This may not be possible if the resulting filter is
    /// inconsistent in some way (e.g. a filter that requires one field to be
    /// two different values simultaneously).
    pub(crate) fn intersect(self, other: Self) -> Option<Self> {
        macro_rules! intersect {
            ($field:ident, $body:expr) => {
                intersect::field(self.$field, other.$field, $body)
            };
        }

        Some(Self {
            function: intersect!(function, FqNameFilter::intersect)?,
            kind: intersect!(kind, intersect::by_eq)?,

            after_checkpoint: intersect!(after_checkpoint, intersect::by_max)?,
            at_checkpoint: intersect!(at_checkpoint, intersect::by_eq)?,
            before_checkpoint: intersect!(before_checkpoint, intersect::by_min)?,

            sign_address: intersect!(sign_address, intersect::by_eq)?,
            sent_address: intersect!(sent_address, intersect::by_eq)?,
            recv_address: intersect!(recv_address, intersect::by_eq)?,
            input_object: intersect!(input_object, intersect::by_eq)?,
            changed_object: intersect!(changed_object, intersect::by_eq)?,
            wrapped_or_deleted_object: intersect!(wrapped_or_deleted_object, intersect::by_eq)?,

            transaction_ids: intersect!(transaction_ids, |a, b| {
                let a = BTreeSet::from_iter(a.into_iter());
                let b = BTreeSet::from_iter(b.into_iter());
                Some(a.intersection(&b).cloned().collect())
            })?,
        })
    }

    /// Most filter conditions require a scan limit if used in tandem with other
    /// filters. The exception to this is sender and checkpoint, since
    /// sender is denormalized on all tables, and the corresponding tx range
    /// can be determined for a checkpoint.
    pub(crate) fn requires_scan_limit(&self) -> bool {
        [
            self.function.is_some(),
            self.kind.is_some(),
            self.recv_address.is_some(),
            self.input_object.is_some(),
            self.changed_object.is_some(),
            self.wrapped_or_deleted_object.is_some(),
            self.transaction_ids.is_some(),
        ]
        .into_iter()
        .filter(|is_set| *is_set)
        .count()
            > 1
    }

    /// Returns the transaction sender to query `tx_sender`.
    ///
    /// If there are other filters set that would query tables with a `sender`
    /// column, then this returns `None`.
    pub(crate) fn explicit_sender(&self) -> Option<IotaAddress> {
        if self.function.is_none()
            && self.kind.is_none()
            && self.recv_address.is_none()
            && self.input_object.is_none()
            && self.changed_object.is_none()
            && self.wrapped_or_deleted_object.is_none()
        {
            self.sent_address.or(self.sign_address)
        } else {
            None
        }
    }

    /// A TransactionBlockFilter is considered not to have any filters if no
    /// filters are specified, or if the only filters are on `checkpoint`.
    pub(crate) fn has_filters(&self) -> bool {
        self.function.is_some()
            || self.kind.is_some()
            || self.sign_address.is_some()
            || self.sent_address.is_some()
            || self.recv_address.is_some()
            || self.input_object.is_some()
            || self.changed_object.is_some()
            || self.wrapped_or_deleted_object.is_some()
            || self.transaction_ids.is_some()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.before_checkpoint == Some(UInt53::from(0))
            || matches!(
                (self.after_checkpoint, self.before_checkpoint),
                (Some(after), Some(before)) if after >= before
            )
            || matches!(
                (self.after_checkpoint, self.at_checkpoint),
                (Some(after), Some(at)) if after >= at
            )
            || matches!(
                (self.at_checkpoint, self.before_checkpoint),
                (Some(at), Some(before)) if at >= before
            )
            // If SystemTx, sender if specified must be 0x0. Conversely, if sender is 0x0, kind must be SystemTx.
            || matches!(
                (self.kind, self.sent_address.or(self.sign_address)),
                (Some(kind), Some(signer))
                    if (kind == TransactionBlockKindInput::SystemTx)
                        != (signer == IotaAddress::from(NativeIotaAddress::ZERO))
            )
            // Temporary while we deprecate `sign_address` in favor of `sent_address`.
            || matches!(
                (self.sign_address, self.sent_address),
                (Some(signer), Some(sent))
                    if signer != sent
            )
    }
}
