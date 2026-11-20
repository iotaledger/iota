// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! # Transaction Filter Lookup Tables
//!
//! ## Schemas
//!
//! Tables backing Transaction filters in GraphQL all follow the same rough
//! shape:
//!
//! 1. They each get their own table, mapping the filter value to the transaction sequence number.
//!
//! 2. They also include a `sender` column, and a secondary index over the sender, filter values and
//!    the transaction sequence number.
//!
//! 3. They also include a secondary index over the transaction sequence number.
//!
//! This pattern allows us to offer a simple rule for users:
//! - If you are filtering on a **single value**, you can do so without worrying.
//! - If you want to additionally filter by the sender, that is also possible, but combining any
//!   other set of filters is not supported.
//!
//! ## Query construction
//!
//! Queries that filter transactions work in two phases: Identify the
//! transaction sequence numbers to fetch, and then fetch their contents.
//! Filtering all happens in the first phase:
//!
//! - Firstly filters are broken down into individual queries targeting the appropriate lookup
//!   table. Each constituent query is expected to return a sorted run of transaction sequence
//!   numbers.
//!
//! - If a `sender` filter is included, then it is incorporated into each constituent query,
//!   leveraging their secondary indices (2), otherwise each constituent query filters only based on
//!   its filter value using the primary index (1). `tx_kinds` is the exception: it has no `sender`
//!   column, so a `kind` filter with a sender is intersected with `tx_senders` instead.
//!
//! - The fact that both the primary and secondary indices contain the transaction sequence number
//!   help to ensure that the output from an index scan is already sorted, which avoids a
//!   potentially expensive materialize and sort operation.
//!
//! - If there are multiple constituent queries, they are intersected using inner joins. Postgres
//!   can occasionally pick a poor query plan for this merge, so filters resulting in such merges
//!   are not supported.
use std::{
    collections::HashMap,
    fmt::Write,
    ops::{Range, RangeInclusive},
};

use diesel::{ExpressionMethods, QueryDsl};
use iota_indexer::{ingestion::common::CommitterTables, schema::pruner_cp_watermark};
use iota_json_rpc_types::IotaTransactionKind;

use crate::{
    data::{Conn, DbConnection, pg::bytea_literal},
    filter, inner_join, query,
    raw_query::RawQuery,
    types::{
        cursor::Page,
        digest::Digest,
        iota_address::IotaAddress,
        transaction_block::{Cursor, TransactionBlockFilter, TransactionBlockKindInput},
        type_filter::{FqNameFilter, ModuleFilter},
    },
};

/// Bounds on transaction sequence number, imposed by filters and cursors. The
/// outermost bounds are determined by the checkpoint filters.
/// These get translated into bounds in terms of transaction sequence numbers:
///
/// ```ignore
///     tx_lo                                                             tx_hi
///     [-----------------------------------------------------------------)
/// ```
///
/// If cursors are provided, they further restrict the range of transactions to
/// scan. Cursors are exclusive, but when issuing database queries, we treat
/// them inclusively so that we can detect previous and next pages based on the
/// existence of cursors in the results:
///
/// ```ignore
///             cursor_lo                                  cursor_hi_inclusive
///             [------------------------------------------]
/// ```
///
/// This data structure can be used to compute the interval of transactions to
/// look in for candidates to include in a page of results.
///
/// NOTE: for consistency, assume that lowerbounds are inclusive and upperbounds
/// are exclusive. Bounds that do not follow this convention will be annotated
/// explicitly (e.g. `lo_exclusive` or `hi_inclusive`).
#[derive(Clone, Debug, Copy)]
pub(crate) struct TxBounds {
    /// The inclusive lower bound tx_sequence_number derived from checkpoint
    /// bounds. If checkpoint bounds are not provided, this will default to
    /// `0`.
    tx_lo: u64,

    /// The exclusive upper bound tx_sequence_number derived from checkpoint
    /// bounds. If checkpoint bounds are not provided, this will default to
    /// the total transaction count at the checkpoint viewed.
    tx_hi: u64,

    /// The starting cursor (aka `after`).
    cursor_lo_exclusive: Option<u64>,

    // The ending cursor (aka `before`).
    cursor_hi: Option<u64>,
}

impl TxBounds {
    /// Determines the `tx_sequence_number` range from the checkpoint bounds for
    /// a transaction block query. The corresponding `tx_sequence_number` range
    /// is fetched from db, and further adjusted by cursors. If there are any
    /// inconsistencies or invalid combinations, i.e. `after` cursor is greater
    /// than the upper bound, return None.
    ///
    /// `cp_bounds` must lie in the range the database has data for. A
    /// checkpoint missing from `pruner_cp_watermark` fails with `NotFound`.
    pub(crate) fn query(
        conn: &mut Conn,
        cp_bounds: RangeInclusive<u64>,
        page: &Page<Cursor>,
    ) -> Result<Option<Self>, diesel::result::Error> {
        let (cp_lo, cp_hi) = (*cp_bounds.start(), *cp_bounds.end());

        use pruner_cp_watermark::dsl;
        let tx_ranges: HashMap<i64, (i64, i64)> = conn
            .results(move || {
                dsl::pruner_cp_watermark
                    .select((
                        dsl::checkpoint_sequence_number,
                        dsl::min_tx_sequence_number,
                        dsl::max_tx_sequence_number,
                    ))
                    .filter(dsl::checkpoint_sequence_number.eq_any([cp_lo as i64, cp_hi as i64]))
            })?
            .into_iter()
            .map(|(cp, min_tx, max_tx): (i64, i64, i64)| (cp, (min_tx, max_tx)))
            .collect();
        // Both checkpoints should be in the range the database has data for.
        let tx_range = |cp: u64| {
            tx_ranges
                .get(&(cp as i64))
                .copied()
                .ok_or(diesel::result::Error::NotFound)
        };

        let (tx_lo, _) = tx_range(cp_lo)?;
        let (_, tx_hi_inclusive) = tx_range(cp_hi)?;
        let (tx_lo, tx_hi) = (tx_lo as u64, tx_hi_inclusive as u64 + 1);

        // If the cursors point outside checkpoint bounds, we can return early.
        if matches!(page.after(), Some(a) if tx_hi <= u64::from(a.tx_sequence_number).saturating_add(1))
        {
            return Ok(None);
        }

        if matches!(page.before(), Some(b) if u64::from(b.tx_sequence_number) <= tx_lo) {
            return Ok(None);
        }

        Ok(Some(Self {
            tx_lo,
            tx_hi,
            cursor_lo_exclusive: page.after().map(|a| a.tx_sequence_number.into()),
            cursor_hi: page.before().map(|b| b.tx_sequence_number.into()),
        }))
    }

    /// Inclusive lowerbound for range of transactions to scan, accounting for
    /// the bounds from filters and the cursor. For the purposes of scanning
    /// records in the DB, cursors are treated inclusively, even though they are
    /// exclusive bounds.
    pub(crate) fn db_lo(&self) -> u64 {
        max_option([self.cursor_lo_exclusive, Some(self.tx_lo)]).unwrap()
    }

    /// Exclusive upperbound for range of transactions to scan, accounting for
    /// the bounds from filters and the cursor. For the purposes of scanning
    /// records in the DB, cursors are treated inclusively, even though they are
    /// exclusive bounds.
    pub(crate) fn db_hi(&self) -> u64 {
        min_option([
            self.cursor_hi.map(|h| h.saturating_add(1)),
            Some(self.tx_hi),
        ])
        .unwrap()
    }
}

/// The inclusive range of checkpoints a transaction block query with `filter`
/// is confined to when viewed at `checkpoint_viewed_at`, or `None` when it is
/// empty.
///
/// The function considers ranges defined by:
/// - `checkpoint_viewed_at`
/// - `after_checkpoint`, `before_checkpoint`, `at_checkpoint` properties of the `filter`
pub(crate) fn checkpoint_bounds_from_filter(
    filter: &TransactionBlockFilter,
    checkpoint_viewed_at: u64,
) -> Option<RangeInclusive<u64>> {
    fn intersect(a: Range<u64>, b: Range<u64>) -> Range<u64> {
        a.start.max(b.start)..a.end.min(b.end)
    }

    // Half-open ranges, so that `before_checkpoint` needs no adjustment.
    let viewed = Some(0..checkpoint_viewed_at + 1);
    let after = filter
        .after_checkpoint
        .map(|cp| u64::from(cp) + 1..u64::MAX);
    let before = filter.before_checkpoint.map(|cp| 0..u64::from(cp));
    let at = filter
        .at_checkpoint
        .map(|cp| u64::from(cp)..u64::from(cp) + 1);

    let range = [viewed, after, before, at]
        .into_iter()
        .flatten()
        .fold(0..u64::MAX, intersect);
    (!range.is_empty()).then(|| range.start..=range.end - 1)
}

/// Determines the maximum value in an arbitrary number of Option<impl Ord>.
fn max_option<T: Ord>(xs: impl IntoIterator<Item = Option<T>>) -> Option<T> {
    xs.into_iter().flatten().max()
}

/// Determines the minimum value in an arbitrary number of Option<impl Ord>.
fn min_option<T: Ord>(xs: impl IntoIterator<Item = Option<T>>) -> Option<T> {
    xs.into_iter().flatten().min()
}

/// Constructs a `RawQuery` as a join over all relevant side tables, filtered on
/// their own filter condition, plus optionally a sender, plus optionally tx/cp
/// bounds. An `affected_address` filter without a `sender` filter is a special
/// case, handled by `select_affected`.
pub(crate) fn subqueries(
    filter: &TransactionBlockFilter,
    tx_bounds: TxBounds,
    page: &Page<Cursor>,
) -> Option<RawQuery> {
    let sender = filter.sent_address;

    let mut subqueries = vec![];

    if let Some(f) = &filter.function {
        subqueries.push(match f {
            FqNameFilter::ByModule(filter) => match filter {
                ModuleFilter::ByPackage(p) => ("tx_calls_pkg", select_pkg(p, sender, tx_bounds)),
                ModuleFilter::ByModule(p, m) => {
                    ("tx_calls_mod", select_mod(p, m.clone(), sender, tx_bounds))
                }
            },
            FqNameFilter::ByFqName(p, m, n) => (
                "tx_calls_fun",
                select_fun(p, m.clone(), n.clone(), sender, tx_bounds),
            ),
        });
    }
    if let Some(kind) = &filter.kind {
        subqueries.push(("tx_kinds", select_kind(*kind, tx_bounds)));
    }
    if let Some(recv) = &filter.recv_address {
        subqueries.push(("tx_recipients", select_recipient(recv, sender, tx_bounds)));
    }
    if let Some(input) = &filter.input_object {
        subqueries.push(("tx_input_objects", select_input(input, sender, tx_bounds)));
    }
    if let Some(changed) = &filter.changed_object {
        subqueries.push((
            "tx_changed_objects",
            select_changed(changed, sender, tx_bounds),
        ));
    }
    if let Some(wrapped_or_deleted_object) = &filter.wrapped_or_deleted_object {
        subqueries.push((
            "tx_wrapped_or_deleted_objects",
            select_wrapped_or_deleted(wrapped_or_deleted_object, sender, tx_bounds),
        ));
    }
    if let Some(sender) = &filter.explicit_sender() {
        subqueries.push(("tx_senders", select_sender(sender, tx_bounds)));
    }
    if let Some(txs) = &filter.transaction_ids {
        subqueries.push(("tx_global_order", select_ids(txs, tx_bounds)));
    }

    if let Some(affected) = &filter.affected_address {
        match sender {
            // A `sender` filter equal to `affected` can only match transactions
            // where `affected` is the sender.
            Some(sender) if sender == *affected => {
                subqueries.push(("tx_affected", select_sender(affected, tx_bounds)));
            }
            // A `sender` filter different from `affected` can only match
            // transactions where `affected` is the recipient.
            Some(_) => {
                subqueries.push(("tx_affected", select_recipient(affected, sender, tx_bounds)));
            }
            // Otherwise `affected` can be either the sender or a recipient,
            // so we query both.
            None => return Some(select_affected(affected, subqueries, tx_bounds, page)),
        }
    }

    join_subqueries(subqueries)
}

/// The lookup tables [`subqueries`] reads for `filter`.
pub(crate) fn subquery_tables(filter: &TransactionBlockFilter) -> Vec<CommitterTables> {
    use CommitterTables::*;

    let sender = filter.sent_address;

    let mut tables = vec![];

    if let Some(f) = &filter.function {
        tables.push(match f {
            FqNameFilter::ByModule(ModuleFilter::ByPackage(_)) => TxCallsPkg,
            FqNameFilter::ByModule(ModuleFilter::ByModule(..)) => TxCallsMod,
            FqNameFilter::ByFqName(..) => TxCallsFun,
        });
    }
    if filter.kind.is_some() {
        tables.push(TxKinds);
    }
    if filter.recv_address.is_some() {
        tables.push(TxRecipients);
    }
    if filter.input_object.is_some() {
        tables.push(TxInputObjects);
    }
    if filter.changed_object.is_some() {
        tables.push(TxChangedObjects);
    }
    if filter.wrapped_or_deleted_object.is_some() {
        tables.push(TxWrappedOrDeletedObjects);
    }
    if filter.explicit_sender().is_some() {
        tables.push(TxSenders);
    }
    if filter.transaction_ids.is_some() {
        tables.push(TxGlobalOrder);
    }

    if let Some(affected) = &filter.affected_address {
        match sender {
            Some(sender) if sender == *affected => tables.push(TxSenders),
            Some(_) => tables.push(TxRecipients),
            None => tables.extend([TxSenders, TxRecipients]),
        }
    }

    tables
}

/// Joins `subqueries` on `tx_sequence_number` into a single query, or `None` if
/// there are no subqueries.
fn join_subqueries(mut subqueries: Vec<(&str, RawQuery)>) -> Option<RawQuery> {
    let (_, mut subquery) = subqueries.pop()?;

    if !subqueries.is_empty() {
        subquery = query!("SELECT tx_sequence_number FROM ({}) AS initial", subquery);
        while let Some((alias, subselect)) = subqueries.pop() {
            subquery = inner_join!(subquery, alias => subselect, using: ["tx_sequence_number"]);
        }
    }

    Some(subquery)
}

fn select_tx(sender: Option<IotaAddress>, bound: TxBounds, from: &str) -> RawQuery {
    let mut query = filter!(
        query!(format!("SELECT tx_sequence_number FROM {from}")),
        format!(
            "{} <= tx_sequence_number AND tx_sequence_number < {}",
            bound.db_lo(),
            bound.db_hi()
        )
    );

    if let Some(sender) = sender {
        query = filter!(
            query,
            format!("sender = {}", bytea_literal(sender.as_slice()))
        );
    }

    query
}

fn select_pkg(pkg: &IotaAddress, sender: Option<IotaAddress>, bound: TxBounds) -> RawQuery {
    filter!(
        select_tx(sender, bound, "tx_calls_pkg"),
        format!("package = {}", bytea_literal(pkg.as_slice()))
    )
}

fn select_mod(
    pkg: &IotaAddress,
    mod_: String,
    sender: Option<IotaAddress>,
    bound: TxBounds,
) -> RawQuery {
    filter!(
        select_tx(sender, bound, "tx_calls_mod"),
        format!(
            "package = {} and module = {{}}",
            bytea_literal(pkg.as_slice())
        ),
        mod_
    )
}

fn select_fun(
    pkg: &IotaAddress,
    mod_: String,
    fun: String,
    sender: Option<IotaAddress>,
    bound: TxBounds,
) -> RawQuery {
    filter!(
        select_tx(sender, bound, "tx_calls_fun"),
        format!(
            "package = {} AND module = {{}} AND func = {{}}",
            bytea_literal(pkg.as_slice()),
        ),
        mod_,
        fun
    )
}

/// Returns a RawQuery that selects transactions of a specific kind from
/// `tx_kinds`. The sender is not applied here, because `tx_kinds` has no
/// `sender` column: a `kind` filter with a sender needs to be intersected with
/// `tx_senders` by the caller.
fn select_kind(kind: TransactionBlockKindInput, bound: TxBounds) -> RawQuery {
    filter!(
        select_tx(None, bound, "tx_kinds"),
        if TransactionBlockKindInput::SystemTx == kind {
            // `tx_kinds` stores `IotaTransactionKind` numbers, and every kind
            // outside its `NON_SYSTEM_KINDS` is a system kind.
            format!(
                "tx_kind NOT IN ({})",
                IotaTransactionKind::NON_SYSTEM_KINDS
                    .iter()
                    .map(|kind| (*kind as i16).to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        } else {
            format!("tx_kind = {}", kind as i16)
        }
    )
}

fn select_sender(sender: &IotaAddress, bound: TxBounds) -> RawQuery {
    select_tx(Some(*sender), bound, "tx_senders")
}

fn select_recipient(recv: &IotaAddress, sender: Option<IotaAddress>, bound: TxBounds) -> RawQuery {
    filter!(
        select_tx(sender, bound, "tx_recipients"),
        format!("recipient = {}", bytea_literal(recv.as_slice()))
    )
}

/// Selects transactions where `affected` is the sender or a recipient, as a
/// `UNION` of the `tx_senders` and `tx_recipients` tables.
///
/// To avoid full table scans on `UNION` we apply order and limit on the
/// subqueries before `UNION`. For the result to be correct we need to apply all
/// other filters on the subquery level, by joining each subquery with all
/// `other_subqueries`.
fn select_affected(
    affected: &IotaAddress,
    other_subqueries: Vec<(&str, RawQuery)>,
    bound: TxBounds,
    page: &Page<Cursor>,
) -> RawQuery {
    let bounded_subquery = |from: RawQuery, other_subqueries: Vec<(&str, RawQuery)>| {
        let mut all = vec![("tx_affected", from)];
        all.extend(other_subqueries);
        join_subqueries(all)
            .expect("subqueries list is not empty")
            .order_by(format!(
                "tx_sequence_number {}",
                if page.is_from_front() { "ASC" } else { "DESC" }
            ))
            // The same limit that `Page::apply` puts on the overall query: the
            // page size plus the two rows pointed at by the page's cursors.
            .limit(page.limit() as i64 + 2)
    };

    query!(
        "SELECT tx_sequence_number FROM (({}) UNION ({})) AS affected",
        bounded_subquery(select_sender(affected, bound), other_subqueries.clone()),
        bounded_subquery(select_recipient(affected, None, bound), other_subqueries)
    )
}

fn select_input(input: &IotaAddress, sender: Option<IotaAddress>, bound: TxBounds) -> RawQuery {
    filter!(
        select_tx(sender, bound, "tx_input_objects"),
        format!("object_id = {}", bytea_literal(input.as_slice()))
    )
}

fn select_changed(changed: &IotaAddress, sender: Option<IotaAddress>, bound: TxBounds) -> RawQuery {
    filter!(
        select_tx(sender, bound, "tx_changed_objects"),
        format!("object_id = {}", bytea_literal(changed.as_slice()))
    )
}

fn select_wrapped_or_deleted(
    wrapped_or_deleted_object: &IotaAddress,
    sender: Option<IotaAddress>,
    bound: TxBounds,
) -> RawQuery {
    filter!(
        select_tx(sender, bound, "tx_wrapped_or_deleted_objects"),
        format!(
            "object_id = {}",
            bytea_literal(wrapped_or_deleted_object.as_slice())
        )
    )
}

fn select_ids(ids: &Vec<Digest>, bound: TxBounds) -> RawQuery {
    let query = select_tx(None, bound, "tx_global_order");
    if ids.is_empty() {
        filter!(query, "1=0")
    } else {
        let mut inner = String::new();
        let mut prefix = "tx_digest IN (";
        for id in ids {
            write!(&mut inner, "{prefix}{}", bytea_literal(id.as_slice())).unwrap();
            prefix = ", ";
        }
        inner.push(')');
        filter!(query, inner)
    }
}
