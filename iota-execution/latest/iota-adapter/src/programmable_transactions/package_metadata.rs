// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Creation of the on-chain package metadata object (`PackageMetadataV1`)
//! during package publish.
//!
//! Two layouts are supported, selected by the
//! `package_metadata_with_dynamic_module_metadata` protocol feature flag:
//! - `V1`: the legacy inline layout, where the module metadata is embedded in
//!   the `PackageMetadataV1` object itself. Built natively here for backwards
//!   compatibility with the existing ledger.
//! - `V1WithDynamicModuleMetadata`: the module metadata is stored in dynamic
//!   fields, built by the framework
//!   `create_package_metadata_v1_with_dynamic_metadata` constructor.

pub(crate) use checked::*;

#[iota_macros::with_checked_arithmetic]
mod checked {
    use std::collections::BTreeMap;

    use iota_protocol_config::ProtocolConfig;
    use iota_sdk_types::{ObjectId, TypeTag};
    use iota_types::{
        IOTA_FRAMEWORK_PACKAGE_ID,
        error::{ExecutionError, ExecutionErrorKind},
        id::ID,
        iota_sdk_types_conversions::type_tag_core_to_sdk,
        move_package::{
            IotaAttributeV1, IotaAttributeV2, PackageMetadata, ProtocolBuildConfig,
            RuntimeModuleMetadata, RuntimeModuleMetadataWrapper,
        },
    };
    use move_binary_format::{
        CompiledModule, file_format::SignatureToken, file_format_common::IOTA_METADATA_KEY,
        normalized,
    };
    use move_core_types::{ident_str, identifier::IdentStr, language_storage::ModuleId};
    use move_trace_format::format::MoveTraceBuilder;

    use crate::{
        execution_mode::System,
        programmable_transactions::{context::*, execution::execute_move_call},
    };

    const CREATE_PACKAGE_METADATA_V1_WITH_DYNAMIC_METADATA_FN_NAME: &IdentStr =
        ident_str!("create_package_metadata_v1_with_dynamic_metadata");
    const PACKAGE_METADATA_MODULE_NAME: &IdentStr = ident_str!("package_metadata");

    #[derive(Copy, Clone)]
    enum PackageMetadataHandler {
        V1,
        V1WithDynamicModuleMetadata,
    }

    impl PackageMetadataHandler {
        fn from_protocol_config(protocol_config: &ProtocolConfig) -> Self {
            if protocol_config.package_metadata_with_dynamic_module_metadata() {
                Self::V1WithDynamicModuleMetadata
            } else {
                Self::V1
            }
        }

        fn supports_view_function_metadata(self) -> bool {
            matches!(self, Self::V1WithDynamicModuleMetadata)
        }

        fn should_publish_package(
            self,
            modules_metadata: &BTreeMap<String, PendingModuleMetadata>,
        ) -> bool {
            match self {
                Self::V1 => modules_metadata
                    .values()
                    .any(|md| !md.authenticator_metadata.is_empty()),
                Self::V1WithDynamicModuleMetadata => {
                    modules_metadata.values().any(|md| !md.is_empty())
                }
            }
        }

        fn publish_package(
            self,
            context: &mut ExecutionContext<'_, '_, '_>,
            modules_metadata: &BTreeMap<String, PendingModuleMetadata>,
            storage_id: ObjectId,
            runtime_id: ObjectId,
            package_version: u64,
            trace_builder_opt: &mut Option<MoveTraceBuilder>,
        ) -> Result<(), ExecutionError> {
            match self {
                Self::V1 => self.publish_v1(
                    context,
                    modules_metadata,
                    storage_id,
                    runtime_id,
                    package_version,
                ),
                Self::V1WithDynamicModuleMetadata => self.publish_v1_with_dynamic_metadata(
                    context,
                    modules_metadata,
                    storage_id,
                    runtime_id,
                    package_version,
                    trace_builder_opt,
                ),
            }
        }

        fn publish_v1(
            self,
            context: &mut ExecutionContext<'_, '_, '_>,
            modules_metadata: &BTreeMap<String, PendingModuleMetadata>,
            storage_id: ObjectId,
            runtime_id: ObjectId,
            package_version: u64,
        ) -> Result<(), ExecutionError> {
            let metadata_uid = context.package_derived_metadata_id(storage_id)?;
            // Create the package metadata object content
            let modules_metadata_v1 = modules_metadata
                .iter()
                .map(|(mod_name, pending_metadata)| {
                    (
                        mod_name.clone(),
                        pending_metadata.authenticator_metadata.clone(),
                    )
                })
                .collect();
            let metadata = PackageMetadata::new_v1(
                metadata_uid,
                storage_id,
                runtime_id,
                package_version,
                modules_metadata_v1,
            );
            // Turn the content into an object
            let package_metadata = context.make_object_value(
                metadata.type_(),
                // used_in_non_entry_move_call
                false,
                &metadata.to_bcs_bytes(),
            )?;
            // Freeze the package metadata object
            context.freeze_object(package_metadata)
        }

        fn publish_v1_with_dynamic_metadata(
            self,
            context: &mut ExecutionContext<'_, '_, '_>,
            modules_metadata: &BTreeMap<String, PendingModuleMetadata>,
            storage_id: ObjectId,
            runtime_id: ObjectId,
            package_version: u64,
            trace_builder_opt: &mut Option<MoveTraceBuilder>,
        ) -> Result<(), ExecutionError> {
            let constructor_args = package_metadata_constructor_args(modules_metadata);
            // The Move constructor derives both the package metadata object address
            // and the per-module metadata object addresses from the storage ID.
            let args = vec![
                bcs::to_bytes(&ID::new(storage_id)).unwrap(),
                bcs::to_bytes(&ID::new(runtime_id)).unwrap(),
                bcs::to_bytes(&package_version).unwrap(),
                bcs::to_bytes(&constructor_args.modules).unwrap(),
                bcs::to_bytes(&constructor_args.auth_functions).unwrap(),
                bcs::to_bytes(&constructor_args.type_names).unwrap(),
                bcs::to_bytes(&constructor_args.view_function_names).unwrap(),
            ];
            execute_package_metadata_constructor(
                context,
                CREATE_PACKAGE_METADATA_V1_WITH_DYNAMIC_METADATA_FN_NAME,
                args,
                trace_builder_opt,
            )
        }
    }

    #[derive(Default)]
    struct PendingModuleMetadata {
        authenticator_metadata: BTreeMap<String, TypeTag>,
        view_function_metadata: Vec<String>,
    }

    impl PendingModuleMetadata {
        fn is_empty(&self) -> bool {
            self.authenticator_metadata.is_empty() && self.view_function_metadata.is_empty()
        }
    }

    /// Module metadata flattened into the parallel vectors expected by the
    /// framework `create_package_metadata_v1_with_dynamic_metadata`
    /// constructor. All vectors share the same length and ordering as
    /// `modules`; the inner vectors of `auth_functions`/`type_names` are
    /// likewise aligned per module.
    struct PackageMetadataConstructorArgs {
        modules: Vec<String>,
        auth_functions: Vec<Vec<String>>,
        type_names: Vec<Vec<String>>,
        view_function_names: Vec<Vec<String>>,
    }

    /// Creates package metadata for a Move package by extracting module
    /// metadata and passing it to the framework package metadata constructor.
    /// The framework constructor builds and freezes the metadata object. If no
    /// relevant metadata is found, the function exits without creating any
    /// package metadata.
    pub(crate) fn create_and_freeze_package_metadata_if_present(
        context: &mut ExecutionContext<'_, '_, '_>,
        modules: &[CompiledModule],
        storage_id: ObjectId,
        runtime_id: ObjectId,
        package_version: u64,
        trace_builder_opt: &mut Option<MoveTraceBuilder>,
    ) -> Result<(), ExecutionError> {
        let package_metadata_handler =
            PackageMetadataHandler::from_protocol_config(context.protocol_config);
        let mut modules_metadata_map = BTreeMap::new();
        // Extract metadata for each module
        for module in modules {
            if let Some(md) = module
                .metadata
                .iter()
                .find(|md| md.key == IOTA_METADATA_KEY.to_vec())
            {
                // At this point, if the metadata is present, it should have been already
                // validated by the iota-verifier during package verification (in
                // `publish_and_verify_modules`).
                let runtime_module_metadata: RuntimeModuleMetadata =
                    bcs::from_bytes::<RuntimeModuleMetadataWrapper>(&md.value)
                        .map_err(|_| {
                            ExecutionError::from_kind(
                                ExecutionErrorKind::VmVerificationOrDeserializationError,
                            )
                        })?
                        .try_into_runtime_module_metadata(ProtocolBuildConfig::from(
                            context.protocol_config,
                        ))
                        .map_err(|_| {
                            ExecutionError::from_kind(
                                ExecutionErrorKind::VmVerificationOrDeserializationError,
                            )
                        })?;

                // Process functions for each module in order to create package
                // metadata.
                let mut pending_module_metadata = PendingModuleMetadata::default();

                match runtime_module_metadata {
                    RuntimeModuleMetadata::V1(runtime_module_metadata_v1) => {
                        for (fn_name, fn_attributes) in
                            runtime_module_metadata_v1.fun_attributes.iter()
                        {
                            // Check attributes
                            for attribute in fn_attributes {
                                match attribute {
                                    IotaAttributeV1::Authenticator(attribute)
                                        if attribute.version == 1 =>
                                    {
                                        let contains =
                                            pending_module_metadata.authenticator_metadata.insert(
                                                fn_name.to_string(),
                                                get_authenticator_first_param_type_tag(
                                                    module, &fn_name,
                                                )?,
                                            );
                                        debug_assert!(
                                            contains.is_none(),
                                            "Duplicate function metadata for authenticator"
                                        );
                                    }

                                    _ => { /* Other attributes are ignored. */ }
                                }
                            }
                        }
                    }
                    RuntimeModuleMetadata::V2(runtime_module_metadata_v2) => {
                        for (fn_name, fn_attributes) in
                            runtime_module_metadata_v2.fun_attributes.iter()
                        {
                            // Check attributes
                            for attribute in fn_attributes {
                                match attribute {
                                    IotaAttributeV2::Authenticator(attribute)
                                        if attribute.version == 1 =>
                                    {
                                        let contains =
                                            pending_module_metadata.authenticator_metadata.insert(
                                                fn_name.to_string(),
                                                get_authenticator_first_param_type_tag(
                                                    module, &fn_name,
                                                )?,
                                            );
                                        debug_assert!(
                                            contains.is_none(),
                                            "Duplicate function metadata for authenticator"
                                        );
                                    }
                                    IotaAttributeV2::View
                                        if package_metadata_handler
                                            .supports_view_function_metadata() =>
                                    {
                                        pending_module_metadata
                                            .view_function_metadata
                                            .push(fn_name.to_string());
                                    }
                                    _ => { /* Other attributes are ignored. */ }
                                }
                            }
                        }
                    }
                }
                // Fill the package metadata with a module handle (and its related function
                // metadata) only if there is at least one function with
                // relevant metadata
                if !pending_module_metadata.is_empty() {
                    modules_metadata_map.insert(module.name().to_string(), pending_module_metadata);
                }
            }
        }

        // Only publish package metadata if there is at least one module with
        // relevant metadata
        if package_metadata_handler.should_publish_package(&modules_metadata_map) {
            package_metadata_handler.publish_package(
                context,
                &modules_metadata_map,
                storage_id,
                runtime_id,
                package_version,
                trace_builder_opt,
            )?;
        }
        Ok(())
    }

    fn package_metadata_constructor_args(
        modules_metadata: &BTreeMap<String, PendingModuleMetadata>,
    ) -> PackageMetadataConstructorArgs {
        let mut modules = Vec::with_capacity(modules_metadata.len());
        let mut auth_functions = Vec::with_capacity(modules_metadata.len());
        let mut type_names = Vec::with_capacity(modules_metadata.len());
        let mut view_function_names = Vec::with_capacity(modules_metadata.len());

        for (module_name, metadata) in modules_metadata {
            modules.push(module_name.clone());

            let mut module_auth_functions =
                Vec::with_capacity(metadata.authenticator_metadata.len());
            let mut module_type_names = Vec::with_capacity(metadata.authenticator_metadata.len());
            for (function_name, account_type) in &metadata.authenticator_metadata {
                module_auth_functions.push(function_name.clone());
                module_type_names.push(account_type.to_canonical_string(false));
            }
            auth_functions.push(module_auth_functions);
            type_names.push(module_type_names);
            view_function_names.push(metadata.view_function_metadata.clone());
        }

        PackageMetadataConstructorArgs {
            modules,
            auth_functions,
            type_names,
            view_function_names,
        }
    }

    fn execute_package_metadata_constructor(
        context: &mut ExecutionContext<'_, '_, '_>,
        function: &IdentStr,
        args: Vec<Vec<u8>>,
        trace_builder_opt: &mut Option<MoveTraceBuilder>,
    ) -> Result<(), ExecutionError> {
        let saved_linkage = context.linkage_view.steal_linkage();
        let restore_inputs_len = context.num_inputs();
        let result = (|| {
            let original_address = context.set_link_context(IOTA_FRAMEWORK_PACKAGE_ID)?;
            let runtime_id =
                ModuleId::new(original_address, PACKAGE_METADATA_MODULE_NAME.to_owned());
            // Register the serialized arguments as pure inputs so they can be
            // passed to `execute_move_call`. `System` mode bypasses the
            // constructor's visibility and permits these raw, non-primitive BCS
            // values to be used as call arguments.
            let mut arguments = Vec::with_capacity(args.len());
            for bytes in args {
                arguments.push(context.add_pure_input(bytes)?);
            }
            let arguments = context.splat_args(0, arguments)?;
            let return_values = execute_move_call::<System>(
                context,
                &mut (),
                // The package metadata module is a system package, so its storage
                // and runtime IDs match.
                &runtime_id,
                &runtime_id,
                function,
                vec![],
                arguments,
                // is_init
                false,
                trace_builder_opt,
            )?;
            assert_invariant!(
                return_values.is_empty(),
                "package metadata constructor should not have return values"
            );
            Ok(())
        })();
        // Drop the pure inputs synthesized for the call above.
        context.truncate_inputs(restore_inputs_len);
        context.linkage_view.reset_linkage();
        context.linkage_view.restore_linkage(saved_linkage)?;
        result
    }

    fn get_authenticator_first_param_type_tag(
        module: &CompiledModule,
        authenticate_fn_name: &impl AsRef<str>,
    ) -> Result<TypeTag, ExecutionError> {
        // Entering into this function, the verifier must have already been run,
        // so we can assume the function exists and has the correct signature.
        let Some((_, fn_definition)) = module.find_function_def_by_name(authenticate_fn_name)
        else {
            return Err(ExecutionError::from_kind(
                ExecutionErrorKind::VmInvariantViolation,
            ));
        };
        let fn_handle = module.function_handle_at(fn_definition.function);
        let fn_signature = module.signature_at(fn_handle.parameters);
        // We need the first parameter to be a reference type so we can extract the
        // inner as the type tag.
        match &fn_signature.0[0] {
            SignatureToken::Reference(ref_param) => {
                let pool = &mut normalized::RcPool::new();
                if let Some(type_tag) =
                    normalized::Type::new(pool, module, ref_param).to_type_tag(pool)
                {
                    Ok(type_tag_core_to_sdk(&type_tag))
                } else {
                    Err(ExecutionError::from_kind(
                        ExecutionErrorKind::VmVerificationOrDeserializationError,
                    ))
                }
            }
            _ => Err(ExecutionError::from_kind(
                ExecutionErrorKind::VmVerificationOrDeserializationError,
            )),
        }
    }
}
