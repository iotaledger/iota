// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use anyhow::{Result, anyhow, bail};
use iota_sdk_types::ObjectId;

/// Defines objects that may have been created by migrating an
/// [`Output`](iota_stardust_types::block::output::Output).
#[derive(Default)]
pub struct CreatedObjects {
    output: Option<ObjectId>,
    package: Option<ObjectId>,
    coin: Option<ObjectId>,
    native_token_coin: Option<ObjectId>,
    native_tokens: Option<Vec<ObjectId>>,
    coin_manager: Option<ObjectId>,
    coin_manager_treasury_cap: Option<ObjectId>,
}

impl CreatedObjects {
    pub fn output(&self) -> Result<&ObjectId> {
        self.output
            .as_ref()
            .ok_or_else(|| anyhow!("no created output object"))
    }

    pub(crate) fn set_output(&mut self, id: ObjectId) -> Result<()> {
        if let Some(id) = self.output {
            bail!("output already set: {id}")
        }
        self.output.replace(id);
        Ok(())
    }

    pub fn package(&self) -> Result<&ObjectId> {
        self.package
            .as_ref()
            .ok_or_else(|| anyhow!("no created package object"))
    }

    pub(crate) fn set_package(&mut self, id: ObjectId) -> Result<()> {
        if let Some(id) = self.package {
            bail!("package already set: {id}")
        }
        self.package.replace(id);
        Ok(())
    }

    pub fn coin(&self) -> Result<&ObjectId> {
        self.coin
            .as_ref()
            .ok_or_else(|| anyhow!("no created coin object"))
    }

    pub(crate) fn set_coin(&mut self, id: ObjectId) -> Result<()> {
        if let Some(id) = self.coin {
            bail!("coin already set: {id}")
        }
        self.coin.replace(id);
        Ok(())
    }

    pub fn native_token_coin(&self) -> Result<&ObjectId> {
        self.native_token_coin
            .as_ref()
            .ok_or_else(|| anyhow!("no native token coin object"))
    }

    pub(crate) fn set_native_token_coin(&mut self, id: ObjectId) -> Result<()> {
        if let Some(id) = self.native_token_coin {
            bail!("native token coin already set: {id}")
        }
        self.native_token_coin.replace(id);
        Ok(())
    }

    pub fn native_tokens(&self) -> Result<&[ObjectId]> {
        self.native_tokens
            .as_deref()
            .ok_or_else(|| anyhow!("no created native token objects"))
    }

    pub(crate) fn set_native_tokens(&mut self, ids: Vec<ObjectId>) -> Result<()> {
        if let Some(id) = &self.native_tokens {
            bail!("native tokens already set: {id:?}")
        }
        self.native_tokens.replace(ids);
        Ok(())
    }

    pub fn coin_manager(&self) -> Result<&ObjectId> {
        self.coin_manager
            .as_ref()
            .ok_or_else(|| anyhow!("no created coin manager object"))
    }

    pub(crate) fn set_coin_manager(&mut self, id: ObjectId) -> Result<()> {
        if let Some(id) = self.coin_manager {
            bail!("coin manager already set: {id}")
        }
        self.coin_manager.replace(id);
        Ok(())
    }

    pub fn coin_manager_treasury_cap(&self) -> Result<&ObjectId> {
        self.coin_manager_treasury_cap
            .as_ref()
            .ok_or_else(|| anyhow!("no coin manager treasury cap object"))
    }

    pub(crate) fn set_coin_manager_treasury_cap(&mut self, id: ObjectId) -> Result<()> {
        if let Some(id) = self.coin_manager_treasury_cap {
            bail!("coin manager treasury cap already set: {id}")
        }
        self.coin_manager_treasury_cap.replace(id);
        Ok(())
    }
}
