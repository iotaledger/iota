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
    Identifier,
    error::ExecutionError,
    move_package::{IotaAttribute, RuntimeModuleMetadata, RuntimeModuleMetadataWrapper},
};
use move_binary_format::{file_format::CompiledModule, file_format_common::IOTA_METADATA_KEY};

use crate::{authenticator_verifier::verify_authenticate_func_v1, verification_failure};

/// Verifies the runtime module metadata of the given module.
/// If the module does not contain any runtime metadata, just pass.
/// If the module contains runtime metadata, it must satisfy the following:
/// 1. The module metadata must contain at most one metadata item, which is
///    indexed by the IOTA metadata key.
/// 2. The metadata item must be deserializable into
///    `RuntimeModuleMetadataWrapper`.
/// 3. The deserialized metadata must satisfy any additional checks imposed by
///    the runtime metadata version.
pub fn verify_module(module: &CompiledModule) -> Result<(), ExecutionError> {
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
            .try_into()
            .map_err(|err| {
                verification_failure(format!(
                    "Failed to deserialize runtime IOTA module metadata from wrapper: {err}",
                ))
            })?;
        verify_runtime_metadata(module, &metadata)?;
    }

    Ok(())
}

fn verify_runtime_metadata(
    module: &CompiledModule,
    metadata: &RuntimeModuleMetadata,
) -> Result<(), ExecutionError> {
    for (fn_name, fn_attributes) in metadata.fun_attributes_iter() {
        let mut seen = BTreeSet::new();
        // Verify each function attribute
        for attribute in fn_attributes {
            if !seen.insert(attribute) {
                return Err(verification_failure(format!(
                    "Duplicate attribute {attribute:?} found for function {fn_name}"
                )));
            }
            match attribute {
                IotaAttribute::Authenticator(attr) => {
                    // Verify authenticator attribute
                    match attr.version {
                        1 => {
                            // Version 1: verify that the function is a valid authenticator
                            verify_authenticate_func_v1(
                                module,
                                Identifier::new(fn_name.clone()).map_err(|err| {
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
    Ok(())
}
