// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{cell::RefCell, rc::Rc};

use better_any::{Tid, TidAble};
use iota_types::{
    auth_context::{
        AuthContext, EnrichedCallArg, EnrichedCommand, EnrichedProgrammableMoveCall,
        ImmOrOwnedObjectArg, MoveCallArg, MoveCommand, MoveObjectArg, MoveProgrammableMoveCall,
        SharedObjectArg,
    },
    base_types::ObjectDigest,
    digests::MoveAuthenticatorDigest,
    type_input::TypeName,
};

// ---------------------------------------------------------------------------
// Downgrade helpers: Enriched → plain (used by tx_inputs_ref / tx_commands_ref)
// ---------------------------------------------------------------------------

/// Converts a stored [`EnrichedCallArg`] back to a [`MoveCallArg`] suitable
/// for BCS-serialization into the plain Move `CallArg` layout.
fn move_call_arg_from_enriched(arg: &EnrichedCallArg) -> MoveCallArg {
    match arg {
        EnrichedCallArg::Pure { value, .. } => MoveCallArg::Pure(value.clone()),
        EnrichedCallArg::ImmOrOwnedObject(obj) => MoveCallArg::Object(
            MoveObjectArg::ImmOrOwnedObject((obj.id, obj.version, obj.digest)),
        ),
        EnrichedCallArg::SharedObject(obj) => MoveCallArg::Object(MoveObjectArg::SharedObject {
            id: obj.id,
            initial_shared_version: obj.initial_shared_version,
            mutable: obj.mutable,
        }),
        EnrichedCallArg::Receiving(obj) => {
            MoveCallArg::Object(MoveObjectArg::Receiving((obj.id, obj.version, obj.digest)))
        }
    }
}

/// Converts a stored [`EnrichedCommand`] back to a [`MoveCommand`] suitable
/// for BCS-serialization into the plain Move `Command` layout.
fn move_command_from_enriched(cmd: &EnrichedCommand) -> MoveCommand {
    match cmd {
        EnrichedCommand::MoveCall(call) => {
            MoveCommand::MoveCall(Box::new(MoveProgrammableMoveCall {
                package: call.package,
                module: call.module.clone(),
                function: call.function.clone(),
                type_arguments: call.type_arguments.clone(),
                arguments: call.arguments.clone(),
            }))
        }
        EnrichedCommand::TransferObjects(objects, recipient) => {
            MoveCommand::TransferObjects(objects.clone(), *recipient)
        }
        EnrichedCommand::SplitCoins(coin, amounts) => {
            MoveCommand::SplitCoins(*coin, amounts.clone())
        }
        EnrichedCommand::MergeCoins(target, sources) => {
            MoveCommand::MergeCoins(*target, sources.clone())
        }
        EnrichedCommand::Publish(modules, deps) => {
            MoveCommand::Publish(modules.clone(), deps.clone())
        }
        EnrichedCommand::MakeMoveVec(type_arg, elements) => {
            MoveCommand::MakeMoveVec(type_arg.clone(), elements.clone())
        }
        EnrichedCommand::Upgrade(modules, deps, package, ticket) => {
            MoveCommand::Upgrade(modules.clone(), deps.clone(), *package, *ticket)
        }
    }
}
use move_binary_format::errors::{PartialVMError, PartialVMResult};
use move_core_types::{
    gas_algebra::AbstractMemorySize,
    runtime_value::{MoveStructLayout, MoveTypeLayout},
    vm_status::StatusCode,
};
use move_vm_runtime::native_extensions::NativeExtensionMarker;
use move_vm_types::values::{GlobalValue, StructRef, Value};
use serde::{Serialize, de::DeserializeOwned};

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
    cached_enriched_tx_inputs: Option<(GlobalValue, AbstractMemorySize)>,
    cached_enriched_tx_commands: Option<(GlobalValue, AbstractMemorySize)>,
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
            cached_enriched_tx_inputs: None,
            cached_enriched_tx_commands: None,
        }
    }

    pub fn new_for_testing(auth_context: Rc<RefCell<AuthContext>>) -> Self {
        Self {
            auth_context,
            test_only: true,
            cached_digest: None,
            cached_tx_inputs: None,
            cached_tx_commands: None,
            cached_enriched_tx_inputs: None,
            cached_enriched_tx_commands: None,
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

            self.cached_digest = Some(to_global_value(&rust_value, digest_move_layout)?.0);
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
    ///
    /// The stored enriched data is downgraded to plain `MoveCallArg` on demand,
    /// dropping type-name and mutability metadata that `CallArg` does not
    /// carry. Result is cached to avoid redundant conversions on subsequent
    /// calls.
    pub fn tx_inputs_ref(
        &mut self,
        input_move_layout: MoveTypeLayout,
    ) -> PartialVMResult<(Value, AbstractMemorySize)> {
        if self.cached_tx_inputs.is_none() {
            let auth_context = self.auth_context.borrow();
            // Downgrade: EnrichedCallArg → MoveCallArg
            let plain_inputs: Vec<MoveCallArg> = auth_context
                .tx_inputs()
                .iter()
                .map(move_call_arg_from_enriched)
                .collect();
            let rust_value = (plain_inputs,);
            let inputs_move_layout = MoveTypeLayout::Vector(Box::new(input_move_layout));
            self.cached_tx_inputs = Some(to_global_value(&rust_value, inputs_move_layout)?);
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
    ///
    /// The stored enriched data is downgraded to plain `MoveCommand` on demand,
    /// dropping `is_entry` and `returns` metadata.
    /// Result is cached to avoid redundant conversions on subsequent calls.
    pub fn tx_commands_ref(
        &mut self,
        command_move_layout: MoveTypeLayout,
    ) -> PartialVMResult<(Value, AbstractMemorySize)> {
        if self.cached_tx_commands.is_none() {
            let auth_context = self.auth_context.borrow();
            // Downgrade: EnrichedCommand → MoveCommand
            let plain_commands: Vec<MoveCommand> = auth_context
                .tx_commands()
                .iter()
                .map(move_command_from_enriched)
                .collect();
            let rust_value = (plain_commands,);
            let commands_move_layout = MoveTypeLayout::Vector(Box::new(command_move_layout));
            self.cached_tx_commands = Some(to_global_value(&rust_value, commands_move_layout)?);
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

    /// Returns a `Value` containing a ref to the enriched tx inputs.
    ///
    /// The enriched data is stored directly in `AuthContext` (built at
    /// auth-context creation time with object-type resolution from the backing
    /// store), so no conversion is needed here — we just BCS-serialize it.
    ///
    /// Result is cached to avoid redundant allocations on subsequent calls.
    pub fn enriched_tx_inputs_ref(
        &mut self,
        input_move_layout: MoveTypeLayout,
    ) -> PartialVMResult<(Value, AbstractMemorySize)> {
        if self.cached_enriched_tx_inputs.is_none() {
            let auth_context = self.auth_context.borrow();
            // Direct: tx_inputs are already EnrichedCallArg
            let rust_value = (auth_context.tx_inputs(),);
            let layout = MoveTypeLayout::Vector(Box::new(input_move_layout));
            self.cached_enriched_tx_inputs = Some(to_global_value(&rust_value, layout)?);
        }

        let (cached, size) = self.cached_enriched_tx_inputs.as_ref().unwrap();
        Ok((
            cached
                .borrow_global()
                .inspect_err(|err| assert!(err.major_status() != StatusCode::MISSING_DATA))?
                .value_as::<StructRef>()?
                .borrow_field(0)?,
            *size,
        ))
    }

    /// Returns a `Value` containing a ref to the enriched tx commands.
    ///
    /// Same as `enriched_tx_inputs_ref`: the enriched data is already stored
    /// in `AuthContext`, so this just BCS-serializes it without conversion.
    ///
    /// Result is cached to avoid redundant allocations on subsequent calls.
    pub fn enriched_tx_commands_ref(
        &mut self,
        command_move_layout: MoveTypeLayout,
    ) -> PartialVMResult<(Value, AbstractMemorySize)> {
        if self.cached_enriched_tx_commands.is_none() {
            let auth_context = self.auth_context.borrow();
            // Direct: tx_commands are already EnrichedCommand
            let rust_value = (auth_context.tx_commands(),);
            let layout = MoveTypeLayout::Vector(Box::new(command_move_layout));
            self.cached_enriched_tx_commands = Some(to_global_value(&rust_value, layout)?);
        }

        let (cached, size) = self.cached_enriched_tx_commands.as_ref().unwrap();
        Ok((
            cached
                .borrow_global()
                .inspect_err(|err| assert!(err.major_status() != StatusCode::MISSING_DATA))?
                .value_as::<StructRef>()?
                .borrow_field(0)?,
            *size,
        ))
    }

    /// Replaces the `AuthContext` with pre-built enriched values.
    /// Only callable in testing scenarios. Expects the input values to be
    /// values, then it tries to convert them back to their original rust
    /// types and updates
    pub fn replace_enriched(
        &mut self,
        auth_digest_value: Vec<u8>,
        tx_inputs_value: Vec<Value>,
        input_move_layout: MoveTypeLayout,
        tx_commands_value: Vec<Value>,
        command_move_layout: MoveTypeLayout,
    ) -> PartialVMResult<()> {
        if !self.test_only {
            return Err(
                PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR).with_message(
                    "`replace_enriched` called on a non testing scenario".to_string(),
                ),
            );
        }

        let tx_commands: Vec<EnrichedCommand> = tx_commands_value
            .into_iter()
            .map(|value| from_value(value, &command_move_layout))
            .collect::<PartialVMResult<_>>()?;

        let tx_inputs: Vec<EnrichedCallArg> = tx_inputs_value
            .into_iter()
            .map(|value| from_value(value, &input_move_layout))
            .collect::<PartialVMResult<_>>()?;

        let auth_digest =
            MoveAuthenticatorDigest::try_from(auth_digest_value.as_slice()).map_err(|err| {
                PartialVMError::new(StatusCode::UNEXPECTED_DESERIALIZATION_ERROR)
                    .with_message(err.to_string())
            })?;

        self.auth_context
            .borrow_mut()
            .replace(auth_digest, tx_inputs, tx_commands);

        self.cached_digest = None;
        self.cached_tx_inputs = None;
        self.cached_tx_commands = None;
        self.cached_enriched_tx_inputs = None;
        self.cached_enriched_tx_commands = None;

        Ok(())
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

        // Deserialize from plain Move types (CallArg / Command) as passed by
        // the Move test helper `new_with_tx_inputs`.
        let plain_commands = tx_commands_value
            .into_iter()
            .map(|value| from_value(value, &command_move_layout))
            .collect::<PartialVMResult<Vec<MoveCommand>>>()?;

        let plain_inputs = tx_inputs_value
            .into_iter()
            .map(|value| from_value(value, &input_move_layout))
            .collect::<PartialVMResult<Vec<MoveCallArg>>>()?;

        // Convert to enriched (type names stay empty in test scenario — no object
        // store is available here).
        let tx_inputs: Vec<EnrichedCallArg> = plain_inputs
            .iter()
            .map(enriched_call_arg_from_move_call_arg)
            .collect();
        let tx_commands: Vec<EnrichedCommand> = plain_commands
            .iter()
            .map(enriched_command_from_move_command)
            .collect();

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
        self.cached_enriched_tx_inputs = None;
        self.cached_enriched_tx_commands = None;

        Ok(())
    }
}

/// Converts a [`MoveCallArg`] to a flat [`EnrichedCallArg`].
///
/// Type names for object arguments are left as empty strings because object
/// type resolution requires external storage that is unavailable in the
/// native-function context.
fn enriched_call_arg_from_move_call_arg(arg: &MoveCallArg) -> EnrichedCallArg {
    match arg {
        MoveCallArg::Pure(bytes) => EnrichedCallArg::Pure {
            value: bytes.clone(),
            type_name: TypeName {
                name: String::new(),
            },
        },
        MoveCallArg::Object(MoveObjectArg::ImmOrOwnedObject((id, version, digest))) => {
            EnrichedCallArg::ImmOrOwnedObject(ImmOrOwnedObjectArg {
                id: *id,
                version: *version,
                digest: *digest,
                mutable: false,
                type_name: TypeName {
                    name: String::new(),
                },
            })
        }
        MoveCallArg::Object(MoveObjectArg::SharedObject {
            id,
            initial_shared_version,
            mutable,
        }) => EnrichedCallArg::SharedObject(SharedObjectArg {
            id: *id,
            initial_shared_version: *initial_shared_version,
            mutable: *mutable,
            digest: ObjectDigest::MIN,
            type_name: TypeName {
                name: String::new(),
            },
        }),
        MoveCallArg::Object(MoveObjectArg::Receiving((id, version, digest))) => {
            EnrichedCallArg::Receiving(ImmOrOwnedObjectArg {
                id: *id,
                version: *version,
                digest: *digest,
                mutable: false,
                type_name: TypeName {
                    name: String::new(),
                },
            })
        }
    }
}

/// Converts a [`MoveCommand`] to an [`EnrichedCommand`].
///
/// For `MoveCall` variants, `is_entry` and `returns` are set to `false`/empty
/// because they require VM function resolution that is unavailable here.
fn enriched_command_from_move_command(cmd: &MoveCommand) -> EnrichedCommand {
    match cmd {
        MoveCommand::MoveCall(call) => {
            EnrichedCommand::MoveCall(Box::new(EnrichedProgrammableMoveCall {
                package: call.package,
                module: call.module.clone(),
                function: call.function.clone(),
                is_entry: false,
                type_arguments: call.type_arguments.clone(),
                arguments: call.arguments.clone(),
                returns: vec![],
            }))
        }
        MoveCommand::TransferObjects(objects, recipient) => {
            EnrichedCommand::TransferObjects(objects.clone(), *recipient)
        }
        MoveCommand::SplitCoins(coin, amounts) => {
            EnrichedCommand::SplitCoins(*coin, amounts.clone())
        }
        MoveCommand::MergeCoins(target, sources) => {
            EnrichedCommand::MergeCoins(*target, sources.clone())
        }
        MoveCommand::Publish(modules, deps) => {
            EnrichedCommand::Publish(modules.clone(), deps.clone())
        }
        MoveCommand::MakeMoveVec(type_arg, elements) => {
            EnrichedCommand::MakeMoveVec(type_arg.clone(), elements.clone())
        }
        MoveCommand::Upgrade(modules, deps, package, ticket) => {
            EnrichedCommand::Upgrade(modules.clone(), deps.clone(), *package, *ticket)
        }
    }
}

fn struct_layout_with_field(field: MoveTypeLayout) -> MoveTypeLayout {
    MoveTypeLayout::Struct(Box::new(MoveStructLayout(Box::new(vec![field]))))
}

fn to_global_value<T: ?Sized + Serialize>(
    field: &T,
    field_move_layout: MoveTypeLayout,
) -> PartialVMResult<(GlobalValue, AbstractMemorySize)> {
    let move_layout = struct_layout_with_field(field_move_layout);

    let move_value = to_value(field, &move_layout)?;
    let move_value_size = move_value.legacy_size();

    Ok((
        GlobalValue::cached(move_value).expect("Failed to cache global value"),
        move_value_size,
    ))
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
