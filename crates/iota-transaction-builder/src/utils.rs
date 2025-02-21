// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeMap, result::Result};

use anyhow::{Ok, anyhow, bail};
use futures::future::join_all;
use iota_json::{
    IotaJsonValue, ResolvedCallArg, is_receiving_argument, resolve_move_function_args,
};
use iota_json_rpc_types::{IotaData, IotaObjectDataOptions, IotaRawData};
use iota_protocol_config::ProtocolConfig;
use iota_types::{
    base_types::{IotaAddress, ObjectID, ObjectRef, ObjectType},
    gas_coin::GasCoin,
    move_package::MovePackage,
    object::{Object, Owner},
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::{Argument, CallArg, ObjectArg},
};
use move_binary_format::{
    CompiledModule, binary_config::BinaryConfig, file_format::SignatureToken,
};
use move_core_types::{identifier::Identifier, language_storage::TypeTag};

use crate::TransactionBuilder;

impl TransactionBuilder {
    /// Select a gas coin for the provided gas budget.
    pub(crate) async fn select_gas(
        &self,
        signer: IotaAddress,
        input_gas: impl Into<Option<ObjectID>>,
        gas_budget: u64,
        input_objects: Vec<ObjectID>,
        gas_price: u64,
    ) -> Result<ObjectRef, anyhow::Error> {
        if gas_budget < gas_price {
            bail!(
                "Gas budget {gas_budget} is less than the reference gas price {gas_price}. The gas budget must be at least the current reference gas price of {gas_price}."
            )
        }
        if let Some(gas) = input_gas.into() {
            self.get_object_ref(gas).await
        } else {
            let gas_objs = self.0.get_owned_objects(signer, GasCoin::type_()).await?;

            for obj in gas_objs {
                let response = self
                    .0
                    .get_object_with_options(obj.object_id, IotaObjectDataOptions::new().with_bcs())
                    .await?;
                let obj = response.object()?;
                let gas: GasCoin = bcs::from_bytes(
                    &obj.bcs
                        .as_ref()
                        .ok_or_else(|| anyhow!("bcs field is unexpectedly empty"))?
                        .try_as_move()
                        .ok_or_else(|| anyhow!("Cannot parse move object to gas object"))?
                        .bcs_bytes,
                )?;
                if !input_objects.contains(&obj.object_id) && gas.value() >= gas_budget {
                    return Ok(obj.object_ref());
                }
            }
            Err(anyhow!(
                "Cannot find gas coin for signer address {signer} with amount sufficient for the required gas budget {gas_budget}. If you are using the pay or transfer commands, you can use the pay-iota command instead, which will use the only object as gas payment."
            ))
        }
    }

    /// Get the object references for a list of object IDs
    pub async fn input_refs(&self, obj_ids: &[ObjectID]) -> Result<Vec<ObjectRef>, anyhow::Error> {
        let handles: Vec<_> = obj_ids.iter().map(|id| self.get_object_ref(*id)).collect();
        let obj_refs = join_all(handles)
            .await
            .into_iter()
            .collect::<anyhow::Result<Vec<ObjectRef>>>()?;
        Ok(obj_refs)
    }

    /// Resolve a provided [`ObjectID`] to the required [`ObjectArg`] for a
    /// given move module.
    async fn get_object_arg(
        &self,
        id: ObjectID,
        objects: &mut BTreeMap<ObjectID, Object>,
        is_mutable_ref: bool,
        view: &CompiledModule,
        arg_type: &SignatureToken,
    ) -> Result<ObjectArg, anyhow::Error> {
        let response = self
            .0
            .get_object_with_options(id, IotaObjectDataOptions::bcs_lossless())
            .await?;

        let obj: Object = response.into_object()?.try_into()?;
        let obj_ref = obj.compute_object_reference();
        let owner = obj.owner;
        objects.insert(id, obj);
        if is_receiving_argument(view, arg_type) {
            return Ok(ObjectArg::Receiving(obj_ref));
        }
        Ok(match owner {
            Owner::Shared {
                initial_shared_version,
            } => ObjectArg::SharedObject {
                id,
                initial_shared_version,
                mutable: is_mutable_ref,
            },
            Owner::AddressOwner(_) | Owner::ObjectOwner(_) | Owner::Immutable => {
                ObjectArg::ImmOrOwnedObject(obj_ref)
            }
        })
    }

    /// Convert provided JSON arguments for a move function to their
    /// [`Argument`] representation and check their validity.
    pub async fn resolve_and_checks_json_args(
        &self,
        builder: &mut ProgrammableTransactionBuilder,
        package_id: ObjectID,
        module: &Identifier,
        function: &Identifier,
        type_args: &[TypeTag],
        json_args: Vec<IotaJsonValue>,
    ) -> Result<Vec<Argument>, anyhow::Error> {
        let object = self
            .0
            .get_object_with_options(package_id, IotaObjectDataOptions::bcs_lossless())
            .await?
            .into_object()?;
        let Some(IotaRawData::Package(package)) = object.bcs else {
            bail!(
                "Bcs field in object [{}] is missing or not a package.",
                package_id
            );
        };
        let package: MovePackage = MovePackage::new(
            package.id,
            object.version,
            package.module_map,
            ProtocolConfig::get_for_min_version().max_move_package_size(),
            package.type_origin_table,
            package.linkage_table,
        )?;

        let json_args_and_tokens = resolve_move_function_args(
            &package,
            module.clone(),
            function.clone(),
            type_args,
            json_args,
        )?;

        let mut args = Vec::new();
        let mut objects = BTreeMap::new();
        let module = package.deserialize_module(module, &BinaryConfig::standard())?;
        for (arg, expected_type) in json_args_and_tokens {
            args.push(match arg {
                ResolvedCallArg::Pure(p) => builder.input(CallArg::Pure(p)),

                ResolvedCallArg::Object(id) => builder.input(CallArg::Object(
                    self.get_object_arg(
                        id,
                        &mut objects,
                        // Is mutable if passed by mutable reference or by value
                        matches!(expected_type, SignatureToken::MutableReference(_))
                            || !expected_type.is_reference(),
                        &module,
                        &expected_type,
                    )
                    .await?,
                )),

                ResolvedCallArg::ObjVec(v) => {
                    let mut object_ids = vec![];
                    for id in v {
                        object_ids.push(
                            self.get_object_arg(
                                id,
                                &mut objects,
                                // is_mutable_ref
                                false,
                                &module,
                                &expected_type,
                            )
                            .await?,
                        )
                    }
                    builder.make_obj_vec(object_ids)
                }
            }?);
        }

        Ok(args)
    }

    /// Get the latest object ref for an object.
    pub async fn get_object_ref(&self, object_id: ObjectID) -> anyhow::Result<ObjectRef> {
        // TODO: we should add retrial to reduce the transaction building error rate
        self.get_object_ref_and_type(object_id)
            .await
            .map(|(oref, _)| oref)
    }

    /// Helper function to get the latest ObjectRef (ObjectID, SequenceNumber,
    /// ObjectDigest) and ObjectType for a provided ObjectID.
    pub(crate) async fn get_object_ref_and_type(
        &self,
        object_id: ObjectID,
    ) -> anyhow::Result<(ObjectRef, ObjectType)> {
        let object = self
            .0
            .get_object_with_options(object_id, IotaObjectDataOptions::new().with_type())
            .await?
            .into_object()?;

        Ok((object.object_ref(), object.object_type()?))
    }
}
