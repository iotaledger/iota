// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use async_trait::async_trait;

use crate::payload::{
    validation::check_objects, MultiGetObjects, ProcessPayload, RpcCommandProcessor, SignerInfo,
};

#[async_trait]
impl<'a> ProcessPayload<'a, &'a MultiGetObjects> for RpcCommandProcessor {
    async fn process(
        &'a self,
        op: &'a MultiGetObjects,
        _signer_info: &Option<SignerInfo>,
    ) -> Result<()> {
        let clients = self.get_clients().await?;
        check_objects(&clients, &op.object_ids, false).await;
        Ok(())
    }
}
