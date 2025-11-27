// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use clap::Parser;
use anyhow::Result;
use iota_types::{base_types::IotaAddress};
use iota_sdk::wallet_context::WalletContext;
use iota_keys::keystore::{AccountKeystore, StoredKey};

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
    Register{ address: IotaAddress, alias: Option<String> }
}

impl AccountCommands {
    pub async fn execute(
        self,
        context: &mut WalletContext,
    ) -> Result<(), anyhow::Error> {
        match self {
            AccountCommands::Register { address, alias } => {
                context.config_mut().keystore_mut().add_key(alias, StoredKey::Account(address))?;
            },
        }

        Ok(())
    }
}
