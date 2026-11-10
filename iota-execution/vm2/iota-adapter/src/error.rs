// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_types::{Identifier, MoveLocation};
use iota_types::{error::ExecutionError, iota_sdk_types_conversions::identifier_core_to_sdk};
use move_binary_format::{
    errors::{Location, VMError},
    file_format::FunctionDefinitionIndex,
};
use move_core_types::{
    language_storage::ModuleId,
    vm_status::{StatusCode, StatusType},
};

use crate::{compat::execution_status::ExecutionErrorKind, sdk_conv::AccountAddressExt};

fn move_location(
    id: &ModuleId,
    function: u16,
    instruction: u16,
    function_name: Option<String>,
) -> MoveLocation {
    MoveLocation {
        package: id.address().to_object_id(),
        module: identifier_core_to_sdk(id.name()),
        function,
        instruction,
        function_name: function_name.and_then(|name| Identifier::new(name).ok()),
    }
}

pub(crate) fn convert_vm_error_impl(
    error: VMError,
    abort_module_id_relocation_fn: &impl Fn(&ModuleId) -> ModuleId,
    function_name_resolution_fn: &impl Fn(&ModuleId, FunctionDefinitionIndex) -> Option<String>,
) -> ExecutionError {
    let kind = match (error.major_status(), error.sub_status(), error.location()) {
        (StatusCode::EXECUTED, _, _) => {
            // If we have an error the status probably shouldn't ever be Executed
            debug_assert!(false, "VmError shouldn't ever report successful execution");
            ExecutionErrorKind::VmInvariantViolation
        }
        (StatusCode::ABORTED, None, _) => {
            debug_assert!(false, "No abort code");
            // this is a Move VM invariant violation, the code should always be there
            ExecutionErrorKind::VmInvariantViolation
        }
        (StatusCode::ABORTED, Some(code), Location::Module(id)) => {
            let abort_location_id = abort_module_id_relocation_fn(id);
            let offset = error.offsets().first().copied().map(|(f, i)| (f.0, i));
            debug_assert!(offset.is_some(), "Move should set the location on aborts");
            let (function, instruction) = offset.unwrap_or((0, 0));
            let function_name = function_name_resolution_fn(id, FunctionDefinitionIndex(function));
            ExecutionErrorKind::MoveAbort {
                location: move_location(&abort_location_id, function, instruction, function_name),
                code,
            }
        }
        (StatusCode::OUT_OF_GAS, _, _) => ExecutionErrorKind::InsufficientGas,
        (_, _, location) => match error.major_status().status_type() {
            StatusType::Execution => {
                debug_assert!(error.major_status() != StatusCode::ABORTED);
                let location = match location {
                    Location::Module(id) => {
                        let offset = error.offsets().first().copied().map(|(f, i)| (f.0, i));
                        debug_assert!(
                            offset.is_some(),
                            "Move should set the location on all execution errors. Error {error}"
                        );
                        let (function, instruction) = offset.unwrap_or((0, 0));
                        let function_name =
                            function_name_resolution_fn(id, FunctionDefinitionIndex(function));
                        Some(move_location(id, function, instruction, function_name))
                    }
                    _ => None,
                };
                ExecutionErrorKind::MovePrimitiveRuntimeError { location }
            }
            StatusType::Validation
            | StatusType::Verification
            | StatusType::Deserialization
            | StatusType::Unknown => ExecutionErrorKind::VmVerificationOrDeserializationError,
            StatusType::InvariantViolation => ExecutionErrorKind::VmInvariantViolation,
        },
    };
    ExecutionError::new_with_source(kind, error)
}
