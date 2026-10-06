// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[expect(dead_code)]
#[cfg(feature = "pg_integration")]
mod common;
#[cfg(feature = "pg_integration")]
mod account_key_links_tests {
    use std::{str::FromStr, time::Duration};

    use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl, SelectableHelper};
    use fastcrypto::encoding::Base64;
    use iota_indexer::{
        account_key_events::{AuthenticatorKind, LinkSource},
        db::get_pool_connection,
        errors::IndexerError,
        models::{
            account_authenticators::StoredAccountAuthenticator,
            account_key_links::{LINK_STATUS_ACTIVE, LINK_STATUS_UNLINKED, StoredAccountKeyLink},
            smart_accounts::StoredSmartAccount,
        },
        schema::{account_authenticators, account_key_links, smart_accounts},
        store::{PgIndexerStore, indexer_store::IndexerStore},
        test_utils::{IndexerTypeConfig, db_url, start_test_indexer},
    };
    use iota_json_rpc_api::ExtendedApiClient;
    use iota_json_rpc_types::{
        AccountAuthenticatorKind, AccountKeyLinkSource, AccountKeyLinkStatus,
        IotaTransactionBlockEffectsAPI,
    };
    use iota_keys::keystore::AccountKeystore;
    use iota_sdk::wallet_context::WalletContext;
    use iota_sdk_crypto::{Signer, secp256k1::Secp256k1PrivateKey, simple::SimpleKeypair};
    use iota_sdk_types::{
        Address, ClaimAccountTransaction, ExecutionStatus, Identifier, MoveAuthenticatorV1,
        ObjectReference, Owner, ProgrammableTransaction, SenderSignedTransaction,
        SharedObjectReference, SignatureScheme, SmartAccountBuildKind, SmartAccountClaim,
        Transaction, TransactionEffects, TransactionKind, TypeTag, UserSignature, WriteKind,
        crypto::{Intent, IntentMessage, SimpleSignature},
    };
    use iota_test_transaction_builder::publish_package;
    use iota_types::{
        IOTA_FRAMEWORK_PACKAGE_ID,
        account_abstraction::public_key::key_id,
        effects::{TransactionEffectsAPI, TransactionEffectsExt},
        move_package::derive_package_metadata_id,
        programmable_transaction_builder::ProgrammableTransactionBuilder,
        transaction::{
            CallArg, TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE, TransactionAPI,
            TransactionEnvelope,
        },
    };
    use jsonrpsee::http_client::HttpClient;
    use rand::{SeedableRng, rngs::StdRng};
    use test_cluster::TestCluster;

    use crate::common::{
        indexer_wait_for_checkpoint, indexer_wait_for_latest_checkpoint,
        rpc_call_error_msg_matches, start_test_cluster_with_read_write_indexer,
    };

    const DB: &str = "account_key_links_tests_db";
    const REPLICA_DB: &str = "account_key_links_tests_replica_db";
    /// The test package in `tests/data` that defines a custom authenticator
    /// for `SmartAccount`, in a module of the same name.
    const CUSTOM_AUTHENTICATOR: &str = "custom_authenticator";

    /// A claim links the claiming key to the account, through the
    /// `PublicKeyAttached` it emits, and records a `SmartAccount`.
    #[tokio::test]
    async fn a_claim_links_the_key_and_records_the_smart_account() -> Result<(), IndexerError> {
        let (cluster, store, _client) =
            start_test_cluster_with_read_write_indexer(DB, None, None).await;

        let (owner, key_id) = claim(&cluster, SmartAccountBuildKind::Mutable).await;
        indexer_wait_for_latest_checkpoint(&store, &cluster).await;

        let links = links_for(&store, &key_id)?;
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].account_id, owner.as_bytes().to_vec());
        assert_eq!(links[0].status, LINK_STATUS_ACTIVE);
        assert_eq!(links[0].source, LinkSource::Attach as i16);

        let account = smart_account_for(&store, &owner)?.expect("the account must be recorded");
        assert!(!account.immutable);

        Ok(())
    }

    #[tokio::test]
    async fn an_immutable_account_is_recorded_as_immutable() -> Result<(), IndexerError> {
        let (cluster, store, _client) =
            start_test_cluster_with_read_write_indexer(DB, None, None).await;

        let (owner, _) = claim(&cluster, SmartAccountBuildKind::Immutable).await;
        indexer_wait_for_latest_checkpoint(&store, &cluster).await;

        let account = smart_account_for(&store, &owner)?.expect("the account must be recorded");
        assert!(account.immutable);

        Ok(())
    }

    /// The RPC surface: a wallet holding only the key recovers the account.
    #[tokio::test]
    async fn the_rpc_returns_the_account_for_its_key() -> Result<(), IndexerError> {
        let (cluster, store, client) =
            start_test_cluster_with_read_write_indexer(DB, None, None).await;

        let (owner, _) = claim(&cluster, SmartAccountBuildKind::Mutable).await;
        indexer_wait_for_latest_checkpoint(&store, &cluster).await;

        let accounts = client
            .get_accounts_by_public_key(
                Base64::from_bytes(&prefixed_public_key(&cluster, owner)),
                None,
            )
            .await
            .expect("the lookup must succeed");

        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].address, owner);
        assert_eq!(accounts[0].status, AccountKeyLinkStatus::Active);
        assert_eq!(accounts[0].source, AccountKeyLinkSource::Attach);
        assert!(accounts[0].smart_account);
        assert_eq!(
            accounts[0].authenticator,
            Some(AccountAuthenticatorKind::Ed25519),
            "a claimed account uses the built-in authenticator of its key's scheme"
        );

        Ok(())
    }

    /// An empty key carries no scheme flag, so there is nothing to hash.
    #[tokio::test]
    async fn the_rpc_rejects_an_empty_public_key() -> Result<(), IndexerError> {
        let (_cluster, _store, client) =
            start_test_cluster_with_read_write_indexer(DB, None, None).await;
        wait_for_reader(&client).await;

        let result = client
            .get_accounts_by_public_key(Base64::from_bytes(&[]), None)
            .await;

        assert!(
            rpc_call_error_msg_matches(
                result,
                r#"{"code":-32603,"message":"Invalid argument with error: `public key must not be empty`"}"#
            ),
            "the handler itself must reject the key, not the transport"
        );

        Ok(())
    }

    /// A key that never touched the chain resolves to nothing, rather than to
    /// the address it derives.
    #[tokio::test]
    async fn an_unseen_key_returns_no_accounts() -> Result<(), IndexerError> {
        let (_cluster, _store, client) =
            start_test_cluster_with_read_write_indexer(DB, None, None).await;
        wait_for_reader(&client).await;

        let unseen = [vec![0x00u8], vec![0xAB; 32]].concat();
        let accounts = client
            .get_accounts_by_public_key(Base64::from_bytes(&unseen), None)
            .await
            .expect("the lookup must succeed");

        assert!(accounts.is_empty());

        Ok(())
    }

    /// Every account holding the key is returned, whoever created it.
    #[tokio::test]
    async fn every_account_holding_the_key_is_returned() -> Result<(), IndexerError> {
        let (cluster, store, client) =
            start_test_cluster_with_read_write_indexer(DB, None, None).await;
        let [other, owner] = two_addresses(&cluster.wallet);

        let (claimed_account, key_id) =
            claim_as(&cluster, owner, SmartAccountBuildKind::Mutable).await;
        let owner_key = keypair_for(&cluster.wallet, owner).public_key();
        let built_account: Address = build_account_with_key(
            &cluster,
            other,
            owner_key.scheme(),
            owner_key.as_ref().to_vec(),
        )
        .await
        .object_id
        .into();
        indexer_wait_for_latest_checkpoint(&store, &cluster).await;

        assert_eq!(links_for(&store, &key_id)?.len(), 2);
        assert!(smart_account_for(&store, &built_account)?.is_some());

        let accounts = client
            .get_accounts_by_public_key(
                Base64::from_bytes(&prefixed_public_key(&cluster, owner)),
                None,
            )
            .await
            .expect("the lookup must succeed");
        assert_eq!(accounts.len(), 2);
        for address in [claimed_account, built_account] {
            let account = accounts
                .iter()
                .find(|account| account.address == address)
                .expect("each account holding the key must be returned");
            assert!(account.smart_account);
            assert_eq!(account.source, AccountKeyLinkSource::Attach);
            assert_eq!(
                account.authenticator,
                Some(AccountAuthenticatorKind::Ed25519)
            );
        }

        Ok(())
    }

    /// A `SmartAccount` without a key has nothing to be found by, but is still
    /// recorded, with its authenticator kind.
    #[tokio::test]
    async fn a_keyless_smart_account_is_recorded_without_a_link() -> Result<(), IndexerError> {
        let (cluster, store, _client) =
            start_test_cluster_with_read_write_indexer(DB, None, None).await;
        let owner = first_address(&cluster.wallet);

        let account: Address = build_keyless_account(&cluster, owner)
            .await
            .object_id
            .into();
        indexer_wait_for_latest_checkpoint(&store, &cluster).await;

        let recorded = smart_account_for(&store, &account)?.expect("the account must be recorded");
        assert!(!recorded.immutable);
        let authenticator =
            authenticator_for(&store, &account)?.expect("its authenticator must be recorded");
        assert_eq!(authenticator.kind, AuthenticatorKind::Ed25519 as i16);
        assert!(
            all_links(&store)?.is_empty(),
            "an account without a key has nothing to be linked by"
        );

        Ok(())
    }

    /// Claiming the same address twice is not prevented yet. The index must
    /// still hold one row per table, carrying the later creation.
    #[tokio::test]
    async fn a_second_claim_of_the_same_address_replaces_the_first() -> Result<(), IndexerError> {
        let (cluster, store, _client) =
            start_test_cluster_with_read_write_indexer(DB, None, None).await;
        let owner = first_address(&cluster.wallet);

        let (_, key_id) = claim_as(&cluster, owner, SmartAccountBuildKind::Immutable).await;
        indexer_wait_for_latest_checkpoint(&store, &cluster).await;
        let first = smart_account_for(&store, &owner)?.expect("the first claim must be recorded");

        claim_as(&cluster, owner, SmartAccountBuildKind::Immutable).await;
        indexer_wait_for_latest_checkpoint(&store, &cluster).await;
        let second = smart_account_for(&store, &owner)?.expect("the second claim must be recorded");

        assert!(second.created_tx_sequence_number > first.created_tx_sequence_number);
        let links = links_for(&store, &key_id)?;
        assert_eq!(links.len(), 1, "both claims address the same pair");
        assert_eq!(
            links[0].last_change_tx_sequence_number,
            second.created_tx_sequence_number
        );
        assert_eq!(links[0].source, LinkSource::Attach as i16);

        Ok(())
    }

    /// A real rotation, sent by the account itself: the old key is kept as a
    /// tombstone, returned only on request, and the new key is active.
    #[tokio::test]
    async fn a_rotation_tombstones_the_old_key_and_links_the_new_one() -> Result<(), IndexerError> {
        let (cluster, store, client) =
            start_test_cluster_with_read_write_indexer(DB, None, None).await;
        let owner = first_address(&cluster.wallet);
        let old_keypair = keypair_for(&cluster.wallet, owner);
        let new_keypair = fresh_secp256k1_keypair();

        let account = build_account_with_key(
            &cluster,
            owner,
            old_keypair.public_key().scheme(),
            old_keypair.public_key().as_ref().to_vec(),
        )
        .await;
        let account_address: Address = account.object_id.into();
        send_from_account(
            &cluster,
            account,
            &old_keypair,
            rotate_key_ptb(account, &new_keypair),
        )
        .await;
        indexer_wait_for_latest_checkpoint(&store, &cluster).await;

        let old_links = links_for(&store, &keypair_key_id(&old_keypair))?;
        assert_eq!(old_links.len(), 1);
        assert_eq!(old_links[0].account_id, account_address.as_bytes().to_vec());
        assert_eq!(old_links[0].status, LINK_STATUS_UNLINKED);
        assert_eq!(old_links[0].source, LinkSource::Rotate as i16);

        let new_links = links_for(&store, &keypair_key_id(&new_keypair))?;
        assert_eq!(new_links.len(), 1);
        assert_eq!(new_links[0].account_id, account_address.as_bytes().to_vec());
        assert_eq!(new_links[0].status, LINK_STATUS_ACTIVE);
        assert_eq!(new_links[0].source, LinkSource::Rotate as i16);

        let old_key = Base64::from_bytes(&prefixed_keypair_public_key(&old_keypair));
        assert!(
            client
                .get_accounts_by_public_key(old_key.clone(), None)
                .await
                .expect("the lookup must succeed")
                .is_empty(),
            "a rotated-away key must not be returned by default"
        );
        let unlinked = client
            .get_accounts_by_public_key(old_key, Some(true))
            .await
            .expect("the lookup must succeed");
        assert_eq!(unlinked.len(), 1);
        assert_eq!(unlinked[0].address, account_address);
        assert_eq!(unlinked[0].status, AccountKeyLinkStatus::Unlinked);
        assert_eq!(unlinked[0].source, AccountKeyLinkSource::Rotate);

        let active = client
            .get_accounts_by_public_key(
                Base64::from_bytes(&prefixed_keypair_public_key(&new_keypair)),
                None,
            )
            .await
            .expect("the lookup must succeed");
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].address, account_address);
        assert_eq!(active[0].status, AccountKeyLinkStatus::Active);

        Ok(())
    }

    /// A real detach, sent by the account itself: the key is kept as a
    /// tombstone and returned only on request.
    #[tokio::test]
    async fn a_detach_tombstones_the_key() -> Result<(), IndexerError> {
        let (cluster, store, client) =
            start_test_cluster_with_read_write_indexer(DB, None, None).await;
        let owner = first_address(&cluster.wallet);
        let keypair = keypair_for(&cluster.wallet, owner);

        let account = build_account_with_key(
            &cluster,
            owner,
            keypair.public_key().scheme(),
            keypair.public_key().as_ref().to_vec(),
        )
        .await;
        send_from_account(&cluster, account, &keypair, detach_key_ptb(account)).await;
        indexer_wait_for_latest_checkpoint(&store, &cluster).await;

        let links = links_for(&store, &keypair_key_id(&keypair))?;
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].status, LINK_STATUS_UNLINKED);
        assert_eq!(links[0].source, LinkSource::Detach as i16);

        let key = Base64::from_bytes(&prefixed_keypair_public_key(&keypair));
        assert!(
            client
                .get_accounts_by_public_key(key.clone(), None)
                .await
                .expect("the lookup must succeed")
                .is_empty()
        );
        let unlinked = client
            .get_accounts_by_public_key(key, Some(true))
            .await
            .expect("the lookup must succeed");
        assert_eq!(unlinked.len(), 1);
        assert_eq!(unlinked[0].address, account.object_id.into());
        assert_eq!(unlinked[0].status, AccountKeyLinkStatus::Unlinked);
        assert_eq!(unlinked[0].source, AccountKeyLinkSource::Detach);

        Ok(())
    }

    /// The index must be a pure function of the checkpoints: a second indexer
    /// replaying the same chain from genesis builds the same rows.
    #[tokio::test]
    async fn a_second_indexer_replaying_the_chain_builds_the_same_tables()
    -> Result<(), IndexerError> {
        let (cluster, store, _client) =
            start_test_cluster_with_read_write_indexer(DB, None, None).await;
        let [other, owner] = two_addresses(&cluster.wallet);
        let owner_keypair = keypair_for(&cluster.wallet, owner);

        claim_as(&cluster, owner, SmartAccountBuildKind::Mutable).await;
        let account = build_account_with_key(
            &cluster,
            other,
            owner_keypair.public_key().scheme(),
            owner_keypair.public_key().as_ref().to_vec(),
        )
        .await;
        send_from_account(
            &cluster,
            account,
            &owner_keypair,
            rotate_key_ptb(account, &fresh_secp256k1_keypair()),
        )
        .await;
        indexer_wait_for_latest_checkpoint(&store, &cluster).await;
        let checkpoint = store
            .get_latest_checkpoint_sequence_number()
            .await?
            .expect("the first indexer must have indexed a checkpoint");

        let (replica, _replica_handle, _replica_token) = start_test_indexer(
            db_url(REPLICA_DB),
            true,
            None,
            cluster.grpc_url(),
            IndexerTypeConfig::writer_mode_with_retention(None),
            None,
        )
        .await;
        indexer_wait_for_checkpoint(&replica, checkpoint).await;

        let links = all_links(&store)?;
        assert_eq!(links.len(), 3, "claim, built-then-rotated old key, new key");
        assert_eq!(links, all_links(&replica)?);
        let accounts = all_smart_accounts(&store)?;
        assert_eq!(accounts.len(), 2);
        assert_eq!(accounts, all_smart_accounts(&replica)?);
        let authenticators = all_account_authenticators(&store)?;
        assert_eq!(authenticators.len(), 2);
        assert_eq!(authenticators, all_account_authenticators(&replica)?);

        Ok(())
    }

    /// Detaching and re-attaching the key in one transaction leaves the account
    /// discoverable, with its authenticator unchanged.
    #[tokio::test]
    async fn detaching_and_reattaching_in_one_transaction_keeps_the_account()
    -> Result<(), IndexerError> {
        let (cluster, store, client) =
            start_test_cluster_with_read_write_indexer(DB, None, None).await;
        let owner = first_address(&cluster.wallet);
        let keypair = keypair_for(&cluster.wallet, owner);

        let account = build_account_with_key(
            &cluster,
            owner,
            keypair.public_key().scheme(),
            keypair.public_key().as_ref().to_vec(),
        )
        .await;
        send_from_account(
            &cluster,
            account,
            &keypair,
            detach_then_reattach_ptb(account),
        )
        .await;
        indexer_wait_for_latest_checkpoint(&store, &cluster).await;

        let accounts = client
            .get_accounts_by_public_key(
                Base64::from_bytes(&prefixed_keypair_public_key(&keypair)),
                None,
            )
            .await
            .expect("the lookup must succeed");
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].status, AccountKeyLinkStatus::Active);
        assert_eq!(accounts[0].source, AccountKeyLinkSource::Attach);
        assert_eq!(
            accounts[0].authenticator,
            Some(AccountAuthenticatorKind::Ed25519)
        );

        Ok(())
    }

    /// Rotating the authenticator and the key together, the way an account
    /// switches scheme, updates the kind reported for the new key.
    #[tokio::test]
    async fn rotating_the_authenticator_with_the_key_updates_the_kind() -> Result<(), IndexerError>
    {
        let (cluster, store, client) =
            start_test_cluster_with_read_write_indexer(DB, None, None).await;
        let owner = first_address(&cluster.wallet);
        let old_keypair = keypair_for(&cluster.wallet, owner);
        let new_keypair = fresh_secp256k1_keypair();

        let account = build_account_with_key(
            &cluster,
            owner,
            old_keypair.public_key().scheme(),
            old_keypair.public_key().as_ref().to_vec(),
        )
        .await;
        send_from_account(
            &cluster,
            account,
            &old_keypair,
            rotate_authenticator_and_key_ptb(account, &new_keypair),
        )
        .await;
        indexer_wait_for_latest_checkpoint(&store, &cluster).await;

        let accounts = client
            .get_accounts_by_public_key(
                Base64::from_bytes(&prefixed_keypair_public_key(&new_keypair)),
                None,
            )
            .await
            .expect("the lookup must succeed");
        assert_eq!(accounts.len(), 1);
        assert_eq!(
            accounts[0].authenticator,
            Some(AccountAuthenticatorKind::Secp256k1)
        );

        Ok(())
    }

    /// An account that rotates to a custom authenticator stays discoverable by
    /// its key, but is reported as one the IOTA wallet cannot authenticate.
    #[tokio::test]
    async fn rotating_to_a_custom_authenticator_reports_custom() -> Result<(), IndexerError> {
        let (cluster, store, client) =
            start_test_cluster_with_read_write_indexer(DB, None, None).await;
        let owner = first_address(&cluster.wallet);
        let keypair = keypair_for(&cluster.wallet, owner);

        let package = publish_package(
            &cluster.wallet,
            [
                env!("CARGO_MANIFEST_DIR"),
                "tests",
                "data",
                CUSTOM_AUTHENTICATOR,
            ]
            .iter()
            .collect(),
        )
        .await;
        let package_metadata = cluster
            .get_latest_object_ref(&derive_package_metadata_id(package.object_id))
            .await;

        let account = build_account_with_key(
            &cluster,
            owner,
            keypair.public_key().scheme(),
            keypair.public_key().as_ref().to_vec(),
        )
        .await;
        send_from_account(
            &cluster,
            account,
            &keypair,
            rotate_to_custom_authenticator_ptb(account, package_metadata),
        )
        .await;
        indexer_wait_for_latest_checkpoint(&store, &cluster).await;

        let accounts = client
            .get_accounts_by_public_key(
                Base64::from_bytes(&prefixed_keypair_public_key(&keypair)),
                None,
            )
            .await
            .expect("the lookup must succeed");
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].status, AccountKeyLinkStatus::Active);
        assert_eq!(
            accounts[0].authenticator,
            Some(AccountAuthenticatorKind::Custom)
        );

        Ok(())
    }

    // === Helpers ===

    /// Runs a real `ClaimAccount` transaction for the cluster's first account
    /// and returns its address and the `key_id` of the claiming key.
    async fn claim(
        cluster: &TestCluster,
        build_kind: SmartAccountBuildKind,
    ) -> (Address, [u8; 32]) {
        claim_as(cluster, first_address(&cluster.wallet), build_kind).await
    }

    /// Runs a real `ClaimAccount` transaction for `owner` and returns its
    /// address and the `key_id` of the claiming key.
    async fn claim_as(
        cluster: &TestCluster,
        owner: Address,
        build_kind: SmartAccountBuildKind,
    ) -> (Address, [u8; 32]) {
        let keypair = keypair_for(&cluster.wallet, owner);
        let public_key = keypair.public_key();

        let claim = SmartAccountClaim {
            public_key_scheme: public_key.scheme().to_u8(),
            public_key_raw_bytes: public_key.as_ref().to_vec(),
            build_kind,
        };
        let kind =
            TransactionKind::new_claim_account(ClaimAccountTransaction::new_smart_account(claim));

        let rgp = cluster.get_reference_gas_price().await;
        let gas = cluster
            .wallet
            .get_gas_objects_owned_by_address(owner, None)
            .await
            .expect("gas lookup must succeed")
            .into_iter()
            .next()
            .expect("owner must have at least one gas coin");

        let tx_data = Transaction::new(
            kind,
            owner,
            gas,
            rgp * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE,
            rgp,
        );
        let response = cluster
            .wallet
            .execute_transaction_may_fail(cluster.wallet.sign_transaction(&tx_data))
            .await
            .expect("ClaimAccount transaction must execute");
        assert!(
            response
                .effects
                .as_ref()
                .expect("response must include effects")
                .status()
                .is_ok(),
            "ClaimAccount transaction must succeed"
        );

        (
            owner,
            key_id(public_key.scheme().to_u8(), public_key.as_ref()),
        )
    }

    /// Builds a shared `SmartAccount` through the public builder, with
    /// `raw_bytes` as its built-in key, sent by `sender`. Returns the account.
    async fn build_account_with_key(
        cluster: &TestCluster,
        sender: Address,
        scheme: SignatureScheme,
        raw_bytes: Vec<u8>,
    ) -> ObjectReference {
        let mut builder = ProgrammableTransactionBuilder::new();
        let public_key = public_key_arg(&mut builder, scheme, raw_bytes);
        let account_builder = framework_call(
            &mut builder,
            "smart_account",
            "builtin_auth_builder_v1",
            vec![public_key],
        );
        framework_call(
            &mut builder,
            "smart_account",
            "build_v1",
            vec![account_builder],
        );
        execute_and_find_created_account(cluster, sender, builder.finish()).await
    }

    /// Sends `pt` from `sender` and returns the one shared object it creates.
    async fn execute_and_find_created_account(
        cluster: &TestCluster,
        sender: Address,
        pt: ProgrammableTransaction,
    ) -> ObjectReference {
        let tx_data = cluster
            .test_transaction_builder_with_sender(sender)
            .await
            .programmable(pt)
            .build();
        let effects = cluster.sign_and_execute_transaction(&tx_data).await;
        assert!(
            matches!(effects.status(), ExecutionStatus::Success),
            "building the account must succeed: {:?}",
            effects.status()
        );

        *effects
            .all_changed_objects()
            .iter()
            .find_map(|(changed, kind)| {
                matches!(
                    (changed.owner(), kind),
                    (Owner::Shared(_), WriteKind::Create)
                )
                .then_some(changed.reference())
            })
            .expect("the account must be created as a shared object")
    }

    /// Builds a shared `SmartAccount` with the built-in Ed25519 authenticator
    /// and no key, sent by `sender`. Returns the account.
    async fn build_keyless_account(cluster: &TestCluster, sender: Address) -> ObjectReference {
        let mut builder = ProgrammableTransactionBuilder::new();
        let authenticator = builtin_authenticator_arg(&mut builder, SignatureScheme::Ed25519);
        let account_builder = framework_call(
            &mut builder,
            "smart_account",
            "builder_v1",
            vec![authenticator],
        );
        framework_call(
            &mut builder,
            "smart_account",
            "build_v1",
            vec![account_builder],
        );
        execute_and_find_created_account(cluster, sender, builder.finish()).await
    }

    /// Sends `pt` with the account itself as sender, authenticated by its
    /// built-in authenticator with `keypair`.
    async fn send_from_account(
        cluster: &TestCluster,
        account: ObjectReference,
        keypair: &SimpleKeypair,
        pt: ProgrammableTransaction,
    ) -> TransactionEffects {
        let sender: Address = account.object_id.into();
        let rgp = cluster.get_reference_gas_price().await;
        let gas = cluster
            .fund_address_and_return_gas(rgp, Some(20_000_000_000), sender)
            .await;
        let tx_data = Transaction::new_programmable_allow_sponsor(
            sender,
            vec![gas],
            pt,
            rgp * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE,
            rgp,
            sender,
        );

        let intent_message = IntentMessage::new(Intent::iota_transaction(), tx_data.clone());
        let signature: SimpleSignature = keypair.sign(&intent_message.signing_digest());
        let signature_bytes = UserSignature::Simple(signature).to_bytes();
        let authenticator = UserSignature::MoveAuthenticator(
            MoveAuthenticatorV1::new_with_shared_account_object(
                vec![CallArg::Pure(bcs::to_bytes(&signature_bytes).unwrap())],
                vec![],
                SharedObjectReference::new(account.object_id, account.version, false),
            )
            .into(),
        );

        let effects = cluster
            .execute_transaction(TransactionEnvelope::new(SenderSignedTransaction::new(
                tx_data,
                vec![authenticator],
            )))
            .await;
        assert!(
            matches!(effects.status(), ExecutionStatus::Success),
            "the account's transaction must succeed: {:?}",
            effects.status()
        );
        effects
    }

    /// `smart_account::rotate_builtin_auth_public_key(account, new_key)`.
    fn rotate_key_ptb(
        account: ObjectReference,
        new_keypair: &SimpleKeypair,
    ) -> ProgrammableTransaction {
        let mut builder = ProgrammableTransactionBuilder::new();
        let account = shared_account_arg(&mut builder, account);
        let new_key = new_keypair.public_key();
        let public_key = public_key_arg(&mut builder, new_key.scheme(), new_key.as_ref().to_vec());
        framework_call(
            &mut builder,
            "smart_account",
            "rotate_builtin_auth_public_key",
            vec![account, public_key],
        );
        builder.finish()
    }

    /// `smart_account::detach_builtin_auth_public_key(account)`.
    fn detach_key_ptb(account: ObjectReference) -> ProgrammableTransaction {
        let mut builder = ProgrammableTransactionBuilder::new();
        let account = shared_account_arg(&mut builder, account);
        framework_call(
            &mut builder,
            "smart_account",
            "detach_builtin_auth_public_key",
            vec![account],
        );
        builder.finish()
    }

    /// Detaches the account's key and attaches it again, in one transaction.
    fn detach_then_reattach_ptb(account: ObjectReference) -> ProgrammableTransaction {
        let mut builder = ProgrammableTransactionBuilder::new();
        let account = shared_account_arg(&mut builder, account);
        let key = framework_call(
            &mut builder,
            "smart_account",
            "detach_builtin_auth_public_key",
            vec![account],
        );
        framework_call(
            &mut builder,
            "smart_account",
            "attach_builtin_auth_public_key",
            vec![account, key],
        );
        builder.finish()
    }

    /// Switches the account to the built-in authenticator and key of
    /// `new_keypair`'s scheme, in one transaction.
    fn rotate_authenticator_and_key_ptb(
        account: ObjectReference,
        new_keypair: &SimpleKeypair,
    ) -> ProgrammableTransaction {
        let mut builder = ProgrammableTransactionBuilder::new();
        let account = shared_account_arg(&mut builder, account);
        let new_key = new_keypair.public_key();
        let authenticator = builtin_authenticator_arg(&mut builder, new_key.scheme());
        framework_call(
            &mut builder,
            "smart_account",
            "rotate_auth_function_ref_v1",
            vec![account, authenticator],
        );
        let public_key = public_key_arg(&mut builder, new_key.scheme(), new_key.as_ref().to_vec());
        framework_call(
            &mut builder,
            "smart_account",
            "rotate_builtin_auth_public_key",
            vec![account, public_key],
        );
        builder.finish()
    }

    /// Switches the account to the custom authenticator published in the
    /// package whose metadata is `package_metadata`.
    fn rotate_to_custom_authenticator_ptb(
        account: ObjectReference,
        package_metadata: ObjectReference,
    ) -> ProgrammableTransaction {
        let mut builder = ProgrammableTransactionBuilder::new();
        let account = shared_account_arg(&mut builder, account);
        let arguments = vec![
            builder
                .obj(CallArg::ImmutableOrOwned(package_metadata))
                .unwrap(),
            builder.pure(CUSTOM_AUTHENTICATOR).unwrap(),
            builder.pure("authenticate").unwrap(),
        ];
        let authenticator = builder.programmable_move_call(
            IOTA_FRAMEWORK_PACKAGE_ID,
            Identifier::new("authenticator_function").unwrap(),
            Identifier::new("create_auth_function_ref_v1").unwrap(),
            vec![smart_account_type()],
            arguments,
        );
        framework_call(
            &mut builder,
            "smart_account",
            "rotate_auth_function_ref_v1",
            vec![account, authenticator],
        );
        builder.finish()
    }

    /// The built-in authenticator for `scheme`, for a `SmartAccount`.
    fn builtin_authenticator_arg(
        builder: &mut ProgrammableTransactionBuilder,
        scheme: SignatureScheme,
    ) -> iota_sdk_types::Argument {
        let function = match scheme {
            SignatureScheme::Ed25519 => "ed25519_authenticator_function_ref_v1",
            SignatureScheme::Secp256k1 => "secp256k1_authenticator_function_ref_v1",
            SignatureScheme::Secp256r1 => "secp256r1_authenticator_function_ref_v1",
            other => panic!("no built-in account test support for {other:?}"),
        };
        builder.programmable_move_call(
            IOTA_FRAMEWORK_PACKAGE_ID,
            Identifier::new("builtin_authenticator_functions").unwrap(),
            Identifier::new(function).unwrap(),
            vec![smart_account_type()],
            vec![],
        )
    }

    fn smart_account_type() -> TypeTag {
        TypeTag::from_str(&format!(
            "{IOTA_FRAMEWORK_PACKAGE_ID}::smart_account::SmartAccount"
        ))
        .unwrap()
    }

    fn shared_account_arg(
        builder: &mut ProgrammableTransactionBuilder,
        account: ObjectReference,
    ) -> iota_sdk_types::Argument {
        builder
            .obj(CallArg::Shared(SharedObjectReference::new(
                account.object_id,
                account.version,
                true,
            )))
            .unwrap()
    }

    /// A Move `PublicKey`, built in the PTB since pure arguments cannot carry
    /// one.
    fn public_key_arg(
        builder: &mut ProgrammableTransactionBuilder,
        scheme: SignatureScheme,
        raw_bytes: Vec<u8>,
    ) -> iota_sdk_types::Argument {
        let scheme_function = match scheme {
            SignatureScheme::Ed25519 => "ed25519",
            SignatureScheme::Secp256k1 => "secp256k1",
            SignatureScheme::Secp256r1 => "secp256r1",
            other => panic!("no built-in account test support for {other:?}"),
        };
        let scheme = framework_call(builder, "signature_scheme", scheme_function, vec![]);
        let raw_bytes = builder.pure(raw_bytes).unwrap();
        framework_call(builder, "public_key", "create", vec![scheme, raw_bytes])
    }

    fn framework_call(
        builder: &mut ProgrammableTransactionBuilder,
        module: &str,
        function: &str,
        arguments: Vec<iota_sdk_types::Argument>,
    ) -> iota_sdk_types::Argument {
        builder.programmable_move_call(
            IOTA_FRAMEWORK_PACKAGE_ID,
            Identifier::new(module).unwrap(),
            Identifier::new(function).unwrap(),
            vec![],
            arguments,
        )
    }

    fn fresh_secp256k1_keypair() -> SimpleKeypair {
        SimpleKeypair::from(Secp256k1PrivateKey::random_with(StdRng::from_seed([7; 32])))
    }

    fn keypair_key_id(keypair: &SimpleKeypair) -> [u8; 32] {
        let public_key = keypair.public_key();
        key_id(public_key.scheme().to_u8(), public_key.as_ref())
    }

    fn prefixed_keypair_public_key(keypair: &SimpleKeypair) -> Vec<u8> {
        let public_key = keypair.public_key();
        [
            vec![public_key.scheme().to_u8()],
            public_key.as_ref().to_vec(),
        ]
        .concat()
    }

    fn two_addresses(wallet: &WalletContext) -> [Address; 2] {
        let addresses = wallet.config().keystore().addresses();
        assert!(
            addresses.len() >= 2,
            "wallet must have at least two accounts"
        );
        [addresses[0], addresses[1]]
    }

    fn prefixed_public_key(cluster: &TestCluster, owner: Address) -> Vec<u8> {
        let public_key = keypair_for(&cluster.wallet, owner).public_key();
        [
            vec![public_key.scheme().to_u8()],
            public_key.as_ref().to_vec(),
        ]
        .concat()
    }

    fn first_address(wallet: &WalletContext) -> Address {
        wallet
            .config()
            .keystore()
            .addresses()
            .into_iter()
            .next()
            .expect("wallet must have at least one account")
    }

    fn keypair_for(wallet: &WalletContext, owner: Address) -> SimpleKeypair {
        wallet
            .config()
            .keystore()
            .get_key(&owner)
            .expect("keypair must exist for owner")
            .as_keypair()
            .expect("stored key must be a keypair")
            .clone()
    }

    fn links_for(
        store: &PgIndexerStore,
        key_id: &[u8; 32],
    ) -> Result<Vec<StoredAccountKeyLink>, IndexerError> {
        let mut conn = get_pool_connection(&store.blocking_cp())?;
        account_key_links::table
            .filter(account_key_links::key_id.eq(key_id.to_vec()))
            .select(StoredAccountKeyLink::as_select())
            .load(&mut conn)
            .map_err(|_| IndexerError::PostgresRead)
    }

    fn smart_account_for(
        store: &PgIndexerStore,
        account: &Address,
    ) -> Result<Option<StoredSmartAccount>, IndexerError> {
        let mut conn = get_pool_connection(&store.blocking_cp())?;
        smart_accounts::table
            .filter(smart_accounts::account_id.eq(account.as_bytes().to_vec()))
            .select(StoredSmartAccount::as_select())
            .load(&mut conn)
            .map(|rows: Vec<StoredSmartAccount>| rows.into_iter().next())
            .map_err(|_| IndexerError::PostgresRead)
    }

    /// Waits until the indexer's JSON-RPC reader, which starts in the
    /// background, answers a lookup.
    async fn wait_for_reader(client: &HttpClient) {
        let any_key = Base64::from_bytes(&[0x00u8; 33]);
        tokio::time::timeout(Duration::from_secs(30), async {
            while client
                .get_accounts_by_public_key(any_key.clone(), None)
                .await
                .is_err()
            {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await
        .expect("timeout waiting for the indexer reader to start");
    }

    fn all_links(store: &PgIndexerStore) -> Result<Vec<StoredAccountKeyLink>, IndexerError> {
        let mut conn = get_pool_connection(&store.blocking_cp())?;
        account_key_links::table
            .order((account_key_links::key_id, account_key_links::account_id))
            .select(StoredAccountKeyLink::as_select())
            .load(&mut conn)
            .map_err(|_| IndexerError::PostgresRead)
    }

    fn authenticator_for(
        store: &PgIndexerStore,
        account: &Address,
    ) -> Result<Option<StoredAccountAuthenticator>, IndexerError> {
        let mut conn = get_pool_connection(&store.blocking_cp())?;
        account_authenticators::table
            .filter(account_authenticators::account_id.eq(account.as_bytes().to_vec()))
            .select(StoredAccountAuthenticator::as_select())
            .load(&mut conn)
            .map(|rows: Vec<StoredAccountAuthenticator>| rows.into_iter().next())
            .map_err(|_| IndexerError::PostgresRead)
    }

    fn all_account_authenticators(
        store: &PgIndexerStore,
    ) -> Result<Vec<StoredAccountAuthenticator>, IndexerError> {
        let mut conn = get_pool_connection(&store.blocking_cp())?;
        account_authenticators::table
            .order(account_authenticators::account_id)
            .select(StoredAccountAuthenticator::as_select())
            .load(&mut conn)
            .map_err(|_| IndexerError::PostgresRead)
    }

    fn all_smart_accounts(store: &PgIndexerStore) -> Result<Vec<StoredSmartAccount>, IndexerError> {
        let mut conn = get_pool_connection(&store.blocking_cp())?;
        smart_accounts::table
            .order(smart_accounts::account_id)
            .select(StoredSmartAccount::as_select())
            .load(&mut conn)
            .map_err(|_| IndexerError::PostgresRead)
    }
}
