// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub mod analysis;
pub mod config;
pub mod resolution;
pub mod resolved_linkage;
pub mod single_linkage;

use crate::{
    data_store::VerifiedPackageStore,
    execution_mode::ExecutionMode,
    static_programmable_transactions::{
        linkage::analysis::LinkageAnalyzer, loading::ast as loading,
    },
};

/// Refine the transaction's per-call linkages into a single, unified linkage for the whole
/// transaction.
pub fn refine_linkage<Mode: ExecutionMode>(
    mut txn: loading::Transaction,
    linkage_analysis: &LinkageAnalyzer,
    package_store: &VerifiedPackageStore<'_>,
) -> Result<loading::Transaction, Mode::Error> {
    single_linkage::refine_to_single_linkage::<Mode::Error>(
        &mut txn,
        linkage_analysis,
        package_store,
    )?;

    Ok(txn)
}
