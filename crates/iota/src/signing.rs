// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::fmt;

use anyhow::{Result, bail};
use iota_json_rpc_types::IotaObjectDataOptions;
use iota_keys::keystore::{AccountKeystore, StoredKey};
use iota_ledger::Ledger;
use iota_ledger_signer::LedgerSigner;
use iota_sdk::wallet_context::WalletContext;
use iota_sdk_types::crypto::Intent;
use iota_types::{
    base_types::{IotaAddress, ObjectID, SequenceNumber},
    crypto::Signature,
    move_authenticator::MoveAuthenticator,
    signature::GenericSignature,
    transaction::{CallArg, TransactionData},
    type_input::TypeInput,
};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignData {
    pub iota_address: IotaAddress,
    // Base64 encoded string of serialized transaction data.
    pub raw_tx_data: String,
    // Intent struct used, see [struct Intent] for field definitions.
    pub intent: Intent,
    // Base64 encoded [struct IntentMessage] consisting of (intent || message)
    // where message can be `TransactionData` etc.
    pub raw_intent_msg: String,
    // Base64 encoded blake2b hash of the intent message, this is what the signature commits to.
    pub digest: String,
    // Base64 encoded `flag || signature || pubkey` for a complete
    // serialized IOTA signature to be send for executing the transaction.
    pub iota_signature: String,
}

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
    auth_args: Option<(Vec<CallArg>, Vec<TypeInput>)>,
) -> Result<GenericSignature> {
    let iota_client = context.get_client().await?;

    if let Some((auth_call_args, auth_type_args)) = auth_args {
        let initial_shared_version =
            get_shared_object_version(&iota_client, signer_address).await?;

        return Ok(GenericSignature::MoveAuthenticator(
            MoveAuthenticator::new_v1(
                auth_call_args,
                auth_type_args,
                CallArg::Object(iota_types::transaction::ObjectArg::SharedObject {
                    id: ObjectID::from(*signer_address),
                    initial_shared_version,
                    mutable: false,
                }),
            ),
        ));
    }

    let key = context.config().keystore().get_key(signer_address)?;

    match key {
        StoredKey::KeyPair(_) => Ok(context
            .config()
            .keystore()
            .sign_secure(signer_address, tx_data, Intent::iota_transaction())?
            .into()),
        StoredKey::Account(_) => {
            bail!(
                "Cannot sign for account address without --auth-call-args (and --auth-type-args if needed)."
            )
        }
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
                        .map(|s| s.signature)?
                        .into())
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
        StoredKey::Account(_) => {
            bail!(
                "Cannot sign for account address without --auth-call-args (and --auth-type-args if needed)."
            )
        }
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

pub(crate) async fn get_shared_object_version(
    iota_client: &iota_sdk::IotaClient,
    signer_address: &IotaAddress,
) -> Result<SequenceNumber> {
    let object_response = iota_client
        .read_api()
        .get_object_with_options(
            ObjectID::from(*signer_address),
            IotaObjectDataOptions {
                show_owner: true,
                ..Default::default()
            },
        )
        .await?;
    if let Some(error) = object_response.error {
        bail!("failed to fetch object data for signer_address {signer_address}: {error:?}");
    }
    let object = object_response.data.expect("missing object data");

    if let Some(iota_types::object::Owner::Shared {
        initial_shared_version,
    }) = object.owner
    {
        Ok(initial_shared_version)
    } else {
        bail!("signer_address {signer_address} is not a shared object")
    }
}
