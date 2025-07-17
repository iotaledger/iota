// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_types::{error::ExecutionError, move_package::FnInfoMap};
use move_binary_format::{CompiledModule, file_format::SignatureToken};
use move_bytecode_utils::format_signature_token;

use crate::verification_failure;

/// Checks validity rules for functions marked as verify
pub fn verify_module(
    module: &CompiledModule,
    _fn_info_map: &FnInfoMap,
) -> Result<(), ExecutionError> {
    // Marking a test function as view makes little sense, but for the prototype
    // we ignore this problem.

    for func_def in module
        .function_defs
        .iter()
        .filter(|func_def| func_def.is_view)
    {
        let handle = module.function_handle_at(func_def.function);

        // has return type
        let return_signature = module.signature_at(handle.return_);
        if return_signature.is_empty() {
            return Err(verification_failure(
                "View function must provide a return value.".into(),
            ));
        }
        // has only non mutable parameters
        let params = module.signature_at(handle.parameters);
        for token in &params.0 {
            if matches!(token, SignatureToken::MutableReference(_)) {
                return Err(verification_failure(format!(
                    "View function can't have mutable arguments. Mutable argument: {}",
                    format_signature_token(module, token)
                )));
            }
        }
    }
    Ok(())
}
