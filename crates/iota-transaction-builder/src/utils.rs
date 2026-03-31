// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::result::Result;

use anyhow::{anyhow, bail};
use futures::future::join_all;
use iota_json::{
    IotaJsonValue, ResolvedCallArg, is_receiving_argument, resolve_call_args,
    resolve_move_function_args,
};
use iota_json_rpc_types::{IotaArgument, IotaData, IotaObjectDataOptions, IotaRawData, PtbInput};
use iota_protocol_config::ProtocolConfig;
use iota_types::{
    base_types::{IotaAddress, ObjectID, ObjectRef, ObjectType, TxContext, TxContextKind},
    error::UserInputError,
    fp_ensure,
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
    pub async fn select_gas(
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
            let mut cursor = None;
            // Paginate through all gas coins owned by the signer
            loop {
                let page = self
                    .0
                    .get_owned_objects(
                        signer,
                        GasCoin::type_(),
                        cursor,
                        None,
                        IotaObjectDataOptions::new().with_bcs(),
                    )
                    .await?;
                for response in &page.data {
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
                if !page.has_next_page {
                    break;
                }
                cursor = page.next_cursor;
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

    /// Resolve a [`ResolvedCallArg`] to a [`CallArg`] or a list of
    /// [`ObjectArg`] for object vectors.
    async fn resolved_call_arg_to_call_arg(
        &self,
        resolved_arg: ResolvedCallArg,
        param: &SignatureToken,
        module: &CompiledModule,
    ) -> Result<ResolvedCallArgResult, anyhow::Error> {
        match resolved_arg {
            ResolvedCallArg::Pure(bytes) => {
                Ok(ResolvedCallArgResult::CallArg(CallArg::Pure(bytes)))
            }
            ResolvedCallArg::Object(id) => {
                let is_mutable =
                    matches!(param, SignatureToken::MutableReference(_)) || !param.is_reference();
                let object_arg = self.get_object_arg(id, is_mutable, module, param).await?;
                Ok(ResolvedCallArgResult::CallArg(CallArg::Object(object_arg)))
            }
            ResolvedCallArg::ObjVec(vec_ids) => {
                let mut object_args = Vec::with_capacity(vec_ids.len());
                for id in vec_ids {
                    object_args.push(self.get_object_arg(id, false, module, param).await?);
                }
                Ok(ResolvedCallArgResult::ObjVec(object_args))
            }
        }
    }

    /// Resolve a single JSON value to a [`ResolvedCallArgResult`].
    async fn resolve_json_value_to_call_arg(
        &self,
        module: &CompiledModule,
        type_args: &[TypeTag],
        value: IotaJsonValue,
        param: &SignatureToken,
        idx: usize,
    ) -> Result<ResolvedCallArgResult, anyhow::Error> {
        let json_slice = [value];
        let param_slice = [param.clone()];
        let resolved = resolve_call_args(module, type_args, &json_slice, &param_slice)?;
        let resolved_arg = resolved
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("Unable to resolve argument at index {idx}"))?;
        self.resolved_call_arg_to_call_arg(resolved_arg, param, module)
            .await
    }

    /// Convert provided JSON arguments for a move function to their
    /// [`Argument`] representation and check their validity.
    pub async fn resolve_and_checks_json_args(
        &self,
        builder: &mut ProgrammableTransactionBuilder,
        package_id: ObjectID,
        module_ident: &Identifier,
        function_ident: &Identifier,
        type_args: &[TypeTag],
        json_args: Vec<IotaJsonValue>,
    ) -> Result<Vec<Argument>, anyhow::Error> {
        // Fetch the move package for the given package ID.
        let package = self.fetch_move_package(package_id).await?;
        let module = package.deserialize_module(module_ident, &BinaryConfig::standard())?;

        // Then resolve the function parameters type.
        let json_args_and_tokens = resolve_move_function_args(
            &package,
            module_ident.clone(),
            function_ident.clone(),
            type_args,
            json_args,
        )?;

        // Finally construct the input arguments for the builder.
        let mut args = Vec::new();
        for (arg, expected_type) in json_args_and_tokens {
            let result = self
                .resolved_call_arg_to_call_arg(arg, &expected_type, &module)
                .await?;
            args.push(match result {
                ResolvedCallArgResult::CallArg(call_arg) => builder.input(call_arg)?,
                ResolvedCallArgResult::ObjVec(object_args) => builder.make_obj_vec(object_args)?,
            });
        }

        Ok(args)
    }

    /// Convert provided PtbInput's for a move function to their
    /// [`Argument`] representation and check their validity.
    pub async fn resolve_and_check_call_args(
        &self,
        builder: &mut ProgrammableTransactionBuilder,
        package_id: ObjectID,
        module: &Identifier,
        function: &Identifier,
        type_args: &[TypeTag],
        call_args: Vec<PtbInput>,
    ) -> Result<Vec<Argument>, anyhow::Error> {
        let package = self.fetch_move_package(package_id).await?;
        let module_compiled = package.deserialize_module(module, &BinaryConfig::standard())?;
        let parameters = get_function_parameters(&module_compiled, function)?;
        let expected_len = expected_arg_count(&module_compiled, parameters);

        if call_args.len() != expected_len {
            bail!("Expected {expected_len} args, found {}", call_args.len());
        }

        let mut arguments = Vec::with_capacity(expected_len);

        for (idx, (arg, param)) in call_args
            .into_iter()
            .zip(parameters.iter().take(expected_len))
            .enumerate()
        {
            let argument = match arg {
                PtbInput::CallArg(value) => {
                    let resolved_arg = self
                        .resolve_json_value_to_call_arg(
                            &module_compiled,
                            type_args,
                            value,
                            param,
                            idx,
                        )
                        .await?;
                    match resolved_arg {
                        ResolvedCallArgResult::CallArg(call_arg) => builder.input(call_arg)?,
                        ResolvedCallArgResult::ObjVec(object_args) => {
                            builder.make_obj_vec(object_args)?
                        }
                    }
                }
                PtbInput::PtbRef(iota_arg) => match iota_arg {
                    IotaArgument::GasCoin => Argument::GasCoin,
                    IotaArgument::Input(idx) => Argument::Input(idx),
                    IotaArgument::Result(idx) => Argument::Result(idx),
                    IotaArgument::NestedResult(idx, nested_idx) => {
                        Argument::NestedResult(idx, nested_idx)
                    }
                },
            };

            arguments.push(argument);
        }

        Ok(arguments)
    }

    /// Convert provided JSON arguments for a move function to their
    /// [`Argument`] representation and check their validity. Also, check that
    /// the passed function is compliant to the Move View
    /// Function specification.
    pub async fn resolve_and_checks_json_view_args(
        &self,
        builder: &mut ProgrammableTransactionBuilder,
        package_id: ObjectID,
        module_ident: &Identifier,
        function_ident: &Identifier,
        type_args: &[TypeTag],
        json_args: Vec<IotaJsonValue>,
    ) -> Result<Vec<Argument>, anyhow::Error> {
        // Fetch the move package for the given package ID.
        let package = self.fetch_move_package(package_id).await?;
        let module = package.deserialize_module(module_ident, &BinaryConfig::standard())?;

        // Extract the expected function signature and check the return type.
        // If the function is a view function, it MUST return at least a value.
        check_function_has_a_return(&module, function_ident)?;

        // Then resolve the function parameters type.
        let json_args_and_tokens = resolve_move_function_args(
            &package,
            module_ident.clone(),
            function_ident.clone(),
            type_args,
            json_args,
        )?;

        // Finally construct the input arguments for the builder.
        let mut args = Vec::new();
        for (arg, expected_type) in json_args_and_tokens {
            args.push(match arg {
                // Move View Functions can accept pure arguments.
                ResolvedCallArg::Pure(p) => builder.input(CallArg::Pure(p)),
                // Move View Functions can accept only immutable object references.
                ResolvedCallArg::Object(id) => {
                    fp_ensure!(
                            matches!(expected_type, SignatureToken::Reference(_)),
                            UserInputError::InvalidMoveViewFunction {
                                error: format!("Found a function parameter which is not an immutable reference {expected_type:?}")
                                    ,
                            }
                            .into()
                        );
                    builder.input(CallArg::Object(
                        self.get_object_arg(
                            id,
                            // Setting false is safe because of fp_ensure! above
                            false,
                            &module,
                            &expected_type,
                        )
                        .await?,
                    ))
                }
                // Move View Functions can not accept vector of object by value (this case).
                ResolvedCallArg::ObjVec(_) => Err(UserInputError::InvalidMoveViewFunction {
                    error: "Found a function parameter which is a vector of objects".to_owned(),
                }
                .into()),
            }?);
        }

        Ok(args)
    }

    /// Convert provided JSON arguments for a move function to their
    /// [`CallArg`] representation and check their validity.
    ///
    /// Note: For object vectors, each object is added as a separate
    /// `CallArg::Object` entry.
    pub async fn resolve_and_check_json_args_to_call_args(
        &self,
        package_id: ObjectID,
        module: &Identifier,
        function: &Identifier,
        type_args: &[TypeTag],
        call_args: Vec<IotaJsonValue>,
    ) -> Result<Vec<CallArg>, anyhow::Error> {
        let package = self.fetch_move_package(package_id).await?;
        let module_compiled = package.deserialize_module(module, &BinaryConfig::standard())?;
        let parameters = get_function_parameters(&module_compiled, function)?;
        let expected_len = expected_arg_count(&module_compiled, parameters);

        let mut arguments = Vec::with_capacity(expected_len);

        for (idx, (value, param)) in call_args
            .into_iter()
            .zip(parameters.iter().take(expected_len))
            .enumerate()
        {
            let resolved_arg = self
                .resolve_json_value_to_call_arg(&module_compiled, type_args, value, param, idx)
                .await?;

            match resolved_arg {
                ResolvedCallArgResult::CallArg(call_arg) => arguments.push(call_arg),
                ResolvedCallArgResult::ObjVec(object_args) => {
                    // For object vectors, add each object as a separate CallArg::Object entry
                    for obj_arg in object_args {
                        arguments.push(CallArg::Object(obj_arg));
                    }
                }
            }
        }

        Ok(arguments)
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

    /// Helper function to get a Move Package for a provided ObjectID.
    async fn fetch_move_package(&self, package_id: ObjectID) -> Result<MovePackage, anyhow::Error> {
        let object = self
            .0
            .get_object_with_options(package_id, IotaObjectDataOptions::bcs_lossless())
            .await?
            .into_object()?;
        let Some(IotaRawData::Package(package)) = object.bcs else {
            bail!("Bcs field in object [{package_id}] is missing or not a package.");
        };
        Ok(MovePackage::new(
            package.id,
            object.version,
            package.module_map,
            ProtocolConfig::get_for_min_version().max_move_package_size(),
            package.type_origin_table,
            package.linkage_table,
        )?)
    }
}

/// Helper function to check if the provided function within a module has at
/// least a return type.
fn check_function_has_a_return(
    module: &CompiledModule,
    function_ident: &Identifier,
) -> Result<(), anyhow::Error> {
    let (_, fdef) = module
        .find_function_def_by_name(function_ident.as_str())
        .ok_or_else(|| {
            anyhow!(
                "Could not resolve function {} in module {}",
                function_ident,
                module.self_id()
            )
        })?;
    let function_signature = module.function_handle_at(fdef.function);
    fp_ensure!(
        !&module.signature_at(function_signature.return_).is_empty(),
        UserInputError::InvalidMoveViewFunction {
            error: "No return type for this function".to_owned(),
        }
        .into()
    );
    Ok(())
}

/// Result of resolving a call argument, distinguishing between single
/// [`CallArg`] and object vectors.
enum ResolvedCallArgResult {
    CallArg(CallArg),
    ObjVec(Vec<ObjectArg>),
}

/// Get function parameters from a compiled module, excluding TxContext.
fn get_function_parameters<'a>(
    module: &'a CompiledModule,
    function: &Identifier,
) -> Result<&'a [SignatureToken], anyhow::Error> {
    let function_str = function.as_ident_str();
    let function_def = module
        .function_defs
        .iter()
        .find(|function_def| {
            module.identifier_at(module.function_handle_at(function_def.function).name)
                == function_str
        })
        .ok_or_else(|| {
            anyhow!(
                "Could not resolve function {function} in module {}",
                module.self_id()
            )
        })?;
    let function_signature = module.function_handle_at(function_def.function);
    Ok(&module.signature_at(function_signature.parameters).0)
}

/// Calculate expected argument count, excluding TxContext if present.
fn expected_arg_count(module: &CompiledModule, parameters: &[SignatureToken]) -> usize {
    match parameters.last() {
        Some(param) if TxContext::kind(module, param) != TxContextKind::None => {
            parameters.len() - 1
        }
        _ => parameters.len(),
    }
}
