// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{cell::RefCell, rc::Rc};

use better_any::{Tid, TidAble};
use iota_types::{
    auth_context::{AuthContext, MoveCallArg, MoveCommand},
    digests::MoveAuthenticatorDigest,
};
use move_binary_format::errors::{PartialVMError, PartialVMResult};
use move_core_types::{
    gas_algebra::AbstractMemorySize, runtime_value::MoveTypeLayout, vm_status::StatusCode,
};
use move_vm_runtime::native_extensions::NativeExtensionMarker;
use move_vm_types::values::{GlobalValue, StructRef, Value};

use crate::utils;

// AuthenticationContext is a wrapper around AuthContext that is exposed to
// NativeContextExtensions in order to provide authentication context
// information to Move native functions. Holds a Rc<RefCell<AuthContext>> to
// allow for mutation of the AuthContext.
#[derive(Tid)]
pub struct AuthenticationContext {
    /// The wrapped `AuthContext` containing the authentication context
    /// information.
    pub(crate) auth_context: Rc<RefCell<AuthContext>>,

    /// Indicates whether this `AuthenticationContext` is being used in a
    /// testing scenario.
    test_only: bool,

    /// Cached `GlobalValue` containing AuthContext data. Caching is used to
    /// avoid redundant conversions and allocations.
    cached_digest: Option<GlobalValue>,
    cached_tx_inputs: Option<(GlobalValue, AbstractMemorySize)>,
    cached_tx_commands: Option<(GlobalValue, AbstractMemorySize)>,
}

impl NativeExtensionMarker<'_> for AuthenticationContext {}

impl AuthenticationContext {
    pub fn new(auth_context: Rc<RefCell<AuthContext>>) -> Self {
        Self {
            auth_context,
            test_only: false,
            cached_digest: None,
            cached_tx_inputs: None,
            cached_tx_commands: None,
        }
    }

    pub fn new_for_testing(auth_context: Rc<RefCell<AuthContext>>) -> Self {
        Self {
            auth_context,
            test_only: true,
            cached_digest: None,
            cached_tx_inputs: None,
            cached_tx_commands: None,
        }
    }

    /// Returns a `Value` containing an auth digest ref.
    /// Caches the result to avoid redundant conversions and allocations on
    /// subsequent calls.
    pub fn digest_ref(&mut self) -> PartialVMResult<Value> {
        if self.cached_digest.is_none() {
            let auth_context = self.auth_context.borrow();

            // Wrap in a tuple to match the expected Move layout of
            // `struct AuthContext {
            //     digest: vector<u8>
            // }`
            let rust_value = (auth_context.digest(),);
            let digest_move_layout = MoveTypeLayout::Vector(Box::new(MoveTypeLayout::U8));

            self.cached_digest = Some(utils::to_global_value(&rust_value, digest_move_layout)?.0);
        }

        self.cached_digest
            .as_ref()
            .unwrap()
            .borrow_global()
            .inspect_err(|err| assert!(err.major_status() != StatusCode::MISSING_DATA))?
            .value_as::<StructRef>()?
            .borrow_field(0)
    }

    /// Returns a `Value` containing an auth tx inputs ref.
    /// Caches the result to avoid redundant conversions and allocations on
    /// subsequent calls.
    pub fn tx_inputs_ref(
        &mut self,
        input_move_layout: MoveTypeLayout,
    ) -> PartialVMResult<(Value, AbstractMemorySize)> {
        if self.cached_tx_inputs.is_none() {
            let auth_context = self.auth_context.borrow();

            // Wrap in a tuple to match the expected Move layout of
            // `struct AuthContext {
            //     tx_inputs: vector<CallArg>
            // }`
            let rust_value = (auth_context.tx_inputs(),);
            let inputs_move_layout = MoveTypeLayout::Vector(Box::new(input_move_layout));

            self.cached_tx_inputs = Some(utils::to_global_value(&rust_value, inputs_move_layout)?);
        }

        let (cached_tx_inputs, move_value_size) = self.cached_tx_inputs.as_ref().unwrap();

        Ok((
            cached_tx_inputs
                .borrow_global()
                .inspect_err(|err| assert!(err.major_status() != StatusCode::MISSING_DATA))?
                .value_as::<StructRef>()?
                .borrow_field(0)?,
            *move_value_size,
        ))
    }

    /// Returns a `Value` containing an auth tx commands ref.
    /// Caches the result to avoid redundant conversions and allocations on
    /// subsequent calls.
    pub fn tx_commands_ref(
        &mut self,
        command_move_layout: MoveTypeLayout,
    ) -> PartialVMResult<(Value, AbstractMemorySize)> {
        if self.cached_tx_commands.is_none() {
            let auth_context = self.auth_context.borrow();

            // Wrap in a tuple to match the expected Move layout of
            //`struct AuthContext {
            //     tx_commands: vector<Command>
            // }`
            let rust_value = (auth_context.tx_commands(),);
            let commands_move_layout = MoveTypeLayout::Vector(Box::new(command_move_layout));

            self.cached_tx_commands =
                Some(utils::to_global_value(&rust_value, commands_move_layout)?);
        }

        let (cached_tx_commands, move_value_size) = self.cached_tx_commands.as_ref().unwrap();

        Ok((
            cached_tx_commands
                .borrow_global()
                .inspect_err(|err| assert!(err.major_status() != StatusCode::MISSING_DATA))?
                .value_as::<StructRef>()?
                .borrow_field(0)?,
            *move_value_size,
        ))
    }

    /// Replaces the contents of the `AuthContext` with the provided values.
    /// Only callable in testing scenarios.
    /// Expects the input values to be values, then it tries to convert them
    /// back to their original rust types and updates the `AuthContext` with
    /// the new values.
    pub fn replace(
        &mut self,
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

        let tx_commands = tx_commands_value
            .into_iter()
            .map(|value| utils::from_value(value, &command_move_layout))
            .collect::<PartialVMResult<Vec<MoveCommand>>>()?;

        let tx_inputs = tx_inputs_value
            .into_iter()
            .map(|value| utils::from_value(value, &input_move_layout))
            .collect::<PartialVMResult<Vec<MoveCallArg>>>()?;

        let auth_digest =
            MoveAuthenticatorDigest::try_from(auth_digest_value.as_slice()).map_err(|err| {
                PartialVMError::new(StatusCode::UNEXPECTED_DESERIALIZATION_ERROR)
                    .with_message(err.to_string())
            })?;

        self.auth_context
            .borrow_mut()
            .replace(auth_digest, tx_inputs, tx_commands);

        // Drop cached values to ensure they are recreated with the updated AuthContext
        // data
        self.cached_digest = None;
        self.cached_tx_inputs = None;
        self.cached_tx_commands = None;

        Ok(())
    }
}
