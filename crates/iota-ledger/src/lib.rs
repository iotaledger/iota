// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::vec;

mod transport;
use transport::{Transport, TransportTypes, create_transport};

pub use crate::api::errors::LedgerError;
mod api;
use iota_types::{
    base_types::IotaAddress,
    crypto::{Ed25519IotaSignature, Signature, SignatureScheme, ToFromBytes},
    object::Object,
};
use serde::Serialize;
use shared_crypto::intent::{Intent, IntentMessage};

use crate::api::{bolos, exit, get_public_key, sign_transaction};
pub use crate::api::{get_public_key::PublicKeyResult, get_version::Version};

/// Get Ledger by transport_type
fn get_ledger_by_type(transport_type: TransportTypes) -> Result<Ledger, LedgerError> {
    let transport = create_transport(transport_type)?;
    Ok(crate::Ledger::new(transport))
}

pub struct Ledger {
    transport: Transport,
}

pub struct SignedTransaction {
    pub signature: Signature,
    pub address: IotaAddress,
}

impl Ledger {
    pub fn new_with_default() -> Result<Ledger, LedgerError> {
        if std::env::var("LEDGER_SIMULATOR").is_ok() {
            get_ledger_by_type(TransportTypes::TCP)
        } else {
            get_ledger_by_type(TransportTypes::NativeHID)
        }
    }

    pub fn new_with_native_hid() -> Result<Ledger, LedgerError> {
        get_ledger_by_type(TransportTypes::NativeHID)
    }

    pub fn new_with_simulator() -> Result<Ledger, LedgerError> {
        get_ledger_by_type(TransportTypes::TCP)
    }

    fn new(transport: Transport) -> Self {
        Ledger { transport }
    }

    /// Get currently opened app
    /// If "BOLOS" is returned, the dashboard is open
    pub fn is_app_open(&self) -> Result<bool, LedgerError> {
        let app = bolos::app_get_name::exec(&self.transport)?;
        Ok(app.app == "IOTA")
    }

    /// Open app on the nano s/x
    /// Only works if dashboard is open
    pub fn bolos_open_app(&self) -> Result<(), LedgerError> {
        bolos::app_open::exec(&self.transport, "IOTA".to_string())
    }

    /// Close current opened app on the nano s/x
    /// Only works if an app is open
    pub fn bolos_exit_app(&self) -> Result<(), LedgerError> {
        bolos::app_exit::exec(&self.transport)
    }

    fn transport(&self) -> &Transport {
        &self.transport
    }

    pub fn get_version(&self) -> Result<Version, LedgerError> {
        let version = crate::api::get_version::exec(&self.transport)?;
        Ok(version)
    }

    pub fn verify_address(
        &self,
        bip32: &bip32::DerivationPath,
    ) -> Result<PublicKeyResult, LedgerError> {
        get_public_key::exec(&self.transport, bip32, true)
    }

    pub fn get_public_key(
        &self,
        bip32: &bip32::DerivationPath,
    ) -> Result<PublicKeyResult, LedgerError> {
        get_public_key::exec(&self.transport, bip32, false)
    }

    pub fn get_signature_scheme(&self) -> SignatureScheme {
        SignatureScheme::ED25519
    }

    pub fn sign_intent<T: Serialize>(
        &self,
        bip32: &bip32::DerivationPath,
        address: &IotaAddress,
        intent: Intent,
        msg: &T,
        objects: Vec<Object>,
    ) -> Result<SignedTransaction, LedgerError> {
        let version = self.get_version()?;
        let key_response = self.get_public_key(bip32)?;

        if key_response.address != *address {
            return Err(LedgerError::AddressMismatch);
        }

        let intent_msg = IntentMessage::new(intent, msg);
        let intent_bytes = bcs::to_bytes(&intent_msg).map_err(|_| LedgerError::Serialization)?;

        let signature = (if version.major > 0 {
            let bcs_objects: Vec<Vec<u8>> = objects
                .iter()
                .map(|o| bcs::to_bytes(&o).map_err(|_| LedgerError::Serialization))
                .collect::<Result<_, _>>()?;
            // If the major version is greater than 0, we assume it supports clear signing
            sign_transaction::exec(self.transport(), bip32, intent_bytes, bcs_objects)
        } else {
            sign_transaction::exec(self.transport(), bip32, intent_bytes, vec![])
        })?;

        let mut signature_bytes: Vec<u8> = Vec::new();
        signature_bytes.extend_from_slice(&[self.get_signature_scheme().flag()]);
        signature_bytes.extend_from_slice(&signature.bytes);
        signature_bytes.extend_from_slice(key_response.public_key.as_ref());

        Ok(SignedTransaction {
            signature: Ed25519IotaSignature::from_bytes(&signature_bytes)
                .map_err(|_| LedgerError::Serialization)?
                .into(),
            address: IotaAddress::from_bytes(key_response.address)
                .map_err(|_| LedgerError::Serialization)?,
        })
    }

    pub fn exit_app(&self) -> Result<(), LedgerError> {
        exit::exec(&self.transport)
    }
}
