// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use anyhow::{anyhow, bail, Result};
use iota_types::base_types::ObjectID;

/// Defines objects that may have been created by migrating an
/// [`Output`](iota_sdk::types::block::output::Output).
#[derive(Default)]
pub struct CreatedObjects {
    output: Option<ObjectID>,
    package: Option<ObjectID>,
    coin: Option<ObjectID>,
    native_token_coin: Option<ObjectID>,
    native_tokens: Option<Vec<ObjectID>>,
    coin_manager: Option<ObjectID>,
    coin_manager_treasury_cap: Option<ObjectID>,
}

impl CreatedObjects {
    pub fn output(&self) -> Result<&ObjectID> {
        self.output
            .as_ref()
            .ok_or_else(|| anyhow!("no created output object"))
    }

    pub(crate) fn set_output(&mut self, id: ObjectID) -> Result<()> {
        if let Some(id) = self.output {
            bail!("output already set: {id}")
        }
        self.output.replace(id);
        Ok(())
    }

    pub fn package(&self) -> Result<&ObjectID> {
        self.package
            .as_ref()
            .ok_or_else(|| anyhow!("no created package object"))
    }

    pub(crate) fn set_package(&mut self, id: ObjectID) -> Result<()> {
        if let Some(id) = self.package {
            bail!("package already set: {id}")
        }
        self.package.replace(id);
        Ok(())
    }

    pub fn coin(&self) -> Result<&ObjectID> {
        self.coin
            .as_ref()
            .ok_or_else(|| anyhow!("no created coin object"))
    }

    pub(crate) fn set_coin(&mut self, id: ObjectID) -> Result<()> {
        if let Some(id) = self.coin {
            bail!("coin already set: {id}")
        }
        self.coin.replace(id);
        Ok(())
    }

    pub fn native_token_coin(&self) -> Result<&ObjectID> {
        self.native_token_coin
            .as_ref()
            .ok_or_else(|| anyhow!("no native token coin object"))
    }

    pub(crate) fn set_native_token_coin(&mut self, id: ObjectID) -> Result<()> {
        if let Some(id) = self.native_token_coin {
            bail!("native token coin already set: {id}")
        }
        self.native_token_coin.replace(id);
        Ok(())
    }

    pub fn native_tokens(&self) -> Result<&[ObjectID]> {
        self.native_tokens
            .as_deref()
            .ok_or_else(|| anyhow!("no created native token objects"))
    }

    pub(crate) fn set_native_tokens(&mut self, ids: Vec<ObjectID>) -> Result<()> {
        if let Some(id) = &self.native_tokens {
            bail!("native tokens already set: {id:?}")
        }
        self.native_tokens.replace(ids);
        Ok(())
    }

    pub fn coin_manager(&self) -> Result<&ObjectID> {
        self.coin_manager
            .as_ref()
            .ok_or_else(|| anyhow!("no created coin manager object"))
    }

    pub(crate) fn set_coin_manager(&mut self, id: ObjectID) -> Result<()> {
        if let Some(id) = self.coin_manager {
            bail!("coin manager already set: {id}")
        }
        self.coin_manager.replace(id);
        Ok(())
    }

    pub fn coin_manager_treasury_cap(&self) -> Result<&ObjectID> {
        self.coin_manager_treasury_cap
            .as_ref()
            .ok_or_else(|| anyhow!("no coin manager treasury cap object"))
    }

    pub(crate) fn set_coin_manager_treasury_cap(&mut self, id: ObjectID) -> Result<()> {
        if let Some(id) = self.coin_manager_treasury_cap {
            bail!("coin manager treasury cap already set: {id}")
        }
        self.coin_manager_treasury_cap.replace(id);
        Ok(())
    }
}
