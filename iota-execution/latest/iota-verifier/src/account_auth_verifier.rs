// Copyright (c) IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_types::{Identifier, error::ExecutionError};
use move_binary_format::CompiledModule;

use crate::verification_failure;

// AA Account authenticate() function verifier
// First we only handle the case where the object we want to
// turn into an AA account exists in the same module as the
// authenticate() function.

// Handling cases where it exists within a different module may require some
// refactoring and access to ExecutionContext and some more because we would
// need to load the referenced Module's CompiledModule representation based on
// linkage data. Likely we would have to touch the linker as well, modify some
// basic concepts. If we allow the usage of authenticate from other packages
// through the native function we also have to somehow ensure that the referred
// to package has been linked to the one trying to set a reference to the
// authenticate function.

// This function should check that within a module if an authenticate() function
// is defined it is appropriately done so, or its usage is satisfactory.
// pub fn verify_module(
//     _module: &CompiledModule,
//     _fn_info_map: &FnInfoMap,
// ) -> Result<(), ExecutionError> {
//     Ok(())
// }

pub fn verify_authenticate_func(
    module: &CompiledModule,
    function_identifier: Identifier,
) -> Result<(), ExecutionError> {
    let module_name = module.name();
    let auth_func_handle = match module.function_handles.iter().find(|handle| {
        let fun_id = module.identifier_at(handle.name);
        fun_id.as_str() == function_identifier.as_str()
    }) {
        Some(handle) => handle,
        None => {
            return Err(verification_failure(format!(
                "Auth function {function_identifier} not found in {module_name}"
            )));
        }
    };

    println!("Verifying ... {:?}", auth_func_handle);

    Ok(())
}
