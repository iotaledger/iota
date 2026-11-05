// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::fmt;

use anyhow::{Result, anyhow, bail, ensure};
use iota_json_rpc_types::{
    IotaGetPastObjectRequest, IotaObjectData, IotaObjectDataOptions, IotaRawData,
};
use iota_keys::keystore::{AccountKeystore, StoredKey};
use iota_sdk::{IotaClient, wallet_context::WalletContext};
use iota_sdk_ledger::{Ledger, LedgerSigner};
use iota_sdk_transaction_builder::TransactionSigner;
use iota_sdk_types::{
    Address, MoveAuthenticatorV1, MoveStruct, Object, ObjectData, ObjectId, Owner,
    SharedObjectReference, Transaction, TypeTag, UserSignature, Version,
    crypto::{Intent, SimpleSignature},
};
use iota_types::transaction::{CallArg, InputObjectKind, TransactionAPI};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignData {
    pub iota_address: Address,
    // Base64 encoded string of serialized transaction data.
    pub raw_tx_data: String,
    // Intent struct used, see [struct Intent] for field definitions.
    pub intent: Intent,
    // Base64 encoded [struct IntentMessage] consisting of (intent || message)
    // where message can be `Transaction` etc.
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
    tx: &Transaction,
    signer_address: &Address,
    auth_args: Option<(Vec<CallArg>, Vec<TypeTag>)>,
) -> Result<UserSignature> {
    let iota_client = context.get_client().await?;
    let key = context.config().keystore().get_key(signer_address)?;

    match key {
        // An abstract account always signs via `MoveAuthenticator`, regardless of whether
        // `--auth-call-args` / `--auth-type-args` were supplied. When they were, the resolved
        // `auth_args` carry the user-facing inputs; when they weren't, the authenticator is
        // expected to take no user-facing inputs (only the signer account and the framework
        // contexts), so we forward empty `CallArg` / `TypeTag` vectors and let the validator
        // reject the transaction at runtime if the on-chain function actually requires more.
        StoredKey::Account(_) => {
            let (auth_call_args, auth_type_args) = auth_args.unwrap_or_default();
            let initial_shared_version =
                get_shared_object_version(&iota_client, signer_address).await?;

            Ok(UserSignature::MoveAuthenticator(
                MoveAuthenticatorV1::new(
                    auth_call_args,
                    auth_type_args,
                    SharedObjectReference::new(
                        (*signer_address).into(),
                        initial_shared_version,
                        false,
                    ),
                )
                .into(),
            ))
        }
        StoredKey::KeyPair(_) => {
            if auth_args.is_some() {
                bail!(
                    "--auth-call-args / --auth-type-args were supplied for {signer_address}, but it is not an abstract account."
                );
            }
            Ok(context
                .config()
                .keystore()
                .sign_secure(signer_address, tx, Intent::iota_transaction())?
                .into())
        }
        StoredKey::External {
            derivation_path,
            source,
            ..
        } => {
            if auth_args.is_some() {
                bail!(
                    "--auth-call-args / --auth-type-args were supplied for {signer_address}, but it is not an abstract account."
                );
            }
            match ExternalKeySource::from(source.as_str()) {
                ExternalKeySource::Ledger => {
                    let Some(derivation_path) = derivation_path else {
                        bail!(
                            "Derivation path is required for Ledger signing. Please specify it in the keystore."
                        );
                    };

                    let signer = ledger_signer(derivation_path, signer_address).await?;
                    let objects = load_clear_signing_objects(&iota_client, tx).await?;
                    Ok(signer.sign_with_objects(tx, &objects).await?)
                }
                ExternalKeySource::Unknown(name) => {
                    bail!("External signing is not supported for source: {name}")
                }
            }
        }
    }
}

pub(crate) async fn sign_secure<T>(
    keystore: &impl AccountKeystore,
    address: &Address,
    msg: &T,
    intent: Intent,
) -> Result<SimpleSignature>
where
    T: Serialize,
{
    let key = keystore.get_key(address)?;
    match key {
        StoredKey::KeyPair(_) => Ok(keystore.sign_secure(address, &msg, intent)?),
        StoredKey::Account(_) => {
            bail!(
                "Cannot sign an arbitrary message for an abstract account — abstract accounts can only sign transactions via `MoveAuthenticator`."
            )
        }
        StoredKey::External {
            derivation_path,
            source,
            ..
        } => match ExternalKeySource::from(source.as_str()) {
            ExternalKeySource::Ledger => {
                let Some(derivation_path) = derivation_path else {
                    bail!(
                        "Derivation path is required for Ledger signing. Please specify it in the keystore."
                    );
                };

                match ledger_signer(derivation_path, address)
                    .await?
                    .sign_intent(intent, msg)
                    .await?
                {
                    UserSignature::Simple(signature) => Ok(signature),
                    signature => {
                        bail!(
                            "expected a simple signature from the Ledger device, got {signature:?}"
                        )
                    }
                }
            }
            ExternalKeySource::Unknown(name) => {
                bail!("External signing is not supported for source: {name}")
            }
        },
    }
}

/// Connect to the Speculos emulator at `127.0.0.1:9999` when `LEDGER_SIMULATOR`
/// is set, and to the first Ledger device attached over USB otherwise.
pub(crate) fn connect_ledger() -> Result<Ledger> {
    Ok(if std::env::var("LEDGER_SIMULATOR").is_ok() {
        Ledger::new_speculos("127.0.0.1", 9999)
    } else {
        Ledger::new_native_hid()?
    })
}

/// Convert a keystore derivation path to the type the Ledger API takes.
pub(crate) fn ledger_path(path: &bip32::DerivationPath) -> Result<iota_sdk_crypto::DerivationPath> {
    Ok(path.to_string().parse()?)
}

/// Connect to the Ledger key at `derivation_path`, failing unless its address
/// is `address`.
async fn ledger_signer(
    derivation_path: &bip32::DerivationPath,
    address: &Address,
) -> Result<LedgerSigner> {
    let signer = LedgerSigner::new(connect_ledger()?, ledger_path(derivation_path)?).await?;
    ensure!(
        signer.address() == *address,
        "the Ledger key at {derivation_path} has address {}, not {address}; connect the correct Ledger device or select the correct derivation path",
        signer.address()
    );
    Ok(signer)
}

/// Load the gas coins and owned or immutable inputs of `transaction` at the
/// versions it references, which the Ledger device needs to show what the
/// transaction does.
async fn load_clear_signing_objects(
    client: &IotaClient,
    transaction: &Transaction,
) -> Result<Vec<Object>> {
    let requests = transaction
        .input_objects()?
        .into_iter()
        .filter_map(|input| match input {
            InputObjectKind::ImmOrOwnedMoveObject(reference) => Some(IotaGetPastObjectRequest {
                object_id: reference.object_id,
                version: reference.version,
            }),
            _ => None,
        })
        .collect::<Vec<_>>();
    if requests.is_empty() {
        return Ok(Vec::new());
    }

    client
        .read_api()
        .try_multi_get_parsed_past_object(requests, IotaObjectDataOptions::bcs_lossless())
        .await?
        .into_iter()
        .map(|response| object_from_data(response.into_object()?))
        .collect()
}

/// Rebuild a Move object from its JSON-RPC representation.
fn object_from_data(data: IotaObjectData) -> Result<Object> {
    let object_id = data.object_id;
    let missing = |field: &str| anyhow!("the RPC response for object {object_id} has no {field}");
    let IotaRawData::MoveObject(raw) = data.bcs.ok_or_else(|| missing("BCS"))? else {
        bail!("object {object_id} is not a Move object");
    };
    Ok(Object::new(
        ObjectData::Struct(MoveStruct::new(
            raw.struct_tag.into(),
            data.version,
            raw.bcs_bytes,
        )?),
        data.owner.ok_or_else(|| missing("owner"))?,
        data.previous_transaction
            .ok_or_else(|| missing("previous transaction"))?,
        data.storage_rebate
            .ok_or_else(|| missing("storage rebate"))?,
    ))
}

pub(crate) async fn get_shared_object_version(
    iota_client: &iota_sdk::IotaClient,
    signer_address: &Address,
) -> Result<Version> {
    let object_response = iota_client
        .read_api()
        .get_object_with_options(
            ObjectId::from(*signer_address),
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

    if let Some(Owner::Shared(initial_shared_version)) = object.owner {
        Ok(initial_shared_version)
    } else {
        bail!("signer_address {signer_address} is not a shared object")
    }
}

#[cfg(test)]
mod tests {
    use iota_json_rpc_types::{IotaObjectData, IotaObjectDataOptions};
    use iota_sdk_types::{
        Address, MoveStruct, Object, ObjectData, ObjectId, Owner, TransactionDigest, Version,
    };
    use iota_types::{gas_coin::GasCoin, object::MoveStructExt};

    use super::object_from_data;

    #[test]
    fn rebuilds_an_object_with_its_digest() {
        let object = Object::new(
            ObjectData::Struct(MoveStruct::new_gas_coin(
                Version::from_u64(5),
                ObjectId::new([1; ObjectId::LENGTH]),
                1_000,
            )),
            Owner::Address(Address::new([2; Address::LENGTH])),
            TransactionDigest::new([3; TransactionDigest::LENGTH]),
            42,
        );
        let data = IotaObjectData::new(
            object.object_ref(),
            object.clone().into(),
            GasCoin::layout(),
            &IotaObjectDataOptions::bcs_lossless(),
            None,
        )
        .unwrap();

        assert_eq!(object_from_data(data).unwrap().digest(), object.digest());
    }
}
