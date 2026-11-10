// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Extraction of the on-chain package metadata (`PackageMetadataV1`) created when a package is
//! published or upgraded.
//!
//! Two layouts are supported, selected by the `package_metadata_with_dynamic_module_metadata`
//! protocol feature flag:
//! - `V1`: the legacy inline layout, where the module metadata is embedded in the
//!   `PackageMetadataV1` object itself, built natively.
//! - `V1WithDynamicModuleMetadata`: the module metadata is stored in dynamic fields, built by the
//!   framework `create_package_metadata_v1_with_dynamic_metadata` constructor.

use std::collections::BTreeMap;

use iota_protocol_config::ProtocolConfig;
use iota_sdk_move_types::iota_framework::object::ID;
use iota_sdk_types::{ObjectId, TypeTag};
use iota_types::{
    error::{ExecutionError, ExecutionErrorKind},
    iota_sdk_types_conversions::type_tag_core_to_sdk,
    move_package::{
        IotaAttributeV1, IotaAttributeV2, PackageMetadata, ProtocolBuildConfig,
        RuntimeModuleMetadata, RuntimeModuleMetadataWrapper, derive_package_metadata_id,
    },
};
use move_binary_format::{
    CompiledModule, file_format::SignatureToken, file_format_common::IOTA_METADATA_KEY, normalized,
};
use move_core_types::{ident_str, identifier::IdentStr};

pub const PACKAGE_METADATA_MODULE_NAME: &IdentStr = ident_str!("package_metadata");
pub const CREATE_PACKAGE_METADATA_V1_WITH_DYNAMIC_METADATA_FN_NAME: &IdentStr =
    ident_str!("create_package_metadata_v1_with_dynamic_metadata");

/// The package metadata to create for a published or upgraded package.
pub enum PackageMetadataCreation {
    /// A `PackageMetadataV1` object with the inline layout, to be frozen. Its `id` is derived
    /// from the package storage ID and still has to be recorded as a new UID.
    Inline {
        id: ObjectId,
        metadata: PackageMetadata,
    },
    /// BCS-serialized arguments for the framework constructor
    /// `package_metadata::create_package_metadata_v1_with_dynamic_metadata`, which builds and
    /// freezes the object.
    DynamicFields(Vec<Vec<u8>>),
}

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
            Self::V1WithDynamicModuleMetadata => modules_metadata.values().any(|md| !md.is_empty()),
        }
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

/// Returns the package metadata to create for `modules`, or `None` when no module carries
/// metadata that is recorded on chain. The modules must have passed the IOTA verifier, which
/// validates their IOTA metadata.
pub fn package_metadata_creation(
    protocol_config: &ProtocolConfig,
    modules: &[CompiledModule],
    storage_id: ObjectId,
    runtime_id: ObjectId,
    package_version: u64,
) -> Result<Option<PackageMetadataCreation>, ExecutionError> {
    let handler = PackageMetadataHandler::from_protocol_config(protocol_config);
    let mut modules_metadata = BTreeMap::new();
    for module in modules {
        let Some(md) = module
            .metadata
            .iter()
            .find(|md| md.key == IOTA_METADATA_KEY.to_vec())
        else {
            continue;
        };
        let runtime_module_metadata: RuntimeModuleMetadata = bcs::from_bytes::<
            RuntimeModuleMetadataWrapper,
        >(&md.value)
        .map_err(|_| {
            ExecutionError::from_kind(ExecutionErrorKind::VmVerificationOrDeserializationError)
        })?
        .try_into_runtime_module_metadata(&ProtocolBuildConfig::from(protocol_config))
        .map_err(|_| {
            ExecutionError::from_kind(ExecutionErrorKind::VmVerificationOrDeserializationError)
        })?;

        let mut pending = PendingModuleMetadata::default();
        match runtime_module_metadata {
            RuntimeModuleMetadata::V1(metadata) => {
                for (fn_name, fn_attributes) in metadata.fun_attributes.iter() {
                    for attribute in fn_attributes {
                        match attribute {
                            IotaAttributeV1::Authenticator(attribute) if attribute.version == 1 => {
                                pending.authenticator_metadata.insert(
                                    fn_name.to_string(),
                                    authenticator_first_param_type_tag(module, fn_name)?,
                                );
                            }
                            _ => {}
                        }
                    }
                }
            }
            RuntimeModuleMetadata::V2(metadata) => {
                for (fn_name, fn_attributes) in metadata.fun_attributes.iter() {
                    for attribute in fn_attributes {
                        match attribute {
                            IotaAttributeV2::Authenticator(attribute) if attribute.version == 1 => {
                                pending.authenticator_metadata.insert(
                                    fn_name.to_string(),
                                    authenticator_first_param_type_tag(module, fn_name)?,
                                );
                            }
                            IotaAttributeV2::View if handler.supports_view_function_metadata() => {
                                pending.view_function_metadata.push(fn_name.to_string());
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        if !pending.is_empty() {
            modules_metadata.insert(module.name().to_string(), pending);
        }
    }

    if !handler.should_publish_package(&modules_metadata) {
        return Ok(None);
    }
    Ok(Some(match handler {
        PackageMetadataHandler::V1 => {
            let modules_metadata_v1 = modules_metadata
                .into_iter()
                .map(|(module_name, pending)| (module_name, pending.authenticator_metadata))
                .collect();
            let id = derive_package_metadata_id(storage_id);
            PackageMetadataCreation::Inline {
                id,
                metadata: PackageMetadata::new_v1(
                    id,
                    storage_id,
                    runtime_id,
                    package_version,
                    modules_metadata_v1,
                ),
            }
        }
        PackageMetadataHandler::V1WithDynamicModuleMetadata => {
            PackageMetadataCreation::DynamicFields(constructor_args(
                &modules_metadata,
                storage_id,
                runtime_id,
                package_version,
            )?)
        }
    }))
}

/// Serializes the arguments of `create_package_metadata_v1_with_dynamic_metadata`: the package
/// IDs and version, followed by parallel per-module vectors (module names, authenticator
/// function names, authenticator account type names, view function names).
fn constructor_args(
    modules_metadata: &BTreeMap<String, PendingModuleMetadata>,
    storage_id: ObjectId,
    runtime_id: ObjectId,
    package_version: u64,
) -> Result<Vec<Vec<u8>>, ExecutionError> {
    let mut modules = Vec::with_capacity(modules_metadata.len());
    let mut auth_functions = Vec::with_capacity(modules_metadata.len());
    let mut type_names = Vec::with_capacity(modules_metadata.len());
    let mut view_function_names = Vec::with_capacity(modules_metadata.len());
    for (module_name, metadata) in modules_metadata {
        modules.push(module_name.clone());
        let (module_auth_functions, module_type_names): (Vec<_>, Vec<_>) = metadata
            .authenticator_metadata
            .iter()
            .map(|(function_name, account_type)| {
                (
                    function_name.clone(),
                    account_type.to_canonical_string(false),
                )
            })
            .unzip();
        auth_functions.push(module_auth_functions);
        type_names.push(module_type_names);
        view_function_names.push(metadata.view_function_metadata.clone());
    }
    [
        bcs::to_bytes(&ID::new(storage_id)),
        bcs::to_bytes(&ID::new(runtime_id)),
        bcs::to_bytes(&package_version),
        bcs::to_bytes(&modules),
        bcs::to_bytes(&auth_functions),
        bcs::to_bytes(&type_names),
        bcs::to_bytes(&view_function_names),
    ]
    .into_iter()
    .map(|bytes| {
        bytes.map_err(|e| {
            ExecutionError::new_with_source(ExecutionErrorKind::VmInvariantViolation, e)
        })
    })
    .collect()
}

fn authenticator_first_param_type_tag(
    module: &CompiledModule,
    authenticate_fn_name: &impl AsRef<str>,
) -> Result<TypeTag, ExecutionError> {
    // The IOTA verifier has already checked that the function exists and has a valid
    // authenticator signature.
    let Some((_, fn_definition)) = module.find_function_def_by_name(authenticate_fn_name) else {
        return Err(ExecutionError::from_kind(
            ExecutionErrorKind::VmInvariantViolation,
        ));
    };
    let fn_handle = module.function_handle_at(fn_definition.function);
    let fn_signature = module.signature_at(fn_handle.parameters);
    // The first parameter is a reference to the account type.
    let Some(SignatureToken::Reference(ref_param)) = fn_signature.0.first() else {
        return Err(ExecutionError::from_kind(
            ExecutionErrorKind::VmVerificationOrDeserializationError,
        ));
    };
    let pool = &mut normalized::RcPool::new();
    normalized::Type::new(pool, module, ref_param)
        .to_type_tag(pool)
        .map(|type_tag| type_tag_core_to_sdk(&type_tag))
        .ok_or_else(|| {
            ExecutionError::from_kind(ExecutionErrorKind::VmVerificationOrDeserializationError)
        })
}
