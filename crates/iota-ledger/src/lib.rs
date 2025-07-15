// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{thread, time, vec};

mod transport;
use transport::{Transport, TransportType, create_transport};

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

pub struct Ledger {
    transport: Transport,
}

pub struct SignedTransaction {
    pub signature: Signature,
    pub address: IotaAddress,
}

const IOTA_APP_NAME: &str = "IOTA";
const DASHBOARD_APP_NAME: &str = "BOLOS";

impl Ledger {
    pub fn new_with_default() -> Result<Ledger, LedgerError> {
        let transport = if std::env::var("LEDGER_SIMULATOR").is_ok() {
            create_transport(TransportType::TCP)?
        } else {
            create_transport(TransportType::NativeHID)?
        };
        Ok(crate::Ledger::new(transport))
    }

    pub fn new_with_native_hid() -> Result<Ledger, LedgerError> {
        Ok(crate::Ledger::new(create_transport(
            TransportType::NativeHID,
        )?))
    }

    pub fn new_with_simulator() -> Result<Ledger, LedgerError> {
        Ok(crate::Ledger::new(create_transport(TransportType::TCP)?))
    }

    fn new(transport: Transport) -> Self {
        Ledger { transport }
    }

    fn recreate_transport(&mut self) -> Result<(), LedgerError> {
        thread::sleep(time::Duration::from_secs(3));
        self.transport.recreate()
    }

    /// Check if the IOTA app is open on the Ledger device
    pub fn is_app_open(&self) -> Result<bool, LedgerError> {
        let app = bolos::app_get_name::exec(&self.transport)?;
        Ok(app.app == IOTA_APP_NAME)
    }

    /// Only works if dashboard is open
    /// This will re-create the transport after opening the app
    fn bolos_open_app(&mut self) -> Result<(), LedgerError> {
        if self.is_app_open()? {
            return Ok(());
        }
        bolos::app_open::exec(&self.transport, IOTA_APP_NAME.to_string())?;
        self.recreate_transport()
    }

    /// Close current opened app
    /// Only works if an app is open
    /// This will re-create the transport after closing the app
    fn bolos_exit_app(&mut self) -> Result<(), LedgerError> {
        bolos::app_exit::exec(&self.transport)?;
        self.recreate_transport()
    }

    /// Ensure the IOTA app is open
    /// If the app is not open, it will open it
    /// If another app is open, it will close it first
    /// This will re-create the transport after closing the app
    pub fn ensure_app_is_open(&mut self) -> Result<(), LedgerError> {
        if self.transport.is_simulator() {
            return Ok(());
        }

        match bolos::app_get_name::exec(&self.transport)?.app.as_str() {
            IOTA_APP_NAME => {
                // App is already open
                return Ok(());
            }
            DASHBOARD_APP_NAME => {
                // Dashboard is open, we need to open the IOTA app
                self.bolos_open_app()?;
            }
            _ => {
                // Some other app is open, we need to close it first
                self.bolos_exit_app()?;
                self.bolos_open_app()?;
            }
        }
        Ok(())
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

    /// Close the IOTA app from within
    /// This will re-create the transport after closing the app
    pub fn exit_app(&mut self) -> Result<(), LedgerError> {
        exit::exec(&self.transport)?;
        self.recreate_transport()
    }
}
