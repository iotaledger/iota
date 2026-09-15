// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[expect(dead_code)]
#[cfg(feature = "pg_integration")]
mod common;
#[cfg(feature = "pg_integration")]
mod account_key_links_tests {
    use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl, SelectableHelper};
    use fastcrypto::encoding::Base64;
    use iota_indexer::{
        account_key_events::LinkSource,
        db::get_pool_connection,
        errors::IndexerError,
        models::{
            account_key_links::{LINK_STATUS_ACTIVE, StoredAccountKeyLink},
            claimed_accounts::StoredClaimedAccount,
        },
        schema::{account_key_links, claimed_accounts},
        store::PgIndexerStore,
    };
    use iota_json_rpc_api::ExtendedApiClient;
    use iota_json_rpc_types::{
        AccountKeyLinkSource, AccountKeyLinkStatus, IotaTransactionBlockEffectsAPI,
    };
    use iota_keys::keystore::AccountKeystore;
    use iota_sdk::wallet_context::WalletContext;
    use iota_sdk_crypto::simple::SimpleKeypair;
    use iota_sdk_types::{
        Address, ClaimAccountTransaction, SmartAccountBuildKind, SmartAccountClaim, Transaction,
        TransactionKind,
    };
    use iota_types::{
        account_abstraction::public_key::key_id,
        transaction::{TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE, TransactionAPI},
    };
    use test_cluster::TestCluster;

    use crate::common::{
        indexer_wait_for_latest_checkpoint, start_test_cluster_with_read_write_indexer,
    };

    const DB: &str = "account_key_links_tests_db";

    /// A claim must be indexed as a claimed account, not as a plain attachment,
    /// even though the claim transaction emits both events for the same pair.
    #[tokio::test]
    async fn a_mutable_claim_is_indexed_as_a_claimed_account() -> Result<(), IndexerError> {
        let (cluster, store, _client) =
            start_test_cluster_with_read_write_indexer(DB, None, None).await;

        let (owner, key_id) = claim(&cluster, SmartAccountBuildKind::Mutable).await;
        indexer_wait_for_latest_checkpoint(&store, &cluster).await;

        let links = links_for(&store, &key_id)?;
        assert_eq!(links.len(), 1, "a claim must collapse to a single link row");
        assert_eq!(links[0].account_id, owner.as_bytes().to_vec());
        assert_eq!(links[0].status, LINK_STATUS_ACTIVE);
        assert_eq!(
            links[0].source,
            LinkSource::Claim as i16,
            "the claim must win over the attachment emitted in the same transaction"
        );

        let claimed =
            claimed_for(&store, &owner)?.expect("the account must be recorded as claimed");
        assert_eq!(claimed.key_id, key_id.to_vec());
        assert!(!claimed.immutable);

        Ok(())
    }

    #[tokio::test]
    async fn an_immutable_claim_is_recorded_as_immutable() -> Result<(), IndexerError> {
        let (cluster, store, _client) =
            start_test_cluster_with_read_write_indexer(DB, None, None).await;

        let (owner, _) = claim(&cluster, SmartAccountBuildKind::Immutable).await;
        indexer_wait_for_latest_checkpoint(&store, &cluster).await;

        let claimed =
            claimed_for(&store, &owner)?.expect("the account must be recorded as claimed");
        assert!(
            claimed.immutable,
            "an immutable account's links can never change, and the index must say so"
        );

        Ok(())
    }

    /// The RPC surface: a wallet holding only the key recovers the account, and
    /// can tell it apart from one someone else created with the same key.
    #[tokio::test]
    async fn the_rpc_returns_the_claimed_account_for_its_key() -> Result<(), IndexerError> {
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
        assert_eq!(accounts[0].source, AccountKeyLinkSource::Claim);
        assert!(accounts[0].claimed);
        assert_eq!(accounts[0].immutable, Some(false));

        Ok(())
    }

    /// An empty key carries no scheme flag, so there is nothing to hash.
    #[tokio::test]
    async fn the_rpc_rejects_an_empty_public_key() -> Result<(), IndexerError> {
        let (_cluster, _store, client) =
            start_test_cluster_with_read_write_indexer(DB, None, None).await;

        assert!(
            client
                .get_accounts_by_public_key(Base64::from_bytes(&[]), None)
                .await
                .is_err()
        );

        Ok(())
    }

    /// A key that never touched the chain resolves to nothing, rather than to
    /// the address it derives.
    #[tokio::test]
    async fn an_unseen_key_returns_no_accounts() -> Result<(), IndexerError> {
        let (_cluster, _store, client) =
            start_test_cluster_with_read_write_indexer(DB, None, None).await;

        let unseen = [vec![0x00u8], vec![0xAB; 32]].concat();
        let accounts = client
            .get_accounts_by_public_key(Base64::from_bytes(&unseen), None)
            .await
            .expect("the lookup must succeed");

        assert!(accounts.is_empty());

        Ok(())
    }

    // === Helpers ===

    /// Runs a real `ClaimAccount` transaction for the cluster's first account
    /// and returns its address and the `key_id` of the claiming key.
    async fn claim(
        cluster: &TestCluster,
        build_kind: SmartAccountBuildKind,
    ) -> (Address, [u8; 32]) {
        let owner = first_address(&cluster.wallet);
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
            .map_err(|e| IndexerError::PostgresRead(e.to_string()))
    }

    fn claimed_for(
        store: &PgIndexerStore,
        account: &Address,
    ) -> Result<Option<StoredClaimedAccount>, IndexerError> {
        let mut conn = get_pool_connection(&store.blocking_cp())?;
        claimed_accounts::table
            .filter(claimed_accounts::account_id.eq(account.as_bytes().to_vec()))
            .select(StoredClaimedAccount::as_select())
            .load(&mut conn)
            .map(|rows: Vec<StoredClaimedAccount>| rows.into_iter().next())
            .map_err(|e| IndexerError::PostgresRead(e.to_string()))
    }
}
