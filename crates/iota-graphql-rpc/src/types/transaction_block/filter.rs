// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeSet;

use async_graphql::InputObject;
use iota_sdk_types::Address as NativeAddress;

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
    /// Limit to transactions that were sent by the given address.
    pub sent_address: Option<IotaAddress>,
    /// Limit to transactions that sent an object to the given address.
    pub recv_address: Option<IotaAddress>,
    /// Limit to transactions that affected the given address (the address is
    /// the sender, a recipient, or the owner of the gas payment).
    pub affected_address: Option<IotaAddress>,
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
    /// Infer whether the provided filter is unsupported.
    ///
    /// We reserve this flag for filter combinations that are either too complex
    /// to serve, or might not make sense (e.g. rich queries on system
    /// transactions).
    pub(crate) fn is_unsupported(&self) -> bool {
        self.is_unsupported_kind_combo()
    }

    fn is_unsupported_kind_combo(&self) -> bool {
        self.kind.is_some() && self.scan_count() > 1
    }

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

            sent_address: intersect!(sent_address, intersect::by_eq)?,
            recv_address: intersect!(recv_address, intersect::by_eq)?,
            affected_address: intersect!(affected_address, intersect::by_eq)?,
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

    /// The number of set filters that force a separate lookup table read.
    ///
    /// For example, `tx_recipients` for `recv_address` or `tx_kinds` for
    /// `kind`.
    ///
    /// Combining two or more of them takes a scan over an unknown range of
    /// transactions on each lookup table.
    ///
    /// Combining the remaining filters does not have this effect:
    ///
    /// * `sent_address` is available as a denormalized column on the other
    ///   filters' tables, and is served by `tx_senders` only when set on its
    ///   own.
    /// * `{after,at,before}_checkpoint` bound the range of transactions each
    ///   read is confined to.
    /// * `transaction_ids` matches at most one transaction per digest given.
    fn scan_count(&self) -> usize {
        [
            self.function.is_some(),
            self.kind.is_some(),
            self.recv_address.is_some(),
            self.affected_address.is_some(),
            self.input_object.is_some(),
            self.changed_object.is_some(),
            self.wrapped_or_deleted_object.is_some(),
        ]
        .into_iter()
        .filter(|is_set| *is_set)
        .count()
    }

    /// A scan limit is required once more than one filter has to be scanned
    /// (see [`Self::scan_count`]).
    pub(crate) fn requires_scan_limit(&self) -> bool {
        self.scan_count() > 1
    }

    /// Returns the transaction sender to query `tx_sender`.
    ///
    /// If there are other filters set that would query tables with a `sender`
    /// column, then this returns `None`.
    pub(crate) fn explicit_sender(&self) -> Option<IotaAddress> {
        if self.scan_count() == 0 {
            self.sent_address
        } else {
            None
        }
    }

    /// A TransactionBlockFilter is considered not to have any filters if no
    /// filters are specified, or if the only filters are on `checkpoint`.
    pub(crate) fn has_filters(&self) -> bool {
        self.function.is_some()
            || self.kind.is_some()
            || self.sent_address.is_some()
            || self.recv_address.is_some()
            || self.affected_address.is_some()
            || self.input_object.is_some()
            || self.changed_object.is_some()
            || self.wrapped_or_deleted_object.is_some()
            || self.transaction_ids.is_some()
    }

    /// Returns the checkpoint sequence number when `at_checkpoint` is the
    /// only filter set.
    pub(crate) fn only_at_checkpoint(&self) -> Option<UInt53> {
        if self.has_filters() || self.after_checkpoint.is_some() || self.before_checkpoint.is_some()
        {
            return None;
        }
        self.at_checkpoint
    }

    /// Returns the affected address when `affected_address` is the only
    /// filter set.
    pub(crate) fn only_affected_address(&self) -> Option<IotaAddress> {
        if self.function.is_some()
            || self.kind.is_some()
            || self.sent_address.is_some()
            || self.recv_address.is_some()
            || self.input_object.is_some()
            || self.changed_object.is_some()
            || self.wrapped_or_deleted_object.is_some()
            || self.transaction_ids.is_some()
            || self.after_checkpoint.is_some()
            || self.at_checkpoint.is_some()
            || self.before_checkpoint.is_some()
        {
            return None;
        }
        self.affected_address
    }

    /// Returns the transaction digests when `transactionIds` is the only
    /// filter set.
    pub(crate) fn only_transaction_ids(&self) -> Option<&Vec<Digest>> {
        let no_other_filters = self.function.is_none()
            && self.kind.is_none()
            && self.sent_address.is_none()
            && self.recv_address.is_none()
            && self.affected_address.is_none()
            && self.input_object.is_none()
            && self.changed_object.is_none()
            && self.wrapped_or_deleted_object.is_none()
            && self.after_checkpoint.is_none()
            && self.at_checkpoint.is_none()
            && self.before_checkpoint.is_none();

        no_other_filters
            .then_some(self.transaction_ids.as_ref())
            .flatten()
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
                (self.kind, self.sent_address),
                (Some(kind), Some(sender))
                    if (kind == TransactionBlockKindInput::SystemTx)
                        != (sender == IotaAddress::from(NativeAddress::ZERO))
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::type_filter::ModuleFilter;

    impl TransactionBlockFilter {
        const OBJECT: IotaAddress = IotaAddress([2; 32]);
        const SENDER: IotaAddress = IotaAddress([1; 32]);

        /// One filter per scanned table.
        fn scanned() -> Vec<Self> {
            vec![
                Self {
                    function: Some(FqNameFilter::ByModule(ModuleFilter::ByPackage(
                        Self::OBJECT,
                    ))),
                    ..Default::default()
                },
                Self {
                    kind: Some(TransactionBlockKindInput::SystemTx),
                    ..Default::default()
                },
                Self {
                    recv_address: Some(Self::SENDER),
                    ..Default::default()
                },
                Self {
                    input_object: Some(Self::OBJECT),
                    ..Default::default()
                },
                Self {
                    changed_object: Some(Self::OBJECT),
                    ..Default::default()
                },
                Self {
                    wrapped_or_deleted_object: Some(Self::OBJECT),
                    ..Default::default()
                },
            ]
        }

        /// Every combination of two distinct scanned filters.
        fn scanned_pairs() -> Vec<Self> {
            let scanned = Self::scanned();
            scanned
                .iter()
                .enumerate()
                .flat_map(|(i, a)| {
                    scanned[i + 1..]
                        .iter()
                        .map(|b| a.clone().intersect(b.clone()).unwrap())
                })
                .collect()
        }

        /// Every filter that is not scanned, in one value.
        fn unscanned() -> Self {
            Self {
                sent_address: Some(Self::SENDER),
                after_checkpoint: Some(UInt53::from(1)),
                at_checkpoint: Some(UInt53::from(5)),
                before_checkpoint: Some(UInt53::from(10)),
                transaction_ids: Some(vec![]),
                ..Default::default()
            }
        }
    }

    #[test]
    fn scan_count_counts_scanned_filters_only() {
        assert_eq!(TransactionBlockFilter::default().scan_count(), 0);
        assert_eq!(TransactionBlockFilter::unscanned().scan_count(), 0);

        for filter in TransactionBlockFilter::scanned() {
            let filter = filter
                .intersect(TransactionBlockFilter::unscanned())
                .unwrap();
            assert_eq!(filter.scan_count(), 1);
        }

        for filter in TransactionBlockFilter::scanned_pairs() {
            assert_eq!(filter.scan_count(), 2);
        }
    }

    #[test]
    fn scan_limit_required_beyond_one_scan() {
        assert!(!TransactionBlockFilter::unscanned().requires_scan_limit());

        for filter in TransactionBlockFilter::scanned() {
            let filter = filter
                .intersect(TransactionBlockFilter::unscanned())
                .unwrap();
            assert!(!filter.requires_scan_limit());
        }

        for filter in TransactionBlockFilter::scanned_pairs() {
            assert!(filter.requires_scan_limit());
        }
    }

    #[test]
    fn kind_unsupported_only_alongside_another_scan() {
        for filter in TransactionBlockFilter::scanned() {
            let filter = filter
                .intersect(TransactionBlockFilter::unscanned())
                .unwrap();
            assert!(!filter.is_unsupported());
        }

        for filter in TransactionBlockFilter::scanned_pairs() {
            assert_eq!(filter.is_unsupported(), filter.kind.is_some());
        }
    }

    #[test]
    fn explicit_sender_dropped_by_any_scan() {
        let unscanned = TransactionBlockFilter::unscanned();
        assert_eq!(
            unscanned.explicit_sender(),
            Some(TransactionBlockFilter::SENDER)
        );

        for filter in TransactionBlockFilter::scanned() {
            let filter = filter.intersect(unscanned.clone()).unwrap();
            assert_eq!(filter.explicit_sender(), None);
        }
    }
}
