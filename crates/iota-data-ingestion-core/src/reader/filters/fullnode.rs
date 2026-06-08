// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Server-side filters for fullnode gRPC connections.
//!
//! Filters tell the fullnode which transactions to include in each
//! checkpoint payload. Filtering happens entirely on the server-side, the
//! ingestion framework performs no client-side filtering.
//!
//! # Filter types
//!
//! - [`TransactionFilter`]: applied to a checkpoint's transactions.
//! - [`EventFilter`]: matches a transaction by its emitted events. Used as
//!   input to [`TransactionFilter::event`].
//! - [`CommandFilter`]: matches a command within a programmable transaction.
//!   Used as input to [`TransactionFilter::command`].
//!
//! # Composition
//!
//! [`TransactionFilter`] and [`EventFilter`] use a chained builder pattern:
//! each leaf method (e.g. `TransactionFilter::new().kinds().sender()`)
//! implicitly `AND`s the new condition with everything accumulated so far.
//! Explicit `OR` composition is available via the [`or`](TransactionFilter::or)
//! method on each type. `NOT` is available via
//! [`negate`](TransactionFilter::negate), the `!` operator, or `.not()` when
//! [`std::ops::Not`] is in scope.
//!
//! [`CommandFilter`] is composed only at the [`TransactionFilter`] level by
//! passing each command to [`TransactionFilter::command`] within a chain.
//! Multiple [`command`](TransactionFilter::command) calls implicitly `AND`,
//! while `OR` and `NOT` follow the rules described above.
//!
//! # Examples
//!
//! Successful programmable transactions:
//!
//! ```rust
//! use iota_data_ingestion_core::filters::fullnode::{TransactionFilter, TransactionKind};
//!
//! let filter = TransactionFilter::new()
//!     .kinds([TransactionKind::Programmable])
//!     .execution_status(true);
//! ```
//!
//! Transactions emitting a specific event type:
//!
//! ```rust
//! use iota_data_ingestion_core::filters::fullnode::{EventFilter, TransactionFilter};
//!
//! let filter =
//!     TransactionFilter::new().event(EventFilter::new().event_type("0xabcd::my_module::Foo"));
//! ```
//!
//! Transactions NOT sent by a specific address:
//!
//! ```ignore
//! let filter = !TransactionFilter::new().sender(some_address);
//! ```

use std::ops::Not;

use iota_grpc_types::v1::{filter as proto, types::ObjectReference};
use iota_sdk_types::ObjectId;
use iota_types::base_types::{IotaAddress, ObjectDigest, SequenceNumber};

/// Available transaction kinds for filtering.
pub type TransactionKind = proto::TransactionKind;

/// Filter applied to transactions in a fullnode checkpoint stream.
///
/// Built with a chained builder pattern. Start from [`TransactionFilter::new`]
/// and add leaves with the named methods (e.g. [`TransactionFilter::kinds`],
/// [`TransactionFilter::sender`], [`TransactionFilter::execution_status`]).
///
/// Each leaf implicitly `AND`s with everything accumulated so far. Use
/// [`TransactionFilter::or`] for `OR` composition and
/// [`TransactionFilter::negate`] (or the `!` operator) for `NOT`.
///
/// # Example
///
/// Implicit AND composition
///
/// ```rust
/// use iota_data_ingestion_core::filters::fullnode::{TransactionFilter, TransactionKind};
///
/// let filter = TransactionFilter::new()
///     .kinds([TransactionKind::Programmable])
///     .execution_status(true);
/// ```
/// Composition with AND, OR and NOT:
///
/// ```ignore
/// // (Programmable AND success AND sender == Alice) OR NOT(receiver == Bob)
/// let filter = TransactionFilter::new()
///     .kinds([TransactionKind::Programmable])
///     .execution_status(true)
///     .sender(alice)
///     .or(!TransactionFilter::new().receiver(bob));
/// ```
#[derive(Clone, Debug, Default)]
pub struct TransactionFilter(proto::TransactionFilter);

impl TransactionFilter {
    /// Creates an empty filter.
    ///
    /// An empty filter is rejected by the fullnode. Add at least one leaf via
    /// the builder methods (e.g. [`TransactionFilter::kinds`],
    /// [`TransactionFilter::sender`]) before passing the filter to the
    /// ingestion framework.
    ///
    /// # Example
    ///
    /// ```rust
    /// use iota_data_ingestion_core::filters::fullnode::{TransactionFilter, TransactionKind};
    ///
    /// let filter = TransactionFilter::new().kinds([TransactionKind::Programmable]);
    /// ```
    pub fn new() -> Self {
        Self::default()
    }

    /// Pushes a leaf into self via implicit `AND`, flattening when self is
    /// already an `All`.
    fn and_with(self, leaf: proto::TransactionFilter) -> Self {
        match self.0.filter {
            None => Self(leaf),
            Some(proto::transaction_filter::Filter::All(mut all)) => {
                all.filters.push(leaf);
                Self(proto::TransactionFilter::default().with_all(all))
            }
            Some(_) => {
                Self(proto::TransactionFilter::default().with_all(
                    proto::AllTransactionFilter::default().with_filters(vec![self.0, leaf]),
                ))
            }
        }
    }

    /// Combines self with `leaf` via `OR`, flattening when self is already an
    /// `Any`.
    fn or_with(self, leaf: proto::TransactionFilter) -> Self {
        match self.0.filter {
            None => Self(leaf),
            Some(proto::transaction_filter::Filter::Any(mut any)) => {
                any.filters.push(leaf);
                Self(proto::TransactionFilter::default().with_any(any))
            }
            Some(_) => {
                Self(proto::TransactionFilter::default().with_any(
                    proto::AnyTransactionFilter::default().with_filters(vec![self.0, leaf]),
                ))
            }
        }
    }

    /// Matches transactions of any of the given [`TransactionKind`]s.
    ///
    /// Passing an empty iterator produces a filter that matches no
    /// transactions (it is accepted by the fullnode but yields no results).
    /// Pass at least one [`TransactionKind`].
    pub fn kinds(self, kinds: impl IntoIterator<Item = TransactionKind>) -> Self {
        let transaction_kinds_filter =
            kinds
                .into_iter()
                .fold(proto::TransactionKindsFilter::default(), |mut acc, kind| {
                    acc.push_kinds(kind);
                    acc
                });

        self.and_with(
            proto::TransactionFilter::default().with_transaction_kinds(transaction_kinds_filter),
        )
    }

    /// Matches transactions by execution status.
    ///
    /// - `true` for successful transactions.
    /// - `false` for failed transactions.
    pub fn execution_status(self, success: bool) -> Self {
        self.and_with(
            proto::TransactionFilter::default().with_execution_status(
                proto::ExecutionStatusFilter::default().with_success(success),
            ),
        )
    }

    /// Matches transactions sent by the given address.
    pub fn sender(self, address: IotaAddress) -> Self {
        self.and_with(
            proto::TransactionFilter::default()
                .with_sender(proto::AddressFilter::default().with_address(address)),
        )
    }

    /// Matches transactions whose recipient is the given address.
    pub fn receiver(self, address: IotaAddress) -> Self {
        self.and_with(
            proto::TransactionFilter::default()
                .with_receiver(proto::AddressFilter::default().with_address(address)),
        )
    }

    /// Matches transactions that touch the given object id.
    pub fn affected_object(self, object_id: ObjectId) -> Self {
        let object_ref = ObjectReference::default().with_object_id(object_id);
        self.and_with(
            proto::TransactionFilter::default()
                .with_affected_object(proto::ObjectIdFilter::default().with_object_ref(object_ref)),
        )
    }

    /// Matches transactions that touch the given object id and version.
    pub fn affected_object_version(self, object_id: ObjectId, version: SequenceNumber) -> Self {
        let object_ref = ObjectReference::default()
            .with_object_id(object_id)
            .with_version(version.as_u64());
        self.and_with(
            proto::TransactionFilter::default()
                .with_affected_object(proto::ObjectIdFilter::default().with_object_ref(object_ref)),
        )
    }

    /// Matches transactions that touch the given object digest.
    pub fn affected_object_digest(self, object_digest: ObjectDigest) -> Self {
        let object_ref = ObjectReference::default().with_digest(object_digest);
        self.and_with(
            proto::TransactionFilter::default()
                .with_affected_object(proto::ObjectIdFilter::default().with_object_ref(object_ref)),
        )
    }

    /// Matches transactions containing a command that satisfies the given
    /// [`CommandFilter`].
    pub fn command(self, filter: CommandFilter) -> Self {
        self.and_with(proto::TransactionFilter::default().with_command(filter))
    }

    /// Matches transactions that contain at least one event satisfying the
    /// given [`EventFilter`].
    pub fn event(self, filter: EventFilter) -> Self {
        self.and_with(proto::TransactionFilter::default().with_event(filter))
    }

    /// Logical `OR` with another filter.
    pub fn or(self, other: Self) -> Self {
        self.or_with(other.0)
    }

    /// Logical `NOT` of this filter.
    ///
    /// Equivalent to `!self` (via [`std::ops::Not`]).
    ///
    /// # Scoping
    ///
    /// `negate` wraps the entire filter accumulated so far, not just the most
    /// recent leaf. This follows from the chained builder pattern: each step
    /// transforms `self` into a new accumulated filter, and `negate` always
    /// operates on whatever `self` represents at that point.
    ///
    /// The two snippets below produce different filters because of where
    /// `negate` is placed in the chain:
    ///
    /// ```ignore
    /// // (NOT kinds) AND sender(alice)
    /// TransactionFilter::new()
    ///     .kinds([TransactionKind::Programmable])
    ///     .negate()
    ///     .sender(alice);
    ///
    /// // NOT (kinds AND sender(alice))
    /// TransactionFilter::new()
    ///     .kinds([TransactionKind::Programmable])
    ///     .sender(alice)
    ///     .negate();
    /// ```
    ///
    /// For complex expressions where the scoping is not visually obvious,
    /// build sub-filters as named bindings and combine them with
    /// [`TransactionFilter::or`].
    pub fn negate(self) -> Self {
        Self(
            proto::TransactionFilter::default()
                .with_negation(proto::NotTransactionFilter::default().with_filter(self.0)),
        )
    }
}

impl Not for TransactionFilter {
    type Output = Self;
    fn not(self) -> Self {
        self.negate()
    }
}

impl From<TransactionFilter> for proto::TransactionFilter {
    fn from(value: TransactionFilter) -> Self {
        value.0
    }
}

/// Filter for commands within a programmable transaction.
///
/// Used as input to [`TransactionFilter::command`].
#[derive(Clone, Debug)]
pub struct CommandFilter(proto::CommandFilter);

impl CommandFilter {
    /// Matches any `MoveCall` to the given package.
    pub fn move_call(package_id: ObjectId) -> Self {
        Self(
            proto::CommandFilter::default().with_move_call(
                proto::MoveCallCommandFilter::default().with_package_id(package_id),
            ),
        )
    }

    /// Matches any `MoveCall` to the given package and module.
    pub fn move_call_in_module(package_id: ObjectId, module: impl Into<String>) -> Self {
        Self(
            proto::CommandFilter::default().with_move_call(
                proto::MoveCallCommandFilter::default()
                    .with_package_id(package_id)
                    .with_module(module),
            ),
        )
    }

    /// Matches a specific `MoveCall` to the given package, module and
    /// function.
    pub fn move_call_to(
        package_id: ObjectId,
        module: impl Into<String>,
        function: impl Into<String>,
    ) -> Self {
        Self(
            proto::CommandFilter::default().with_move_call(
                proto::MoveCallCommandFilter::default()
                    .with_package_id(package_id)
                    .with_module(module)
                    .with_function(function),
            ),
        )
    }

    /// Matches any `TransferObjects` command.
    pub fn transfer_objects() -> Self {
        Self(
            proto::CommandFilter::default()
                .with_transfer_objects(proto::TransferObjectsCommandFilter::default()),
        )
    }

    /// Matches any `SplitCoins` command.
    pub fn split_coins() -> Self {
        Self(
            proto::CommandFilter::default()
                .with_split_coins(proto::SplitCoinsCommandFilter::default()),
        )
    }

    /// Matches any `MergeCoins` command.
    pub fn merge_coins() -> Self {
        Self(
            proto::CommandFilter::default()
                .with_merge_coins(proto::MergeCoinsCommandFilter::default()),
        )
    }

    /// Matches any `Publish` command.
    pub fn publish() -> Self {
        Self(proto::CommandFilter::default().with_publish(proto::PublishCommandFilter::default()))
    }

    /// Matches any `MakeMoveVec` command.
    pub fn make_move_vec() -> Self {
        Self(
            proto::CommandFilter::default()
                .with_make_move_vec(proto::MakeMoveVecCommandFilter::default()),
        )
    }

    /// Matches any `Upgrade` command.
    pub fn upgrade() -> Self {
        Self(proto::CommandFilter::default().with_upgrade(proto::UpgradeCommandFilter::default()))
    }

    /// Matches an `Upgrade` command for the given package.
    pub fn upgrade_of(package_id: ObjectId) -> Self {
        Self(
            proto::CommandFilter::default()
                .with_upgrade(proto::UpgradeCommandFilter::default().with_package_id(package_id)),
        )
    }
}

impl From<CommandFilter> for proto::CommandFilter {
    fn from(value: CommandFilter) -> Self {
        value.0
    }
}

/// Filter for events emitted by transactions.
///
/// Used as input to [`TransactionFilter::event`] to match transactions that
/// contain at least one event satisfying this filter.
///
/// Built with a chained builder pattern. Start from [`EventFilter::new`] and
/// add leaves with the named methods (e.g. [`EventFilter::event_type`],
/// [`EventFilter::sender`], [`EventFilter::emitted_in`]).
///
/// Each leaf implicitly `AND`s with everything accumulated so far. Use
/// [`EventFilter::or`] for `OR` composition and [`EventFilter::negate`] (or the
/// `!` operator) for `NOT`.
///
/// # Example
///
/// ```rust
/// use iota_data_ingestion_core::filters::fullnode::EventFilter;
///
/// let filter = EventFilter::new().event_type("0xabcd::my_module::Foo");
/// ```
///
/// More complex composition with OR and NOT
///
/// ```rust,ignore
/// use iota_data_ingestion_core::filters::fullnode::EventFilter;
///
/// // NOT (sender == Alice OR sender == Bob)
/// let filter = EventFilter::new()
///     .sender(alice)
///     .or(EventFilter::new().sender(bob))
///     .negate();
/// ```
#[derive(Clone, Debug, Default)]
pub struct EventFilter(proto::EventFilter);

impl EventFilter {
    /// Creates an empty filter.
    ///
    /// An empty filter is rejected by the fullnode. Add at least one leaf via
    /// the builder methods (e.g. [`EventFilter::event_type`]) before passing
    /// the filter to the ingestion framework.
    ///
    /// # Example
    ///
    /// ```rust
    /// use iota_data_ingestion_core::filters::fullnode::EventFilter;
    ///
    /// let filter = EventFilter::new().event_type("0xabcd::my_module::Foo");
    /// ```
    pub fn new() -> Self {
        Self::default()
    }

    fn and_with(self, leaf: proto::EventFilter) -> Self {
        match self.0.filter {
            None => Self(leaf),
            Some(proto::event_filter::Filter::All(mut all)) => {
                all.filters.push(leaf);
                Self(proto::EventFilter::default().with_all(all))
            }
            Some(_) => Self(
                proto::EventFilter::default()
                    .with_all(proto::AllEventFilter::default().with_filters(vec![self.0, leaf])),
            ),
        }
    }

    fn or_with(self, leaf: proto::EventFilter) -> Self {
        match self.0.filter {
            None => Self(leaf),
            Some(proto::event_filter::Filter::Any(mut any)) => {
                any.filters.push(leaf);
                Self(proto::EventFilter::default().with_any(any))
            }
            Some(_) => Self(
                proto::EventFilter::default()
                    .with_any(proto::AnyEventFilter::default().with_filters(vec![self.0, leaf])),
            ),
        }
    }

    /// Matches events whose enclosing transaction was sent by the given
    /// address.
    pub fn sender(self, address: IotaAddress) -> Self {
        self.and_with(
            proto::EventFilter::default()
                .with_sender(proto::AddressFilter::default().with_address(address)),
        )
    }

    /// Matches events emitted by a transaction whose top-level `MoveCall`
    /// targets the given package.
    ///
    /// This matches the package the event was *emitted from*,
    /// not where the event struct is defined. For the latter, use
    /// [`EventFilter::defined_in`] / [`EventFilter::defined_in_module`].
    pub fn emitted_in(self, package_id: ObjectId) -> Self {
        self.and_with(proto::EventFilter::default().with_move_package_and_module(
            proto::MovePackageAndModuleFilter::default().with_package_id(package_id),
        ))
    }

    /// Matches events emitted by a transaction whose top-level `MoveCall`
    /// targets the given package and module.
    ///
    /// This matches the package and module the event was *emitted from*,
    /// not where the event struct is defined. For the latter, use
    /// [`EventFilter::defined_in`] / [`EventFilter::defined_in_module`].
    pub fn emitted_in_module(self, package_id: ObjectId, module: impl Into<String>) -> Self {
        self.and_with(
            proto::EventFilter::default().with_move_package_and_module(
                proto::MovePackageAndModuleFilter::default()
                    .with_package_id(package_id)
                    .with_module(module),
            ),
        )
    }

    /// Matches events whose struct is defined in the given package.
    ///
    /// This matches the package the event struct is *defined
    /// in*, not where it was emitted from. For the latter, use
    /// [`EventFilter::emitted_in`] / [`EventFilter::emitted_in_module`].
    pub fn defined_in(self, package_id: ObjectId) -> Self {
        self.and_with(
            proto::EventFilter::default().with_move_event_package_and_module(
                proto::MovePackageAndModuleFilter::default().with_package_id(package_id),
            ),
        )
    }

    /// Matches events whose struct is defined in the given package and module.
    ///
    /// This matches the package and module the event struct is *defined
    /// in*, not where it was emitted from. For the latter, use
    /// [`EventFilter::emitted_in`] / [`EventFilter::emitted_in_module`].
    pub fn defined_in_module(self, package_id: ObjectId, module: impl Into<String>) -> Self {
        self.and_with(
            proto::EventFilter::default().with_move_event_package_and_module(
                proto::MovePackageAndModuleFilter::default()
                    .with_package_id(package_id)
                    .with_module(module),
            ),
        )
    }

    /// Matches events with the given Move event struct tag (e.g.
    /// `"0xabcd::my_module::Foo"`).
    pub fn event_type(self, struct_tag: impl Into<String>) -> Self {
        self.and_with(proto::EventFilter::default().with_move_event_type(
            proto::MoveEventTypeFilter::default().with_struct_tag(struct_tag),
        ))
    }

    /// Logical `OR` with another filter.
    pub fn or(self, other: Self) -> Self {
        self.or_with(other.0)
    }

    /// Logical `NOT` of this filter.
    ///
    /// Equivalent to `!self` (via [`std::ops::Not`]).
    ///
    /// # Scoping
    ///
    /// `negate` wraps the entire filter accumulated so far, not just the most
    /// recent leaf. This follows from the chained builder pattern: each step
    /// transforms `self` into a new accumulated filter, and `negate` always
    /// operates on whatever `self` represents at that point.
    ///
    /// The two snippets below produce different filters because of where
    /// `negate` is placed in the chain:
    ///
    /// ```rust,ignore
    /// use iota_data_ingestion_core::filters::fullnode::EventFilter;
    ///
    /// // (NOT event_type Foo) AND sender(alice)
    /// EventFilter::new()
    ///     .event_type("0xabcd::my_module::Foo")
    ///     .negate()
    ///     .sender(alice);
    ///
    /// // NOT (event_type Foo AND sender(alice))
    /// EventFilter::new()
    ///     .event_type("0xabcd::my_module::Foo")
    ///     .sender(alice)
    ///     .negate();
    /// ```
    ///
    /// For complex expressions where the scoping is not visually obvious,
    /// build sub-filters as named bindings and combine them with
    /// [`EventFilter::or`].
    pub fn negate(self) -> Self {
        Self(
            proto::EventFilter::default()
                .with_negation(proto::NotEventFilter::default().with_filter(self.0)),
        )
    }
}

impl Not for EventFilter {
    type Output = Self;
    fn not(self) -> Self {
        self.negate()
    }
}

impl From<EventFilter> for proto::EventFilter {
    fn from(value: EventFilter) -> Self {
        value.0
    }
}

#[cfg(test)]
mod tests {
    use crate::reader::filters::fullnode::*;

    #[test]
    fn and_chain_flattens() {
        let f = TransactionFilter::new()
            .kinds([TransactionKind::Programmable])
            .execution_status(true)
            .sender(IotaAddress::ZERO);

        let expected = proto::TransactionFilter::default().with_all(
            proto::AllTransactionFilter::default().with_filters(vec![
                proto::TransactionFilter::default().with_transaction_kinds({
                    let mut k = proto::TransactionKindsFilter::default();
                    k.push_kinds(proto::TransactionKind::Programmable);
                    k
                }),
                proto::TransactionFilter::default().with_execution_status(
                    proto::ExecutionStatusFilter::default().with_success(true),
                ),
                proto::TransactionFilter::default()
                    .with_sender(proto::AddressFilter::default().with_address(IotaAddress::ZERO)),
            ]),
        );

        assert_eq!(proto::TransactionFilter::from(f), expected);
    }

    #[test]
    fn or_chain_flattens() {
        let f = TransactionFilter::new()
            .sender(IotaAddress::ZERO)
            .or(TransactionFilter::new().sender(IotaAddress::ZERO))
            .or(TransactionFilter::new().sender(IotaAddress::ZERO));

        let expected = proto::TransactionFilter::default().with_any(
            proto::AnyTransactionFilter::default().with_filters(vec![
                proto::TransactionFilter::default()
                    .with_sender(proto::AddressFilter::default().with_address(IotaAddress::ZERO)),
                proto::TransactionFilter::default()
                    .with_sender(proto::AddressFilter::default().with_address(IotaAddress::ZERO)),
                proto::TransactionFilter::default()
                    .with_sender(proto::AddressFilter::default().with_address(IotaAddress::ZERO)),
            ]),
        );

        assert_eq!(proto::TransactionFilter::from(f), expected);
    }

    #[test]
    fn negation_wraps_filter() {
        let f = !TransactionFilter::new().sender(IotaAddress::ZERO);

        let expected = proto::TransactionFilter::default().with_negation(
            proto::NotTransactionFilter::default().with_filter(
                proto::TransactionFilter::default()
                    .with_sender(proto::AddressFilter::default().with_address(IotaAddress::ZERO)),
            ),
        );

        assert_eq!(proto::TransactionFilter::from(f), expected);
    }

    #[test]
    fn complex_nested_composition_matches_proto() {
        let pkg = ObjectId::ZERO;
        let alice = IotaAddress::ZERO;
        let bob = IotaAddress::ZERO;

        // (Programmable AND success AND command(MoveCall pkg::events))
        // OR (sender(alice) AND event(event_type OR emitted_in_module))
        // OR NOT(receiver(bob))
        let wrapper = TransactionFilter::new()
            .kinds([TransactionKind::Programmable])
            .execution_status(true)
            .command(CommandFilter::move_call_in_module(pkg, "events"))
            .or(TransactionFilter::new().sender(alice).event(
                EventFilter::new()
                    .event_type("0x1::events::Foo")
                    .or(EventFilter::new().emitted_in_module(pkg, "events")),
            ))
            .or(!TransactionFilter::new().receiver(bob));

        let expected = proto::TransactionFilter::default().with_any(
            proto::AnyTransactionFilter::default().with_filters(vec![
                // Branch 1: ALL [kinds, success, command]
                proto::TransactionFilter::default().with_all(
                    proto::AllTransactionFilter::default().with_filters(vec![
                        proto::TransactionFilter::default().with_transaction_kinds({
                            let mut k = proto::TransactionKindsFilter::default();
                            k.push_kinds(proto::TransactionKind::Programmable);
                            k
                        }),
                        proto::TransactionFilter::default().with_execution_status(
                            proto::ExecutionStatusFilter::default().with_success(true),
                        ),
                        proto::TransactionFilter::default().with_command(
                            proto::CommandFilter::default().with_move_call(
                                proto::MoveCallCommandFilter::default()
                                    .with_package_id(pkg)
                                    .with_module("events"),
                            ),
                        ),
                    ]),
                ),
                // Branch 2: ALL [sender, event(Any[event_type, emitted_in_module])]
                proto::TransactionFilter::default().with_all(
                    proto::AllTransactionFilter::default().with_filters(vec![
                        proto::TransactionFilter::default()
                            .with_sender(proto::AddressFilter::default().with_address(alice)),
                        proto::TransactionFilter::default().with_event(
                            proto::EventFilter::default().with_any(
                                proto::AnyEventFilter::default().with_filters(vec![
                                    proto::EventFilter::default().with_move_event_type(
                                        proto::MoveEventTypeFilter::default()
                                            .with_struct_tag("0x1::events::Foo"),
                                    ),
                                    proto::EventFilter::default().with_move_package_and_module(
                                        proto::MovePackageAndModuleFilter::default()
                                            .with_package_id(pkg)
                                            .with_module("events"),
                                    ),
                                ]),
                            ),
                        ),
                    ]),
                ),
                // Branch 3: NOT receiver(bob)
                proto::TransactionFilter::default().with_negation(
                    proto::NotTransactionFilter::default().with_filter(
                        proto::TransactionFilter::default()
                            .with_receiver(proto::AddressFilter::default().with_address(bob)),
                    ),
                ),
            ]),
        );

        assert_eq!(proto::TransactionFilter::from(wrapper), expected);
    }

    #[test]
    fn negate_in_middle_negates_only_accumulated() {
        // .kinds().negate().execution_status() => (NOT kinds) AND status
        let f = TransactionFilter::new()
            .kinds([TransactionKind::Programmable])
            .negate()
            .execution_status(true);

        let kinds_leaf = proto::TransactionFilter::default().with_transaction_kinds({
            let mut k = proto::TransactionKindsFilter::default();
            k.push_kinds(proto::TransactionKind::Programmable);
            k
        });
        let status_leaf = proto::TransactionFilter::default()
            .with_execution_status(proto::ExecutionStatusFilter::default().with_success(true));
        let negated_kinds = proto::TransactionFilter::default()
            .with_negation(proto::NotTransactionFilter::default().with_filter(kinds_leaf));
        let expected = proto::TransactionFilter::default().with_all(
            proto::AllTransactionFilter::default().with_filters(vec![negated_kinds, status_leaf]),
        );

        assert_eq!(proto::TransactionFilter::from(f), expected);
    }

    #[test]
    fn negate_at_end_wraps_full_chain() {
        // .kinds().execution_status().negate() => NOT (kinds AND status)
        let f = TransactionFilter::new()
            .kinds([TransactionKind::Programmable])
            .execution_status(true)
            .negate();

        let kinds_leaf = proto::TransactionFilter::default().with_transaction_kinds({
            let mut k = proto::TransactionKindsFilter::default();
            k.push_kinds(proto::TransactionKind::Programmable);
            k
        });
        let status_leaf = proto::TransactionFilter::default()
            .with_execution_status(proto::ExecutionStatusFilter::default().with_success(true));
        let all_filter = proto::TransactionFilter::default().with_all(
            proto::AllTransactionFilter::default().with_filters(vec![kinds_leaf, status_leaf]),
        );
        let expected = proto::TransactionFilter::default()
            .with_negation(proto::NotTransactionFilter::default().with_filter(all_filter));

        assert_eq!(proto::TransactionFilter::from(f), expected);
    }

    #[test]
    fn negate_method_and_not_operator_are_equivalent() {
        // .negate() and !filter must produce the same proto.
        use std::ops::Not;

        let via_method = TransactionFilter::new().sender(IotaAddress::ZERO).negate();
        let via_operator = TransactionFilter::new().sender(IotaAddress::ZERO).not();

        assert_eq!(
            proto::TransactionFilter::from(via_method),
            proto::TransactionFilter::from(via_operator),
        );
    }

    #[test]
    fn negate_after_or_wraps_the_or() {
        // .sender(a).or(.sender(b)).negate() => NOT (sender(a) OR sender(b))
        let f = TransactionFilter::new()
            .sender(IotaAddress::ZERO)
            .or(TransactionFilter::new().sender(IotaAddress::ZERO))
            .negate();

        let inner_any = proto::TransactionFilter::default().with_any(
            proto::AnyTransactionFilter::default().with_filters(vec![
                proto::TransactionFilter::default()
                    .with_sender(proto::AddressFilter::default().with_address(IotaAddress::ZERO)),
                proto::TransactionFilter::default()
                    .with_sender(proto::AddressFilter::default().with_address(IotaAddress::ZERO)),
            ]),
        );
        let expected = proto::TransactionFilter::default()
            .with_negation(proto::NotTransactionFilter::default().with_filter(inner_any));

        assert_eq!(proto::TransactionFilter::from(f), expected);
    }

    #[test]
    fn continue_chaining_after_negate() {
        // .negate() returns a TransactionFilter that can be further chained.
        // Confirms that the post-negate self acts as a normal accumulator.
        let f = TransactionFilter::new()
            .sender(IotaAddress::ZERO)
            .negate() // wraps sender in Negation
            .execution_status(true) // ANDs status on top
            .receiver(IotaAddress::ZERO); // ANDs receiver on top

        // Expected: All([Negation(sender), status, receiver])
        let sender_leaf = proto::TransactionFilter::default()
            .with_sender(proto::AddressFilter::default().with_address(IotaAddress::ZERO));
        let negated_sender = proto::TransactionFilter::default()
            .with_negation(proto::NotTransactionFilter::default().with_filter(sender_leaf));
        let status_leaf = proto::TransactionFilter::default()
            .with_execution_status(proto::ExecutionStatusFilter::default().with_success(true));
        let receiver_leaf = proto::TransactionFilter::default()
            .with_receiver(proto::AddressFilter::default().with_address(IotaAddress::ZERO));
        let expected = proto::TransactionFilter::default().with_all(
            proto::AllTransactionFilter::default().with_filters(vec![
                negated_sender,
                status_leaf,
                receiver_leaf,
            ]),
        );

        assert_eq!(proto::TransactionFilter::from(f), expected);
    }
}
