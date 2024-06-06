// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_keys::keystore::AccountKeystore;
use iota_sdk::{wallet_context::WalletContext, IotaClient, IotaClientBuilder};
use iota_types::{
    base_types::IotaAddress,
    crypto::{KeypairTraits, Signature},
    transaction::TransactionData,
};
use shared_crypto::intent::Intent;
use tracing::{info, info_span, Instrument};

use super::Cluster;
use crate::cluster::new_wallet_context_from_cluster;

pub struct WalletClient {
    wallet_context: WalletContext,
    address: IotaAddress,
    fullnode_client: IotaClient,
}

#[allow(clippy::borrowed_box)]
impl WalletClient {
    pub async fn new_from_cluster(cluster: &(dyn Cluster + Sync + Send)) -> Self {
        let key = cluster.user_key();
        let address: IotaAddress = key.public().into();
        let wallet_context = new_wallet_context_from_cluster(cluster, key)
            .instrument(info_span!("init_wallet_context_for_test_user"));

        let rpc_url = String::from(cluster.fullnode_url());
        info!("Use fullnode rpc: {}", &rpc_url);
        let fullnode_client = IotaClientBuilder::default().build(rpc_url).await.unwrap();

        Self {
            wallet_context: wallet_context.into_inner(),
            address,
            fullnode_client,
        }
    }

    pub fn get_wallet(&self) -> &WalletContext {
        &self.wallet_context
    }

    pub fn get_wallet_mut(&mut self) -> &mut WalletContext {
        &mut self.wallet_context
    }

    pub fn get_wallet_address(&self) -> IotaAddress {
        self.address
    }

    pub fn get_fullnode_client(&self) -> &IotaClient {
        &self.fullnode_client
    }

    pub fn sign(&self, txn_data: &TransactionData, desc: &str) -> Signature {
        self.get_wallet()
            .config
            .keystore
            .sign_secure(&self.address, txn_data, Intent::iota_transaction())
            .unwrap_or_else(|e| panic!("Failed to sign transaction for {}. {}", desc, e))
    }
}
