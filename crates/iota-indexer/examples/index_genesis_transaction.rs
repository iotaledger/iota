// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_genesis_builder::{Builder as GenesisBuilder, SnapshotSource, SnapshotUrl};
use iota_indexer::{
    errors::IndexerError,
    store::indexer_store::IndexerStore,
    test_utils::create_pg_store,
    types::{IndexedTransaction, TransactionKind},
};
use iota_swarm_config::genesis_config::ValidatorGenesisConfigBuilder;
use rand::rngs::OsRng;

const DEFAULT_DB_URL: &str = "postgres://postgres:postgrespw@localhost:5432/iota_indexer";

// Build genesis with `Iota` stardust snapshot
fn genesis_builder() -> GenesisBuilder {
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
        key_pairs.push(validator_config.key_pair);
        builder = builder.add_validator(validator_info.info, validator_info.proof_of_possession);
    }

    builder = builder.add_migration_source(SnapshotSource::S3(SnapshotUrl::Iota));

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

    // Create genesis transaction
    let (tx_digest, sender_signed_data, effects, summary) = {
        tokio::task::spawn_blocking(|| {
            let mut builder = genesis_builder();
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
        tx_sequence_number: 1,
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

    let pg_store = create_pg_store(Some(DEFAULT_DB_URL.to_owned()), None);
    pg_store.persist_transactions(vec![db_txn]).await.unwrap();

    Ok(())
}
