// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_config::{genesis::GenesisCeremonyParameters, migration_tx_data::MigrationTxData};
use iota_sdk_types::TransactionDigest;
use iota_types::object::Object;

use crate::prepare_and_execute_genesis_transaction;

/// Recovers the objects created by the migration transactions of a
/// [`MigrationTxData`] by re-executing them. Executing needs the Move VM, so
/// these operations live here instead of next to the data type in
/// `iota-config`.
pub trait MigrationTxDataExt {
    /// Executes all the migration transactions for this migration data and
    /// returns the objects created by these executions.
    fn get_objects(&self) -> impl Iterator<Item = Object> + '_;

    /// Executes the migration transaction identified by `digest` and returns
    /// the vector of objects created by the execution.
    fn objects_by_tx_digest(&self, digest: TransactionDigest) -> Option<Vec<Object>>;
}

impl MigrationTxDataExt for MigrationTxData {
    fn get_objects(&self) -> impl Iterator<Item = Object> + '_ {
        self.txs_data().values().flat_map(|(tx, _, _)| {
            self.objects_by_tx_digest(*tx.digest())
                .expect("the migration data is corrupted")
                .into_iter()
        })
    }

    fn objects_by_tx_digest(&self, digest: TransactionDigest) -> Option<Vec<Object>> {
        let (tx, effects, _) = self.txs_data().get(&digest)?;

        // We use default ceremony parameters, not the real ones. This should not affect
        // the execution of a genesis transaction.
        let default_ceremony_parameters = GenesisCeremonyParameters::default();

        // Execute the transaction
        let (execution_effects, _, execution_objects) = prepare_and_execute_genesis_transaction(
            default_ceremony_parameters.chain_start_timestamp_ms,
            default_ceremony_parameters.protocol_version,
            tx,
        );

        // Validate the results
        assert_eq!(
            effects.digest(),
            execution_effects.digest(),
            "invalid execution"
        );

        // Return
        Some(execution_objects)
    }
}
