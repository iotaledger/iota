// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use async_trait::async_trait;
use diesel::r2d2::R2D2Connection;
use iota_json_rpc::transaction_builder_api::TransactionBuilderApi as IotaTransactionBuilderApi;
use iota_json_rpc_types::{IotaObjectDataFilter, IotaObjectDataOptions, IotaObjectResponse};
use iota_transaction_builder::DataReader;
use iota_types::{
    base_types::{IotaAddress, ObjectID, ObjectInfo},
    object::Object,
};
use move_core_types::language_storage::StructTag;

use super::governance_api::GovernanceReadApi;
use crate::indexer_reader::IndexerReader;

pub(crate) struct TransactionBuilderApi<T: R2D2Connection + 'static> {
    inner: IndexerReader<T>,
}

impl<T: R2D2Connection> Clone for TransactionBuilderApi<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T: R2D2Connection> core::fmt::Debug for TransactionBuilderApi<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TransactionBuilderApi")
            .field("inner", &self.inner)
            .finish()
    }
}

impl<T: R2D2Connection> TransactionBuilderApi<T> {
    pub fn new(inner: IndexerReader<T>) -> IotaTransactionBuilderApi<TransactionBuilderApi<T>> {
        IotaTransactionBuilderApi::new_with_data_reader(Self { inner })
    }
}

#[async_trait]
impl<T: R2D2Connection> DataReader for TransactionBuilderApi<T> {
    async fn get_owned_objects(
        &self,
        address: IotaAddress,
        object_type: StructTag,
    ) -> Result<Vec<ObjectInfo>, anyhow::Error> {
        let stored_objects = self
            .inner
            .get_owned_objects_in_blocking_task(
                address,
                Some(IotaObjectDataFilter::StructType(object_type)),
                None,
                50, // Limit the number of objects returned to 50
            )
            .await?;

        stored_objects
            .into_iter()
            .map(|object| {
                let object = Object::try_from(object)?;
                let object_ref = object.compute_object_reference();
                let info = ObjectInfo::new(&object_ref, &object);
                Ok(info)
            })
            .collect::<Result<Vec<_>, _>>()
    }

    async fn get_object_with_options(
        &self,
        object_id: ObjectID,
        options: IotaObjectDataOptions,
    ) -> Result<IotaObjectResponse, anyhow::Error> {
        let result = self
            .inner
            .get_object_read_in_blocking_task(object_id)
            .await?;
        Ok((result, options).try_into()?)
    }

    async fn get_reference_gas_price(&self) -> Result<u64, anyhow::Error> {
        let epoch_info = GovernanceReadApi::new(self.inner.clone())
            .get_epoch_info(None)
            .await?;
        Ok(epoch_info
            .reference_gas_price
            .ok_or_else(|| anyhow::anyhow!("missing latest reference_gas_price"))?)
    }
}
