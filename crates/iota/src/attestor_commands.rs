// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::{
    fmt::{Debug, Display, Formatter, Write as _},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Result, anyhow, bail};
use clap::Parser;
use colored::Colorize;
use fastcrypto::{
    ed25519::Ed25519KeyPair, encoding::Hex, secp256k1::Secp256k1KeyPair,
    secp256r1::Secp256r1KeyPair,
};
use iota_json_rpc_types::{
    IotaData, IotaObjectDataOptions, IotaProtocolConfigValue, IotaTransactionBlockResponse,
    IotaTransactionBlockResponseOptions,
};
use iota_keys::keypair_file::{read_keypair_from_file, write_keypair_to_file};
use iota_sdk::{IotaClient, wallet_context::WalletContext};
use iota_sdk_types::{Address, Argument, Command, Identifier, ObjectId, ObjectReference};
use iota_types::{
    crypto::{IotaKeyPair, SignatureScheme, get_key_pair_from_rng},
    dynamic_field::Field,
    iota_system_state::attestor_registry::{
        AttestorMetadataKey, AttestorMetadataV1, AttestorRegistryKey, AttestorRegistryV1,
        derive_attestor_metadata_object_id, derive_attestor_registry_object_id,
        generate_attestor_proof_of_possession,
    },
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::{CallArg, Transaction, TransactionData, TransactionDataAPI},
};
use rand::rngs::OsRng;
use serde::Serialize;

use crate::{
    PrintableResult, signing::sign_transaction, validator_commands::write_transaction_response,
};

#[path = "unit_tests/attestor_tests.rs"]
#[cfg(test)]
mod attestor_tests;

const DEFAULT_GAS_BUDGET: u64 = 200_000_000; // 0.2 IOTA

#[derive(Parser)]
pub enum IotaAttestorCommand {
    /// Generate a fresh attestor signing key, save it to `attestor.key`
    /// next to the client config, and submit the registration transaction
    /// (bond + proof of possession + display metadata). Takes effect at the
    /// next epoch boundary.
    Register {
        #[arg(long)]
        name: String,
        #[arg(long)]
        description: String,
        #[arg(long)]
        url: String,
        #[arg(long)]
        logo: String,
        /// Bond in nanos; defaults to the network's minimum joining bond.
        #[arg(long)]
        bond: Option<u64>,
        /// ed25519 (default), secp256k1 or secp256r1.
        #[arg(long, default_value = "ed25519")]
        key_scheme: SignatureScheme,
        #[arg(long)]
        gas_budget: Option<u64>,
    },
    /// Deregister the sender as an attestor. A pending attestor is refunded
    /// immediately; an active one is removed and refunded at the next epoch
    /// boundary.
    Deregister {
        #[arg(long)]
        gas_budget: Option<u64>,
    },
    /// Add to the sender's attestor bond (nanos), effective immediately.
    DepositBond {
        #[arg(long)]
        amount: u64,
        #[arg(long)]
        gas_budget: Option<u64>,
    },
    /// Generate a new signing key and submit its rotation (effective at the
    /// next epoch boundary). Fails if `attestor.key` already exists — the
    /// current key stays valid on-chain until the rotation lands, so move it
    /// aside first if you want to keep it, then re-run this command; the new
    /// key is written to `attestor.key` before submitting.
    RotateKey {
        #[arg(long, default_value = "ed25519")]
        key_scheme: SignatureScheme,
        #[arg(long)]
        gas_budget: Option<u64>,
    },
    /// Update the attestor display name; effective immediately.
    UpdateName {
        name: String,
        #[arg(long)]
        gas_budget: Option<u64>,
    },
    /// Update the attestor description; effective immediately.
    UpdateDescription {
        description: String,
        #[arg(long)]
        gas_budget: Option<u64>,
    },
    /// Update the attestor url; effective immediately.
    UpdateUrl {
        url: String,
        #[arg(long)]
        gas_budget: Option<u64>,
    },
    /// Update the attestor logo url; effective immediately.
    UpdateLogo {
        logo: String,
        #[arg(long)]
        gas_budget: Option<u64>,
    },
    /// Show an attestor's registry entry and metadata.
    Display {
        /// Defaults to the active address.
        #[arg(long)]
        address: Option<Address>,
    },
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum IotaAttestorCommandResponse {
    Register(IotaTransactionBlockResponse),
    Deregister {
        response: IotaTransactionBlockResponse,
        /// Whether the bond refund is deferred to the next epoch boundary
        /// (the attestor was active) rather than immediate (it was still
        /// pending).
        refund_deferred: bool,
    },
    DepositBond(IotaTransactionBlockResponse),
    RotateKey(IotaTransactionBlockResponse),
    UpdateMetadata(IotaTransactionBlockResponse),
    Display(String),
}

impl IotaAttestorCommand {
    pub async fn execute(
        self,
        context: &mut WalletContext,
    ) -> Result<IotaAttestorCommandResponse, anyhow::Error> {
        Ok(match self {
            IotaAttestorCommand::Register {
                name,
                description,
                url,
                logo,
                bond,
                key_scheme,
                gas_budget,
            } => {
                let gas_budget = gas_budget.unwrap_or(DEFAULT_GAS_BUDGET);
                let sender = context.active_address()?;
                let iota_client = context.get_client().await?;
                let bond_amount = match bond {
                    Some(bond) => bond,
                    None => default_bond_amount(&iota_client).await?,
                };

                // Written before submitting so a failed submit never loses a freshly
                // generated key, and an existing file aborts before any transaction.
                let keypair = generate_attestor_keypair(key_scheme)?;
                let key_path = attestor_key_path(context)?;
                write_attestor_key(&key_path, &keypair, false)?;
                // Round-trip through disk so the key used below is provably the one
                // that ended up in `attestor.key`, not just the in-memory copy.
                let keypair = read_attestor_key(&key_path)?;

                let pubkey = flagged_pubkey(&keypair);
                let proof_of_possession = generate_attestor_proof_of_possession(&keypair, sender);

                let result = call_0x5_with_bond(
                    context,
                    "register_attestor",
                    bond_amount,
                    vec![
                        CallArg::pure(&pubkey),
                        CallArg::pure(&proof_of_possession),
                        CallArg::pure(&name.into_bytes()),
                        CallArg::pure(&description.into_bytes()),
                        CallArg::pure(&url.into_bytes()),
                        CallArg::pure(&logo.into_bytes()),
                    ],
                    gas_budget,
                )
                .await;

                warn_if_key_not_confirmed(&key_path, &result);
                IotaAttestorCommandResponse::Register(result?)
            }

            IotaAttestorCommand::Deregister { gas_budget } => {
                let gas_budget = gas_budget.unwrap_or(DEFAULT_GAS_BUDGET);
                let sender = context.active_address()?;
                let iota_client = context.get_client().await?;
                // Read before submitting: whether the refund is immediate or deferred depends
                // on which set the sender is in right now, not on the post-transaction state.
                let registry = read_attestor_registry(&iota_client).await?;
                let refund_deferred = registry
                    .active_attestors
                    .iter()
                    .any(|a| a.attestor_address == sender);

                let response = call_0x5(context, "deregister_attestor", vec![], gas_budget).await?;
                IotaAttestorCommandResponse::Deregister {
                    response,
                    refund_deferred,
                }
            }

            IotaAttestorCommand::DepositBond { amount, gas_budget } => {
                let gas_budget = gas_budget.unwrap_or(DEFAULT_GAS_BUDGET);
                let response = call_0x5_with_bond(
                    context,
                    "deposit_attestor_bond",
                    amount,
                    vec![],
                    gas_budget,
                )
                .await?;
                IotaAttestorCommandResponse::DepositBond(response)
            }

            IotaAttestorCommand::RotateKey {
                key_scheme,
                gas_budget,
            } => {
                let gas_budget = gas_budget.unwrap_or(DEFAULT_GAS_BUDGET);
                let sender = context.active_address()?;
                let key_path = attestor_key_path(context)?;
                if key_path.exists() {
                    bail!(
                        "an attestor key already exists at {key_path:?}; it remains valid \
                         on-chain until the next epoch boundary, so move it away first if you \
                         want to keep it, then re-run rotate-key"
                    );
                }

                // Written before submitting so a failed or ambiguous submit never loses the
                // freshly generated key.
                let keypair = generate_attestor_keypair(key_scheme)?;
                write_attestor_key(&key_path, &keypair, true)?;
                let keypair = read_attestor_key(&key_path)?;

                let pubkey = flagged_pubkey(&keypair);
                let proof_of_possession = generate_attestor_proof_of_possession(&keypair, sender);

                let result = call_0x5(
                    context,
                    "rotate_attestor_key",
                    vec![CallArg::pure(&pubkey), CallArg::pure(&proof_of_possession)],
                    gas_budget,
                )
                .await;

                warn_if_key_not_confirmed(&key_path, &result);
                IotaAttestorCommandResponse::RotateKey(result?)
            }

            IotaAttestorCommand::UpdateName { name, gas_budget } => {
                let gas_budget = gas_budget.unwrap_or(DEFAULT_GAS_BUDGET);
                let args = vec![CallArg::pure(&name.into_bytes())];
                let response = call_0x5(context, "update_attestor_name", args, gas_budget).await?;
                IotaAttestorCommandResponse::UpdateMetadata(response)
            }

            IotaAttestorCommand::UpdateDescription {
                description,
                gas_budget,
            } => {
                let gas_budget = gas_budget.unwrap_or(DEFAULT_GAS_BUDGET);
                let args = vec![CallArg::pure(&description.into_bytes())];
                let response =
                    call_0x5(context, "update_attestor_description", args, gas_budget).await?;
                IotaAttestorCommandResponse::UpdateMetadata(response)
            }

            IotaAttestorCommand::UpdateUrl { url, gas_budget } => {
                let gas_budget = gas_budget.unwrap_or(DEFAULT_GAS_BUDGET);
                let args = vec![CallArg::pure(&url.into_bytes())];
                let response = call_0x5(context, "update_attestor_url", args, gas_budget).await?;
                IotaAttestorCommandResponse::UpdateMetadata(response)
            }

            IotaAttestorCommand::UpdateLogo { logo, gas_budget } => {
                let gas_budget = gas_budget.unwrap_or(DEFAULT_GAS_BUDGET);
                let args = vec![CallArg::pure(&logo.into_bytes())];
                let response = call_0x5(context, "update_attestor_logo", args, gas_budget).await?;
                IotaAttestorCommandResponse::UpdateMetadata(response)
            }

            IotaAttestorCommand::Display { address } => {
                let address = address.unwrap_or(context.active_address()?);
                let iota_client = context.get_client().await?;
                let resp = display_attestor(&iota_client, address).await?;
                IotaAttestorCommandResponse::Display(resp)
            }
        })
    }
}

/// `flag || raw pubkey`, the on-chain encoding for an attestor signing key.
fn flagged_pubkey(keypair: &IotaKeyPair) -> Vec<u8> {
    let pk = keypair.public();
    let mut bytes = vec![pk.flag()];
    bytes.extend_from_slice(pk.as_ref());
    bytes
}

fn generate_attestor_keypair(scheme: SignatureScheme) -> Result<IotaKeyPair> {
    let mut rng = OsRng;
    Ok(match scheme {
        SignatureScheme::Ed25519 => {
            IotaKeyPair::Ed25519(get_key_pair_from_rng::<Ed25519KeyPair, _>(&mut rng).1)
        }
        SignatureScheme::Secp256k1 => {
            IotaKeyPair::Secp256k1(get_key_pair_from_rng::<Secp256k1KeyPair, _>(&mut rng).1)
        }
        SignatureScheme::Secp256r1 => {
            IotaKeyPair::Secp256r1(get_key_pair_from_rng::<Secp256r1KeyPair, _>(&mut rng).1)
        }
        other => bail!(
            "unsupported attestor key scheme: {other}, expected ed25519, secp256k1 or secp256r1"
        ),
    })
}

fn attestor_key_path(context: &WalletContext) -> Result<PathBuf> {
    let config_dir = context.config().path().parent().ok_or_else(|| {
        anyhow!(
            "client config path {:?} has no parent directory",
            context.config().path()
        )
    })?;
    Ok(config_dir.join("attestor.key"))
}

fn write_attestor_key(path: &Path, keypair: &IotaKeyPair, allow_overwrite: bool) -> Result<()> {
    if !allow_overwrite && path.exists() {
        bail!("an attestor key already exists at {path:?}; move it away if you are re-registering");
    }
    write_keypair_to_file(keypair, path)?;
    set_key_file_permissions(path)?;
    Ok(())
}

/// The attestor key file is written before submitting and is never deleted
/// automatically, in either the register or rotate-key command — not even on
/// a confirmed on-chain failure. Prints a warning telling the operator to
/// remove it by hand before retrying, and to check `iota attestor display`
/// first when the outcome isn't already known to have failed.
fn warn_if_key_not_confirmed(key_path: &Path, result: &Result<IotaTransactionBlockResponse>) {
    match result {
        Ok(response) => match response.status_ok() {
            Some(true) => {}
            Some(false) => eprintln!(
                "warning: the transaction failed on-chain; kept the attestor key at {}. Remove \
                 it by hand before retrying.",
                key_path.display()
            ),
            None => eprintln!(
                "warning: transaction status is unknown; kept the attestor key at {}. Check \
                 `iota attestor display` first, then remove it by hand before retrying if it did \
                 not take effect.",
                key_path.display()
            ),
        },
        Err(_) => eprintln!(
            "warning: submitting the transaction failed; kept the attestor key at {}. Check \
             `iota attestor display` first, then remove it by hand before retrying if it did \
             not take effect.",
            key_path.display()
        ),
    }
}

#[cfg(unix)]
fn set_key_file_permissions(path: &Path) -> Result<()> {
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_key_file_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

fn read_attestor_key(path: &Path) -> Result<IotaKeyPair> {
    read_keypair_from_file(path)
}

/// Mirrors `attestor_registry.move`'s `min_joining_bond`: the joining bond is
/// `min_validator_joining_stake` scaled by `attestor_joining_bond_rate`
/// (basis points). The Move implementation cannot be called from the client,
/// so the derivation is repeated here.
async fn default_bond_amount(client: &IotaClient) -> Result<u64> {
    const BASIS_POINT_DENOMINATOR: u128 = 10_000;

    let cfg = client.read_api().get_protocol_config(None).await?;
    let attr = |name: &str| match cfg.attributes.get(name) {
        Some(Some(IotaProtocolConfigValue::U64(value))) => Some(*value),
        _ => None,
    };
    match (
        attr("min_validator_joining_stake"),
        attr("attestor_joining_bond_rate"),
    ) {
        (Some(stake), Some(rate)) => {
            Ok((stake as u128 * rate as u128 / BASIS_POINT_DENOMINATOR) as u64)
        }
        _ => bail!(
            "Could not automatically determine the network's minimum attestor joining bond. \
             Please provide a bond amount with --bond. The parameter's absence usually means \
             the network does not support attestors."
        ),
    }
}

async fn construct_unsigned_0x5_txn(
    context: &mut WalletContext,
    sender: Address,
    function: &'static str,
    call_args: Vec<CallArg>,
    gas_budget: u64,
) -> Result<TransactionData> {
    let iota_client = context.get_client().await?;
    let mut args = vec![CallArg::IOTA_SYSTEM_MUTABLE];
    args.extend(call_args);
    let rgp = iota_client
        .governance_api()
        .get_reference_gas_price()
        .await?;

    let gas_obj_ref = get_gas_obj_ref(sender, &iota_client, gas_budget).await?;
    TransactionData::new_move_call(
        sender,
        ObjectId::SYSTEM,
        Identifier::IOTA_SYSTEM_MODULE,
        Identifier::from_static(function),
        vec![],
        gas_obj_ref,
        args,
        gas_budget,
        rgp,
    )
}

async fn call_0x5(
    context: &mut WalletContext,
    function: &'static str,
    call_args: Vec<CallArg>,
    gas_budget: u64,
) -> Result<IotaTransactionBlockResponse> {
    let sender = context.active_address()?;
    let tx_data =
        construct_unsigned_0x5_txn(context, sender, function, call_args, gas_budget).await?;
    execute_0x5_txn(context, tx_data).await
}

/// Like [`call_0x5`], but for the entry points that additionally take a
/// `Coin<IOTA>` (register / deposit bond): splits `bond_amount` off the gas
/// coin instead of requiring the caller to supply a separate coin object.
async fn call_0x5_with_bond(
    context: &mut WalletContext,
    function: &'static str,
    bond_amount: u64,
    trailing_args: Vec<CallArg>,
    gas_budget: u64,
) -> Result<IotaTransactionBlockResponse> {
    let sender = context.active_address()?;
    let iota_client = context.get_client().await?;
    let rgp = iota_client
        .governance_api()
        .get_reference_gas_price()
        .await?;
    // The same coin pays gas and funds the bond, so it must cover both.
    let gas_obj_ref =
        get_gas_obj_ref(sender, &iota_client, gas_budget.saturating_add(bond_amount)).await?;

    let mut builder = ProgrammableTransactionBuilder::new();
    let system_state_arg = builder.obj(CallArg::IOTA_SYSTEM_MUTABLE)?;
    let bond_amount_arg = builder.pure(bond_amount)?;
    let coin_arg = builder.command(Command::new_split_coins(
        Argument::Gas,
        vec![bond_amount_arg],
    ));
    let mut arguments = vec![system_state_arg, coin_arg];
    for arg in trailing_args {
        arguments.push(builder.input(arg)?);
    }
    builder.programmable_move_call(
        ObjectId::SYSTEM,
        Identifier::IOTA_SYSTEM_MODULE,
        Identifier::from_static(function),
        vec![],
        arguments,
    );
    let pt = builder.finish();
    let tx_data = TransactionData::new_programmable(sender, vec![gas_obj_ref], pt, gas_budget, rgp);

    execute_0x5_txn(context, tx_data).await
}

async fn execute_0x5_txn(
    context: &mut WalletContext,
    tx_data: TransactionData,
) -> Result<IotaTransactionBlockResponse> {
    let iota_client = context.get_client().await?;
    let signature = sign_transaction(context, &tx_data, &tx_data.sender(), None).await?;
    let transaction = Transaction::from_user_sig_data(tx_data, vec![signature]);

    iota_client
        .quorum_driver_api()
        .execute_transaction_block(
            transaction,
            IotaTransactionBlockResponseOptions::new()
                .with_input()
                .with_effects(),
            Some(iota_types::quorum_driver_types::ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await
        .map_err(|err| anyhow::anyhow!(err.to_string()))
}

async fn get_gas_obj_ref(
    iota_address: Address,
    iota_client: &IotaClient,
    minimal_gas_balance: u64,
) -> Result<ObjectReference> {
    let coins = iota_client
        .coin_read_api()
        .get_coins(iota_address, Some("0x2::iota::IOTA".into()), None, None)
        .await?
        .data;
    let gas_obj = coins.iter().find(|c| c.balance >= minimal_gas_balance);
    if gas_obj.is_none() {
        bail!("Account doesn't have a single IOTA coin large enough to cover this transaction.");
    }
    Ok(gas_obj.unwrap().object_ref())
}

async fn read_attestor_registry(client: &IotaClient) -> Result<AttestorRegistryV1> {
    let id = derive_attestor_registry_object_id()?;
    let resp = client
        .read_api()
        .get_object_with_options(id, IotaObjectDataOptions::bcs_lossless())
        .await?;
    // The registry is created lazily on-chain; absence means it is empty.
    let Some(data) = resp.data else {
        return Ok(AttestorRegistryV1::default());
    };
    let bcs = data
        .bcs
        .ok_or_else(|| anyhow!("attestor registry object {id} is missing bcs bytes"))?;
    let field = bcs
        .try_into_move()
        .ok_or_else(|| anyhow!("attestor registry object {id} is not a Move object"))?
        .deserialize::<Field<AttestorRegistryKey, AttestorRegistryV1>>()?;
    Ok(field.value)
}

async fn read_attestor_metadata(
    client: &IotaClient,
    address: Address,
) -> Result<Option<AttestorMetadataV1>> {
    let id = derive_attestor_metadata_object_id(address)?;
    let resp = client
        .read_api()
        .get_object_with_options(id, IotaObjectDataOptions::bcs_lossless())
        .await?;
    let Some(data) = resp.data else {
        return Ok(None);
    };
    let bcs = data
        .bcs
        .ok_or_else(|| anyhow!("attestor metadata object {id} is missing bcs bytes"))?;
    let field = bcs
        .try_into_move()
        .ok_or_else(|| anyhow!("attestor metadata object {id} is not a Move object"))?
        .deserialize::<Field<AttestorMetadataKey, AttestorMetadataV1>>()?;
    Ok(Some(field.value))
}

async fn display_attestor(client: &IotaClient, address: Address) -> Result<String> {
    let registry = read_attestor_registry(client).await?;

    let (status, entry) = if let Some(index) = registry
        .active_attestors
        .iter()
        .position(|a| a.attestor_address == address)
    {
        let status = if registry.pending_removals.contains(&(index as u64)) {
            format!("active (index {index}, removal scheduled at next epoch boundary)")
        } else {
            format!("active (index {index})")
        };
        (status, Some(&registry.active_attestors[index]))
    } else if let Some(entry) = registry
        .pending_active
        .iter()
        .find(|a| a.attestor_address == address)
    {
        ("pending".to_string(), Some(entry))
    } else {
        ("not registered".to_string(), None)
    };

    let Some(entry) = entry else {
        return Ok(format!(
            "Attestor {address}\n  status:            not registered"
        ));
    };

    let metadata = read_attestor_metadata(client, address).await?;

    let mut out = format!("Attestor {address}\n");
    writeln!(out, "  status:            {status}")?;
    writeln!(
        out,
        "  pubkey:            {}",
        Hex::encode_with_format(&entry.attestor_pubkey)
    )?;
    match &entry.next_epoch_attestor_pubkey {
        Some(pubkey) => writeln!(
            out,
            "  staged rotation:   {}",
            Hex::encode_with_format(pubkey)
        )?,
        None => writeln!(out, "  staged rotation:   none")?,
    }
    writeln!(out, "  bond:              {} nanos", entry.bond.value())?;
    writeln!(out, "  activation epoch:  {}", entry.activation_epoch)?;
    writeln!(out, "  last active epoch: {}", entry.last_active_epoch)?;
    if let Some(metadata) = metadata {
        writeln!(out, "  name:              {}", metadata.name)?;
        writeln!(out, "  description:       {}", metadata.description)?;
        writeln!(out, "  url:               {}", metadata.url)?;
        writeln!(out, "  logo:              {}", metadata.logo)?;
    }
    Ok(out.trim_end().to_string())
}

// Uses the trait's default `print`: pretty (non-`--json`) prints `Display`,
// non-pretty (`--json`) prints `Debug`, which below is JSON.
impl PrintableResult for IotaAttestorCommandResponse {}

impl Display for IotaAttestorCommandResponse {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut writer = String::new();
        match self {
            IotaAttestorCommandResponse::Display(resp) => write!(writer, "{resp}")?,
            IotaAttestorCommandResponse::Register(resp)
            | IotaAttestorCommandResponse::DepositBond(resp)
            | IotaAttestorCommandResponse::RotateKey(resp)
            | IotaAttestorCommandResponse::UpdateMetadata(resp) => {
                write!(writer, "{}", write_transaction_response(resp)?)?;
            }
            IotaAttestorCommandResponse::Deregister {
                response,
                refund_deferred,
            } => {
                write!(writer, "{}", write_transaction_response(response)?)?;
                let refund = if *refund_deferred {
                    "deferred to the next epoch boundary (the attestor was active)"
                } else {
                    "immediate (the attestor was still pending)"
                };
                writeln!(writer)?;
                write!(writer, "  bond refund:       {refund}")?;
            }
        }
        write!(f, "{}", writer.trim_end_matches('\n'))
    }
}

impl Debug for IotaAttestorCommandResponse {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let string = serde_json::to_string_pretty(self);
        let s = match string {
            Ok(s) => s,
            Err(err) => format!("{err}").red().to_string(),
        };
        write!(f, "{s}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attestor_key_roundtrip_and_overwrite_protection() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("attestor.key");
        let kp = IotaKeyPair::Ed25519(iota_types::crypto::get_key_pair().1);
        write_attestor_key(&path, &kp, false).unwrap();
        let read = read_attestor_key(&path).unwrap();
        assert_eq!(read.public(), kp.public());
        // second write without overwrite permission must fail: this is what makes both
        // register and rotate-key refuse to run while a key file is already present.
        assert!(write_attestor_key(&path, &kp, false).is_err());
        // callers that have already made their own overwrite decision may still force
        // it.
        write_attestor_key(&path, &kp, true).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn attestor_key_file_is_owner_read_write_only() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("attestor.key");
        let kp = IotaKeyPair::Ed25519(iota_types::crypto::get_key_pair().1);
        write_attestor_key(&path, &kp, false).unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}
