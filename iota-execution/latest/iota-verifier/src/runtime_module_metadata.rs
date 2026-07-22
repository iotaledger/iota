// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This pass verifies necessary properties for runtime module metadata, i.e.
//! the module metadata used by the IOTA at runtime.
//! A compiled module may contain at most one metadata item, which must be
//! indexed by the IOTA metadata key. If present, the metadata item must be
//! deserializable and must satisfy any additional checks imposed by the runtime
//! metadata version.

use std::collections::BTreeSet;

use iota_types::{
    error::ExecutionError,
    move_package::{
        IotaAttributeV1, IotaAttributeV2, ProtocolBuildConfig, RuntimeModuleMetadata,
        RuntimeModuleMetadataWrapper,
    },
};
use move_binary_format::{file_format::CompiledModule, file_format_common::IOTA_METADATA_KEY};
use move_core_types::identifier::Identifier;

use crate::{
    authenticator_verifier::verify_authenticate_func_v1, verification_failure,
    view_function_verifier::verify_view_func,
};

/// Verifies the runtime module metadata of the given module.
/// If the module does not contain any runtime metadata, just pass.
/// If the module contains runtime metadata, it must satisfy the following:
/// 1. The module metadata must contain at most one metadata item, which is
///    indexed by the IOTA metadata key.
/// 2. The metadata item must be deserializable into
///    `RuntimeModuleMetadataWrapper`.
/// 3. The deserialized metadata must satisfy any additional checks imposed by
///    the runtime metadata version.
///
/// `protocol_build_config.allow_view_function` reflects the
/// `package_metadata_with_dynamic_module_metadata` protocol feature. While it
/// is disabled, a package carrying the `View` attribute is rejected: the
/// variant did not exist in older binaries, so accepting it would let a package
/// onto the chain that a not-yet-upgraded validator cannot even deserialize.
pub fn verify_module(
    module: &CompiledModule,
    protocol_build_config: &ProtocolBuildConfig,
) -> Result<(), ExecutionError> {
    if !module.metadata.is_empty() {
        if module.metadata.len() > 1 {
            return Err(verification_failure(
                "Module metadata must contain at most one metadata item, that is the IOTA metadata"
                    .to_string(),
            ));
        }
        let iota_metadata = &module.metadata[0];
        if iota_metadata.key != IOTA_METADATA_KEY {
            return Err(verification_failure(
                "Module metadata must contain at most one metadata item, indexed by the IOTA metadata key"
                    .to_string(),
            ));
        }
        let metadata = bcs::from_bytes::<RuntimeModuleMetadataWrapper>(&iota_metadata.value)
            .map_err(|err| {
                verification_failure(format!(
                    "Failed to read bcs bytes for IOTA module metadata: {err}",
                ))
            })?
            .try_into_runtime_module_metadata(protocol_build_config)
            .map_err(|err| {
                verification_failure(format!(
                    "Failed to deserialize runtime IOTA module metadata from wrapper: {err}",
                ))
            })?;
        verify_runtime_metadata(module, &metadata, protocol_build_config)?;
    }

    Ok(())
}

fn verify_runtime_metadata(
    module: &CompiledModule,
    metadata: &RuntimeModuleMetadata,
    protocol_build_config: &ProtocolBuildConfig,
) -> Result<(), ExecutionError> {
    match metadata {
        RuntimeModuleMetadata::V1(v1) => {
            for (fn_name, fn_attributes) in v1.fun_attributes_iter() {
                let mut seen = BTreeSet::new();
                // Verify each function attribute
                for attribute in fn_attributes {
                    if !seen.insert(attribute) {
                        return Err(verification_failure(format!(
                            "Duplicate attribute {attribute:?} found for function {fn_name}"
                        )));
                    }
                    match attribute {
                        IotaAttributeV1::Authenticator(attr) => {
                            // Verify authenticator attribute
                            match attr.version {
                                1 => {
                                    // Version 1: verify that the function is a valid authenticator
                                    verify_authenticate_func_v1(
                                        module,
                                        &Identifier::new(fn_name.clone()).map_err(|err| {
                                            verification_failure(format!(
                                                "Failed to read function name: {err}",
                                            ))
                                        })?,
                                    )?;
                                }
                                _ => {
                                    return Err(verification_failure(format!(
                                        "Unsupported authenticator attribute version {} for function {}",
                                        attr.version, fn_name
                                    )));
                                }
                            }
                        }
                    }
                }
            }
        }
        RuntimeModuleMetadata::V2(v2) => {
            for (fn_name, fn_attributes) in v2.fun_attributes_iter() {
                let mut seen = BTreeSet::new();
                // Verify each function attribute
                for attribute in fn_attributes {
                    if !seen.insert(attribute) {
                        return Err(verification_failure(format!(
                            "Duplicate attribute {attribute:?} found for function {fn_name}"
                        )));
                    }
                    match attribute {
                        IotaAttributeV2::Authenticator(attr) => {
                            // Verify authenticator attribute
                            match attr.version {
                                1 => {
                                    // Version 1: verify that the function is a valid authenticator
                                    verify_authenticate_func_v1(
                                        module,
                                        &Identifier::new(fn_name.clone()).map_err(|err| {
                                            verification_failure(format!(
                                                "Failed to read function name: {err}",
                                            ))
                                        })?,
                                    )?;
                                }
                                _ => {
                                    return Err(verification_failure(format!(
                                        "Unsupported authenticator attribute version {} for function {}",
                                        attr.version, fn_name
                                    )));
                                }
                            }
                        }
                        IotaAttributeV2::View => {
                            if !protocol_build_config.allow_view_function {
                                return Err(verification_failure(format!(
                                    "View attribute for function {fn_name} is not supported by the current protocol version"
                                )));
                            }
                            verify_view_func(
                                module,
                                &Identifier::new(fn_name.clone()).map_err(|err| {
                                    verification_failure(format!(
                                        "Failed to read function name: {err}",
                                    ))
                                })?,
                            )?;
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use iota_types::move_package::IotaAttribute;
    use move_binary_format::file_format::{
        FunctionDefinition, FunctionHandle, IdentifierIndex, ModuleHandleIndex, Signature,
        SignatureIndex, Visibility, empty_module,
    };
    use move_core_types::metadata::Metadata;

    use super::*;

    fn module_with_view_metadata(visibility: Visibility, returns: Signature) -> CompiledModule {
        let mut module = empty_module();
        module
            .identifiers
            .push(Identifier::new("view".to_owned()).unwrap());
        let name = IdentifierIndex((module.identifiers.len() - 1) as u16);

        module.signatures.push(returns);
        let return_ = SignatureIndex((module.signatures.len() - 1) as u16);
        module.function_handles.push(FunctionHandle {
            module: ModuleHandleIndex(0),
            name,
            parameters: SignatureIndex(0),
            return_,
            type_parameters: vec![],
        });
        module.function_defs.push(FunctionDefinition {
            visibility,
            ..Default::default()
        });

        let mut metadata = RuntimeModuleMetadata::v2();
        metadata.add_function_attribute(
            "view".to_owned(),
            IotaAttribute::V2(IotaAttributeV2::view_attribute()),
        );
        module.metadata.push(Metadata {
            key: IOTA_METADATA_KEY.to_vec(),
            value: RuntimeModuleMetadataWrapper::from(metadata).to_bcs_bytes(),
        });

        module
    }

    fn assert_error_contains(
        module: &CompiledModule,
        expected: &str,
        protocol_build_config: &ProtocolBuildConfig,
    ) {
        let err = verify_module(module, protocol_build_config).unwrap_err();
        let source = err.source().as_ref().unwrap().to_string();
        assert!(
            source.contains(expected),
            "expected error to contain {expected:?}, got {source:?}"
        );
    }

    #[test]
    fn verifies_view_attribute_from_runtime_metadata() {
        let module = module_with_view_metadata(
            Visibility::Public,
            Signature(vec![move_binary_format::file_format::SignatureToken::Bool]),
        );

        verify_module(
            &module,
            &ProtocolBuildConfig {
                allow_view_function: true,
            },
        )
        .unwrap();
    }

    #[test]
    fn rejects_invalid_view_attribute_from_runtime_metadata() {
        let module = module_with_view_metadata(
            Visibility::Private,
            Signature(vec![move_binary_format::file_format::SignatureToken::Bool]),
        );

        assert_error_contains(
            &module,
            "View function 'view' must be public",
            &ProtocolBuildConfig {
                allow_view_function: true,
            },
        );
    }

    #[test]
    fn rejects_view_attribute_when_metadata_disabled() {
        // While `package_metadata_with_dynamic_module_metadata` is disabled, a
        // package carrying the `View` attribute must be rejected so a new binary
        // agrees with an old one that cannot deserialize the variant at all. The
        // `View` attribute only lives in V2 (dynamic) metadata, so the rejection
        // happens when the wrapper is decoded: V2 metadata is refused outright.
        let module = module_with_view_metadata(
            Visibility::Public,
            Signature(vec![move_binary_format::file_format::SignatureToken::Bool]),
        );

        let err = verify_module(
            &module,
            &ProtocolBuildConfig {
                allow_view_function: false,
            },
        )
        .unwrap_err();
        let source = err.source().as_ref().unwrap().to_string();
        assert!(
            source.contains("Unsupported runtime module metadata version: 2"),
            "expected error about the unsupported V2 metadata, got {source:?}"
        );
    }
}
