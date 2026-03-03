// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::fmt::Display;

use iota_types::{
    base_types::TransactionDigest, effects::TransactionEffects, transaction::Transaction,
};

use crate::{ExecutionEffects, workloads::ExpectedFailureType};

// =========== Soft-bundle / batched execution results ===========

/// The result for a single transaction within a soft-bundle execution round.
#[derive(Debug)]
pub enum SoftBundleTransactionResult {
    /// The transaction was executed and effects are available.
    Executed(Box<TransactionEffects>),
    /// The transaction was rejected (e.g. owned-object conflict via white
    /// flag).
    Rejected(String),
    /// A transient error occurred.
    Failed(String),
}

/// Aggregated results from [`ValidatorProxy::execute_soft_bundle`].
#[derive(Debug)]
pub struct SoftBundleExecutionResults {
    /// One entry per transaction in the bundle, in submission order.
    pub results: Vec<(TransactionDigest, SoftBundleTransactionResult)>,
}

// =========== Payload trait ===========

/// A Payload is a transaction wrapper of a particular type (transfer object,
/// shared counter, etc). Calling `make_transaction()` on a payload produces the
/// transaction it is wrapping. Once that transaction is returned with effects
/// (by quorum driver), a new payload can be generated with that
/// effect by invoking `make_new_payload(effects)`
pub trait Payload: Send + Sync + std::fmt::Debug + Display {
    fn make_new_payload(&mut self, effects: &ExecutionEffects);
    fn make_transaction(&mut self) -> Transaction;
    fn get_failure_type(&self) -> Option<ExpectedFailureType> {
        None // Default implementation returns None
    }

    /// Returns `true` if this payload submits batches of transactions as
    /// soft bundles rather than individual transactions.
    ///
    /// When `true` the bench driver will call
    /// [`Payload::make_transaction_batch`]
    /// and [`ValidatorProxy::execute_soft_bundle`] instead of the single-tx
    /// path.
    fn is_batched(&self) -> bool {
        false
    }

    /// Creates a batch of transactions for soft-bundle submission.
    ///
    /// Only called when [`Payload::is_batched`] returns `true`.
    /// The default implementation wraps the single transaction from
    /// [`Payload::make_transaction`] in a one-element `Vec`.
    fn make_transaction_batch(&mut self) -> Vec<Transaction> {
        vec![self.make_transaction()]
    }

    /// Updates payload state based on the results of a soft-bundle round.
    ///
    /// Only called when [`Payload::is_batched`] returns `true`.
    /// The default implementation is a no-op.
    fn handle_batch_results(&mut self, _results: &SoftBundleExecutionResults) {}
}
