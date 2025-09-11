// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::fmt;

use anyhow::{Result, bail};
use iota_keys::keystore::{AccountKeystore, StoredKey};
use iota_ledger::Ledger;
use iota_ledger_signer::LedgerSigner;
use iota_sdk::wallet_context::WalletContext;
use iota_types::{base_types::IotaAddress, crypto::Signature, transaction::TransactionData};
use serde::Serialize;
use shared_crypto::intent::Intent;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ExternalKeySource {
    Ledger,
    Unknown(String),
}

impl ExternalKeySource {
    pub(crate) fn as_str(&self) -> &str {
        match self {
            ExternalKeySource::Ledger => "ledger",
            ExternalKeySource::Unknown(source) => source.as_str(),
        }
    }
}

impl From<&str> for ExternalKeySource {
    fn from(s: &str) -> Self {
        match s {
            "ledger" => ExternalKeySource::Ledger,
            other => ExternalKeySource::Unknown(other.to_string()),
        }
    }
}

impl fmt::Display for ExternalKeySource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

pub(crate) async fn sign_transaction(
    context: &mut WalletContext,
    tx_data: &TransactionData,
    signer_address: &IotaAddress,
) -> Result<Signature> {
    let iota_client = context.get_client().await?;

    let key = context.config().keystore().get_key(signer_address)?;

    match key {
        StoredKey::KeyPair(_) => Ok(context.config().keystore().sign_secure(
            signer_address,
            tx_data,
            Intent::iota_transaction(),
        )?),
        StoredKey::External {
            derivation_path,
            source,
            ..
        } => {
            match ExternalKeySource::from(source.as_str()) {
                ExternalKeySource::Ledger => {
                    let Some(derivation_path) = derivation_path else {
                        bail!(
                            "Derivation path is required for Ledger signing. Please specify it in the keystore."
                        );
                    };

                    let signer =
                        LedgerSigner::new_with_default(derivation_path.clone(), Some(iota_client))?;
                    // pass the transaction sender to the signer to ensure the correct
                    // key is used
                    Ok(signer
                        .sign_transaction(tx_data, signer_address)
                        .await
                        .map(|s| s.signature)?)
                }
                ExternalKeySource::Unknown(name) => {
                    bail!("External signing is not supported for source: {name}")
                }
            }
        }
    }
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
    match key {
        StoredKey::KeyPair(_) => Ok(keystore.sign_secure(address, &msg, intent)?),
        StoredKey::External {
            derivation_path,
            source,
            ..
        } => {
            match ExternalKeySource::from(source.as_str()) {
                ExternalKeySource::Ledger => {
                    let Some(derivation_path) = derivation_path else {
                        bail!(
                            "Derivation path is required for Ledger signing. Please specify it in the keystore."
                        );
                    };

                    let ledger = Ledger::new_with_default()?;
                    // Pass the expected address to the ledger to ensure the signature is for
                    // the correct address.
                    Ok(ledger
                        .sign_intent(derivation_path, address, intent, &msg, vec![])?
                        .signature)
                }
                ExternalKeySource::Unknown(name) => {
                    bail!("External signing is not supported for source: {name}")
                }
            }
        }
    }
}
