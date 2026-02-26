// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{cell::RefCell, rc::Rc};

use better_any::{Tid, TidAble};
use iota_types::{
    auth_context::{AuthContext, AuthContextCallArg, AuthContextCommand},
    digests::MoveAuthenticatorDigest,
};
use move_binary_format::errors::{PartialVMError, PartialVMResult};
use move_core_types::{runtime_value::MoveTypeLayout, vm_status::StatusCode};
use move_vm_runtime::native_extensions::NativeExtensionMarker;
use move_vm_types::values::{GlobalValue, Value};
use serde::{Serialize, de::DeserializeOwned};

// AuthenticationContext is a wrapper around AuthContext that is exposed to
// NativeContextExtensions in order to provide authentication context
// information to Move native functions. Holds a Rc<RefCell<AuthContext>> to
// allow for mutation of the AuthContext.
#[derive(Tid)]
pub struct AuthenticationContext {
    pub(crate) auth_context: Rc<RefCell<AuthContext>>,
    test_only: bool,
    cached_with_digest: Option<GlobalValue>,
    cached_with_tx_inputs: Option<GlobalValue>,
    cached_with_tx_commands: Option<GlobalValue>,
}

impl NativeExtensionMarker<'_> for AuthenticationContext {}

impl AuthenticationContext {
    pub fn new(auth_context: Rc<RefCell<AuthContext>>) -> Self {
        Self {
            auth_context,
            test_only: false,
            cached_with_digest: None,
            cached_with_tx_inputs: None,
            cached_with_tx_commands: None,
        }
    }

    pub fn new_for_testing(auth_context: Rc<RefCell<AuthContext>>) -> Self {
        Self {
            auth_context,
            test_only: true,
            cached_with_digest: None,
            cached_with_tx_inputs: None,
            cached_with_tx_commands: None,
        }
    }

    /// Returns a `GlobalValue` containing a struct with the auth digest field.
    /// Caches the result to avoid redundant conversions and allocations on
    /// subsequent calls.
    ///
    /// The returned `GlobalValue` is expected to be used as a reference in
    /// native functions, so it should not be mutated or stored beyond the
    /// scope of a single native function call.
    pub fn struct_with_digest(&mut self) -> PartialVMResult<&GlobalValue> {
        if self.cached_with_digest.is_none() {
            let auth_context_ref = self.auth_context.borrow();
            // Wrap in a tuple to match the expected Move layout of
            // `struct AuthContext {
            //     digest: vector<u8>
            // }`
            let struct_value_rust = (auth_context_ref.digest(),);
            let digest_move_layout = MoveTypeLayout::Vector(Box::new(MoveTypeLayout::U8));
            let value = to_value(
                &struct_value_rust,
                &AuthContext::layout_with_custom_field(digest_move_layout),
            )?;
            self.cached_with_digest =
                Some(GlobalValue::cached(value).expect("Failed to cache global value"));
        }

        Ok(self.cached_with_digest.as_ref().unwrap())
    }

    /// Returns a `GlobalValue` containing a struct with the auth tx inputs.
    /// Caches the result to avoid redundant conversions and allocations on
    /// subsequent calls.
    ///
    /// The returned `GlobalValue` is expected to be used as a reference in
    /// native functions, so it should not be mutated or stored beyond the
    /// scope of a single native function call.
    pub fn struct_with_tx_inputs(
        &mut self,
        input_move_layout: MoveTypeLayout,
    ) -> PartialVMResult<&GlobalValue> {
        // For fields V1 the tx inputs are a vector<CallArg>, so check that
        // input_move_layout is an Enum, i.e. a CallArg enum
        if !matches!(input_move_layout, MoveTypeLayout::Enum(_)) {
            return Err(
                PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR).with_message(
                    format!("Unexpected `MoveTypeLayout` for tx inputs: {input_move_layout:?}"),
                ),
            );
        }

        if self.cached_with_tx_inputs.is_none() {
            let auth_context_ref = self.auth_context.borrow();
            // Wrap in a tuple to match the expected Move layout of
            // `struct AuthContext {
            //     tx_inputs: vector<CallArg>
            // }`
            let struct_value_rust = (auth_context_ref.tx_inputs(),);
            let vector_input_move_layout = MoveTypeLayout::Vector(Box::new(input_move_layout));
            let value = to_value(
                &struct_value_rust,
                &AuthContext::layout_with_custom_field(vector_input_move_layout),
            )?;
            self.cached_with_tx_inputs =
                Some(GlobalValue::cached(value).expect("Failed to cache valid global value"));
        }

        Ok(self.cached_with_tx_inputs.as_ref().unwrap())
    }

    /// Returns a `GlobalValue` containing a struct with the auth tx commands.
    /// Caches the result to avoid redundant conversions and allocations on
    /// subsequent calls.
    ///
    /// The returned `GlobalValue` is expected to be used as a reference in
    /// native functions, so it should not be mutated or stored beyond the
    /// scope of a single native function call.
    pub fn struct_with_tx_commands(
        &mut self,
        command_move_layout: MoveTypeLayout,
    ) -> PartialVMResult<&GlobalValue> {
        // For fields V1 the tx commands are a vector<Command>, so check that
        // command_move_layout is an Enum, i.e. a Command enum
        if !matches!(command_move_layout, MoveTypeLayout::Enum(_)) {
            return Err(
                PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR).with_message(
                    format!("Unexpected `MoveTypeLayout` for tx commands: {command_move_layout:?}"),
                ),
            );
        }

        if self.cached_with_tx_commands.is_none() {
            let auth_context_ref = self.auth_context.borrow();
            // Wrap in a tuple to match the expected Move layout of
            //`struct AuthContext {
            //     tx_commands: vector<Command>
            // }`
            let struct_value_rust = (auth_context_ref.tx_commands(),);
            let vector_command_move_layout = MoveTypeLayout::Vector(Box::new(command_move_layout));
            let value = to_value(
                &struct_value_rust,
                &AuthContext::layout_with_custom_field(vector_command_move_layout),
            )?;
            self.cached_with_tx_commands =
                Some(GlobalValue::cached(value).expect("Failed to cache global value"));
        }

        Ok(self.cached_with_tx_commands.as_ref().unwrap())
    }

    /// Replaces the contents of the `AuthContext` with the provided values.
    /// Only callable in testing scenarios.
    /// Expects the input values to be values, then it tries to convert them
    /// back to their original rust types and updates the `AuthContext` with
    /// the new values.
    pub fn replace(
        &self,
        auth_digest_value: Vec<u8>,
        tx_inputs_value: Vec<Value>,
        input_move_layout: MoveTypeLayout,
        tx_commands_value: Vec<Value>,
        command_move_layout: MoveTypeLayout,
    ) -> PartialVMResult<()> {
        if !self.test_only {
            return Err(
                PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR)
                    .with_message("`replace` called on a non testing scenario".to_string()),
            );
        }

        // For fields V1 the tx inputs are a vector<CallArg>, so check that
        // input_move_layout is an Enum, i.e. a CallArg enum
        if !matches!(input_move_layout, MoveTypeLayout::Enum(_)) {
            return Err(
                PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR).with_message(
                    format!("Unexpected `MoveTypeLayout` for tx inputs: {input_move_layout:?}"),
                ),
            );
        }

        // For fields V1 the tx commands are a vector<Command>, so check that
        // command_move_layout is an Enum, i.e. a Command enum
        if !matches!(command_move_layout, MoveTypeLayout::Enum(_)) {
            return Err(
                PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR).with_message(
                    format!("Unexpected `MoveTypeLayout` for tx commands: {command_move_layout:?}"),
                ),
            );
        }

        let tx_commands = tx_commands_value
            .into_iter()
            .map(|value| from_value::<AuthContextCommand>(value, &command_move_layout))
            .collect::<PartialVMResult<Vec<_>>>()?;

        let tx_inputs = tx_inputs_value
            .into_iter()
            .map(|value| from_value::<AuthContextCallArg>(value, &input_move_layout))
            .collect::<PartialVMResult<Vec<_>>>()?;

        let auth_digest =
            MoveAuthenticatorDigest::try_from(auth_digest_value.as_slice()).map_err(|err| {
                PartialVMError::new(StatusCode::UNEXPECTED_DESERIALIZATION_ERROR)
                    .with_message(err.to_string())
            })?;

        self.auth_context
            .borrow_mut()
            .replace(auth_digest, tx_inputs, tx_commands);

        Ok(())
    }
}

fn to_value<T: ?Sized + Serialize>(
    input: &T,
    input_move_layout: &MoveTypeLayout,
) -> PartialVMResult<Value> {
    let bytes = bcs::to_bytes(input).map_err(|err| {
        PartialVMError::new(StatusCode::VALUE_SERIALIZATION_ERROR)
            .with_message(format!("Failed to serialize an input: {err}"))
    })?;
    Value::simple_deserialize(&bytes, input_move_layout).ok_or_else(|| {
        PartialVMError::new(StatusCode::UNEXPECTED_DESERIALIZATION_ERROR)
            .with_message("Failed to deserialize an input to a Move value".to_string())
    })
}

fn from_value<T: DeserializeOwned>(
    value: Value,
    value_move_layout: &MoveTypeLayout,
) -> PartialVMResult<T> {
    let bytes = value.simple_serialize(value_move_layout).ok_or_else(|| {
        PartialVMError::new(StatusCode::VALUE_SERIALIZATION_ERROR)
            .with_message("Failed to serialize a value".to_string())
    })?;
    bcs::from_bytes::<T>(&bytes).map_err(|err| {
        PartialVMError::new(StatusCode::UNEXPECTED_DESERIALIZATION_ERROR)
            .with_message(format!("Failed to deserialize a value: {err}"))
    })
}
