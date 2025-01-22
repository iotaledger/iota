// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use clap::Parser;
use diesel::{ExpressionMethods, PgConnection, QueryDsl, RunQueryDsl};
use iota_genesis_builder::{Builder as GenesisBuilder, SnapshotSource, SnapshotUrl};
use iota_indexer::{
    db::{self, reset_database},
    errors::IndexerError,
    indexer_reader::IndexerReader,
    models::transactions::StoredTransaction,
    schema::transactions,
    store::indexer_store::IndexerStore,
    test_utils::create_pg_store,
    types::{IndexedTransaction, TransactionKind},
};
use iota_swarm_config::genesis_config::ValidatorGenesisConfigBuilder;
use rand::rngs::OsRng;

#[derive(Parser, Debug)]
#[command(about = "Example that indexes the genesis transaction into the database")]
struct Args {
    /// Remotely stored migration snapshots.
    #[arg(
        long,
        name = "iota|<full-url>",
        help = "Remote migration snapshots.",
        default_values_t = vec![SnapshotUrl::Iota],
        num_args(0..)
    )]
    migration_snapshots: Vec<SnapshotUrl>,
}

const DEFAULT_DB_URL: &str = "postgres://postgres:postgrespw@localhost:5432/iota_indexer";

// Build genesis with `IOTA` stardust snapshot
fn genesis_builder(migration_sources: Vec<SnapshotSource>) -> GenesisBuilder {
    // Create the builder
    let mut builder = GenesisBuilder::new();

    // Create validators
    let mut validators = Vec::new();
    let mut key_pairs = Vec::new();
    let mut rng = OsRng;
    for i in 0..2 {
        let validator_config = ValidatorGenesisConfigBuilder::default().build(&mut rng);
        let validator_info = validator_config.to_validator_info(format!("validator-{i}"));
        let validator_addr = validator_info.info.iota_address();
        validators.push(validator_addr);
        key_pairs.push(validator_config.authority_key_pair);
        builder = builder.add_validator(validator_info.info, validator_info.proof_of_possession);
    }

    for migration_source in migration_sources {
        builder = builder.add_migration_source(migration_source);
    }

    for key in &key_pairs {
        builder = builder.add_validator_signature(key);
    }
    builder
}

#[tokio::main]
pub async fn main() -> Result<(), IndexerError> {
    let _guard = telemetry_subscribers::TelemetryConfig::new()
        .with_env()
        .init();

    let args = Args::parse();
    let migration_sources = args
        .migration_snapshots
        .into_iter()
        .map(SnapshotSource::S3)
        .collect();

    // Create genesis transaction
    let (tx_digest, sender_signed_data, effects, summary) = {
        tokio::task::spawn_blocking(|| {
            let mut builder = genesis_builder(migration_sources);
            let genesis = builder.get_or_build_unsigned_genesis();
            tracing::info!("genesis built");
            let summary = genesis.checkpoint.clone();
            let effects = genesis.effects.clone();
            let tx_digest = *genesis.transaction.digest();
            let data = genesis.transaction.data().clone();
            (tx_digest, data, effects, summary)
        })
        .await
        .unwrap()
    };
    let db_txn = IndexedTransaction {
        tx_sequence_number: 0,
        tx_digest,
        sender_signed_data,
        effects,
        checkpoint_sequence_number: *summary.sequence_number(),
        timestamp_ms: summary.timestamp_ms,
        object_changes: Default::default(),
        balance_change: Default::default(),
        events: Default::default(),
        transaction_kind: TransactionKind::SystemTransaction,
        successful_tx_num: 1,
    };

    let digest_to_bytes = db_txn.tx_digest.into_inner().to_vec();
    let expected_transactions = bcs::to_bytes(&db_txn.sender_signed_data).unwrap();
    let expected_effects = bcs::to_bytes(&db_txn.effects).unwrap();

    let pg_store = create_pg_store::<PgConnection>(DEFAULT_DB_URL.to_string().into(), true);
    reset_database(&mut pg_store.blocking_cp().get().unwrap()).unwrap();
    pg_store.persist_transactions(vec![db_txn]).await.unwrap();

    let mut conn = db::get_pool_connection(&pg_store.blocking_cp())?;
    let stored = transactions::table
        .filter(transactions::transaction_digest.eq(digest_to_bytes))
        .first::<StoredTransaction>(&mut conn)
        .unwrap();

    let stored = stored
        .set_genesis_large_object_as_inner_data(&pg_store.blocking_cp())
        .unwrap();

    let reader = IndexerReader::<PgConnection>::new(DEFAULT_DB_URL.to_owned())?;
    // We just want to verify that the call succeeds.
    let _coin_metadata = reader
        .get_coin_metadata_in_blocking_task("0x2::iota::IOTA".parse().unwrap())
        .await?;

    assert!(expected_transactions == stored.raw_transaction);
    assert!(expected_effects == stored.raw_effects);
    Ok(())
}
