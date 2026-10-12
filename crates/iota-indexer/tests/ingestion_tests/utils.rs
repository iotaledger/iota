// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use iota_common::random_util::TempDir;
use iota_indexer::{
    ingestion::OperationalLevel, models::obj_indices::StoredObjectVersion, store::PgIndexerStore,
    test_utils::DBInitHook,
};
use iota_sdk_types::{
    Address, ObjectId, ObjectReference, OwnedObjectReference, Transaction, TransactionEffects,
};
use iota_types::{
    effects::TransactionEffectsAPI,
    transaction::{TransactionAPI, TransactionEnvelope},
};
use simulacrum::Simulacrum;

use crate::common::start_simulacrum_grpc_with_write_indexer;

macro_rules! read_only_blocking {
    ($pool:expr, $query:expr) => {{
        let pool = $pool.clone();
        ::iota_indexer::store::diesel_macro::spawn_blocking_task(move || {
            let mut pg_pool_conn = ::iota_indexer::db::get_pool_connection(&pool)?;
            pg_pool_conn
                .build_transaction()
                .read_only()
                .run($query)
                .map_err(|e| {
                    eprintln!("failed to read from PostgresDB: {e:?}");
                    ::iota_indexer::errors::IndexerError::PostgresRead
                })
        })
        .await
        .expect("failed to join Tokio blocking task")
    }};
}
pub(crate) use read_only_blocking;

pub(crate) struct TestSim {
    pub(crate) inner: Arc<Simulacrum>,
    data_dir: TempDir,
    operational_level: OperationalLevel,
    database_name: String,
}

impl TestSim {
    /// Creates a simulacrum with an indexer writer.
    ///
    /// The indexer ingests the tables of `operational_level` into the database
    /// `database_name_{operational_level}`.
    pub(crate) fn new(operational_level: OperationalLevel, database_name: &str) -> Self {
        let data_dir = iota_common::tempdir();
        let sim = Simulacrum::new();
        sim.set_data_ingestion_path(data_dir.path().to_path_buf());
        Self {
            inner: Arc::new(sim),
            data_dir,
            operational_level,
            database_name: database_name.to_string(),
        }
    }

    /// Starts a writer indexer that ingests the checkpoints.
    pub(crate) async fn start_indexer(&self, db_init_hook: Option<DBInitHook>) -> PgIndexerStore {
        let (_, pg_store, _) = start_simulacrum_grpc_with_write_indexer(
            self.inner.clone(),
            self.data_dir.path().to_path_buf(),
            None,
            Some(&self.database_name),
            db_init_hook,
            self.operational_level,
        )
        .await;
        pg_store
    }

    /// Returns the address of the first account in the keystore.
    pub(crate) fn sender(&self) -> Address {
        self.inner
            .with_keystore(|ks| *ks.accounts().next().unwrap().0)
    }

    /// Returns the reference of a gas coin that the sender owns.
    pub(crate) fn pick_gas(&self) -> ObjectReference {
        let sender = self.sender();
        self.inner.with_store(|s| {
            s.owned_objects(sender)
                .find(|o| o.is_gas_coin())
                .unwrap()
                .object_ref()
        })
    }

    /// Signs `tx` with the key of the sender, executes it, asserts
    /// success and returns the effects.
    pub(crate) fn execute_signed(&self, tx: Transaction) -> TransactionEffects {
        let key = self
            .inner
            .with_keystore(|ks| ks.accounts().next().unwrap().1.clone());
        assert_eq!(tx.sender(), self.sender());
        let tx = TransactionEnvelope::from_data_and_signer(tx, vec![&key]);
        let (effects, err) = self.inner.execute_transaction(tx).unwrap();
        assert!(err.is_none(), "tx failed: {err:?}");
        effects
    }

    /// Executes a transfer of a new coin to a random address and asserts
    /// success.
    ///
    /// Returns the transaction, its effects, and the reference of the
    /// coin that it creates.
    pub(crate) fn execute_transfer(
        &self,
    ) -> (TransactionEnvelope, TransactionEffects, ObjectReference) {
        let (transaction, _) = self.inner.transfer_txn(Address::random());
        let (effects, err) = self.inner.execute_transaction(transaction.clone()).unwrap();
        assert!(err.is_none(), "tx failed: {err:?}");
        let created = effects.created();
        assert_eq!(
            created.len(),
            1,
            "transfer_txn should create exactly 1 coin"
        );
        let created_coin = *created[0].reference();
        (transaction, effects, created_coin)
    }

    /// Executes a transfer and returns the rows that it must add to
    /// `objects_version` at checkpoint `cp`.
    pub(crate) fn execute_transfer_version_rows(&self, cp: i64) -> [StoredObjectVersion; 2] {
        let (tx, effects, coin) = self.execute_transfer();
        let gas = find_object_ref(
            effects
                .mutated()
                .iter()
                .map(OwnedObjectReference::reference),
            tx.gas()[0].object_id,
        );
        [gas, coin].map(|r| StoredObjectVersion {
            object_id: r.object_id.as_bytes().to_vec(),
            object_version: r.version.as_u64() as i64,
            cp_sequence_number: cp,
        })
    }
}

/// Returns the reference of the object `id` in `refs`.
pub(crate) fn find_object_ref<'a>(
    refs: impl IntoIterator<Item = &'a ObjectReference>,
    id: ObjectId,
) -> ObjectReference {
    *refs
        .into_iter()
        .find(|r| r.object_id == id)
        .unwrap_or_else(|| panic!("object {id:?} not found"))
}
