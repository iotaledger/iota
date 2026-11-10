// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    fmt,
    rc::Rc,
    sync::Arc,
};

use indexmap::{IndexMap, IndexSet};
use iota_move_natives::{
    object_runtime,
    object_runtime::{
        LoadedRuntimeObject, ObjectRuntime, RuntimeResults, get_all_uids, max_event_error,
    },
};
use iota_protocol_config::ProtocolConfig;
use iota_sdk_move_types::iota_framework::package::UpgradeTicket;
use iota_sdk_types::{
    Address, Argument, Event, MoveObjectType, MovePackage, MoveStruct, ObjectId, Owner, Version,
};
use iota_types::{
    IOTA_FRAMEWORK_PACKAGE_ID,
    base_types::{RESOLVED_ASCII_STR, RESOLVED_UTF8_STR, TxContext},
    error::{ExecutionError, ExecutionErrorKind, IotaError, command_argument_error},
    execution::{ExecutionResults, ExecutionResultsV1},
    execution_config_utils::to_binary_config,
    iota_sdk_types_conversions::{
        identifier_core_to_sdk, struct_tag_core_to_sdk, struct_tag_sdk_to_core,
    },
    metrics::LimitsMetrics,
    move_package::{
        IotaAttributeV2, MovePackageExt, ProtocolBuildConfig, RuntimeModuleMetadata, UpgradeCap,
        UpgradePolicy, UpgradeReceipt, normalize_deserialized_modules_with_metadata,
        normalize_modules_with_metadata,
    },
    object::{MoveStructExt, Object},
    storage::{BackingPackageStore, DenyListResult, PackageObject, get_package_objects},
};
use iota_verifier::INIT_FN_NAME;
use move_binary_format::{
    CompiledModule,
    compatibility::{Compatibility, InclusionCheck},
    errors::{Location, PartialVMError, PartialVMResult, VMResult},
    file_format::FunctionDefinitionIndex,
    normalized,
};
use move_core_types::{
    account_address::AccountAddress,
    identifier::{IdentStr, Identifier},
    language_storage::{ModuleId, StructTag, TypeTag},
    u256::U256,
};
use move_trace_format::format::MoveTraceBuilder;
use move_vm_runtime::{
    execution::{
        Type as VMType, TypeSubst as _,
        values::{VMValueCast, Value as VMValue},
        vm::{LoadedFunctionInformation, MoveVM},
    },
    natives::extensions::NativeExtensions,
    shared::{
        gas::{GasMeter, SimpleInstruction},
        linkage_context::LinkageHash,
    },
    validation::verification::ast::Package as VerifiedPackage,
};
use quick_cache::unsync::Cache as QCache;
use serde::{Deserialize, de::DeserializeSeed};
use tracing::instrument;

use crate::{
    adapter,
    compat::{
        ExecutionErrorTrait, ObjectPermissions, SafeIndex, ZipDebugEqIteratorExt,
        execution_status::{CommandArgumentError, PackageUpgradeError},
    },
    data_store::serialized_package,
    execution_mode::ExecutionMode,
    execution_value::ExecutionState,
    gas_charger::{GasCharger, GasPayment},
    gas_meter::IotaGasMeter,
    sdk_conv::{
        AccountAddressExt, ToAccountAddress, core_struct_tag, move_object_type, try_as_move,
    },
    sp,
    static_programmable_transactions::{
        env::Env,
        execution,
        execution::{
            argument_nodes::ArgumentNodeBudget,
            package_metadata::{
                self, CREATE_PACKAGE_METADATA_V1_WITH_DYNAMIC_METADATA_FN_NAME,
                PACKAGE_METADATA_MODULE_NAME, PackageMetadataCreation,
            },
            trace_utils,
            values::{Local, Locals, Value},
        },
        linkage::resolved_linkage::{ExecutableLinkage, ResolvedLinkage},
        loading::ast::{Datatype, DeserializedPackage, PackagePayload},
        typing::{ast as T, ast::Type, verify::input_arguments::primitive_serialization_layout},
    },
};

/// Publish init runs before any command arguments are read, so the gas stack is still empty.
const PUBLISH_INIT_EXPECTED_STACK_HEIGHT: u64 = 0;

macro_rules! unwrap {
    ($e:expr, $($args:expr),* $(,)?) => {
        match $e {
            Some(v) => v,
            None => {
                invariant_violation!("Unexpected none: {}", format!($($args),*))
            }
        }

    };
}

#[macro_export]
macro_rules! object_runtime {
    ($context:ident) => {
        $context
            .native_extensions
            .try_borrow()
            .map_err(|_| {
                make_invariant_violation!(
                    "Should be able to borrow object runtime native extension"
                )
            })?
            .get::<iota_move_natives::object_runtime::ObjectRuntime>()
            .map_err(|e| {
                $context
                    .env
                    .convert_vm_error(e.finish(move_binary_format::errors::Location::Undefined))
            })
    };
}

macro_rules! object_runtime_mut {
    ($context:ident) => {
        $context
            .native_extensions
            .try_borrow_mut()
            .map_err(|_| {
                make_invariant_violation!(
                    "Should be able to borrow object runtime native extension"
                )
            })?
            .get_mut::<ObjectRuntime>()
            .map_err(|e| $context.env.convert_vm_error(e.finish(Location::Undefined)))
    };
}

macro_rules! charge_gas_ {
    ($gas_charger:expr, $env:expr, $call:ident($($args:expr),*)) => {{
        if $crate::compat::CHARGE_PTB_VALUE_OPERATIONS {
            IotaGasMeter($gas_charger.move_gas_status_mut())
                .$call($($args),*)
                .map_err(|e| $env.convert_vm_error(e.finish(Location::Undefined)))
        } else {
            Ok(())
        }
    }};
    ($gas_charger:expr, $env:expr, $case:ident, $value_view:expr) => {
        charge_gas_!($gas_charger, $env, $case($value_view))
    };
}

macro_rules! charge_gas {
    ($context:ident, $case:ident, $value_view:expr) => {{ charge_gas_!($context.gas_charger, $context.env, $case, $value_view) }};
}

// Helper macro to manage Move VM cache for different linkage contexts. If the given linkage is
// found the VM is reused, otherwise a new VM is created and inserted into the cache.
macro_rules! with_vm {
    ($self:ident, $linkage:expr, $error:ty, $body:expr) => {{
        let link_context = $linkage.linkage_context::<$error>()?;
        let linkage_hash = link_context.to_linkage_hash();
        let mut vm = if let Some((_, vm)) = $self.executable_vm_cache.remove(&linkage_hash) {
            vm
        } else {
            let data_store = &$self.env.linkable_store.package_store;
            $self
                .env
                .vm
                .make_vm_with_native_extensions(
                    data_store,
                    link_context.clone(),
                    $self.native_extensions.clone(),
                )
                .map_err(|e| $self.env.convert_linked_vm_error(e, $linkage))?
        };
        let result = $body(&mut vm)?;
        $self.executable_vm_cache.insert(linkage_hash, vm);
        Ok(result)
    }};
}

/// Type wrapper around Value to ensure safe usage
#[derive(Debug)]
pub struct CtxValue(Value);

#[derive(Clone, Debug)]
pub struct InputObjectMetadata {
    pub newly_created: bool,
    pub id: ObjectId,
    pub refined_permissions: ObjectPermissions,
    pub owner: Owner,
    pub version: Version,
    pub type_: Type,
}

/// Metadata in the case that the GasCoin is transferred as an object to another recipient. This
/// is needed at the end to both refund the gas budget and set the correct location from which to
/// charge gas.
#[derive(Debug, Clone, Copy)]
pub(crate) enum GasCoinTransfer {
    /// Sent using the TransferObjects command
    TransferObjects,
}

#[derive(Copy, Clone)]
enum UsageKind {
    Move,
    Copy,
    Borrow,
}

// Locals and metadata for all `Location`s. Separated from `Context` for lifetime reasons.
struct Locations {
    // A single local for holding the TxContext
    tx_context_value: Locals,
    // A single local for holding the AuthContext, invalid unless the execution mode supplies it
    auth_context_value: Locals,
    /// The runtime value for the Gas coin, None if no gas coin is provided
    gas: Option<(InputObjectMetadata, Locals)>,
    /// The runtime value for the input objects args
    input_object_metadata: Vec<(T::InputIndex, InputObjectMetadata)>,
    object_inputs: Locals,
    pure_input_bytes: IndexSet<Vec<u8>>,
    pure_input_metadata: Vec<T::PureInput>,
    pure_inputs: Locals,
    receiving_input_metadata: Vec<T::ReceivingInput>,
    receiving_inputs: Locals,
    /// The results of a given command. For most commands, the inner vector will have length 1.
    /// It will only not be 1 for Move calls with multiple return values.
    /// Inner values are None if taken/moved by-value
    results: Vec<Locals>,
}

enum ResolvedLocation<'a> {
    Local(Local<'a>),
    Pure {
        bytes: &'a [u8],
        metadata: &'a T::PureInput,
        local: Local<'a>,
    },
    Receiving {
        metadata: &'a T::ReceivingInput,
        local: Local<'a>,
    },
}

/// Maintains all runtime state specific to programmable transactions
pub struct Context<'env, 'pc, 'vm, 'state, 'linkage, 'gas, 'extension, Mode>
where
    Mode: ExecutionMode,
{
    pub env: &'env Env<'pc, 'vm, 'state, 'linkage, 'extension, Mode>,
    /// Metrics for reporting exceeded limits
    pub metrics: Arc<LimitsMetrics>,
    pub native_extensions: NativeExtensions<'env>,
    /// A shared transaction context, contains transaction digest information and manages the
    /// creation of new object IDs
    pub tx_context: Rc<RefCell<TxContext>>,
    /// The gas charger used for metering
    pub gas_charger: &'gas mut GasCharger,
    /// User events are claimed after each Move call
    user_events: Vec<(ModuleId, StructTag, Vec<u8>)>,
    /// Budget for the struct, enum and vector values in the arguments of all Move calls of the
    /// transaction; `None` when the protocol version sets no limit or the execution is unmetered.
    argument_nodes: Option<ArgumentNodeBudget>,
    // runtime data
    locations: Locations,
    /// Tracks where the gas coin was sent, if it was moved by value
    gas_coin_transfer: Option<GasCoinTransfer>,
    // cache of Move VMs created this transaction for different linkage contexts so that we can
    // reuse them.
    executable_vm_cache: QCache<LinkageHash, MoveVM<'env>>,
}

impl Locations {
    /// NOTE! This does not charge gas and should not be used directly. It is exposed for
    /// dev-inspect
    fn resolve(&mut self, location: T::Location) -> Result<ResolvedLocation<'_>, ExecutionError> {
        Ok(match location {
            T::Location::TxContext => ResolvedLocation::Local(self.tx_context_value.local(0)?),
            T::Location::AuthContext => ResolvedLocation::Local(self.auth_context_value.local(0)?),
            T::Location::GasCoin => {
                let (_, gas_locals) = unwrap!(self.gas.as_mut(), "Gas coin not provided");
                ResolvedLocation::Local(gas_locals.local(0)?)
            }
            T::Location::ObjectInput(i) => ResolvedLocation::Local(self.object_inputs.local(i)?),
            T::Location::Result(i, j) => {
                let result = unwrap!(self.results.get_mut(i as usize), "bounds already verified");
                ResolvedLocation::Local(result.local(j)?)
            }
            T::Location::PureInput(i) => {
                let local = self.pure_inputs.local(i)?;
                let metadata = &self.pure_input_metadata.safe_get(i as usize)?;
                let bytes = self
                    .pure_input_bytes
                    .get_index(metadata.byte_index)
                    .ok_or_else(|| {
                        make_invariant_violation!(
                            "Pure input {} bytes out of bounds at index {}",
                            metadata.original_input_index.0,
                            metadata.byte_index,
                        )
                    })?;
                ResolvedLocation::Pure {
                    bytes,
                    metadata,
                    local,
                }
            }
            T::Location::ReceivingInput(i) => ResolvedLocation::Receiving {
                metadata: self.receiving_input_metadata.safe_get(i as usize)?,
                local: self.receiving_inputs.local(i)?,
            },
        })
    }
}

impl<'env, 'pc, 'vm, 'state, 'linkage, 'gas, 'extension, Mode>
    Context<'env, 'pc, 'vm, 'state, 'linkage, 'gas, 'extension, Mode>
where
    Mode: ExecutionMode,
{
    #[instrument(name = "Context::new", level = "trace", skip_all)]
    pub fn new(
        env: &'env Env<'pc, 'vm, 'state, 'linkage, 'extension, Mode>,
        metrics: Arc<LimitsMetrics>,
        tx_context: Rc<RefCell<TxContext>>,
        gas_charger: &'gas mut GasCharger,
        payment_location: Option<GasPayment>,
        pure_input_bytes: IndexSet<Vec<u8>>,
        object_inputs: Vec<T::ObjectInput>,
        pure_input_metadata: Vec<T::PureInput>,
        receiving_input_metadata: Vec<T::ReceivingInput>,
    ) -> Result<Self, Mode::Error>
    where
        'pc: 'state,
    {
        let mut input_object_map = BTreeMap::new();
        let mut input_object_metadata = Vec::with_capacity(object_inputs.len());
        let mut object_values = Vec::with_capacity(object_inputs.len());
        for object_input in object_inputs {
            let (i, m, v) = load_object_arg(gas_charger, env, &mut input_object_map, object_input)?;
            input_object_metadata.push((i, m));
            object_values.push(Some(v));
        }
        let object_inputs = Locals::new(object_values)?;
        let pure_inputs = Locals::new_invalid(pure_input_metadata.len())?;
        let receiving_inputs = Locals::new_invalid(receiving_input_metadata.len())?;
        let gas = match payment_location {
            Some(gas_payment) => {
                let ty = env.gas_coin_type()?;
                let (gas_metadata, gas_value) = load_object_arg_impl(
                    gas_charger,
                    env,
                    &mut input_object_map,
                    gas_payment.coin,
                    ObjectPermissions::ALL,
                    ty,
                )?;
                let mut gas_locals = Locals::new([Some(gas_value)])?;
                let mut gas_local = gas_locals.local(0)?;
                let gas_ref = gas_local.borrow()?;
                // We have already checked that the gas balance is enough to cover the gas
                // budget
                let max_gas_in_balance = gas_charger.gas_budget();
                gas_ref.coin_ref_subtract_balance(max_gas_in_balance)?;
                Some((gas_metadata, gas_locals))
            }
            None => None,
        };
        let native_extensions = adapter::new_native_extensions(
            env.state_view,
            input_object_map,
            !gas_charger.is_unmetered(),
            env.protocol_config,
            metrics.clone(),
            tx_context.clone(),
            env.state_view.read_auth_context(),
        )?;
        debug_assert!(
            !crate::compat::CHARGE_PTB_VALUE_OPERATIONS
                || gas_charger.move_gas_status().stack_height_current() == 0
        );
        let argument_nodes = ArgumentNodeBudget::new(
            env.protocol_config
                .max_move_call_argument_nodes_total_as_option(),
            gas_charger.is_unmetered(),
        );
        let tx_context_value = Locals::new(vec![Some(Value::new_tx_context(
            &tx_context.borrow().to_bcs_legacy_context(),
        )?)])?;
        let auth_context = if Mode::allow_auth_context() {
            env.state_view
                .read_auth_context()
                .map(|auth_context| {
                    Value::deserialize(
                        env,
                        &auth_context.borrow().to_move_bcs_bytes(),
                        env.auth_context_type()?,
                    )
                })
                .transpose()?
        } else {
            None
        };
        let auth_context_value = Locals::new(vec![auth_context])?;
        Ok(Self {
            env,
            metrics,
            native_extensions,
            tx_context,
            gas_charger,
            user_events: vec![],
            argument_nodes,
            locations: Locations {
                tx_context_value,
                auth_context_value,
                gas,
                input_object_metadata,
                object_inputs,
                pure_input_bytes,
                pure_input_metadata,
                pure_inputs,
                receiving_input_metadata,
                receiving_inputs,
                results: vec![],
            },
            gas_coin_transfer: None,
            executable_vm_cache: QCache::new(1024),
        })
    }

    pub(crate) fn record_gas_coin_transfer(
        &mut self,
        transfer: GasCoinTransfer,
    ) -> Result<(), Mode::Error> {
        if self.gas_coin_transfer.is_some() {
            invariant_violation!("Gas coin destination set more than once");
        }
        self.gas_coin_transfer = Some(transfer);
        Ok(())
    }

    pub fn finish(mut self) -> Result<ExecutionResults, Mode::Error> {
        assert_invariant!(
            !self.locations.tx_context_value.local(0)?.is_invalid()?,
            "tx context value should be present"
        );
        let gas_coin_transfer = self.gas_coin_transfer;
        let gas = std::mem::take(&mut self.locations.gas);
        let object_input_metadata = std::mem::take(&mut self.locations.input_object_metadata);
        let mut object_inputs =
            std::mem::replace(&mut self.locations.object_inputs, Locals::new_invalid(0)?);
        let mut created_input_object_ids = BTreeSet::new();
        let mut loaded_runtime_objects = BTreeMap::new();
        let mut by_value_shared_objects = BTreeSet::new();
        let gas = gas
            .map(|(m, mut g)| {
                let value_opt = g.local(0)?.move_if_valid()?;
                let moved = value_opt.is_none();
                assert_invariant!(
                    moved == gas_coin_transfer.is_some(),
                    "Gas coin moved requires gas coin transfer to be recorded, and vice versa"
                );
                Result::<_, ExecutionError>::Ok((m, value_opt))
            })
            .transpose()?;

        let gas_id_opt = gas.as_ref().map(|(m, _)| m.id);
        let object_inputs = object_input_metadata
            .into_iter()
            .enumerate()
            .map(|(i, (_, m))| {
                let v_opt = object_inputs.local(checked_as!(i, u16)?)?.move_if_valid()?;
                Ok((m, v_opt))
            })
            .collect::<Result<Vec<_>, ExecutionError>>()?;
        for (metadata, value_opt) in object_inputs.into_iter().chain(gas) {
            let InputObjectMetadata {
                newly_created,
                id,
                refined_permissions,
                owner,
                version,
                type_,
            } = metadata;
            if !refined_permissions.can_use_mutably() {
                continue;
            }

            if newly_created {
                created_input_object_ids.insert(id);
            } else {
                loaded_runtime_objects.insert(
                    id,
                    LoadedRuntimeObject {
                        version,
                        is_modified: true,
                    },
                );
            }
            if let Some(object) = value_opt {
                self.transfer_object_(
                    owner,
                    type_,
                    CtxValue(object),
                    // end of transaction
                    true,
                )?;
            } else if owner.is_shared() {
                by_value_shared_objects.insert(id);
            }
        }

        let Self {
            env,
            native_extensions,
            tx_context,
            gas_charger,
            user_events,
            ..
        } = self;
        let ref_context: &RefCell<TxContext> = &tx_context;
        let tx_context: &TxContext = &ref_context.borrow();
        let tx_digest = ref_context.borrow().digest();

        let object_runtime: ObjectRuntime = native_extensions
            .try_borrow_mut()
            .map_err(|_| {
                make_invariant_violation!(
                "Should be able to borrow object runtime native extension at the end of execution"
            )
            })?
            .remove()
            .map_err(|e| env.convert_vm_error(e.finish(Location::Undefined)))?;

        let RuntimeResults {
            mut writes,
            user_events: remaining_events,
            loaded_child_objects,
            mut created_object_ids,
            deleted_object_ids,
            received_coin_types,
        } = object_runtime.finish()?;
        assert_invariant!(
            loaded_runtime_objects
                .keys()
                .all(|id| !created_object_ids.contains(id)),
            "Loaded input objects should not be in the created objects set"
        );
        // TODO generalize post transaction checks using permissions

        assert_invariant!(
            remaining_events.is_empty(),
            "Events should be taken after every Move call"
        );
        // Refund the unused gas budget to the gas coin
        if let Some(gas_id) = gas_id_opt {
            assert_invariant!(
                !deleted_object_ids.contains(&gas_id),
                "Gas coin should not be deleted"
            );
            finish_gas_coin(gas_charger, &mut writes, gas_id)?;
        }

        loaded_runtime_objects.extend(loaded_child_objects);

        let mut written_objects = BTreeMap::new();

        let (writeout_vm, ty_linkage) =
            Self::make_writeout_vm(env, writes.values().map(|(_, ty, _)| ty.clone()))?;

        for (id, (recipient, ty, value)) in writes {
            let (ty, layout) = Self::load_type_and_layout_from_struct_for_writeout(
                env,
                &writeout_vm,
                &ty_linkage,
                core_struct_tag(&ty),
            )?;
            let Some(bytes) = value.typed_serialize(&layout) else {
                invariant_violation!("Failed to serialize already deserialized Move value");
            };
            // SAFETY: has_public_transfer is derived from the abilities of the type above
            let move_object = unsafe {
                create_written_object::<Mode>(env, &loaded_runtime_objects, id, ty, bytes)?
            };
            let object = Object::new_move(move_object, recipient, tx_digest);
            written_objects.insert(id, object);
        }

        for package in self
            .env
            .linkable_store
            .package_store
            .to_new_packages()
            .into_iter()
        {
            let package_obj = Object::new_from_package(package, tx_digest);
            let id = package_obj.id();
            created_object_ids.insert(id);
            written_objects.insert(id, package_obj);
        }

        Ok(execution::context::finish(
            env.protocol_config,
            env.state_view,
            gas_charger,
            tx_context,
            &by_value_shared_objects,
            loaded_runtime_objects,
            written_objects,
            created_object_ids,
            deleted_object_ids,
            user_events,
            received_coin_types,
        )?)
    }

    pub fn take_user_events(
        &mut self,
        vm: &MoveVM<'_>,
        version_mid: ModuleId,
        function_def_idx: FunctionDefinitionIndex,
        instr_length: u16,
        linkage: &ExecutableLinkage,
    ) -> Result<(), Mode::Error> {
        let events = object_runtime_mut!(self)?.take_user_events();
        let Some(num_events) = self.user_events.len().checked_add(events.len()) else {
            invariant_violation!("usize overflow, too many events emitted")
        };
        let max_events = self.env.protocol_config.max_num_event_emit();
        if num_events as u64 > max_events {
            let err = max_event_error(max_events)
                .at_code_offset(function_def_idx, instr_length)
                .finish(Location::Module(version_mid));
            return Err(self.env.convert_linked_vm_error(err, linkage));
        }
        let new_events = events
            .into_iter()
            .map(|(tag, value)| {
                let type_tag = TypeTag::Struct(Box::new(tag));
                let layout = vm
                    .runtime_type_layout(&type_tag)
                    .map_err(|e| self.env.convert_linked_vm_error(e, linkage))?;
                let Some(bytes) = value.typed_serialize(&layout) else {
                    invariant_violation!("Failed to serialize Move event");
                };
                let TypeTag::Struct(tag) = type_tag else {
                    unreachable!()
                };
                Ok((version_mid.clone(), *tag, bytes))
            })
            .collect::<Result<Vec<_>, Mode::Error>>()?;
        self.user_events.extend(new_events);
        Ok(())
    }

    // Final serialization of written objects
    //

    /// The writeout VM is used to serialize all written objects at the end of execution. This
    /// needs access to all types that were written during the transaction [`writes`]. Importantly,
    /// it needs to be able to create a VM over any newly published packages as the `init`
    /// functions in those packages may have created objects of types defined in those packages.
    fn make_writeout_vm<I>(
        env: &Env<'pc, 'vm, 'state, 'linkage, 'extension, Mode>,
        writes: I,
    ) -> Result<(MoveVM<'extension>, ExecutableLinkage), Mode::Error>
    where
        I: IntoIterator<Item = MoveObjectType>,
    {
        let tys_addrs = writes
            .into_iter()
            .flat_map(|ty| core_struct_tag(&ty).all_addresses())
            .map(AccountAddressExt::to_object_id)
            .collect::<BTreeSet<_>>();

        let ty_linkage = ExecutableLinkage::type_linkage::<_, Mode::Error>(
            env.linkage_analysis.config().clone(),
            &tys_addrs,
            env.linkable_store,
        )?;
        env.vm
            .make_vm(
                &env.linkable_store.package_store,
                ty_linkage.linkage_context::<Mode::Error>()?,
            )
            .map_err(|e| env.convert_linked_vm_error(e, &ty_linkage))
            .map(|vm| (vm, ty_linkage))
    }

    /// Load the type and layout for a struct tag.
    /// It is important that this use the VM passed in, and not the `resolution_vm` in the `env` as
    /// the types requested may have only been created during the execution of the transaction and
    /// therefore will not be present in the `resolution_vm`.
    fn load_type_and_layout_from_struct_for_writeout(
        env: &Env<'pc, 'vm, 'state, 'linkage, 'extension, Mode>,
        vm: &MoveVM,
        linkage: &ExecutableLinkage,
        tag: StructTag,
    ) -> Result<(Type, move_core_types::runtime_value::MoveTypeLayout), Mode::Error> {
        let type_tag = TypeTag::Struct(Box::new(tag));
        let vm_type = vm
            .load_type(&type_tag)
            .map_err(|e| env.convert_linked_vm_error(e, linkage))?;
        let layout = vm
            .runtime_type_layout(&type_tag)
            .map_err(|e| env.convert_vm_error(e))?;
        env.adapter_type_from_vm_type(vm, &vm_type)
            .map(|ty| (ty, layout))
    }

    // Arguments and Values
    //

    fn location(&mut self, usage: UsageKind, location: T::Location) -> Result<Value, Mode::Error> {
        let resolved = self.locations.resolve(location)?;
        let mut local = match resolved {
            ResolvedLocation::Local(l) => l,
            ResolvedLocation::Pure {
                bytes,
                metadata,
                mut local,
            } => {
                if local.is_invalid()? {
                    let v = load_pure_value(self.gas_charger, self.env, bytes, metadata)?;
                    local.store(v)?;
                }
                local
            }
            ResolvedLocation::Receiving {
                metadata,
                mut local,
            } => {
                if local.is_invalid()? {
                    let v = load_receiving_value(self.gas_charger, self.env, metadata)?;
                    local.store(v)?;
                }
                local
            }
        };
        Ok(match usage {
            UsageKind::Move => {
                let value = local.move_()?;
                charge_gas_!(self.gas_charger, self.env, charge_move_loc, &value)?;
                value
            }
            UsageKind::Copy => {
                let value = local.copy()?;
                charge_gas_!(self.gas_charger, self.env, charge_copy_loc, &value)?;
                value
            }
            UsageKind::Borrow => {
                charge_gas_!(
                    self.gas_charger,
                    self.env,
                    charge_simple_instr(SimpleInstruction::MutBorrowLoc)
                )?;
                local.borrow()?
            }
        })
    }

    fn location_usage(&mut self, usage: T::Usage) -> Result<Value, Mode::Error> {
        match usage {
            T::Usage::Move(location) => self.location(UsageKind::Move, location),
            T::Usage::Copy { location, .. } => self.location(UsageKind::Copy, location),
        }
    }

    fn argument_value(&mut self, sp!(_, (arg_, _)): T::Argument) -> Result<Value, Mode::Error> {
        match arg_ {
            T::Argument__::Use(usage) => self.location_usage(usage),
            // freeze is a no-op for references since the value does not track mutability
            T::Argument__::Freeze(usage) => self.location_usage(usage),
            T::Argument__::Borrow(_, location) => self.location(UsageKind::Borrow, location),
            T::Argument__::Read(usage) => {
                let reference = self.location_usage(usage)?;
                charge_gas!(self, charge_read_ref, &reference)?;
                Ok(reference.read_ref()?)
            }
        }
    }

    pub fn argument<V>(&mut self, arg: T::Argument) -> Result<V, Mode::Error>
    where
        VMValue: VMValueCast<V>,
    {
        let before_height = self.gas_charger.move_gas_status().stack_height_current();
        let value = self.argument_value(arg)?;
        let after_height = self.gas_charger.move_gas_status().stack_height_current();
        debug_assert!(
            !crate::compat::CHARGE_PTB_VALUE_OPERATIONS
                || before_height.saturating_add(1) == after_height
        );
        let value: V = value.cast()?;
        Ok(value)
    }

    pub fn arguments<V>(&mut self, args: Vec<T::Argument>) -> Result<Vec<V>, Mode::Error>
    where
        VMValue: VMValueCast<V>,
    {
        args.into_iter().map(|arg| self.argument(arg)).collect()
    }

    pub fn result(&mut self, result: Vec<Option<CtxValue>>) -> Result<(), Mode::Error> {
        self.locations
            .results
            .push(Locals::new(result.into_iter().map(|v| v.map(|v| v.0)))?);
        Ok(())
    }

    pub fn charge_command(
        &mut self,
        is_move_call: bool,
        num_args: usize,
        num_return: usize,
    ) -> Result<(), Mode::Error> {
        if !crate::compat::CHARGE_PTB_VALUE_OPERATIONS {
            return Ok(());
        }
        let move_gas_status = self.gas_charger.move_gas_status_mut();
        let before_size = move_gas_status.stack_size_current();
        // Pop all of the arguments
        // If the return values came from the Move VM directly (via a Move call), pop those
        // as well
        let num_popped = if is_move_call {
            num_args.checked_add(num_return).ok_or_else(|| {
                make_invariant_violation!("usize overflow when charging gas for command",)
            })?
        } else {
            num_args
        };
        move_gas_status
            .charge(1, 0, num_popped as u64, 0, /* unused */ 1)
            .map_err(|e| self.env.convert_vm_error(e.finish(Location::Undefined)))?;
        let after_size = move_gas_status.stack_size_current();
        assert_invariant!(
            before_size == after_size,
            "We assume currently that the stack size is not decremented. \
            If this changes, we need to actually account for it here"
        );
        Ok(())
    }

    pub fn copy_value(&mut self, value: &CtxValue) -> Result<CtxValue, Mode::Error> {
        Ok(CtxValue(copy_value(self.gas_charger, self.env, &value.0)?))
    }

    pub fn new_coin(&mut self, amount: u64) -> Result<CtxValue, Mode::Error> {
        let id = self.tx_context.borrow_mut().fresh_id();
        object_runtime_mut!(self)?
            .new_id(id)
            .map_err(|e| self.env.convert_vm_error(e.finish(Location::Undefined)))?;
        Ok(CtxValue(Value::coin(id, amount)))
    }

    pub fn destroy_coin(&mut self, coin: CtxValue) -> Result<u64, Mode::Error> {
        let (id, amount) = coin.0.unpack_coin()?;
        object_runtime_mut!(self)?
            .delete_id(id)
            .map_err(|e| self.env.convert_vm_error(e.finish(Location::Undefined)))?;
        Ok(amount)
    }

    pub fn new_upgrade_cap(&mut self, version_id: ObjectId) -> Result<CtxValue, Mode::Error> {
        let id = self.tx_context.borrow_mut().fresh_id();
        object_runtime_mut!(self)?
            .new_id(id)
            .map_err(|e| self.env.convert_vm_error(e.finish(Location::Undefined)))?;
        let cap = UpgradeCap::new(id, version_id);
        Ok(CtxValue(Value::upgrade_cap(cap)))
    }

    pub fn upgrade_receipt(
        &self,
        upgrade_ticket: UpgradeTicket,
        upgraded_package_id: ObjectId,
    ) -> CtxValue {
        let receipt = UpgradeReceipt::new(upgrade_ticket, upgraded_package_id);
        CtxValue(Value::upgrade_receipt(receipt))
    }

    /// IOTA bounds the serialized size of a vector built by `MakeMoveVec` from values whose type
    /// has `copy` (`max_ptb_value_size`, scaled down by how much the element type can grow when
    /// deserialized). The size counts the length prefix and the elements up to the one that
    /// exceeds the bound, as the old executor serialized them.
    pub fn check_move_vector_size(
        &self,
        elem_ty: &Type,
        items: &[CtxValue],
    ) -> Result<(), Mode::Error> {
        // Do not cap size for epoch change/genesis
        if Mode::packages_are_predefined() {
            return Ok(());
        }
        let protocol_config = self.env.protocol_config;
        let Some(bound) = protocol_config.max_ptb_value_size_as_option() else {
            return Ok(());
        };
        if protocol_config.max_ptb_value_size_v2() && !elem_ty.abilities().has_copy() {
            return Ok(());
        }
        let amplification = match primitive_serialization_layout::<Mode::Error>(elem_ty)? {
            // No primitive layout: assume the value is of maximal depth.
            None => protocol_config.max_move_value_depth(),
            Some(layout) => primitive_layout_amplification(&layout),
        };
        let Some(max_scaled_size) = bound.checked_div(amplification) else {
            invariant_violation!("amplification must not be zero");
        };
        let layout = self.env.runtime_layout(elem_ty)?;
        let mut prefix = vec![];
        leb128::write::unsigned(&mut prefix, items.len() as u64).map_err(|e| {
            make_invariant_violation!("unable to write the vector length prefix: {e}")
        })?;
        let mut value_size = prefix.len() as u64;
        for item in items {
            let Some(bytes) = item.0.typed_serialize(&layout) else {
                invariant_violation!("unable to serialize a vector element");
            };
            value_size = value_size.saturating_add(bytes.len() as u64);
            if value_size > max_scaled_size {
                return Err(Mode::Error::from_kind(
                    ExecutionErrorKind::MoveVectorElemTooBig {
                        value_size,
                        max_scaled_size,
                    },
                ));
            }
        }
        Ok(())
    }

    /// Counts the struct, enum and vector values in the arguments of a Move call against the
    /// call's limit and the transaction's, as the old executor did. Arguments the executor
    /// supplies (`TxContext`, `AuthContext`) are not counted. Fails the call when either limit
    /// runs out.
    pub fn check_move_call_argument_nodes(
        &mut self,
        arguments: &[(bool, &CtxValue)],
    ) -> Result<(), Mode::Error> {
        let mut call_budget = ArgumentNodeBudget::new(
            self.env
                .protocol_config
                .max_move_call_argument_nodes_as_option(),
            self.gas_charger.is_unmetered(),
        );
        for (counted, value) in arguments {
            if !counted {
                continue;
            }
            // The tighter budget caps the count and is the one an error names; a tie goes to
            // the transaction's.
            let (limit_type, scope, budget) =
                match (call_budget.as_ref(), self.argument_nodes.as_ref()) {
                    (None, None) => return Ok(()),
                    (Some(call), Some(tx)) if call.remaining() < tx.remaining() => {
                        ("call", "one call", call)
                    }
                    (Some(call), None) => ("call", "one call", call),
                    (_, Some(tx)) => ("transaction", "the transaction's calls", tx),
                };
            let (max_nodes, limit) = (budget.remaining(), budget.limit());
            match value.0.count_argument_nodes(max_nodes)? {
                Some(nodes) => {
                    if let Some(call) = &mut call_budget {
                        call.spend(nodes)?;
                    }
                    if let Some(tx) = &mut self.argument_nodes {
                        tx.spend(nodes)?;
                    }
                }
                None => {
                    self.metrics
                        .excessive_move_call_argument_nodes
                        .with_label_values(&[limit_type])
                        .inc();
                    return Err(Mode::Error::new_with_source(
                        ExecutionErrorKind::MovePrimitiveRuntimeError { location: None },
                        format!("arguments of {scope} hold more than {limit} values"),
                    ));
                }
            }
        }
        if let Some(budget) = call_budget {
            self.metrics
                .move_call_argument_nodes
                .observe(budget.used() as f64);
        }
        Ok(())
    }

    // Move calls
    //

    pub fn vm_move_call(
        &mut self,
        function: T::LoadedFunction,
        args: Vec<CtxValue>,
        trace_builder_opt: &mut Option<MoveTraceBuilder>,
    ) -> Result<Vec<CtxValue>, Mode::Error> {
        with_vm!(self, &function.linkage, Mode::Error, |vm: &mut MoveVM<
            'env,
        >| {
            let ty_args = function
                .type_arguments
                .iter()
                .map(|ty| {
                    let tag: TypeTag = ty.clone().try_into().map_err(|e| {
                        Mode::Error::new_with_source(ExecutionErrorKind::VmInvariantViolation, e)
                    })?;
                    vm.load_type(&tag)
                        .map_err(|e| self.env.convert_linked_vm_error(e, &function.linkage))
                })
                .collect::<Result<Vec<_>, Mode::Error>>()?;
            let result = self.execute_function_bypass_visibility_with_vm(
                vm,
                &function.original_mid,
                &function.name,
                ty_args,
                args,
                &function.linkage,
                trace_builder_opt,
            )?;
            self.take_user_events(
                vm,
                function.version_mid,
                function.definition_index,
                function.instruction_length,
                &function.linkage,
            )?;
            Ok::<Vec<CtxValue>, Mode::Error>(result)
        })
    }

    fn execute_function_bypass_visibility_with_vm(
        &mut self,
        vm: &mut MoveVM<'env>,
        original_mid: &ModuleId,
        function_name: &IdentStr,
        ty_args: Vec<VMType>,
        args: Vec<CtxValue>,
        linkage: &ExecutableLinkage,
        tracer: &mut Option<MoveTraceBuilder>,
    ) -> Result<Vec<CtxValue>, Mode::Error> {
        let gas_status = self.gas_charger.move_gas_status_mut();
        let values = vm
            .execute_function_bypass_visibility(
                original_mid,
                function_name,
                ty_args,
                args.into_iter().map(|v| v.0.into()).collect(),
                &mut IotaGasMeter(gas_status),
                tracer.as_mut(),
            )
            .map_err(|e| self.env.convert_linked_vm_error(e, linkage))?;
        Ok(values.into_iter().map(|v| CtxValue(v.into())).collect())
    }

    // Publish and Upgrade
    //

    pub fn deserialize_package(
        &mut self,
        package_payload: PackagePayload,
        dep_ids: &[ObjectId],
    ) -> Result<DeserializedPackage, Mode::Error> {
        Ok(match package_payload {
            PackagePayload::Deserialized(deserialized_pkg) => deserialized_pkg,
            PackagePayload::Serialized(module_bytes) => {
                // This assertion is also checked in the call to `deserialize_modules`, but we
                // want to check it here first to keep existing behavior around checking this
                // invariant before the charge on pre-existing pathways.
                assert_invariant!(
                    !module_bytes.is_empty(),
                    "empty package is checked in transaction input checker"
                );
                let total_bytes = module_bytes.iter().map(|v| v.len()).sum();
                self.gas_charger.charge_publish_package(total_bytes)?;
                self.env.deserialize_package(&module_bytes, dep_ids)?
            }
        })
    }

    fn fetch_package(&mut self, dependency_id: &ObjectId) -> Result<Rc<MovePackage>, Mode::Error> {
        let [fetched_package] = self.fetch_packages(&[*dependency_id])?.try_into().map_err(
            |_| {
                make_invariant_violation!(
                    "We should always fetch a single package for each object or return a dependency error."
                )
            },
        )?;
        Ok(fetched_package)
    }

    fn fetch_packages(
        &mut self,
        dependency_ids: &[ObjectId],
    ) -> Result<Vec<Rc<MovePackage>>, Mode::Error> {
        let mut fetched = vec![];
        let mut missing = vec![];

        // Collect into a set to avoid duplicate fetches and preserve existing behavior
        let dependency_ids: BTreeSet<_> = dependency_ids.iter().collect();

        for id in &dependency_ids {
            match self.env.linkable_store.get_move_package(id) {
                Err(e) => {
                    return Err(Mode::Error::new_with_source(
                        ExecutionErrorKind::PublishUpgradeMissingDependency,
                        e,
                    ));
                }
                Ok(Some(inner)) => {
                    fetched.push(inner);
                }
                Ok(None) => {
                    missing.push(*id);
                }
            }
        }

        if missing.is_empty() {
            assert_invariant!(
                fetched.len() == dependency_ids.len(),
                "all dependencies should be fetched"
            );
            Ok(fetched)
        } else {
            let msg = format!(
                "Missing dependencies: {}",
                missing
                    .into_iter()
                    .map(|dep| format!("{dep}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            Err(Mode::Error::new_with_source(
                ExecutionErrorKind::PublishUpgradeMissingDependency,
                msg,
            ))
        }
    }

    fn publish_and_verify_modules(
        &mut self,
        package_id: ObjectId,
        pkg: &MovePackage,
        modules: &[CompiledModule],
        linkage: &ExecutableLinkage,
    ) -> Result<(VerifiedPackage, MoveVM<'env>), Mode::Error> {
        let serialized_pkg = serialized_package(pkg).map_err(|e| {
            make_invariant_violation!("Failed to serialize package for verification: {}", e)
        })?;
        let data_store = &self.env.linkable_store.package_store;
        let vm = self
            .env
            .vm
            .validate_package(
                data_store,
                package_id.to_account_address(),
                serialized_pkg,
                &mut IotaGasMeter(self.gas_charger.move_gas_status_mut()),
                self.native_extensions.clone(),
            )
            .map_err(|e| self.env.convert_linked_vm_error(e, linkage))?;

        // run the IOTA verifier
        for module in modules {
            // Run IOTA bytecode verifier, which runs some additional checks that assume the Move
            // bytecode verifier has passed.
            iota_verifier::verifier::iota_verify_module_unmetered(
                module,
                &BTreeMap::new(),
                &ProtocolBuildConfig::from(self.env.protocol_config),
            )?;
        }

        Ok(vm)
    }

    // Here we optimistically push the package that is being published/upgraded
    // and if there is an error of any kind (verification or module init) we
    // remove it.
    // The call to `pop_last_package` later is fine because we cannot re-enter and
    // the last package we pushed is the one we are verifying and running the init from
    fn push_package_and_init_selected_modules<'a>(
        &mut self,
        package_id: ObjectId,
        package: MovePackage,
        verified_pkg: VerifiedPackage,
        vm: MoveVM<'env>,
        modules: impl IntoIterator<Item = &'a CompiledModule>,
        expected_inits: BTreeSet<Identifier>,
        linkage: &ExecutableLinkage,
        trace_builder_opt: &mut Option<MoveTraceBuilder>,
        expected_stack_height: u64,
    ) -> Result<(), Mode::Error> {
        self.env.linkable_store.package_store.push_package(
            package_id,
            Rc::new(package),
            verified_pkg,
        )?;

        match self.init_selected_modules(
            vm,
            package_id,
            modules,
            expected_inits,
            linkage,
            trace_builder_opt,
            expected_stack_height,
        ) {
            Ok(()) => Ok(()),
            Err(e) => {
                self.env
                    .linkable_store
                    .package_store
                    .pop_package(package_id)?;
                Err(e)
            }
        }
    }

    fn init_selected_modules<'a>(
        &mut self,
        mut vm: MoveVM<'env>,
        package_id: ObjectId,
        modules: impl IntoIterator<Item = &'a CompiledModule>,
        mut expected_inits: BTreeSet<Identifier>,
        linkage: &ExecutableLinkage,
        trace_builder_opt: &mut Option<MoveTraceBuilder>,
        expected_stack_height: u64,
    ) -> Result<(), Mode::Error> {
        debug_assert!(
            !crate::compat::CHARGE_PTB_VALUE_OPERATIONS
                || self.gas_charger.move_gas_status().stack_height_current()
                    == expected_stack_height
        );
        let check_expected_inits = true;
        for module in modules {
            let Some((fdef_idx, fdef)) = module.find_function_def_by_name(INIT_FN_NAME.as_str())
            else {
                continue;
            };
            let module_name = module.identifier_at(module.self_handle().name);
            let fhandle = module.function_handle_at(fdef.function);
            let fparameters = module.signature_at(fhandle.parameters);
            assert_invariant!(
                fparameters.0.len() <= 2,
                "init function should have at most 2 parameters"
            );
            let has_otw = fparameters.0.len() == 2;
            let tx_context = self
                .location(UsageKind::Borrow, T::Location::TxContext)
                .map_err(|e| {
                    make_invariant_violation!("Failed to get tx context for init function: {}", e)
                })?;
            // balance the stack after borrowing the tx context
            charge_gas!(self, charge_store_loc, &tx_context)?;

            if check_expected_inits {
                assert_invariant!(
                    expected_inits.remove(module_name),
                    "module {module_name} defines an `init` but was not recorded as doing so when \
                     the package payload was deserialized"
                );
            }

            let args = if has_otw {
                vec![CtxValue(Value::one_time_witness()?), CtxValue(tx_context)]
            } else {
                vec![CtxValue(tx_context)]
            };
            debug_assert!(
                !crate::compat::CHARGE_PTB_VALUE_OPERATIONS
                    || self.gas_charger.move_gas_status().stack_height_current()
                        == expected_stack_height
            );
            trace_utils::trace_move_call_start(trace_builder_opt);
            let return_values = self.execute_function_bypass_visibility_with_vm(
                &mut vm,
                &module.self_id(),
                INIT_FN_NAME,
                vec![],
                args,
                linkage,
                trace_builder_opt,
            )?;
            trace_utils::trace_move_call_end(trace_builder_opt);

            let version_mid = ModuleId::new(
                package_id.to_account_address(),
                module.self_id().name().to_owned(),
            );
            self.take_user_events(
                &vm,
                version_mid,
                fdef_idx,
                fdef.code
                    .as_ref()
                    .map(|c| checked_as!(c.code.len(), u16))
                    .transpose()?
                    .unwrap_or(0),
                linkage,
            )?;
            assert_invariant!(
                return_values.is_empty(),
                "init should not have return values"
            );
            debug_assert!(
                !crate::compat::CHARGE_PTB_VALUE_OPERATIONS
                    || self.gas_charger.move_gas_status().stack_height_current()
                        == expected_stack_height
            );
        }

        // Every module we expected to initialize from earlier was initialized.
        assert_invariant!(
            !check_expected_inits || expected_inits.is_empty(),
            "modules {expected_inits:?} define an `init` that was never run"
        );

        Ok(())
    }

    pub fn publish_and_init_package(
        &mut self,
        package_payload: DeserializedPackage,
        dep_ids: &[ObjectId],
        linkage: ResolvedLinkage,
        trace_builder_opt: &mut Option<MoveTraceBuilder>,
    ) -> Result<ObjectId, Mode::Error> {
        let DeserializedPackage {
            deserialized_modules: mut modules,
            modules_with_init,
            ..
        } = package_payload;
        let original_id = if Mode::packages_are_predefined() {
            // do not calculate or substitute id for predefined packages
            modules.safe_get(0)?.self_id().address().to_object_id()
        } else {
            // It should be fine that this does not go through the object runtime since it does not
            // need to know about new packages created, since Move objects and Move packages
            // cannot interact
            let id = self.tx_context.borrow_mut().fresh_id();
            adapter::substitute_package_id(&mut modules, id)?;
            id
        };

        let dependencies = self.fetch_packages(dep_ids)?;
        let package = MovePackage::new_initial(
            &modules,
            self.env.protocol_config,
            dependencies.iter().map(|p| p.as_ref()),
        )?;
        let package_id = package.id();
        let package_version = package.version().as_u64();

        let linkage = ResolvedLinkage::update_for_publication(package_id, original_id, linkage);

        let (pkg, vm) =
            self.publish_and_verify_modules(original_id, &package, &modules, &linkage)?;
        self.push_package_and_init_selected_modules(
            package_id,
            package,
            pkg,
            vm,
            &modules,
            modules_with_init,
            &linkage,
            trace_builder_opt,
            PUBLISH_INIT_EXPECTED_STACK_HEIGHT,
        )?;
        // Genesis packages are predefined and get no package metadata.
        if !Mode::packages_are_predefined() {
            self.create_package_metadata_if_present(
                &modules,
                package_id,
                original_id,
                package_version,
                trace_builder_opt,
            )?;
        }
        Ok(original_id)
    }

    pub fn upgrade(
        &mut self,
        package_payload: DeserializedPackage,
        dep_ids: &[ObjectId],
        current_package_id: ObjectId,
        upgrade_ticket_policy: u8,
        linkage: ResolvedLinkage,
        trace_builder_opt: &mut Option<MoveTraceBuilder>,
    ) -> Result<ObjectId, Mode::Error> {
        let DeserializedPackage {
            deserialized_modules: mut modules,
            ..
        } = package_payload;
        // Check that this package ID points to a package and get the package we're upgrading.
        let current_move_package = self.fetch_package(&current_package_id)?;

        let original_id = current_move_package.original_package_id();
        adapter::substitute_package_id(&mut modules, original_id)?;

        // Upgraded packages share their predecessor's runtime ID but get a new storage ID.
        // It should be fine that this does not go through the object runtime since it does not
        // need to know about new packages created, since Move objects and Move packages
        // cannot interact
        let version_id = self.tx_context.borrow_mut().fresh_id();

        let dependencies = self.fetch_packages(dep_ids)?;
        let package = current_move_package.new_upgraded(
            version_id,
            &modules,
            self.env.protocol_config,
            dependencies.iter().map(|p| p.as_ref()),
        )?;

        let linkage = ResolvedLinkage::update_for_publication(version_id, original_id, linkage);
        let (verified_pkg, _vm) =
            self.publish_and_verify_modules(original_id, &package, &modules, &linkage)?;

        check_compatibility(
            self.env.protocol_config,
            current_move_package.as_ref(),
            &modules,
            upgrade_ticket_policy,
        )?;

        let package_version = package.version().as_u64();
        // `init` functions are not called on upgrade, not even for newly added modules.
        self.env.linkable_store.package_store.push_package(
            version_id,
            Rc::new(package),
            verified_pkg,
        )?;
        self.create_package_metadata_if_present(
            &modules,
            version_id,
            original_id,
            package_version,
            trace_builder_opt,
        )?;

        Ok(version_id)
    }

    /// Creates and freezes the package metadata object of a published or upgraded package when
    /// one of its modules carries metadata that is recorded on chain.
    fn create_package_metadata_if_present(
        &mut self,
        modules: &[CompiledModule],
        storage_id: ObjectId,
        runtime_id: ObjectId,
        package_version: u64,
        trace_builder_opt: &mut Option<MoveTraceBuilder>,
    ) -> Result<(), Mode::Error> {
        if !self.env.protocol_config.publish_package_metadata() {
            return Ok(());
        }
        let Some(creation) = package_metadata::package_metadata_creation(
            self.env.protocol_config,
            modules,
            storage_id,
            runtime_id,
            package_version,
        )?
        else {
            return Ok(());
        };
        match creation {
            PackageMetadataCreation::Inline { id, metadata } => {
                object_runtime_mut!(self)?
                    .new_id(id)
                    .map_err(|e| self.env.convert_vm_error(e.finish(Location::Undefined)))?;
                let ty = self
                    .env
                    .load_type_from_struct(&struct_tag_sdk_to_core(&metadata.struct_tag()))?;
                let value = Value::deserialize(self.env, &metadata.to_bcs_bytes(), ty.clone())?;
                self.transfer_object(Owner::Immutable, ty, CtxValue(value))
            }
            PackageMetadataCreation::DynamicFields(args) => {
                let function = self.env.load_function(
                    IOTA_FRAMEWORK_PACKAGE_ID,
                    PACKAGE_METADATA_MODULE_NAME.to_string(),
                    CREATE_PACKAGE_METADATA_V1_WITH_DYNAMIC_METADATA_FN_NAME.to_string(),
                    vec![],
                )?;
                assert_invariant!(
                    function.signature.parameters.len() == args.len(),
                    "package metadata constructor parameter count mismatch"
                );
                let args = function
                    .signature
                    .parameters
                    .iter()
                    .zip(args)
                    .map(|(ty, bytes)| {
                        Value::deserialize(self.env, &bytes, ty.clone()).map(CtxValue)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let return_values = self.vm_move_call(function, args, trace_builder_opt)?;
                assert_invariant!(
                    return_values.is_empty(),
                    "package metadata constructor should not have return values"
                );
                Ok(())
            }
        }
    }

    // Commands
    //

    pub fn transfer_object(
        &mut self,
        recipient: Owner,
        ty: Type,
        object: CtxValue,
    ) -> Result<(), Mode::Error> {
        self.transfer_object_(recipient, ty, object, /* end of transaction */ false)
    }

    fn transfer_object_(
        &mut self,
        recipient: Owner,
        ty: Type,
        object: CtxValue,
        end_of_transaction: bool,
    ) -> Result<(), Mode::Error> {
        let tag = TypeTag::try_from(ty)
            .map_err(|_| make_invariant_violation!("Unable to convert Type to TypeTag"))?;
        let TypeTag::Struct(tag) = tag else {
            invariant_violation!("Expected struct type tag");
        };
        let ty = move_object_type(*tag);
        object_runtime_mut!(self)?
            .transfer(recipient, ty, object.0.into(), end_of_transaction)
            .map_err(|e| self.env.convert_vm_error(e.finish(Location::Undefined)))?;
        Ok(())
    }

    // Dev Inspect tracking
    //

    #[allow(clippy::type_complexity)]
    pub fn argument_updates(
        &mut self,
        args: Vec<T::Argument>,
    ) -> Result<Vec<(Argument, Vec<u8>, TypeTag)>, Mode::Error> {
        args.into_iter()
            .filter_map(|arg| self.argument_update(arg).transpose())
            .collect()
    }

    #[allow(clippy::type_complexity)]
    fn argument_update(
        &mut self,
        sp!(_, (arg, ty)): T::Argument,
    ) -> Result<Option<(Argument, Vec<u8>, TypeTag)>, Mode::Error> {
        use Argument as TxArgument;
        let ty = match ty {
            Type::Reference(true, inner) => (*inner).clone(),
            ty => {
                debug_assert!(
                    false,
                    "Unexpected non reference type in location update: {ty:?}"
                );
                return Ok(None);
            }
        };
        let Ok(tag): Result<TypeTag, _> = ty.clone().try_into() else {
            invariant_violation!("unable to generate type tag from type")
        };
        let location = arg.location();
        let resolved = self.locations.resolve(location)?;
        let local = match resolved {
            ResolvedLocation::Local(local)
            | ResolvedLocation::Pure { local, .. }
            | ResolvedLocation::Receiving { local, .. } => local,
        };
        if local.is_invalid()? {
            return Ok(None);
        }
        // copy the value from the local
        let value = local.copy()?;
        let value = match arg {
            T::Argument__::Use(_) => {
                // dereference the reference
                value.read_ref()?
            }
            T::Argument__::Borrow(_, _) => {
                // value is not a reference, nothing to do
                value
            }
            T::Argument__::Freeze(_) => {
                invariant_violation!("freeze should not be used for a mutable reference")
            }
            T::Argument__::Read(_) => {
                invariant_violation!("read should not return a reference")
            }
        };
        let layout = self.env.runtime_layout(&ty)?;
        let Some(bytes) = value.typed_serialize(&layout) else {
            invariant_violation!("Failed to serialize Move value");
        };
        let arg = match location {
            T::Location::TxContext | T::Location::AuthContext => return Ok(None),
            T::Location::GasCoin => TxArgument::Gas,
            T::Location::Result(i, j) => TxArgument::NestedResult(i, j),
            T::Location::ObjectInput(i) => TxArgument::Input(
                self.locations
                    .input_object_metadata
                    .safe_get(i as usize)?
                    .0
                    .0,
            ),
            T::Location::PureInput(i) => TxArgument::Input(
                self.locations
                    .pure_input_metadata
                    .safe_get(i as usize)?
                    .original_input_index
                    .0,
            ),
            T::Location::ReceivingInput(i) => TxArgument::Input(
                self.locations
                    .receiving_input_metadata
                    .safe_get(i as usize)?
                    .original_input_index
                    .0,
            ),
        };
        Ok(Some((arg, bytes, tag)))
    }

    pub fn tracked_results(
        &self,
        results: &[CtxValue],
        result_tys: &T::ResultType,
    ) -> Result<Vec<(Vec<u8>, TypeTag)>, Mode::Error> {
        assert_invariant!(
            results.len() == result_tys.len(),
            "results and result types should match"
        );
        results
            .iter()
            .zip_debug_eq(result_tys)
            .map(|(v, ty)| self.tracked_result(&v.0, ty.clone()))
            .collect()
    }

    fn tracked_result(&self, result: &Value, ty: Type) -> Result<(Vec<u8>, TypeTag), Mode::Error> {
        let inner_value;
        let (v, ty) = match ty {
            Type::Reference(_, inner) => {
                inner_value = result.copy()?.read_ref()?;
                (&inner_value, (*inner).clone())
            }
            _ => (result, ty),
        };
        let layout = self.env.runtime_layout(&ty)?;
        let Some(bytes) = v.typed_serialize(&layout) else {
            invariant_violation!("Failed to serialize Move value");
        };
        let Ok(tag): Result<TypeTag, _> = ty.try_into() else {
            invariant_violation!("unable to generate type tag from type")
        };
        Ok((bytes, tag))
    }
}

impl VMValueCast<CtxValue> for VMValue {
    fn cast(self) -> Result<CtxValue, PartialVMError> {
        Ok(CtxValue(self.into()))
    }
}

impl CtxValue {
    pub fn vec_pack(ty: Type, values: Vec<CtxValue>) -> Result<CtxValue, ExecutionError> {
        Ok(CtxValue(Value::vec_pack(
            ty,
            values.into_iter().map(|v| v.0).collect(),
        )?))
    }

    pub fn coin_ref_value(self) -> Result<u64, ExecutionError> {
        self.0.coin_ref_value()
    }

    pub fn coin_ref_subtract_balance(self, amount: u64) -> Result<(), ExecutionError> {
        self.0.coin_ref_subtract_balance(amount)
    }

    pub fn coin_ref_add_balance(self, amount: u64) -> Result<(), ExecutionError> {
        self.0.coin_ref_add_balance(amount)
    }

    pub fn into_upgrade_ticket(self) -> Result<UpgradeTicket, ExecutionError> {
        self.0.into_upgrade_ticket()
    }

    pub fn to_address(&self) -> Result<AccountAddress, ExecutionError> {
        self.0.copy()?.cast()
    }

    /// Used to get access the inner Value for tracing.
    pub(super) fn inner_for_tracing(&self) -> &Value {
        &self.0
    }
}

fn load_object_arg<Mode: ExecutionMode>(
    meter: &mut GasCharger,
    env: &Env<Mode>,
    input_object_map: &mut BTreeMap<ObjectId, object_runtime::InputObject>,
    input: T::ObjectInput,
) -> Result<(T::InputIndex, InputObjectMetadata, Value), Mode::Error> {
    let id = input.arg.id();
    let refined_permissions = input.arg.refined_permissions;
    let (metadata, value) = load_object_arg_impl(
        meter,
        env,
        input_object_map,
        id,
        refined_permissions,
        input.ty,
    )?;
    Ok((input.original_input_index, metadata, value))
}

fn load_object_arg_impl<Mode: ExecutionMode>(
    meter: &mut GasCharger,
    env: &Env<Mode>,
    input_object_map: &mut BTreeMap<ObjectId, object_runtime::InputObject>,
    id: ObjectId,
    refined_permissions: ObjectPermissions,
    ty: T::Type,
) -> Result<(InputObjectMetadata, Value), Mode::Error> {
    let obj = env.read_object(&id)?;
    let owner = obj.owner;
    let version = obj.version();
    let object_metadata = InputObjectMetadata {
        newly_created: false,
        id,
        refined_permissions,
        owner,
        version,
        type_: ty.clone(),
    };
    let Some(move_obj) = try_as_move(&obj.data) else {
        invariant_violation!("Expected a Move object");
    };
    assert_expected_move_object_type(&object_metadata.type_, move_obj.object_type())?;
    let contained_uids = {
        let fully_annotated_layout = env.fully_annotated_layout(&ty)?;
        get_all_uids(&fully_annotated_layout, move_obj.contents()).map_err(|e| {
            make_invariant_violation!("Unable to retrieve UIDs for object. Got error: {e}")
        })?
    };
    input_object_map.insert(
        id,
        object_runtime::InputObject {
            contained_uids,
            version,
            owner,
        },
    );

    let v = Value::deserialize(env, move_obj.contents(), ty)?;
    charge_gas_!(meter, env, charge_copy_loc, &v)?;
    charge_gas_!(meter, env, charge_store_loc, &v)?;
    Ok((object_metadata, v))
}

fn load_pure_value<Mode: ExecutionMode>(
    meter: &mut GasCharger,
    env: &Env<Mode>,
    bytes: &[u8],
    metadata: &T::PureInput,
) -> Result<Value, Mode::Error> {
    let loaded = Value::deserialize(env, bytes, metadata.ty.clone())?;
    // ByteValue::Receiving { id, version } => Value::receiving(*id, *version),
    charge_gas_!(meter, env, charge_copy_loc, &loaded)?;
    charge_gas_!(meter, env, charge_store_loc, &loaded)?;
    Ok(loaded)
}

fn load_receiving_value<Mode: ExecutionMode>(
    meter: &mut GasCharger,
    env: &Env<Mode>,
    metadata: &T::ReceivingInput,
) -> Result<Value, Mode::Error> {
    let id = *metadata.object_ref.object_id();
    let version = metadata.object_ref.version();
    let loaded = Value::receiving(id, version);
    charge_gas_!(meter, env, charge_copy_loc, &loaded)?;
    charge_gas_!(meter, env, charge_store_loc, &loaded)?;
    Ok(loaded)
}

fn copy_value<Mode: ExecutionMode>(
    meter: &mut GasCharger,
    env: &Env<Mode>,
    value: &Value,
) -> Result<Value, Mode::Error> {
    charge_gas_!(meter, env, charge_copy_loc, value)?;
    charge_gas_!(meter, env, charge_pop, value)?;
    Ok(value.copy()?)
}

/// The max budget was deducted from the gas coin at the beginning of the transaction,
/// now we return exactly that amount. Gas will be charged by the execution engine.
fn refund_max_gas_budget<OType>(
    writes: &mut IndexMap<ObjectId, (Owner, OType, VMValue)>,
    gas_charger: &mut GasCharger,
    gas_id: ObjectId,
) -> Result<(), ExecutionError> {
    let Some((_, _, value_ref)) = writes.get_mut(&gas_id) else {
        invariant_violation!("Gas object cannot be wrapped or destroyed")
    };
    // replace with dummy value
    let value = std::mem::replace(value_ref, VMValue::u8(0));
    let mut locals = Locals::new([Some(value.into())])?;
    let mut local = locals.local(0)?;
    let coin_value = local.borrow()?.coin_ref_value()?;
    let additional = gas_charger.gas_budget();
    if coin_value.checked_add(additional).is_none() {
        return Err(ExecutionError::new_with_source(
            ExecutionErrorKind::CoinBalanceOverflow,
            "Gas coin too large after returning the max gas budget",
        ));
    };
    local.borrow()?.coin_ref_add_balance(additional)?;
    // put the value back
    *value_ref = local.move_()?.into();
    Ok(())
}

/// Returns the max gas budget to the gas coin. The coin stays the charge location even when it
/// was transferred, since gas is charged to the smashed gas coin.
fn finish_gas_coin<OType>(
    gas_charger: &mut GasCharger,
    writes: &mut IndexMap<ObjectId, (Owner, OType, VMValue)>,
    gas_id: ObjectId,
) -> Result<(), ExecutionError> {
    refund_max_gas_budget(writes, gas_charger, gas_id)
}

/// Generate an MoveStruct given an updated/written object
/// # Safety
///
/// This function assumes proper generation of has_public_transfer, either from the abilities of
/// the StructTag, or from the runtime correctly propagating from the inputs
unsafe fn create_written_object<Mode: ExecutionMode>(
    env: &Env<Mode>,
    objects_modified_at: &BTreeMap<ObjectId, LoadedRuntimeObject>,
    id: ObjectId,
    type_: Type,
    contents: Vec<u8>,
) -> Result<MoveStruct, ExecutionError> {
    debug_assert_eq!(
        Some(id),
        contents
            .get(..ObjectId::LENGTH)
            .and_then(|bytes| ObjectId::from_bytes(bytes).ok()),
        "object contents should start with the object id"
    );
    let old_obj_ver = objects_modified_at
        .get(&id)
        .map(|obj: &LoadedRuntimeObject| obj.version);

    let Ok(type_tag): Result<TypeTag, _> = type_.try_into() else {
        invariant_violation!("unable to generate type tag from type")
    };

    let struct_tag = match type_tag {
        TypeTag::Struct(inner) => *inner,
        _ => invariant_violation!("Non struct type for object"),
    };
    MoveStruct::new_from_execution(
        struct_tag_core_to_sdk(&struct_tag),
        old_obj_ver.unwrap_or(Version::from_u64(0)),
        contents,
        env.protocol_config,
        Mode::packages_are_predefined(),
    )
}

/// substitutes the type arguments into the parameter and return types
pub fn subst_signature(
    signature: LoadedFunctionInformation,
    type_arguments: &[VMType],
) -> VMResult<LoadedFunctionInformation> {
    let LoadedFunctionInformation {
        parameters,
        return_,
        is_entry,
        is_native,
        visibility,
        index,
        instruction_count,
    } = signature;
    let parameters = parameters
        .into_iter()
        .map(|ty| ty.subst(type_arguments))
        .collect::<PartialVMResult<Vec<_>>>()
        .map_err(|err| err.finish(Location::Undefined))?;
    let return_ = return_
        .into_iter()
        .map(|ty| ty.subst(type_arguments))
        .collect::<PartialVMResult<Vec<_>>>()
        .map_err(|err| err.finish(Location::Undefined))?;
    Ok(LoadedFunctionInformation {
        parameters,
        return_,
        is_entry,
        is_native,
        visibility,
        index,
        instruction_count,
    })
}

pub enum EitherError<E: ExecutionErrorTrait = ExecutionError> {
    CommandArgument(CommandArgumentError),
    Execution(E),
}

impl<E: ExecutionErrorTrait> From<ExecutionError> for EitherError<E> {
    fn from(e: ExecutionError) -> Self {
        EitherError::Execution(e.into())
    }
}

impl<E: ExecutionErrorTrait> From<CommandArgumentError> for EitherError<E> {
    fn from(e: CommandArgumentError) -> Self {
        EitherError::CommandArgument(e)
    }
}

impl<E: ExecutionErrorTrait> EitherError<E> {
    pub fn into_execution_error(self, command_index: usize) -> E {
        match self {
            EitherError::CommandArgument(e) => command_argument_error(e, command_index).into(),
            EitherError::Execution(e) => e,
        }
    }
}

// ************************************************************************************************
// * Special serialization formats
// ************************************************************************************************

/// How much a value with the given primitive layout can grow per serialized byte when it is
/// deserialized, as the old executor bounded PTB values.
fn primitive_layout_amplification(layout: &PrimitiveArgumentLayout) -> u64 {
    use PrimitiveArgumentLayout as PAL;
    match layout {
        PAL::Option(inner) => primitive_layout_amplification(inner).saturating_add(1),
        PAL::Vector(inner) => primitive_layout_amplification(inner),
        PAL::Ascii | PAL::UTF8 => 2,
        PAL::Bool | PAL::U8 | PAL::U16 | PAL::U32 | PAL::U64 => 1,
        PAL::U128 | PAL::U256 | PAL::Address => 2,
    }
}

/// Special enum for values that need additional validation, in other words
/// There is validation to do on top of the BCS layout. Currently only needed for
/// strings
#[derive(Debug)]
pub enum PrimitiveArgumentLayout {
    /// An option
    Option(Box<PrimitiveArgumentLayout>),
    /// A vector
    Vector(Box<PrimitiveArgumentLayout>),
    /// An ASCII encoded string
    Ascii,
    /// A UTF8 encoded string
    UTF8,
    // needed for Option validation
    Bool,
    U8,
    U16,
    U32,
    U64,
    U128,
    U256,
    Address,
}

impl PrimitiveArgumentLayout {
    /// returns true iff all BCS compatible bytes are actually values for this type.
    /// For example, this function returns false for Option and Strings since they need additional
    /// validation.
    pub fn bcs_only(&self) -> bool {
        match self {
            // have additional restrictions past BCS
            PrimitiveArgumentLayout::Option(_)
            | PrimitiveArgumentLayout::Ascii
            | PrimitiveArgumentLayout::UTF8 => false,
            // Move primitives are BCS compatible and do not need additional validation
            PrimitiveArgumentLayout::Bool
            | PrimitiveArgumentLayout::U8
            | PrimitiveArgumentLayout::U16
            | PrimitiveArgumentLayout::U32
            | PrimitiveArgumentLayout::U64
            | PrimitiveArgumentLayout::U128
            | PrimitiveArgumentLayout::U256
            | PrimitiveArgumentLayout::Address => true,
            // vector only needs validation if it's inner type does
            PrimitiveArgumentLayout::Vector(inner) => inner.bcs_only(),
        }
    }
}

/// Checks the bytes against the `SpecialArgumentLayout` using `bcs`. It does not actually generate
/// the deserialized value, only walks the bytes. While not necessary if the layout does not contain
/// special arguments (e.g. Option or String) we check the BCS bytes for predictability
pub fn bcs_argument_validate(
    bytes: &[u8],
    idx: u16,
    layout: PrimitiveArgumentLayout,
) -> Result<(), ExecutionError> {
    bcs::from_bytes_seed(&layout, bytes).map_err(|_| {
        ExecutionError::new_with_source(
            ExecutionErrorKind::command_argument_error(CommandArgumentError::InvalidBcsBytes, idx),
            format!("Function expects {layout} but provided argument's value does not match",),
        )
    })
}

impl<'d> serde::de::DeserializeSeed<'d> for &PrimitiveArgumentLayout {
    type Value = ();
    fn deserialize<D: serde::de::Deserializer<'d>>(
        self,
        deserializer: D,
    ) -> Result<Self::Value, D::Error> {
        use serde::de::Error;
        match self {
            PrimitiveArgumentLayout::Ascii => {
                let s: &str = serde::Deserialize::deserialize(deserializer)?;
                if !s.is_ascii() {
                    Err(D::Error::custom("not an ascii string"))
                } else {
                    Ok(())
                }
            }
            PrimitiveArgumentLayout::UTF8 => {
                deserializer.deserialize_string(serde::de::IgnoredAny)?;
                Ok(())
            }
            PrimitiveArgumentLayout::Option(layout) => {
                deserializer.deserialize_option(OptionElementVisitor(layout))
            }
            PrimitiveArgumentLayout::Vector(layout) => {
                deserializer.deserialize_seq(VectorElementVisitor(layout))
            }
            // primitive move value cases, which are hit to make sure the correct number of bytes
            // are removed for elements of an option/vector
            PrimitiveArgumentLayout::Bool => {
                deserializer.deserialize_bool(serde::de::IgnoredAny)?;
                Ok(())
            }
            PrimitiveArgumentLayout::U8 => {
                deserializer.deserialize_u8(serde::de::IgnoredAny)?;
                Ok(())
            }
            PrimitiveArgumentLayout::U16 => {
                deserializer.deserialize_u16(serde::de::IgnoredAny)?;
                Ok(())
            }
            PrimitiveArgumentLayout::U32 => {
                deserializer.deserialize_u32(serde::de::IgnoredAny)?;
                Ok(())
            }
            PrimitiveArgumentLayout::U64 => {
                deserializer.deserialize_u64(serde::de::IgnoredAny)?;
                Ok(())
            }
            PrimitiveArgumentLayout::U128 => {
                deserializer.deserialize_u128(serde::de::IgnoredAny)?;
                Ok(())
            }
            PrimitiveArgumentLayout::U256 => {
                U256::deserialize(deserializer)?;
                Ok(())
            }
            PrimitiveArgumentLayout::Address => {
                Address::deserialize(deserializer)?;
                Ok(())
            }
        }
    }
}

struct VectorElementVisitor<'a>(&'a PrimitiveArgumentLayout);

impl<'d> serde::de::Visitor<'d> for VectorElementVisitor<'_> {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Vector")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::SeqAccess<'d>,
    {
        while seq.next_element_seed(self.0)?.is_some() {}
        Ok(())
    }
}

struct OptionElementVisitor<'a>(&'a PrimitiveArgumentLayout);

impl<'d> serde::de::Visitor<'d> for OptionElementVisitor<'_> {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Option")
    }

    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(())
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'d>,
    {
        self.0.deserialize(deserializer)
    }
}

impl fmt::Display for PrimitiveArgumentLayout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PrimitiveArgumentLayout::Vector(inner) => {
                write!(f, "vector<{inner}>")
            }
            PrimitiveArgumentLayout::Option(inner) => {
                write!(f, "std::option::Option<{inner}>")
            }
            PrimitiveArgumentLayout::Ascii => {
                write!(f, "std::{}::{}", RESOLVED_ASCII_STR.1, RESOLVED_ASCII_STR.2)
            }
            PrimitiveArgumentLayout::UTF8 => {
                write!(f, "std::{}::{}", RESOLVED_UTF8_STR.1, RESOLVED_UTF8_STR.2)
            }
            PrimitiveArgumentLayout::Bool => write!(f, "bool"),
            PrimitiveArgumentLayout::U8 => write!(f, "u8"),
            PrimitiveArgumentLayout::U16 => write!(f, "u16"),
            PrimitiveArgumentLayout::U32 => write!(f, "u32"),
            PrimitiveArgumentLayout::U64 => write!(f, "u64"),
            PrimitiveArgumentLayout::U128 => write!(f, "u128"),
            PrimitiveArgumentLayout::U256 => write!(f, "u256"),
            PrimitiveArgumentLayout::Address => write!(f, "address"),
        }
    }
}

pub fn finish(
    protocol_config: &ProtocolConfig,
    state_view: &dyn ExecutionState,
    gas_charger: &mut GasCharger,
    tx_context: &TxContext,
    by_value_shared_objects: &BTreeSet<ObjectId>,
    loaded_runtime_objects: BTreeMap<ObjectId, LoadedRuntimeObject>,
    written_objects: BTreeMap<ObjectId, Object>,
    created_object_ids: IndexSet<ObjectId>,
    deleted_object_ids: IndexSet<ObjectId>,
    user_events: Vec<(ModuleId, StructTag, Vec<u8>)>,
    received_coin_types: BTreeSet<String>,
) -> Result<ExecutionResults, ExecutionError> {
    // Before finishing, ensure that any shared object taken by value by the transaction is either:
    // 1. Mutated (and still has a shared ownership); or
    // 2. Deleted.
    // Otherwise, the shared object operation is not allowed and we fail the transaction.
    for id in by_value_shared_objects {
        // If it's been written it must have been reshared so must still have an ownership
        // of `Shared`.
        if let Some(obj) = written_objects.get(id) {
            if !obj.is_shared() {
                return Err(ExecutionError::new(
                    ExecutionErrorKind::SharedObjectOperationNotAllowed,
                    Some(
                        format!(
                            "Shared object operation on {id} not allowed: \
                                 cannot be frozen, transferred, or wrapped"
                        )
                        .into(),
                    ),
                ));
            }
        } else {
            // If it's not in the written objects, the object must have been deleted. Otherwise
            // it's an error.
            if !deleted_object_ids.contains(id) {
                return Err(ExecutionError::new(
                    ExecutionErrorKind::SharedObjectOperationNotAllowed,
                    Some(
                        format!(
                            "Shared object operation on {id} not allowed: \
                             shared objects used by value must be re-shared if not deleted"
                        )
                        .into(),
                    ),
                ));
            }
        }
    }

    let user_events: Vec<Event> = user_events
        .into_iter()
        .map(|(module_id, tag, contents)| Event {
            package_id: module_id.address().to_object_id(),
            module: identifier_core_to_sdk(module_id.name()),
            sender: tx_context.sender(),
            struct_tag: struct_tag_core_to_sdk(&tag),
            contents,
        })
        .collect();

    // Deny-list v2 checks
    let DenyListResult {
        result,
        num_non_gas_coin_owners,
    } = state_view.check_coin_deny_list(&written_objects);
    gas_charger.charge_coin_transfers(protocol_config, num_non_gas_coin_owners)?;
    result?;

    // Under the P-COOL flow, the sender is checked again, once the commands have
    // run, against the coins the Move code received, with the deny list as the epoch started.
    if protocol_config.enable_pcool_flow() {
        state_view.check_received_coin_deny_list(tx_context.sender(), received_coin_types)?;
    }

    let created_object_ids: BTreeSet<ObjectId> = created_object_ids.into_iter().collect();
    let deleted_object_ids: BTreeSet<ObjectId> = deleted_object_ids.into_iter().collect();
    let modified_objects: BTreeSet<ObjectId> = loaded_runtime_objects
        .into_iter()
        .filter_map(|(id, loaded)| loaded.is_modified.then_some(id))
        .collect();

    assert_invariant!(
        created_object_ids.is_disjoint(&deleted_object_ids),
        "Created and deleted object sets should be disjoint"
    );
    assert_invariant!(
        modified_objects.is_disjoint(&created_object_ids),
        "Modified and created object sets should be disjoint"
    );
    assert_invariant!(
        written_objects
            .keys()
            .all(|id| !deleted_object_ids.contains(id)),
        "Written objects should not be deleted"
    );
    Ok(ExecutionResults::V1(ExecutionResultsV1 {
        written_objects,
        modified_objects,
        created_object_ids,
        deleted_object_ids,
        user_events,
    }))
}

pub fn fetch_package(
    state_view: &impl BackingPackageStore,
    package_id: &ObjectId,
) -> Result<PackageObject, ExecutionError> {
    let mut fetched_packages = fetch_packages(state_view, vec![package_id])?;
    assert_invariant!(
        fetched_packages.len() == 1,
        "Number of fetched packages must match the number of package object IDs if successful."
    );
    match fetched_packages.pop() {
        Some(pkg) => Ok(pkg),
        None => invariant_violation!(
            "We should always fetch a package for each object or return a dependency error."
        ),
    }
}

pub fn fetch_packages<'ctx, 'state>(
    state_view: &'state impl BackingPackageStore,
    package_ids: impl IntoIterator<Item = &'ctx ObjectId>,
) -> Result<Vec<PackageObject>, ExecutionError> {
    let package_ids: BTreeSet<_> = package_ids.into_iter().collect();
    match get_package_objects(state_view, package_ids) {
        Err(e) => Err(ExecutionError::new_with_source(
            ExecutionErrorKind::PublishUpgradeMissingDependency,
            e,
        )),
        Ok(Err(missing_deps)) => {
            let msg = format!(
                "Missing dependencies: {}",
                missing_deps
                    .into_iter()
                    .map(|dep| format!("{dep}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            Err(ExecutionError::new_with_source(
                ExecutionErrorKind::PublishUpgradeMissingDependency,
                msg,
            ))
        }
        Ok(Ok(pkgs)) => Ok(pkgs),
    }
}

pub fn check_compatibility(
    protocol_config: &ProtocolConfig,
    existing_package: &MovePackage,
    upgrading_modules: &[CompiledModule],
    policy: u8,
) -> Result<(), ExecutionError> {
    // Make sure this is a known upgrade policy.
    let Ok(policy) = UpgradePolicy::try_from(policy) else {
        return Err(ExecutionError::from_kind(
            ExecutionErrorKind::PackageUpgradeError {
                kind: PackageUpgradeError::UnknownUpgradePolicy { policy },
            },
        ));
    };

    let pool = &mut normalized::RcPool::new();
    let binary_config = to_binary_config(protocol_config, None);
    let current_normalized = match normalize_modules_with_metadata(
        pool,
        existing_package.serialized_module_map().values(),
        &binary_config,
        // include code
        true,
        Some(protocol_config),
    ) {
        Ok(modules) => modules,
        Err(IotaError::ModuleDeserializationFailure { .. }) => {
            invariant_violation!("Tried to normalize modules in existing package but failed")
        }
        Err(e) => {
            return Err(ExecutionError::new_with_source(
                ExecutionErrorKind::PackageUpgradeError {
                    kind: PackageUpgradeError::IncompatibleUpgrade,
                },
                e,
            ));
        }
    };

    let existing_modules_len = current_normalized.len();
    let upgrading_modules_len = upgrading_modules.len();
    let disallow_new_modules = protocol_config.disallow_new_modules_in_deps_only_packages()
        && policy as u8 == UpgradePolicy::DEP_ONLY;

    if disallow_new_modules && existing_modules_len != upgrading_modules_len {
        return Err(ExecutionError::new_with_source(
            ExecutionErrorKind::PackageUpgradeError {
                kind: PackageUpgradeError::IncompatibleUpgrade,
            },
            format!(
                "Existing package has {existing_modules_len} modules, but new package has \
                     {upgrading_modules_len}. Adding or removing a module to a deps only package is not allowed."
            ),
        ));
    }

    let mut new_normalized = normalize_deserialized_modules_with_metadata(
        pool,
        upgrading_modules.iter(),
        // include code
        true,
        Some(protocol_config),
    )
    .map_err(|e| {
        ExecutionError::new_with_source(
            ExecutionErrorKind::PackageUpgradeError {
                kind: PackageUpgradeError::IncompatibleUpgrade,
            },
            e,
        )
    })?;
    for (name, (cur_module, cur_metadata)) in current_normalized {
        let Some((new_module, new_metadata)) = new_normalized.remove(&name) else {
            return Err(ExecutionError::new_with_source(
                ExecutionErrorKind::PackageUpgradeError {
                    kind: PackageUpgradeError::IncompatibleUpgrade,
                },
                format!("Existing module {name} not found in next version of package"),
            ));
        };

        check_view_function_compatibility(&name, &cur_metadata, &new_metadata)?;
        check_module_compatibility(&policy, &cur_module, &new_module)?;
    }

    // If we disallow new modules double check that there are no modules left in `new_normalized`.
    debug_assert!(!disallow_new_modules || new_normalized.is_empty());

    Ok(())
}

/// Checks that every function marked `#[view]` in the current package is still marked `#[view]`
/// in the upgraded package.
fn check_view_function_compatibility(
    module_name: &str,
    cur_metadata: &RuntimeModuleMetadata,
    new_metadata: &RuntimeModuleMetadata,
) -> Result<(), ExecutionError> {
    let cur_view_functions = view_functions(cur_metadata);
    if cur_view_functions.is_empty() {
        return Ok(());
    }

    let new_view_functions = view_functions(new_metadata);
    for function_name in cur_view_functions {
        if !new_view_functions.contains(&function_name) {
            return Err(ExecutionError::new_with_source(
                ExecutionErrorKind::PackageUpgradeError {
                    kind: PackageUpgradeError::IncompatibleUpgrade,
                },
                format!(
                    "Function {module_name}::{function_name} was marked #[view] in the previous \
                     package version but is not marked #[view] in the upgraded package"
                ),
            ));
        }
    }

    Ok(())
}

fn view_functions(metadata: &RuntimeModuleMetadata) -> BTreeSet<String> {
    match metadata {
        RuntimeModuleMetadata::V1(_) => BTreeSet::new(),
        RuntimeModuleMetadata::V2(metadata) => metadata
            .fun_attributes
            .iter()
            .filter(|(_, attributes)| {
                attributes
                    .iter()
                    .any(|attribute| matches!(attribute, IotaAttributeV2::View))
            })
            .map(|(function_name, _)| function_name.clone())
            .collect(),
    }
}

fn check_module_compatibility(
    policy: &UpgradePolicy,
    cur_module: &move_binary_format::compatibility::Module,
    new_module: &move_binary_format::compatibility::Module,
) -> Result<(), ExecutionError> {
    match policy {
        UpgradePolicy::Additive => InclusionCheck::Subset.check(cur_module, new_module),
        UpgradePolicy::DepOnly => InclusionCheck::Equal.check(cur_module, new_module),
        UpgradePolicy::Compatible => {
            let compatibility = Compatibility::upgrade_check();

            compatibility.check(cur_module, new_module)
        }
    }
    .map_err(|e| {
        ExecutionError::new_with_source(
            ExecutionErrorKind::PackageUpgradeError {
                kind: PackageUpgradeError::IncompatibleUpgrade,
            },
            e,
        )
    })
}

/// Assert the type inferred matches the object's type. This has already been done during loading,
/// but is checked again as an invariant. This may be removed safely at a later time if needed.
fn assert_expected_move_object_type(
    actual: &Type,
    expected: &MoveObjectType,
) -> Result<(), ExecutionError> {
    let Type::Datatype(actual) = actual else {
        invariant_violation!("Expected a datatype for a Move object");
    };
    let expected = core_struct_tag(expected);
    let (a, m, n) = actual.qualified_ident();
    assert_invariant!(
        a == &expected.address,
        "Actual address does not match expected. actual: {actual:?} vs expected: {expected:?}"
    );
    assert_invariant!(
        m == expected.module.as_ident_str(),
        "Actual module does not match expected. actual: {actual:?} vs expected: {expected:?}"
    );
    assert_invariant!(
        n == expected.name.as_ident_str(),
        "Actual struct does not match expected. actual: {actual:?} vs expected: {expected:?}"
    );
    let actual_type_arguments = &actual.type_arguments;
    let expected_type_arguments = &expected.type_params;
    assert_invariant!(
        actual_type_arguments.len() == expected_type_arguments.len(),
        "Actual type arg length does not match expected. \
       actual: {actual:?} vs expected: {expected:?}",
    );
    for (actual_ty, expected_ty) in actual_type_arguments
        .iter()
        .zip_debug_eq(expected_type_arguments)
    {
        assert_expected_type(actual_ty, expected_ty)?;
    }
    Ok(())
}

fn assert_expected_type(actual: &Type, expected: &TypeTag) -> Result<(), ExecutionError> {
    match (actual, expected) {
        (Type::Bool, TypeTag::Bool)
        | (Type::U8, TypeTag::U8)
        | (Type::U16, TypeTag::U16)
        | (Type::U32, TypeTag::U32)
        | (Type::U64, TypeTag::U64)
        | (Type::U128, TypeTag::U128)
        | (Type::U256, TypeTag::U256)
        | (Type::Address, TypeTag::Address)
        | (Type::Signer, TypeTag::Signer) => Ok(()),
        (Type::Vector(inner_actual), TypeTag::Vector(inner_expected)) => {
            assert_expected_type(&inner_actual.element_type, inner_expected)
        }
        (Type::Datatype(actual_dt), TypeTag::Struct(expected_st)) => {
            assert_expected_data_type(actual_dt, expected_st)
        }
        _ => invariant_violation!(
            "Type mismatch between actual: {actual:?} and expected: {expected:?}"
        ),
    }
}
/// Assert the type inferred matches the expected type. This has already been done during typing,
/// but is checked again as an invariant. This may be removed safely at a later time if needed.
fn assert_expected_data_type(
    actual: &Datatype,
    expected: &StructTag,
) -> Result<(), ExecutionError> {
    let (a, m, n) = actual.qualified_ident();
    assert_invariant!(
        a == &expected.address,
        "Actual address does not match expected. actual: {actual:?} vs expected: {expected:?}"
    );
    assert_invariant!(
        m == expected.module.as_ident_str(),
        "Actual module does not match expected. actual: {actual:?} vs expected: {expected:?}"
    );
    assert_invariant!(
        n == expected.name.as_ident_str(),
        "Actual struct does not match expected. actual: {actual:?} vs expected: {expected:?}"
    );
    let actual_type_arguments = &actual.type_arguments;
    let expected_type_arguments = &expected.type_params;
    assert_invariant!(
        actual_type_arguments.len() == expected_type_arguments.len(),
        "Actual type arg length does not match expected. \
       actual: {actual:?} vs expected: {expected:?}",
    );
    for (actual_ty, expected_ty) in actual_type_arguments
        .iter()
        .zip_debug_eq(expected_type_arguments)
    {
        assert_expected_type(actual_ty, expected_ty)?;
    }
    Ok(())
}
