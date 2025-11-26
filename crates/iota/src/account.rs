// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// TODO sender does not work

use std::str::FromStr;

use clap::Parser;
use anyhow::Result;
use iota_types::{move_package, base_types::ObjectID, base_types::IotaAddress};
use iota_sdk::wallet_context::WalletContext;
use iota_keys::keystore::{AccountKeystore, StoredKey};
use iota_json_rpc_types::IotaObjectDataOptions;

use crate::{
    key_identity::{KeyIdentity, get_identity_address},
    client_ptb::ptb::PTB,
    client_commands::{TxProcessingArgs, PaymentArgs, GasDataArgs}
};

#[derive(Parser)]
pub struct Account {
    #[command(subcommand)]
    command: AccountCommands,
}

impl Account {
    pub async fn execute(self, context: &mut WalletContext,) -> Result<()> {
        self.command.execute(context).await
    }
}

#[derive(Parser)]
pub enum AccountCommands {
    AttachAuthInfo {
        // TODO make ot stored key or whatever
        address: KeyIdentity,
        // TODO also take an account?
        method_path: String,
        #[command(flatten)]
        payment: PaymentArgs,
        #[command(flatten)]
        gas_data: GasDataArgs,
        #[command(flatten)]
        processing: TxProcessingArgs
    },
    Register{ address: IotaAddress, alias: Option<String> }
}

impl AccountCommands {
    pub async fn execute(
        self,
        context: &mut WalletContext,
    ) -> Result<(), anyhow::Error> {
        match self {
            AccountCommands::AttachAuthInfo { address, method_path,payment, gas_data, mut processing} => {
                let account_address = get_identity_address(Some(address), context).await?;
                // TODO figure out unwrap
                let sender = processing.sender.unwrap_or(context.active_address().unwrap());
                // let method = MoveFunctionName::try_from(&method_path)?;
                let method = method_path.split("::").collect::<Vec<_>>();
                let package_metadata = move_package::derive_package_metadata_id(ObjectID::from_str(method[0])?);

                let account_type = context.get_client().await?.read_api().get_object_with_options(account_address.into(), IotaObjectDataOptions::full_content()).await?.data.unwrap().type_.unwrap();

                println!("{account_type}" );

                let mut args = vec![
                    format!("--move-call iota::account::create_auth_info_v1 <{}> @{} '{}' '{}'", account_type.to_string(), package_metadata, method[1], method[2]),
                    "--assign authenticator".to_string(),
                    format!(
                        "--move-call iota::account::check_auth_info_v1_compatibility <{}> @{account_address} authenticator", account_type.to_string()
                    ),
                    "--assign authenticator_proof".to_string(),
                    format!("--move-call iota::account::attach_auth_info_v1 <{}> @{account_address} authenticator_proof", account_type.to_string())
                ];
                let display = core::mem::take(&mut processing.display);
                args.extend(payment.into_args());
                args.extend(gas_data.into_args());
                args.extend(processing.into_args());

                println!("{args:?}");

                let ptb = PTB { args, display };
                let res = ptb.execute(context).await?;

                println!("Transaction result: {res}");

                // TODO return?
            }
            AccountCommands::Register { address, alias } => {
                context.config_mut().keystore_mut().add_key(alias, StoredKey::Account(address))?;
            },
        }

        Ok(())
    }
}