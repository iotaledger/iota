// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{str::FromStr, sync::Arc};

use async_trait::async_trait;
use clap::{Arg, Command};
use iota_grpc_client::{GrpcClient, read_mask_fields::ObjectField};
use iota_ledger_signer::{LedgerSigner, LedgerSignerError, ObjectProvider};
use iota_sdk_types::{ObjectId, Transaction};
use iota_types::object::Object;

/// Loads the transaction's input objects over gRPC so the Ledger device can
/// clear-sign them.
struct GrpcObjectProvider {
    client: GrpcClient,
}

#[async_trait]
impl ObjectProvider for GrpcObjectProvider {
    async fn load_objects(
        &self,
        object_ids: Vec<ObjectId>,
    ) -> Result<Vec<Object>, LedgerSignerError> {
        let objects = self
            .client
            .objects(object_ids)
            .read_mask(ObjectField::BCS)
            .await
            .map_err(|e| LedgerSignerError::ObjectProvider(e.to_string()))?
            .into_inner()
            .into_iter()
            .filter_map(|proto_object| Some(proto_object.ok()?.object().ok()?.into()))
            .collect();
        Ok(objects)
    }
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let matches = Command::new("ledger_signer")
        .version("1.0")
        .arg(
            Arg::new("bip32-path")
                .short('p')
                .long("path")
                .help("bip32 path to use (default \"m/44'/4218'/0'/0'/0'\")")
                .value_name("PATH")
                .required(false),
        )
        .arg(
            Arg::new("network")
                .short('n')
                .long("network")
                .help("select the network to connect to for fetching inputs (local, devnet, testnet, mainnet or custom URL)")
                .required(false),
        )
        .arg(
            Arg::new("transaction")
                .long("tx")
                .help("transaction bytes in base64 format")
                .required(true),
        )
        .get_matches();

    let derivation_path = bip32::DerivationPath::from_str(
        matches
            .get_one::<String>("bip32-path")
            .map(|s| s.as_str())
            .unwrap_or("m/44'/4218'/0'/0'/0'"),
    )?;

    let network = matches.get_one::<String>("network").map(|s| s.as_str());
    let client = match network {
        Some("local") => Some(GrpcClient::new_localnet()?),
        Some("devnet") => Some(GrpcClient::new_devnet()?),
        Some("testnet") => Some(GrpcClient::new_testnet()?),
        Some("mainnet") => Some(GrpcClient::new_mainnet()?),
        Some(url) => Some(GrpcClient::new(url)?),
        None => None,
    };
    let object_provider: Option<Arc<dyn ObjectProvider>> = match client {
        Some(client) => {
            println!("Connected to IOTA network: {} via gRPC", network.unwrap(),);
            Some(Arc::new(GrpcObjectProvider { client }))
        }
        None => {
            println!("No IOTA network specified, only blind-signing supported.");
            None
        }
    };

    let transaction = Transaction::from_base64(matches.get_one::<String>("transaction").unwrap())
        .map_err(|e| anyhow::format_err!("Invalid transaction: {e}"))?;

    let signer = LedgerSigner::new_with_default(derivation_path, object_provider)?;

    // Get the signer's address
    let address = signer.get_address()?;
    println!("Signer address: {address}");

    let signed_tx = signer.sign_transaction(&transaction, &address).await?;
    println!("Signature: {}", signed_tx.signature.to_base64());

    Ok(())
}
