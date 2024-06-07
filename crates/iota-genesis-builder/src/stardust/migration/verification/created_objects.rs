// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use anyhow::{anyhow, bail, Result};
use iota_types::base_types::ObjectID;

/// Defines objects that may have been created by migrating an [`Output`].
#[derive(Default)]
pub struct CreatedObjects {
    output: Option<ObjectID>,
    coin: Option<ObjectID>,
    coin_metadata: Option<ObjectID>,
    package: Option<ObjectID>,
    max_supply_policy: Option<ObjectID>,
    native_tokens: Option<Vec<ObjectID>>,
    minted_coin: Option<ObjectID>,
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

    pub fn coin_metadata(&self) -> Result<&ObjectID> {
        self.coin_metadata
            .as_ref()
            .ok_or_else(|| anyhow!("no created coin metadata object"))
    }

    pub(crate) fn set_coin_metadata(&mut self, id: ObjectID) -> Result<()> {
        if let Some(id) = self.coin_metadata {
            bail!("coin metadata already set: {id}")
        }
        self.coin_metadata.replace(id);
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

    pub fn max_supply_policy(&self) -> Result<&ObjectID> {
        self.max_supply_policy
            .as_ref()
            .ok_or_else(|| anyhow!("no created max supply policy object"))
    }

    pub(crate) fn set_max_supply_policy(&mut self, id: ObjectID) -> Result<()> {
        if let Some(id) = self.max_supply_policy {
            bail!("max supply policy already set: {id}")
        }
        self.max_supply_policy.replace(id);
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

    pub fn minted_coin(&self) -> Result<&ObjectID> {
        self.minted_coin
            .as_ref()
            .ok_or_else(|| anyhow!("no minted coin object"))
    }

    pub(crate) fn set_minted_coin(&mut self, id: ObjectID) -> Result<()> {
        if let Some(id) = self.minted_coin {
            bail!("minted coin already set: {id}")
        }
        self.minted_coin.replace(id);
        Ok(())
    }
}
