// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use anyhow::{Result, bail};
use iota_keys::keystore::{AccountKeystore, StoredKey};
use iota_ledger::Ledger;
use iota_ledger_signer::LedgerSigner;
use iota_sdk::wallet_context::WalletContext;
use iota_types::{
    base_types::IotaAddress,
    crypto::Signature,
    transaction::{TransactionData, TransactionDataAPI},
};
use serde::Serialize;
use shared_crypto::intent::Intent;

pub(crate) const EXTERNAL_KEY_SOURCE_LEDGER: &str = "ledger";

pub(crate) async fn sign_transaction(
    context: &mut WalletContext,
    tx_data: &TransactionData,
) -> Result<Signature> {
    let iota_client = context.get_client().await?;
    let sender = &tx_data.sender();

    let signature = {
        let key = context.config().keystore().get_key(sender)?;

        match key {
            StoredKey::KeyPair(_) => context.config().keystore().sign_secure(
                sender,
                tx_data,
                Intent::iota_transaction(),
            )?,
            StoredKey::External {
                derivation_path,
                source,
                ..
            } => {
                match source.as_str() {
                    EXTERNAL_KEY_SOURCE_LEDGER => {
                        if let Some(derivation_path) = derivation_path {
                            let signer = LedgerSigner::new_with_default(
                                derivation_path.clone(),
                                Some(iota_client.clone()),
                            )?;
                            // pass the transaction sender to the signer to ensure the correct
                            // key is used
                            signer
                                .sign_transaction(tx_data, sender)
                                .await
                                .map(|s| s.signature)?
                        } else {
                            bail!(
                                "Derivation path is required for Ledger signing. Please specify it in the keystore."
                            );
                        }
                    }
                    _ => {
                        bail!("External signing is not supported for source: {source}")
                    }
                }
            }
        }
    };
    Ok(signature)
}

pub(crate) fn sign_secure<T>(
    keystore: &impl AccountKeystore,
    address: &IotaAddress,
    msg: &T,
    intent: Intent,
) -> Result<Signature>
where
    T: Serialize,
{
    let key = keystore.get_key(address)?;
    let iota_signature = match key {
        StoredKey::KeyPair(_) => keystore.sign_secure(address, &msg, intent)?,
        StoredKey::External {
            derivation_path,
            source,
            ..
        } => {
            match source.as_str() {
                EXTERNAL_KEY_SOURCE_LEDGER => {
                    if let Some(derivation_path) = derivation_path {
                        let ledger = Ledger::new_with_default()?;
                        // Pass the expected address to the ledger to ensure the signature is for
                        // the correct address.
                        ledger
                            .sign_intent(derivation_path, address, intent, &msg, vec![])?
                            .signature
                    } else {
                        bail!(
                            "Derivation path is required for Ledger signing. Please specify it in the keystore."
                        );
                    }
                }
                _ => {
                    bail!("External signing is not supported for source: {source}")
                }
            }
        }
    };
    Ok(iota_signature)
}
