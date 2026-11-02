// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use iota_ledger::{Ledger, SignedTransaction};
use iota_sdk_types::{Address, Intent, ObjectId, SignatureScheme, Transaction};
use iota_types::{crypto::PublicKey, object::Object};
use tracing::warn;

mod errors;
pub use errors::LedgerSignerError;
mod utils;

/// Loads the input objects of a transaction so the Ledger device can display
/// their contents while signing (clear-signing). Supplied by the caller to keep
/// this crate independent of any particular node API.
#[async_trait::async_trait]
pub trait ObjectProvider: Send + Sync {
    async fn load_objects(
        &self,
        object_ids: Vec<ObjectId>,
    ) -> Result<Vec<Object>, LedgerSignerError>;
}

pub struct LedgerSigner {
    path: bip32::DerivationPath,
    ledger: Ledger,
    object_provider: Option<Arc<dyn ObjectProvider>>,
}

impl LedgerSigner {
    pub fn new_with_default(
        path: bip32::DerivationPath,
        object_provider: Option<Arc<dyn ObjectProvider>>,
    ) -> Result<Self, LedgerSignerError> {
        let ledger = Ledger::new_with_default()?;
        Ok(Self::new(ledger, path, object_provider))
    }

    pub fn new(
        ledger: Ledger,
        path: bip32::DerivationPath,
        object_provider: Option<Arc<dyn ObjectProvider>>,
    ) -> Self {
        LedgerSigner {
            ledger,
            path,
            object_provider,
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

    pub async fn sign_transaction(
        &self,
        transaction: &Transaction,
        address: &Address,
    ) -> Result<SignedTransaction, LedgerSignerError> {
        let objects = if let Some(provider) = &self.object_provider {
            match load_clear_sign_objects(provider.as_ref(), transaction).await {
                Ok(objects) => objects,
                Err(e) => {
                    warn!("Failed to load objects: {e}. Falling back to blind-signing.");
                    vec![]
                }
            }
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

async fn load_clear_sign_objects(
    provider: &dyn ObjectProvider,
    transaction: &Transaction,
) -> Result<Vec<Object>, LedgerSignerError> {
    let object_ids = utils::object_ids_from_transaction(transaction)?;

    if object_ids.is_empty() {
        return Ok(vec![]);
    }

    provider.load_objects(object_ids).await
}
