// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Ingestion tests for the account-discoverability reverse index.

#[expect(dead_code)]
#[cfg(feature = "pg_integration")]
mod common;

#[cfg(feature = "pg_integration")]
mod account_key_links_tests {
    use std::sync::Arc;

    use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
    use iota_indexer::{
        account_key_events::{AccountKeyLinkOp, LinkSource, account_key_link_ops},
        db::get_pool_connection,
        errors::{Context, IndexerError},
        models::account_key_links::{
            LINK_STATUS_ACTIVE, LINK_STATUS_UNLINKED, StoredAccountKeyLink,
        },
        schema::account_key_links,
        store::{PgIndexerStore, indexer_store::IndexerStore},
    };
    use iota_protocol_config::ProtocolConfig;
    use iota_sdk_types::{
        Address, ExecutionStatus, Identifier, ObjectId, StructTag, TransactionKind, events::Event,
    };
    use iota_types::{
        claim_registry::{get_claim_registry_obj_initial_shared_version, key_id},
        crypto::KeypairTraits,
        effects::TransactionEffectsAPI,
        programmable_transaction_builder::ProgrammableTransactionBuilder,
        transaction::{
            CallArg, GasData, SharedObjectRef, Transaction, TransactionData, TransactionDataAPI,
        },
    };
    use simulacrum::Simulacrum;

    use crate::common::{indexer_wait_for_checkpoint, start_simulacrum_grpc_with_write_indexer};

    const DB_NAME: &str = "indexer_account_key_links_tests_db";

    macro_rules! read_only_blocking {
        ($pool:expr, $query:expr) => {{
            let mut pg_pool_conn = get_pool_connection($pool)?;
            pg_pool_conn
                .build_transaction()
                .read_only()
                .run($query)
                .map_err(|e| IndexerError::PostgresRead(e.to_string()))
        }};
    }

    /// Builds the `Event` the framework would emit for `0x2::<module>::<name>`
    /// with the given BCS payload.
    fn framework_event(module: &str, name: &str, contents: Vec<u8>) -> Event {
        let module = Identifier::new(module).unwrap();
        Event {
            package_id: ObjectId::FRAMEWORK,
            module: module.clone(),
            sender: Address::ZERO,
            type_: StructTag::new(
                Address::FRAMEWORK,
                module,
                Identifier::new(name).unwrap(),
                vec![],
            ),
            contents,
        }
    }

    fn all_links(pg_store: &PgIndexerStore) -> Result<Vec<StoredAccountKeyLink>, IndexerError> {
        read_only_blocking!(&pg_store.blocking_cp(), |conn| {
            account_key_links::table
                .order((
                    account_key_links::key_id.asc(),
                    account_key_links::account_id.asc(),
                ))
                .load::<StoredAccountKeyLink>(conn)
        })
        .context("failed reading account_key_links from PostgresDB")
    }

    fn links_for_key(
        pg_store: &PgIndexerStore,
        key_id: &Address,
    ) -> Result<Vec<StoredAccountKeyLink>, IndexerError> {
        let key_id = key_id.as_bytes().to_vec();
        read_only_blocking!(&pg_store.blocking_cp(), |conn| {
            account_key_links::table
                .filter(account_key_links::key_id.eq(key_id.clone()))
                .order(account_key_links::account_id.asc())
                .load::<StoredAccountKeyLink>(conn)
        })
        .context("failed reading account_key_links from PostgresDB")
    }

    /// Collapses one event's ops the way the writer does, then upserts them.
    async fn persist(
        pg_store: &PgIndexerStore,
        ops: &[AccountKeyLinkOp],
    ) -> Result<(), IndexerError> {
        let mut rows: Vec<StoredAccountKeyLink> = Vec::new();
        for op in ops {
            let row = StoredAccountKeyLink::from(op);
            match rows
                .iter_mut()
                .find(|r| r.key_id == row.key_id && r.account_id == row.account_id)
            {
                Some(existing) => *existing = row,
                None => rows.push(row),
            }
        }
        pg_store.persist_account_key_links(rows).await
    }

    /// Claiming an address emits `ClaimedAddress` and then `PublicKeyAttached`;
    /// both name the same `(key_id, account)` pair, so the fold must leave
    /// exactly one active row behind.
    #[tokio::test]
    async fn claim_creates_an_active_link() -> Result<(), IndexerError> {
        let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
            config.set_enable_claim_registry_for_testing(true);
            config.set_enable_builtin_move_authenticators_for_testing(true);
            config
        });

        let tmp_dir = iota_common::tempdir();
        let sim = Simulacrum::new();
        let data_ingestion_path = tmp_dir.path().to_path_buf();
        sim.set_data_ingestion_path(data_ingestion_path.clone());

        let (sender, keypair) = sim.with_keystore(|keystore| {
            let (address, keypair) = keystore.accounts().next().unwrap();
            (*address, keypair.copy())
        });
        // The wire format `public_key::from_prefixed_bytes` expects, which is
        // also the preimage of the key id the indexer will record.
        let prefixed_public_key: Vec<u8> = std::iter::once(0x00u8)
            .chain(keypair.public().as_ref().iter().copied())
            .collect();
        let expected_key_id = key_id(prefixed_public_key[0], &prefixed_public_key[1..]);

        let registry_initial_shared_version = sim
            .with_store(|store| get_claim_registry_obj_initial_shared_version(store))?
            .expect("ClaimRegistry must exist at genesis when the flag is enabled");

        let gas_object = sim.with_store(|store| {
            store
                .owned_objects(sender)
                .find(|object| object.is_gas_coin())
                .expect("the genesis account must own a gas coin")
                .clone()
        });

        let programmable_transaction = {
            let mut builder = ProgrammableTransactionBuilder::new();
            let public_key_bytes = builder.pure(prefixed_public_key).unwrap();
            let public_key = builder.programmable_move_call(
                ObjectId::FRAMEWORK,
                Identifier::new("public_key").unwrap(),
                Identifier::new("from_prefixed_bytes").unwrap(),
                vec![],
                vec![public_key_bytes],
            );
            let registry = builder
                .obj(CallArg::Shared(SharedObjectRef::new(
                    ObjectId::CLAIM_REGISTRY,
                    registry_initial_shared_version,
                    true,
                )))
                .unwrap();
            let account_builder = builder.programmable_move_call(
                ObjectId::FRAMEWORK,
                Identifier::new("smart_account").unwrap(),
                Identifier::new("claim_builder_v1").unwrap(),
                vec![],
                vec![registry, public_key],
            );
            builder.programmable_move_call(
                ObjectId::FRAMEWORK,
                Identifier::new("smart_account").unwrap(),
                Identifier::new("build_v1").unwrap(),
                vec![],
                vec![account_builder],
            );
            builder.finish()
        };

        let transaction_data = TransactionData::new_with_gas_data(
            TransactionKind::Programmable(programmable_transaction),
            sender,
            GasData {
                objects: vec![gas_object.object_ref()],
                owner: sender,
                price: sim.reference_gas_price(),
                budget: 1_000_000_000,
            },
        );
        let transaction = Transaction::from_data_and_signer(transaction_data, vec![&keypair]);

        let (effects, execution_error) = sim.execute_transaction(transaction).unwrap();
        assert!(
            execution_error.is_none(),
            "claim transaction failed: {execution_error:?}"
        );
        assert_eq!(
            effects.status(),
            &ExecutionStatus::Success,
            "claim transaction did not succeed"
        );
        sim.create_checkpoint();

        let (_, pg_store, _) = start_simulacrum_grpc_with_write_indexer(
            Arc::new(sim),
            data_ingestion_path,
            None,
            Some(DB_NAME),
            None,
        )
        .await;
        indexer_wait_for_checkpoint(&pg_store, 1).await;

        let links = links_for_key(&pg_store, &expected_key_id)?;

        assert_eq!(
            links.len(),
            1,
            "the claim's two events must collapse into a single row"
        );
        assert_eq!(links[0].account_id, sender.as_bytes().to_vec());
        assert_eq!(links[0].status, LINK_STATUS_ACTIVE);
        assert_eq!(links[0].scheme, 0);
        // `claim_builder_v1` claims and then attaches, so the attach is the
        // later of the two events and wins the collapse.
        assert_eq!(links[0].source, LinkSource::Attach as i16);

        Ok(())
    }

    /// The indexer only ever sees events, so the fold across a rotation can be
    /// driven by feeding it the exact bytes the framework emits. Simulacrum
    /// cannot execute the rotation itself: rotating requires the account to be
    /// the transaction sender, which needs a `MoveAuthenticator`, and
    /// Simulacrum does not support those.
    #[tokio::test]
    async fn rotation_tombstones_the_old_key_and_activates_the_new_one() -> Result<(), IndexerError>
    {
        let tmp_dir = iota_common::tempdir();
        let sim = Simulacrum::new();
        let data_ingestion_path = tmp_dir.path().to_path_buf();
        sim.set_data_ingestion_path(data_ingestion_path.clone());
        sim.create_checkpoint();

        let (_, pg_store, _) = start_simulacrum_grpc_with_write_indexer(
            Arc::new(sim),
            data_ingestion_path,
            None,
            Some(DB_NAME),
            None,
        )
        .await;
        indexer_wait_for_checkpoint(&pg_store, 1).await;

        let account = ObjectId::new([0x11; 32]);
        let old_key = [0xAA; 32];
        let new_key = [0xBB; 33];
        let old_key_id = key_id(0x00, &old_key);
        let new_key_id = key_id(0x01, &new_key);

        let attached = framework_event(
            "builtin_authenticator_functions",
            "PublicKeyAttached",
            bcs::to_bytes(&(account, 0x00u8, old_key.to_vec())).unwrap(),
        );
        let attach_ops = account_key_link_ops(&attached, 1, 0);
        persist(&pg_store, &attach_ops).await?;

        let links = links_for_key(&pg_store, &old_key_id)?;
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].status, LINK_STATUS_ACTIVE);

        let rotated = framework_event(
            "builtin_authenticator_functions",
            "PublicKeyRotated",
            bcs::to_bytes(&(account, 0x00u8, old_key.to_vec(), 0x01u8, new_key.to_vec())).unwrap(),
        );
        let rotate_ops = account_key_link_ops(&rotated, 2, 0);
        assert_eq!(rotate_ops.len(), 2);
        persist(&pg_store, &rotate_ops).await?;

        let old_links = links_for_key(&pg_store, &old_key_id)?;
        assert_eq!(old_links.len(), 1);
        assert_eq!(
            old_links[0].status, LINK_STATUS_UNLINKED,
            "the rotated-away key must be tombstoned, not deleted"
        );
        assert_eq!(old_links[0].source, LinkSource::Rotate as i16);

        let new_links = links_for_key(&pg_store, &new_key_id)?;
        assert_eq!(new_links.len(), 1);
        assert_eq!(new_links[0].status, LINK_STATUS_ACTIVE);
        assert_eq!(new_links[0].scheme, 1);
        assert_eq!(new_links[0].account_id, account.as_bytes().to_vec());

        // Replaying the older attach must not resurrect the tombstoned link:
        // the upsert is guarded on transaction order.
        persist(&pg_store, &attach_ops).await?;
        let old_links = links_for_key(&pg_store, &old_key_id)?;
        assert_eq!(
            old_links[0].status, LINK_STATUS_UNLINKED,
            "an out-of-order replay must not move a link backwards"
        );

        // Re-applying the rotation changes nothing: the fold is idempotent.
        let before = all_links(&pg_store)?;
        persist(&pg_store, &rotate_ops).await?;
        assert_eq!(before, all_links(&pg_store)?);

        Ok(())
    }

    /// Detaching a key tombstones the link the same way a rotation does.
    #[tokio::test]
    async fn detach_tombstones_the_link() -> Result<(), IndexerError> {
        let tmp_dir = iota_common::tempdir();
        let sim = Simulacrum::new();
        let data_ingestion_path = tmp_dir.path().to_path_buf();
        sim.set_data_ingestion_path(data_ingestion_path.clone());
        sim.create_checkpoint();

        let (_, pg_store, _) = start_simulacrum_grpc_with_write_indexer(
            Arc::new(sim),
            data_ingestion_path,
            None,
            Some(DB_NAME),
            None,
        )
        .await;
        indexer_wait_for_checkpoint(&pg_store, 1).await;

        let account = ObjectId::new([0x22; 32]);
        let public_key = [0xCC; 32];
        let public_key_id = key_id(0x00, &public_key);

        let attached = framework_event(
            "builtin_authenticator_functions",
            "PublicKeyAttached",
            bcs::to_bytes(&(account, 0x00u8, public_key.to_vec())).unwrap(),
        );
        persist(&pg_store, &account_key_link_ops(&attached, 3, 0)).await?;

        let detached = framework_event(
            "builtin_authenticator_functions",
            "PublicKeyDetached",
            bcs::to_bytes(&(account, 0x00u8, public_key.to_vec())).unwrap(),
        );
        persist(&pg_store, &account_key_link_ops(&detached, 4, 0)).await?;

        let links = links_for_key(&pg_store, &public_key_id)?;
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].status, LINK_STATUS_UNLINKED);
        assert_eq!(links[0].source, LinkSource::Detach as i16);

        Ok(())
    }
}
