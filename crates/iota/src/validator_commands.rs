// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, HashSet},
    fmt::{self, Debug, Display, Formatter, Write},
    fs,
    path::PathBuf,
};

use anyhow::{anyhow, bail, Result};
use clap::*;
use colored::Colorize;
use fastcrypto::{
    encoding::{Base64, Encoding},
    traits::{KeyPair, ToFromBytes},
};
use iota_genesis_builder::validator_info::GenesisValidatorInfo;
use iota_json_rpc_types::{
    IotaObjectDataOptions, IotaTransactionBlockResponse, IotaTransactionBlockResponseOptions,
};
use iota_keys::{
    key_derive::generate_new_key,
    keypair_file::{
        read_authority_keypair_from_file, read_keypair_from_file, read_network_keypair_from_file,
        write_authority_keypair_to_file, write_keypair_to_file,
    },
    keystore::AccountKeystore,
};
use iota_sdk::{wallet_context::WalletContext, IotaClient};
use iota_types::{
    base_types::{IotaAddress, ObjectID, ObjectRef},
    crypto::{
        generate_proof_of_possession, get_authority_key_pair, AuthorityKeyPair, AuthorityPublicKey,
        AuthorityPublicKeyBytes, IotaKeyPair, NetworkKeyPair, NetworkPublicKey, Signable,
        SignatureScheme, DEFAULT_EPOCH_ID,
    },
    iota_system_state::{
        iota_system_state_inner_v1::{UnverifiedValidatorOperationCapV1, ValidatorV1},
        iota_system_state_summary::{IotaSystemStateSummary, IotaValidatorSummary},
    },
    multiaddr::Multiaddr,
    object::Owner,
    transaction::{CallArg, ObjectArg, Transaction, TransactionData},
    IOTA_SYSTEM_PACKAGE_ID,
};
use move_core_types::ident_str;
use serde::Serialize;
use shared_crypto::intent::{Intent, IntentMessage, IntentScope};
use tap::tap::TapOptional;

use crate::fire_drill::get_gas_obj_ref;

#[path = "unit_tests/validator_tests.rs"]
#[cfg(test)]
mod validator_tests;

const DEFAULT_GAS_BUDGET: u64 = 200_000_000; // 0.2 IOTA

#[derive(Parser)]
#[clap(rename_all = "kebab-case")]
pub enum IotaValidatorCommand {
    #[clap(name = "make-validator-info")]
    MakeValidatorInfo {
        name: String,
        description: String,
        image_url: String,
        project_url: String,
        host_name: String,
        gas_price: u64,
    },
    #[clap(name = "become-candidate")]
    BecomeCandidate {
        #[clap(name = "validator-info-path")]
        file: PathBuf,
        #[clap(name = "gas-budget", long)]
        gas_budget: Option<u64>,
    },
    #[clap(name = "join-committee")]
    JoinCommittee {
        /// Gas budget for this transaction.
        #[clap(name = "gas-budget", long)]
        gas_budget: Option<u64>,
    },
    #[clap(name = "leave-committee")]
    LeaveCommittee {
        /// Gas budget for this transaction.
        #[clap(name = "gas-budget", long)]
        gas_budget: Option<u64>,
    },
    #[clap(name = "display-metadata")]
    DisplayMetadata {
        #[clap(name = "validator-address")]
        validator_address: Option<IotaAddress>,
        #[clap(name = "json", long)]
        json: Option<bool>,
    },
    #[clap(name = "update-metadata")]
    UpdateMetadata {
        #[clap(subcommand)]
        metadata: MetadataUpdate,
        /// Gas budget for this transaction.
        #[clap(name = "gas-budget", long)]
        gas_budget: Option<u64>,
    },
    /// Update gas price that is used to calculate Reference Gas Price
    #[clap(name = "update-gas-price")]
    UpdateGasPrice {
        /// Optional when sender is the validator itself and it holds the Cap
        /// object. Required when sender is not the validator itself.
        /// Validator's OperationCap ID can be found by using the
        /// `display-metadata` subcommand.
        #[clap(name = "operation-cap-id", long)]
        operation_cap_id: Option<ObjectID>,
        #[clap(name = "gas-price")]
        gas_price: u64,
        /// Gas budget for this transaction.
        #[clap(name = "gas-budget", long)]
        gas_budget: Option<u64>,
    },
    /// Report or un-report a validator.
    #[clap(name = "report-validator")]
    ReportValidator {
        /// Optional when sender is reporter validator itself and it holds the
        /// Cap object. Required when sender is not the reporter
        /// validator itself. Validator's OperationCap ID can be found
        /// by using the `display-metadata` subcommand.
        #[clap(name = "operation-cap-id", long)]
        operation_cap_id: Option<ObjectID>,
        /// The Iota Address of the validator is being reported or un-reported
        #[clap(name = "reportee-address")]
        reportee_address: IotaAddress,
        /// If true, undo an existing report.
        #[clap(name = "undo-report", long)]
        undo_report: Option<bool>,
        /// Gas budget for this transaction.
        #[clap(name = "gas-budget", long)]
        gas_budget: Option<u64>,
    },
    /// Serialize the payload that is used to generate Proof of Possession.
    /// This is useful to take the payload offline for an Authority protocol
    /// keypair to sign.
    #[clap(name = "serialize-payload-pop")]
    SerializePayloadForPoP {
        /// Authority account address encoded in hex with 0x prefix.
        #[clap(name = "account-address", long)]
        account_address: IotaAddress,
        /// Authority protocol public key encoded in hex.
        #[clap(name = "protocol-public-key", long)]
        protocol_public_key: AuthorityPublicKeyBytes,
    },
    /// Print out the serialized data of a transaction that sets the gas price
    /// quote for a validator.
    DisplayGasPriceUpdateRawTxn {
        /// Address of the transaction sender.
        #[clap(name = "sender-address", long)]
        sender_address: IotaAddress,
        /// Object ID of a validator's OperationCap, used for setting gas price
        /// and reportng validators.
        #[clap(name = "operation-cap-id", long)]
        operation_cap_id: ObjectID,
        /// Gas price to be set to.
        #[clap(name = "new-gas-price", long)]
        new_gas_price: u64,
        /// Gas budget for this transaction.
        #[clap(name = "gas-budget", long)]
        gas_budget: Option<u64>,
    },
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum IotaValidatorCommandResponse {
    MakeValidatorInfo,
    DisplayMetadata,
    BecomeCandidate(IotaTransactionBlockResponse),
    JoinCommittee(IotaTransactionBlockResponse),
    LeaveCommittee(IotaTransactionBlockResponse),
    UpdateMetadata(IotaTransactionBlockResponse),
    UpdateGasPrice(IotaTransactionBlockResponse),
    ReportValidator(IotaTransactionBlockResponse),
    SerializedPayload(String),
    DisplayGasPriceUpdateRawTxn {
        data: TransactionData,
        serialized_data: String,
    },
}

fn make_key_files(
    file_name: PathBuf,
    is_protocol_key: bool,
    key: Option<IotaKeyPair>,
) -> anyhow::Result<()> {
    if file_name.exists() {
        println!("Use existing {:?} key file.", file_name);
        return Ok(());
    } else if is_protocol_key {
        let (_, keypair) = get_authority_key_pair();
        write_authority_keypair_to_file(&keypair, file_name.clone())?;
        println!("Generated new key file: {:?}.", file_name);
    } else {
        let kp = match key {
            Some(key) => {
                println!(
                    "Generated new key file {:?} based on iota.keystore file.",
                    file_name
                );
                key
            }
            None => {
                let (_, kp, _, _) = generate_new_key(SignatureScheme::ED25519, None, None)?;
                println!("Generated new key file: {:?}.", file_name);
                kp
            }
        };
        write_keypair_to_file(&kp, &file_name)?;
    }
    Ok(())
}

impl IotaValidatorCommand {
    pub async fn execute(
        self,
        context: &mut WalletContext,
    ) -> Result<IotaValidatorCommandResponse, anyhow::Error> {
        let iota_address = context.active_address()?;

        let ret = Ok(match self {
            IotaValidatorCommand::MakeValidatorInfo {
                name,
                description,
                image_url,
                project_url,
                host_name,
                gas_price,
            } => {
                let dir = std::env::current_dir()?;
                let protocol_key_file_name = dir.join("protocol.key");
                let account_key = match context.config.keystore.get_key(&iota_address)? {
                    IotaKeyPair::Ed25519(account_key) => IotaKeyPair::Ed25519(account_key.copy()),
                    _ => panic!(
                        "Other account key types supported yet, please use Ed25519 keys for now."
                    ),
                };
                let account_key_file_name = dir.join("account.key");
                let network_key_file_name = dir.join("network.key");
                let worker_key_file_name = dir.join("worker.key");
                make_key_files(protocol_key_file_name.clone(), true, None)?;
                make_key_files(account_key_file_name.clone(), false, Some(account_key))?;
                make_key_files(network_key_file_name.clone(), false, None)?;
                make_key_files(worker_key_file_name.clone(), false, None)?;

                let keypair: AuthorityKeyPair =
                    read_authority_keypair_from_file(protocol_key_file_name)?;
                let account_keypair: IotaKeyPair = read_keypair_from_file(account_key_file_name)?;
                let worker_keypair: NetworkKeyPair =
                    read_network_keypair_from_file(worker_key_file_name)?;
                let network_keypair: NetworkKeyPair =
                    read_network_keypair_from_file(network_key_file_name)?;
                let pop =
                    generate_proof_of_possession(&keypair, (&account_keypair.public()).into());
                let validator_info = GenesisValidatorInfo {
                    info: iota_genesis_builder::validator_info::ValidatorInfo {
                        name,
                        protocol_key: keypair.public().into(),
                        worker_key: worker_keypair.public().clone(),
                        account_address: IotaAddress::from(&account_keypair.public()),
                        network_key: network_keypair.public().clone(),
                        gas_price,
                        commission_rate: iota_config::node::DEFAULT_COMMISSION_RATE,
                        network_address: Multiaddr::try_from(format!(
                            "/dns/{}/tcp/8080/http",
                            host_name
                        ))?,
                        p2p_address: Multiaddr::try_from(format!("/dns/{}/udp/8084", host_name))?,
                        narwhal_primary_address: Multiaddr::try_from(format!(
                            "/dns/{}/udp/8081",
                            host_name
                        ))?,
                        narwhal_worker_address: Multiaddr::try_from(format!(
                            "/dns/{}/udp/8082",
                            host_name
                        ))?,
                        description,
                        image_url,
                        project_url,
                    },
                    proof_of_possession: pop,
                };
                // TODO set key files permission
                let validator_info_file_name = dir.join("validator.info");
                let validator_info_bytes = serde_yaml::to_string(&validator_info)?;
                fs::write(validator_info_file_name.clone(), validator_info_bytes)?;
                println!(
                    "Generated validator info file: {:?}.",
                    validator_info_file_name
                );
                IotaValidatorCommandResponse::MakeValidatorInfo
            }
            IotaValidatorCommand::BecomeCandidate { file, gas_budget } => {
                let gas_budget = gas_budget.unwrap_or(DEFAULT_GAS_BUDGET);
                let validator_info_bytes = fs::read(file)?;
                // Note: we should probably rename the struct or evolve it accordingly.
                let validator_info: GenesisValidatorInfo =
                    serde_yaml::from_slice(&validator_info_bytes)?;
                let validator = validator_info.info;

                let args = vec![
                    CallArg::Pure(
                        bcs::to_bytes(&AuthorityPublicKeyBytes::from_bytes(
                            validator.protocol_key().as_bytes(),
                        )?)
                        .unwrap(),
                    ),
                    CallArg::Pure(
                        bcs::to_bytes(&validator.network_key().as_bytes().to_vec()).unwrap(),
                    ),
                    CallArg::Pure(
                        bcs::to_bytes(&validator.worker_key().as_bytes().to_vec()).unwrap(),
                    ),
                    CallArg::Pure(
                        bcs::to_bytes(&validator_info.proof_of_possession.as_ref().to_vec())
                            .unwrap(),
                    ),
                    CallArg::Pure(
                        bcs::to_bytes(&validator.name().to_owned().into_bytes()).unwrap(),
                    ),
                    CallArg::Pure(
                        bcs::to_bytes(&validator.description.clone().into_bytes()).unwrap(),
                    ),
                    CallArg::Pure(
                        bcs::to_bytes(&validator.image_url.clone().into_bytes()).unwrap(),
                    ),
                    CallArg::Pure(
                        bcs::to_bytes(&validator.project_url.clone().into_bytes()).unwrap(),
                    ),
                    CallArg::Pure(bcs::to_bytes(validator.network_address()).unwrap()),
                    CallArg::Pure(bcs::to_bytes(validator.p2p_address()).unwrap()),
                    CallArg::Pure(bcs::to_bytes(validator.narwhal_primary_address()).unwrap()),
                    CallArg::Pure(bcs::to_bytes(validator.narwhal_worker_address()).unwrap()),
                    CallArg::Pure(bcs::to_bytes(&validator.gas_price()).unwrap()),
                    CallArg::Pure(bcs::to_bytes(&validator.commission_rate()).unwrap()),
                ];
                let response =
                    call_0x5(context, "request_add_validator_candidate", args, gas_budget).await?;
                IotaValidatorCommandResponse::BecomeCandidate(response)
            }

            IotaValidatorCommand::JoinCommittee { gas_budget } => {
                let gas_budget = gas_budget.unwrap_or(DEFAULT_GAS_BUDGET);
                let response =
                    call_0x5(context, "request_add_validator", vec![], gas_budget).await?;
                IotaValidatorCommandResponse::JoinCommittee(response)
            }

            IotaValidatorCommand::LeaveCommittee { gas_budget } => {
                // Only an active validator can leave committee.
                let _status =
                    check_status(context, HashSet::from([ValidatorStatus::Active])).await?;
                let gas_budget = gas_budget.unwrap_or(DEFAULT_GAS_BUDGET);
                let response =
                    call_0x5(context, "request_remove_validator", vec![], gas_budget).await?;
                IotaValidatorCommandResponse::LeaveCommittee(response)
            }

            IotaValidatorCommand::DisplayMetadata {
                validator_address,
                json,
            } => {
                let validator_address = validator_address.unwrap_or(context.active_address()?);
                // Default display with json serialization for better UX.
                let iota_client = context.get_client().await?;
                display_metadata(&iota_client, validator_address, json.unwrap_or(true)).await?;
                IotaValidatorCommandResponse::DisplayMetadata
            }

            IotaValidatorCommand::UpdateMetadata {
                metadata,
                gas_budget,
            } => {
                let gas_budget = gas_budget.unwrap_or(DEFAULT_GAS_BUDGET);
                let resp = update_metadata(context, metadata, gas_budget).await?;
                IotaValidatorCommandResponse::UpdateMetadata(resp)
            }

            IotaValidatorCommand::UpdateGasPrice {
                operation_cap_id,
                gas_price,
                gas_budget,
            } => {
                let gas_budget = gas_budget.unwrap_or(DEFAULT_GAS_BUDGET);
                let resp =
                    update_gas_price(context, operation_cap_id, gas_price, gas_budget).await?;
                IotaValidatorCommandResponse::UpdateGasPrice(resp)
            }

            IotaValidatorCommand::ReportValidator {
                operation_cap_id,
                reportee_address,
                undo_report,
                gas_budget,
            } => {
                let gas_budget = gas_budget.unwrap_or(DEFAULT_GAS_BUDGET);
                let undo_report = undo_report.unwrap_or(false);
                let resp = report_validator(
                    context,
                    reportee_address,
                    operation_cap_id,
                    undo_report,
                    gas_budget,
                )
                .await?;
                IotaValidatorCommandResponse::ReportValidator(resp)
            }

            IotaValidatorCommand::SerializePayloadForPoP {
                account_address,
                protocol_public_key,
            } => {
                let mut msg: Vec<u8> = Vec::new();
                msg.extend_from_slice(protocol_public_key.as_bytes());
                msg.extend_from_slice(account_address.as_ref());
                let mut intent_msg_bytes = bcs::to_bytes(&IntentMessage::new(
                    Intent::iota_app(IntentScope::ProofOfPossession),
                    msg,
                ))
                .expect("Message serialization should not fail");
                DEFAULT_EPOCH_ID.write(&mut intent_msg_bytes);
                IotaValidatorCommandResponse::SerializedPayload(Base64::encode(&intent_msg_bytes))
            }

            IotaValidatorCommand::DisplayGasPriceUpdateRawTxn {
                sender_address,
                operation_cap_id,
                new_gas_price,
                gas_budget,
            } => {
                let gas_budget = gas_budget.unwrap_or(DEFAULT_GAS_BUDGET);
                let (_status, _summary, cap_obj_ref) =
                    get_cap_object_ref(context, Some(operation_cap_id)).await?;

                let args = vec![
                    CallArg::Object(ObjectArg::ImmOrOwnedObject(cap_obj_ref)),
                    CallArg::Pure(bcs::to_bytes(&new_gas_price).unwrap()),
                ];
                let data = construct_unsigned_0x5_txn(
                    context,
                    sender_address,
                    "request_set_gas_price",
                    args,
                    gas_budget,
                )
                .await?;
                let serialized_data = Base64::encode(bcs::to_bytes(&data)?);
                IotaValidatorCommandResponse::DisplayGasPriceUpdateRawTxn {
                    data,
                    serialized_data,
                }
            }
        });
        ret
    }
}

async fn get_cap_object_ref(
    context: &mut WalletContext,
    operation_cap_id: Option<ObjectID>,
) -> Result<(ValidatorStatus, IotaValidatorSummary, ObjectRef)> {
    let iota_client = context.get_client().await?;
    if let Some(operation_cap_id) = operation_cap_id {
        let (status, summary) =
            get_validator_summary_from_cap_id(&iota_client, operation_cap_id).await?;
        let cap_obj_ref = iota_client
            .read_api()
            .get_object_with_options(
                summary.operation_cap_id,
                IotaObjectDataOptions::default().with_owner(),
            )
            .await?
            .object_ref_if_exists()
            .ok_or_else(|| anyhow!("OperationCap {} does not exist", operation_cap_id))?;
        Ok::<(ValidatorStatus, IotaValidatorSummary, ObjectRef), anyhow::Error>((
            status,
            summary,
            cap_obj_ref,
        ))
    } else {
        // Sender is Reporter Validator itself.
        let validator_address = context.active_address()?;
        let (status, summary) = get_validator_summary(&iota_client, validator_address)
            .await?
            .ok_or_else(|| anyhow::anyhow!("{} is not a validator.", validator_address))?;
        // TODO we should allow validator to perform this operation even though the Cap
        // is not at hand. But for now we need to make sure the cap is owned by
        // the sender.
        let cap_object_id = summary.operation_cap_id;
        let resp = iota_client
            .read_api()
            .get_object_with_options(cap_object_id, IotaObjectDataOptions::default().with_owner())
            .await
            .map_err(|e| anyhow!(e))?;
        // Safe to unwrap as we ask with `with_owner`.
        let owner = resp.owner().unwrap();
        let cap_obj_ref = resp
            .object_ref_if_exists()
            .unwrap_or_else(|| panic!("OperationCap {} shall exist.", cap_object_id));
        if owner != Owner::AddressOwner(context.active_address()?) {
            anyhow::bail!(
                "OperationCap {} is not owned by the sender address {} but {:?}",
                summary.operation_cap_id,
                validator_address,
                owner
            );
        }
        Ok((status, summary, cap_obj_ref))
    }
}

async fn update_gas_price(
    context: &mut WalletContext,
    operation_cap_id: Option<ObjectID>,
    gas_price: u64,
    gas_budget: u64,
) -> Result<IotaTransactionBlockResponse> {
    let (_status, _summary, cap_obj_ref) = get_cap_object_ref(context, operation_cap_id).await?;

    // TODO: Only active/pending validators can set gas price.

    let args = vec![
        CallArg::Object(ObjectArg::ImmOrOwnedObject(cap_obj_ref)),
        CallArg::Pure(bcs::to_bytes(&gas_price).unwrap()),
    ];
    call_0x5(context, "request_set_gas_price", args, gas_budget).await
}

async fn report_validator(
    context: &mut WalletContext,
    reportee_address: IotaAddress,
    operation_cap_id: Option<ObjectID>,
    undo_report: bool,
    gas_budget: u64,
) -> Result<IotaTransactionBlockResponse> {
    let (status, summary, cap_obj_ref) = get_cap_object_ref(context, operation_cap_id).await?;

    let validator_address = summary.iota_address;
    // Only active validators can report/un-report.
    if !matches!(status, ValidatorStatus::Active) {
        anyhow::bail!(
            "Only active Validator can report/un-report Validators, but {} is {:?}.",
            validator_address,
            status
        );
    }
    let args = vec![
        CallArg::Object(ObjectArg::ImmOrOwnedObject(cap_obj_ref)),
        CallArg::Pure(bcs::to_bytes(&reportee_address).unwrap()),
    ];
    let function_name = if undo_report {
        "undo_report_validator"
    } else {
        "report_validator"
    };
    call_0x5(context, function_name, args, gas_budget).await
}

async fn get_validator_summary_from_cap_id(
    client: &IotaClient,
    operation_cap_id: ObjectID,
) -> anyhow::Result<(ValidatorStatus, IotaValidatorSummary)> {
    let resp = client
        .read_api()
        .get_object_with_options(
            operation_cap_id,
            IotaObjectDataOptions::default().with_bcs(),
        )
        .await?;
    let bcs = resp.move_object_bcs().ok_or_else(|| {
        anyhow::anyhow!(
            "Object {} does not exist or does not return bcs bytes",
            operation_cap_id
        )
    })?;
    let cap = bcs::from_bytes::<UnverifiedValidatorOperationCapV1>(bcs).map_err(|e| {
        anyhow::anyhow!(
            "Can't convert bcs bytes of object {} to UnverifiedValidatorOperationCapV1: {}",
            operation_cap_id,
            e,
        )
    })?;
    let validator_address = cap.authorizer_validator_address;
    let (status, summary) = get_validator_summary(client, validator_address)
        .await?
        .ok_or_else(|| anyhow::anyhow!("{} is not a validator", validator_address))?;
    if summary.operation_cap_id != operation_cap_id {
        anyhow::bail!(
            "Validator {}'s current operation cap id is {}",
            validator_address,
            summary.operation_cap_id
        );
    }
    Ok((status, summary))
}

async fn construct_unsigned_0x5_txn(
    context: &mut WalletContext,
    sender: IotaAddress,
    function: &'static str,
    call_args: Vec<CallArg>,
    gas_budget: u64,
) -> anyhow::Result<TransactionData> {
    let iota_client = context.get_client().await?;
    let mut args = vec![CallArg::IOTA_SYSTEM_MUT];
    args.extend(call_args);
    let rgp = iota_client
        .governance_api()
        .get_reference_gas_price()
        .await?;

    let gas_obj_ref = get_gas_obj_ref(sender, &iota_client, gas_budget).await?;
    TransactionData::new_move_call(
        sender,
        IOTA_SYSTEM_PACKAGE_ID,
        ident_str!("iota_system").to_owned(),
        ident_str!(function).to_owned(),
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
) -> anyhow::Result<IotaTransactionBlockResponse> {
    let sender = context.active_address()?;
    let tx_data =
        construct_unsigned_0x5_txn(context, sender, function, call_args, gas_budget).await?;
    let signature =
        context
            .config
            .keystore
            .sign_secure(&sender, &tx_data, Intent::iota_transaction())?;
    let transaction = Transaction::from_data(tx_data, vec![signature]);
    let iota_client = context.get_client().await?;
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

impl Display for IotaValidatorCommandResponse {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut writer = String::new();
        match self {
            IotaValidatorCommandResponse::MakeValidatorInfo => {}
            IotaValidatorCommandResponse::DisplayMetadata => {}
            IotaValidatorCommandResponse::BecomeCandidate(response) => {
                write!(writer, "{}", write_transaction_response(response)?)?;
            }
            IotaValidatorCommandResponse::JoinCommittee(response) => {
                write!(writer, "{}", write_transaction_response(response)?)?;
            }
            IotaValidatorCommandResponse::LeaveCommittee(response) => {
                write!(writer, "{}", write_transaction_response(response)?)?;
            }
            IotaValidatorCommandResponse::UpdateMetadata(response) => {
                write!(writer, "{}", write_transaction_response(response)?)?;
            }
            IotaValidatorCommandResponse::UpdateGasPrice(response) => {
                write!(writer, "{}", write_transaction_response(response)?)?;
            }
            IotaValidatorCommandResponse::ReportValidator(response) => {
                write!(writer, "{}", write_transaction_response(response)?)?;
            }
            IotaValidatorCommandResponse::SerializedPayload(response) => {
                write!(writer, "Serialized payload: {}", response)?;
            }
            IotaValidatorCommandResponse::DisplayGasPriceUpdateRawTxn {
                data,
                serialized_data,
            } => {
                write!(
                    writer,
                    "Transaction: {:?}, \nSerialized transaction: {:?}",
                    data, serialized_data
                )?;
            }
        }
        write!(f, "{}", writer.trim_end_matches('\n'))
    }
}

pub fn write_transaction_response(
    response: &IotaTransactionBlockResponse,
) -> Result<String, fmt::Error> {
    // we requested with for full_content, so the following content should be
    // available.
    let success = response.status_ok().unwrap();
    let lines = vec![
        String::from("----- Transaction Digest ----"),
        response.digest.to_string(),
        String::from("\n----- Transaction Data ----"),
        response.transaction.as_ref().unwrap().to_string(),
        String::from("----- Transaction Effects ----"),
        response.effects.as_ref().unwrap().to_string(),
    ];
    let mut writer = String::new();
    for line in lines {
        let colorized_line = if success { line.green() } else { line.red() };
        writeln!(writer, "{}", colorized_line)?;
    }
    Ok(writer)
}

impl Debug for IotaValidatorCommandResponse {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let string = serde_json::to_string_pretty(self);
        let s = match string {
            Ok(s) => s,
            Err(err) => format!("{err}").red().to_string(),
        };
        write!(f, "{}", s)
    }
}

impl IotaValidatorCommandResponse {
    pub fn print(&self, pretty: bool) {
        match self {
            // Don't print empty responses
            IotaValidatorCommandResponse::MakeValidatorInfo
            | IotaValidatorCommandResponse::DisplayMetadata => {}
            other => {
                let line = if pretty {
                    format!("{other}")
                } else {
                    format!("{:?}", other)
                };
                // Log line by line
                for line in line.lines() {
                    println!("{line}");
                }
            }
        }
    }
}

#[derive(Debug, Hash, PartialEq, Eq)]
pub enum ValidatorStatus {
    Active,
    Pending,
}

pub async fn get_validator_summary(
    client: &IotaClient,
    validator_address: IotaAddress,
) -> anyhow::Result<Option<(ValidatorStatus, IotaValidatorSummary)>> {
    let IotaSystemStateSummary {
        active_validators,
        pending_active_validators_id,
        ..
    } = client
        .governance_api()
        .get_latest_iota_system_state()
        .await?;
    let mut status = None;
    let mut active_validators = active_validators
        .into_iter()
        .map(|s| (s.iota_address, s))
        .collect::<BTreeMap<_, _>>();
    let validator_info = if active_validators.contains_key(&validator_address) {
        status = Some(ValidatorStatus::Active);
        Some(active_validators.remove(&validator_address).unwrap())
    } else {
        // Check panding validators
        get_pending_candidate_summary(validator_address, client, pending_active_validators_id)
            .await?
            .map(|v| v.into_iota_validator_summary())
            .tap_some(|_s| status = Some(ValidatorStatus::Pending))

        // TODO also check candidate and inactive valdiators
    };
    if validator_info.is_none() {
        return Ok(None);
    }
    // status is safe unwrap because it has to be Some when the code recahes here
    // validator_info is safe to unwrap because of the above check
    Ok(Some((status.unwrap(), validator_info.unwrap())))
}

async fn display_metadata(
    client: &IotaClient,
    validator_address: IotaAddress,
    json: bool,
) -> anyhow::Result<()> {
    match get_validator_summary(client, validator_address).await? {
        None => println!(
            "{} is not an active or pending Validator.",
            validator_address
        ),
        Some((status, info)) => {
            println!("{}'s valdiator status: {:?}", validator_address, status);
            if json {
                println!("{}", serde_json::to_string_pretty(&info)?);
            } else {
                println!("{:#?}", info);
            }
        }
    }
    Ok(())
}

async fn get_pending_candidate_summary(
    validator_address: IotaAddress,
    iota_client: &IotaClient,
    pending_active_validators_id: ObjectID,
) -> anyhow::Result<Option<ValidatorV1>> {
    let pending_validators = iota_client
        .read_api()
        .get_dynamic_fields(pending_active_validators_id, None, None)
        .await?
        .data
        .into_iter()
        .map(|dyi| dyi.object_id)
        .collect::<Vec<_>>();
    let resps = iota_client
        .read_api()
        .multi_get_object_with_options(
            pending_validators,
            IotaObjectDataOptions::default().with_bcs(),
        )
        .await?;
    for resp in resps {
        // We always expect an objectId from the response as one of data/error should be
        // included.
        let object_id = resp.object_id()?;
        let bcs = resp.move_object_bcs().ok_or_else(|| {
            anyhow::anyhow!(
                "Object {} does not exist or does not return bcs bytes",
                object_id
            )
        })?;
        let val = bcs::from_bytes::<ValidatorV1>(bcs).map_err(|e| {
            anyhow::anyhow!(
                "Can't convert bcs bytes of object {} to ValidatorV1: {}",
                object_id,
                e,
            )
        })?;
        if val.verified_metadata().iota_address == validator_address {
            return Ok(Some(val));
        }
    }
    Ok(None)
}

#[derive(Subcommand)]
#[clap(rename_all = "kebab-case")]
pub enum MetadataUpdate {
    /// Update name. Effectuate immediately.
    Name { name: String },
    /// Update description. Effectuate immediately.
    Description { description: String },
    /// Update Image URL. Effectuate immediately.
    ImageUrl { image_url: String },
    /// Update Project URL. Effectuate immediately.
    ProjectUrl { project_url: String },
    /// Update Network Address. Effectuate from next epoch.
    NetworkAddress { network_address: Multiaddr },
    /// Update Primary Address. Effectuate from next epoch.
    PrimaryAddress { primary_address: Multiaddr },
    /// Update Worker Address. Effectuate from next epoch.
    WorkerAddress { worker_address: Multiaddr },
    /// Update P2P Address. Effectuate from next epoch.
    P2pAddress { p2p_address: Multiaddr },
    /// Update Network Public Key. Effectuate from next epoch.
    NetworkPubKey {
        #[clap(name = "network-key-path")]
        file: PathBuf,
    },
    /// Update Worker Public Key. Effectuate from next epoch.
    WorkerPubKey {
        #[clap(name = "worker-key-path")]
        file: PathBuf,
    },
    /// Update Protocol Public Key and Proof and Possession. Effectuate from
    /// next epoch.
    ProtocolPubKey {
        #[clap(name = "protocol-key-path")]
        file: PathBuf,
    },
}

async fn update_metadata(
    context: &mut WalletContext,
    metadata: MetadataUpdate,
    gas_budget: u64,
) -> anyhow::Result<IotaTransactionBlockResponse> {
    use ValidatorStatus::*;
    match metadata {
        MetadataUpdate::Name { name } => {
            let args = vec![CallArg::Pure(bcs::to_bytes(&name.into_bytes()).unwrap())];
            call_0x5(context, "update_validator_name", args, gas_budget).await
        }
        MetadataUpdate::Description { description } => {
            let args = vec![CallArg::Pure(
                bcs::to_bytes(&description.into_bytes()).unwrap(),
            )];
            call_0x5(context, "update_validator_description", args, gas_budget).await
        }
        MetadataUpdate::ImageUrl { image_url } => {
            let args = vec![CallArg::Pure(
                bcs::to_bytes(&image_url.into_bytes()).unwrap(),
            )];
            call_0x5(context, "update_validator_image_url", args, gas_budget).await
        }
        MetadataUpdate::ProjectUrl { project_url } => {
            let args = vec![CallArg::Pure(
                bcs::to_bytes(&project_url.into_bytes()).unwrap(),
            )];
            call_0x5(context, "update_validator_project_url", args, gas_budget).await
        }
        MetadataUpdate::NetworkAddress { network_address } => {
            let _status = check_status(context, HashSet::from([Pending, Active])).await?;
            let args = vec![CallArg::Pure(bcs::to_bytes(&network_address).unwrap())];
            call_0x5(
                context,
                "update_validator_next_epoch_network_address",
                args,
                gas_budget,
            )
            .await
        }
        MetadataUpdate::PrimaryAddress { primary_address } => {
            let _status = check_status(context, HashSet::from([Pending, Active])).await?;
            let args = vec![CallArg::Pure(bcs::to_bytes(&primary_address).unwrap())];
            call_0x5(
                context,
                "update_validator_next_epoch_primary_address",
                args,
                gas_budget,
            )
            .await
        }
        MetadataUpdate::WorkerAddress { worker_address } => {
            // Only an active validator can leave committee.
            let _status = check_status(context, HashSet::from([Pending, Active])).await?;
            let args = vec![CallArg::Pure(bcs::to_bytes(&worker_address).unwrap())];
            call_0x5(
                context,
                "update_validator_next_epoch_worker_address",
                args,
                gas_budget,
            )
            .await
        }
        MetadataUpdate::P2pAddress { p2p_address } => {
            let _status = check_status(context, HashSet::from([Pending, Active])).await?;
            let args = vec![CallArg::Pure(bcs::to_bytes(&p2p_address).unwrap())];
            call_0x5(
                context,
                "update_validator_next_epoch_p2p_address",
                args,
                gas_budget,
            )
            .await
        }
        MetadataUpdate::NetworkPubKey { file } => {
            let _status = check_status(context, HashSet::from([Pending, Active])).await?;
            let network_pub_key: NetworkPublicKey =
                read_network_keypair_from_file(file)?.public().clone();
            let args = vec![CallArg::Pure(
                bcs::to_bytes(&network_pub_key.as_bytes().to_vec()).unwrap(),
            )];
            call_0x5(
                context,
                "update_validator_next_epoch_network_pubkey",
                args,
                gas_budget,
            )
            .await
        }
        MetadataUpdate::WorkerPubKey { file } => {
            let _status = check_status(context, HashSet::from([Pending, Active])).await?;
            let worker_pub_key: NetworkPublicKey =
                read_network_keypair_from_file(file)?.public().clone();
            let args = vec![CallArg::Pure(
                bcs::to_bytes(&worker_pub_key.as_bytes().to_vec()).unwrap(),
            )];
            call_0x5(
                context,
                "update_validator_next_epoch_worker_pubkey",
                args,
                gas_budget,
            )
            .await
        }
        MetadataUpdate::ProtocolPubKey { file } => {
            let _status = check_status(context, HashSet::from([Pending, Active])).await?;
            let iota_address = context.active_address()?;
            let protocol_key_pair: AuthorityKeyPair = read_authority_keypair_from_file(file)?;
            let protocol_pub_key: AuthorityPublicKey = protocol_key_pair.public().clone();
            let pop = generate_proof_of_possession(&protocol_key_pair, iota_address);
            let args = vec![
                CallArg::Pure(
                    bcs::to_bytes(&AuthorityPublicKeyBytes::from_bytes(
                        protocol_pub_key.as_bytes(),
                    )?)
                    .unwrap(),
                ),
                CallArg::Pure(bcs::to_bytes(&pop.as_ref().to_vec()).unwrap()),
            ];
            call_0x5(
                context,
                "update_validator_next_epoch_protocol_pubkey",
                args,
                gas_budget,
            )
            .await
        }
    }
}

async fn check_status(
    context: &mut WalletContext,
    allowed_status: HashSet<ValidatorStatus>,
) -> Result<ValidatorStatus> {
    let iota_client = context.get_client().await?;
    let validator_address = context.active_address()?;
    let summary = get_validator_summary(&iota_client, validator_address).await?;
    if summary.is_none() {
        bail!("{validator_address} is not a Validator.");
    }
    let (status, _summary) = summary.unwrap();
    if allowed_status.contains(&status) {
        return Ok(status);
    }
    bail!(
        "Validator {validator_address} is {:?}, this operation is not supported in this tool or prohibited.",
        status
    )
}
