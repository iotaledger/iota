// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_ledger::{Ledger, SignedTransaction};
use iota_sdk::{IotaClient, types::crypto::PublicKey};
use iota_sdk_types::{Address, Intent, SignatureScheme, Transaction};

mod errors;
pub use errors::LedgerSignerError;
mod utils;

pub struct LedgerSigner {
    path: bip32::DerivationPath,
    ledger: Ledger,
    client: Option<IotaClient>,
}

impl LedgerSigner {
    pub fn new_with_default(
        path: bip32::DerivationPath,
        client: Option<IotaClient>,
    ) -> Result<Self, LedgerSignerError> {
        let ledger = Ledger::new_with_default()?;
        Ok(Self::new(ledger, path, client))
    }

    pub fn new(ledger: Ledger, path: bip32::DerivationPath, client: Option<IotaClient>) -> Self {
        LedgerSigner {
            ledger,
            path,
            client,
        }
    }

    pub fn get_signature_scheme(&self) -> SignatureScheme {
        self.ledger.get_signature_scheme()
    }

    pub fn get_address(&self) -> Result<Address, LedgerSignerError> {
        let public_key = self.ledger.get_public_key(&self.path)?;
        Ok(public_key.address)
    }

    pub fn get_public_key(&self) -> Result<PublicKey, LedgerSignerError> {
        let public_key = self.ledger.get_public_key(&self.path)?;
        Ok(public_key.public_key)
    }

    /// With a client, the transaction's owned and immutable input objects are
    /// sent to the device for clear signing, and signing fails if any of them
    /// cannot be loaded. Without a client, the device blind-signs.
    pub async fn sign_transaction(
        &self,
        transaction: &Transaction,
        address: &Address,
    ) -> Result<SignedTransaction, LedgerSignerError> {
        let objects = if let Some(client) = &self.client {
            utils::load_objects_with_client(client, transaction).await?
        } else {
            vec![]
        };

        self.ledger
            .sign_intent(
                &self.path,
                address,
                Intent::iota_transaction(),
                transaction,
                objects,
            )
            .map_err(LedgerSignerError::from)
    }

    pub fn sign_message(
        &self,
        message: Vec<u8>,
        address: &Address,
    ) -> Result<SignedTransaction, LedgerSignerError> {
        self.ledger
            .sign_intent(
                &self.path,
                address,
                Intent::personal_message(),
                &message,
                vec![],
            )
            .map_err(LedgerSignerError::from)
    }
}
