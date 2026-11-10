// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This module defines the shared environment, `Env`, used for the
//! compilation/translation and execution of programmable transactions. While
//! the "context" for each pass might be different, the `Env` provides
//! consistent access to shared components such as the VM or the protocol
//! config.

use std::{cell::OnceCell, marker::PhantomData, rc::Rc};

use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::{MovePackage, ObjectId, StructTag as SdkStructTag};
use iota_types::{
    IOTA_FRAMEWORK_ADDRESS, IOTA_FRAMEWORK_PACKAGE_ID,
    auth_context::{AUTH_CONTEXT_MODULE_NAME, AUTH_CONTEXT_STRUCT_NAME},
    coin::RESOLVED_COIN_STRUCT,
    error::ExecutionErrorKind,
    execution_config_utils::to_binary_config,
    iota_sdk_types_conversions::{struct_tag_sdk_to_core, type_tag_sdk_to_core},
    object::Object,
};
use move_binary_format::{
    CompiledModule,
    errors::{Location, VMError, VMResult},
    file_format::{Ability, AbilitySet, TypeParameterIndex},
};
use move_core_types::{
    annotated_value, ident_str,
    identifier::{IdentStr, Identifier},
    language_storage::{ModuleId, StructTag, TypeTag},
    resolver::IntraPackageName,
    runtime_value,
    runtime_value::MoveTypeLayout,
    vm_status::StatusCode,
};
use move_vm_runtime::{execution as vm_runtime, execution::vm::MoveVM, runtime::MoveRuntime};

use crate::{
    compat::{ExecutionErrorTrait, RESOLVED_BALANCE_STRUCT, execution_status::TypeArgumentError},
    data_store::{VerifiedPackageStore, cached_package_store::CachedPackageStore},
    execution_mode::ExecutionMode,
    execution_value::ExecutionState,
    sdk_conv::{AccountAddressExt, ToAccountAddress},
    static_programmable_transactions::{
        execution::context::subst_signature,
        linkage::{analysis::LinkageAnalyzer, resolved_linkage::ExecutableLinkage},
        loading::{
            ast as L,
            ast::{
                Datatype, DeserializedPackage, LoadedFunction, LoadedFunctionInstantiation, Type,
            },
        },
    },
};

pub struct Env<'pc, 'vm, 'state, 'linkage, 'extensions, Mode>
where
    Mode: ExecutionMode,
{
    pub protocol_config: &'pc ProtocolConfig,
    pub vm: &'vm MoveRuntime,
    pub state_view: &'state mut dyn ExecutionState,
    pub linkable_store: &'linkage CachedPackageStore<'state, 'vm>,
    pub linkage_analysis: &'linkage LinkageAnalyzer,
    gas_coin_type: OnceCell<Type>,
    upgrade_ticket_type: OnceCell<Type>,
    upgrade_receipt_type: OnceCell<Type>,
    upgrade_cap_type: OnceCell<Type>,
    tx_context_type: OnceCell<Type>,
    auth_context_type: OnceCell<Type>,
    // The VM used for type resolution of input types (and types statically present in the PTB)
    // only. This VM should only be used for resolution of input types, but should not be used for
    // resolution around function calls, execution, or final serialization of execution values.
    input_type_resolution_vm: &'linkage MoveVM<'extensions>,
    _mode: PhantomData<fn() -> Mode>,
}

macro_rules! get_or_init_ty {
    ($env:expr, $ident:ident, $tag:expr) => {{
        let env = $env;
        if env.$ident.get().is_none() {
            let tag = $tag;
            let ty = env.load_type_from_struct(&tag)?;
            env.$ident.set(ty.clone()).unwrap();
        }
        Ok(env.$ident.get().unwrap().clone())
    }};
}

impl<'pc, 'vm, 'state, 'linkage, 'extensions, Mode>
    Env<'pc, 'vm, 'state, 'linkage, 'extensions, Mode>
where
    Mode: ExecutionMode,
{
    pub fn new(
        protocol_config: &'pc ProtocolConfig,
        vm: &'vm MoveRuntime,
        state_view: &'state mut dyn ExecutionState,
        linkable_store: &'linkage CachedPackageStore<'state, 'vm>,
        linkage_analysis: &'linkage LinkageAnalyzer,
        input_type_resolution_vm: &'linkage MoveVM<'extensions>,
    ) -> Self {
        Self {
            protocol_config,
            vm,
            state_view,
            linkable_store,
            linkage_analysis,
            gas_coin_type: OnceCell::new(),
            upgrade_ticket_type: OnceCell::new(),
            upgrade_receipt_type: OnceCell::new(),
            upgrade_cap_type: OnceCell::new(),
            tx_context_type: OnceCell::new(),
            auth_context_type: OnceCell::new(),
            input_type_resolution_vm,
            _mode: PhantomData,
        }
    }

    pub fn convert_linked_vm_error(&self, e: VMError, linkage: &ExecutableLinkage) -> Mode::Error {
        convert_vm_error(e, self.linkable_store, Some(linkage))
    }

    pub fn convert_vm_error(&self, e: VMError) -> Mode::Error {
        convert_vm_error(e, self.linkable_store, None)
    }

    pub fn convert_type_argument_error(
        &self,
        idx: usize,
        e: VMError,
        linkage: &ExecutableLinkage,
    ) -> Mode::Error {
        use move_core_types::vm_status::StatusCode;
        let argument_idx = match checked_as!(idx, TypeParameterIndex) {
            Err(e) => return e.into(),
            Ok(v) => v,
        };
        match e.major_status() {
            StatusCode::NUMBER_OF_TYPE_ARGUMENTS_MISMATCH => {
                Mode::Error::from_kind(ExecutionErrorKind::TypeArityMismatch)
            }
            StatusCode::EXTERNAL_RESOLUTION_REQUEST_ERROR => {
                Mode::Error::from_kind(ExecutionErrorKind::TypeArgumentError {
                    type_argument: argument_idx,
                    kind: TypeArgumentError::TypeNotFound,
                })
            }
            StatusCode::CONSTRAINT_NOT_SATISFIED => {
                Mode::Error::from_kind(ExecutionErrorKind::TypeArgumentError {
                    type_argument: argument_idx,
                    kind: TypeArgumentError::ConstraintNotSatisfied,
                })
            }
            _ => self.convert_linked_vm_error(e, linkage),
        }
    }

    pub fn fully_annotated_layout(
        &self,
        ty: &Type,
    ) -> Result<annotated_value::MoveTypeLayout, Mode::Error> {
        let tag: TypeTag = ty.clone().try_into().map_err(|s| {
            Mode::Error::new_with_source(ExecutionErrorKind::VmInvariantViolation, s)
        })?;
        let objects = tag.all_addresses();
        let tag_linkage = ExecutableLinkage::type_linkage::<_, Mode::Error>(
            self.linkage_analysis.config().clone(),
            objects.into_iter().map(AccountAddressExt::to_object_id),
            self.linkable_store,
        )?;
        self.input_type_resolution_vm
            .annotated_type_layout(&tag)
            .map_err(|e| self.convert_linked_vm_error(e, &tag_linkage))
    }

    pub fn runtime_layout(&self, ty: &Type) -> Result<runtime_value::MoveTypeLayout, Mode::Error> {
        let tag: TypeTag = ty.clone().try_into().map_err(|s| {
            Mode::Error::new_with_source(ExecutionErrorKind::VmInvariantViolation, s)
        })?;
        let objects = tag.all_addresses();
        let tag_linkage = ExecutableLinkage::type_linkage::<_, Mode::Error>(
            self.linkage_analysis.config().clone(),
            objects.into_iter().map(AccountAddressExt::to_object_id),
            self.linkable_store,
        )?;
        self.input_type_resolution_vm
            .runtime_type_layout(&tag)
            .map_err(|e| self.convert_linked_vm_error(e, &tag_linkage))
    }

    pub fn load_framework_function(
        &self,
        module: &IdentStr,
        function: &IdentStr,
        type_arguments: Vec<Type>,
        unified_linkage: Option<&ExecutableLinkage>,
    ) -> Result<LoadedFunction, Mode::Error> {
        let mut loaded = self.load_function(
            IOTA_FRAMEWORK_PACKAGE_ID,
            module.to_string(),
            function.to_string(),
            type_arguments,
        )?;
        let Some(unified_linkage) = unified_linkage else {
            invariant_violation!(
                "Unified linkage is required when hardened linkage consistency is enabled"
            )
        };
        assert_invariant!(
            loaded
                .linkage
                .0
                .linkage
                .keys()
                .all(|original_id| unified_linkage.0.linkage.contains_key(original_id)),
            "transaction linkage drops a package resolved by a framework MoveCall"
        );
        loaded.linkage = unified_linkage.clone();
        Ok(loaded)
    }

    pub fn load_function(
        &self,
        package: ObjectId,
        module: String,
        function: String,
        type_arguments: Vec<Type>,
    ) -> Result<LoadedFunction, Mode::Error> {
        let module = to_identifier(module)?;
        let name = to_identifier(function)?;

        let linkage = self.linkage_analysis.compute_call_linkage::<Mode::Error>(
            &package,
            module.as_ident_str(),
            name.as_ident_str(),
            &type_arguments,
            self.linkable_store,
        )?;

        let Some(original_id) = linkage.0.resolve_to_original_id(&package) else {
            invariant_violation!(
                "Package ID {:?} is not found in linkage generated for that package",
                package
            );
        };
        let version_mid = ModuleId::new(package.to_account_address(), module.clone());
        let original_mid = ModuleId::new(original_id.to_account_address(), module);
        let loaded_type_arguments = type_arguments
            .iter()
            .enumerate()
            .map(|(idx, ty)| self.load_vm_type_argument_from_adapter_type(idx, ty))
            .collect::<Result<Vec<_>, _>>()?;
        // NB: We cannot use the resolution VM here because the linkage for that unifies up, and if
        // this is a private entry function, it may have been removed in future versions of the
        // package.
        let vm = self
            .vm
            .make_vm(
                &self.linkable_store.package_store,
                linkage.linkage_context::<Mode::Error>()?,
            )
            .map_err(|e| self.convert_linked_vm_error(e, &linkage))?;
        let runtime_signature = vm
            .function_information(&original_mid, name.as_ident_str(), &loaded_type_arguments)
            .map_err(|e| {
                if e.major_status() == StatusCode::EXTERNAL_RESOLUTION_REQUEST_ERROR {
                    Mode::Error::new_with_source(
                        ExecutionErrorKind::FunctionNotFound,
                        format!("Could not resolve function '{name}' in module '{version_mid}'",),
                    )
                } else {
                    self.convert_linked_vm_error(e, &linkage)
                }
            })?;
        let runtime_signature = subst_signature(runtime_signature, &loaded_type_arguments)
            .map_err(|e| self.convert_linked_vm_error(e, &linkage))?;
        let parameters = runtime_signature
            .parameters
            .into_iter()
            .map(|ty| self.adapter_type_from_vm_type(&vm, &ty))
            .collect::<Result<Vec<_>, _>>()?;
        let return_ = runtime_signature
            .return_
            .into_iter()
            .map(|ty| self.adapter_type_from_vm_type(&vm, &ty))
            .collect::<Result<Vec<_>, _>>()?;
        let signature = LoadedFunctionInstantiation {
            parameters,
            return_,
        };
        Ok(LoadedFunction {
            version_mid,
            original_mid,
            name,
            type_arguments,
            signature,
            linkage,
            instruction_length: runtime_signature.instruction_count,
            definition_index: runtime_signature.index,
            visibility: runtime_signature.visibility,
            is_entry: runtime_signature.is_entry,
            is_native: runtime_signature.is_native,
        })
    }

    pub fn load_type_input(
        &self,
        idx: usize,
        ty: iota_sdk_types::TypeTag,
    ) -> Result<Type, Mode::Error> {
        let vm_type = self.load_vm_type_from_type_input(idx, ty)?;
        self.adapter_type_from_vm_type(self.input_type_resolution_vm, &vm_type)
    }

    pub fn load_type_tag(&self, idx: usize, ty: &TypeTag) -> Result<Type, Mode::Error> {
        let vm_type = self.load_vm_type_from_type_tag(Some(idx), ty)?;
        self.adapter_type_from_vm_type(self.input_type_resolution_vm, &vm_type)
    }

    /// We verify that all types in the `StructTag` are defining ID-based types.
    pub fn load_type_from_struct(&self, tag: &StructTag) -> Result<Type, Mode::Error> {
        let vm_type =
            self.load_vm_type_from_type_tag(None, &TypeTag::Struct(Box::new(tag.clone())))?;
        self.adapter_type_from_vm_type(self.input_type_resolution_vm, &vm_type)
    }

    pub fn type_layout_for_struct(&self, tag: &StructTag) -> Result<MoveTypeLayout, Mode::Error> {
        let ty: Type = self.load_type_from_struct(tag)?;
        self.runtime_layout(&ty)
    }

    pub fn gas_coin_type(&self) -> Result<Type, Mode::Error> {
        get_or_init_ty!(
            self,
            gas_coin_type,
            struct_tag_sdk_to_core(&SdkStructTag::new_gas_coin())
        )
    }

    pub fn upgrade_ticket_type(&self) -> Result<Type, Mode::Error> {
        get_or_init_ty!(
            self,
            upgrade_ticket_type,
            framework_struct_tag(ident_str!("package"), ident_str!("UpgradeTicket"))
        )
    }

    pub fn upgrade_receipt_type(&self) -> Result<Type, Mode::Error> {
        get_or_init_ty!(
            self,
            upgrade_receipt_type,
            framework_struct_tag(ident_str!("package"), ident_str!("UpgradeReceipt"))
        )
    }

    pub fn upgrade_cap_type(&self) -> Result<Type, Mode::Error> {
        get_or_init_ty!(
            self,
            upgrade_cap_type,
            framework_struct_tag(ident_str!("package"), ident_str!("UpgradeCap"))
        )
    }

    pub fn tx_context_type(&self) -> Result<Type, Mode::Error> {
        get_or_init_ty!(
            self,
            tx_context_type,
            framework_struct_tag(ident_str!("tx_context"), ident_str!("TxContext"))
        )
    }

    pub fn auth_context_type(&self) -> Result<Type, Mode::Error> {
        get_or_init_ty!(
            self,
            auth_context_type,
            framework_struct_tag(AUTH_CONTEXT_MODULE_NAME, AUTH_CONTEXT_STRUCT_NAME)
        )
    }

    pub fn coin_type(&self, inner_type: Type) -> Result<Type, Mode::Error> {
        const COIN_ABILITIES: AbilitySet =
            AbilitySet::singleton(Ability::Key).union(AbilitySet::singleton(Ability::Store));
        let (a, m, n) = RESOLVED_COIN_STRUCT;
        let module = ModuleId::new(*a, m.to_owned());
        Ok(Type::Datatype(Rc::new(Datatype {
            abilities: COIN_ABILITIES,
            module,
            name: n.to_owned(),
            type_arguments: vec![inner_type],
        })))
    }

    pub fn balance_type(&self, inner_type: Type) -> Result<Type, Mode::Error> {
        const BALANCE_ABILITIES: AbilitySet = AbilitySet::singleton(Ability::Store);
        let (a, m, n) = RESOLVED_BALANCE_STRUCT;
        let module = ModuleId::new(*a, m.to_owned());
        Ok(Type::Datatype(Rc::new(Datatype {
            abilities: BALANCE_ABILITIES,
            module,
            name: n.to_owned(),
            type_arguments: vec![inner_type],
        })))
    }

    pub fn vector_type(&self, element_type: Type) -> Result<Type, Mode::Error> {
        let abilities = AbilitySet::polymorphic_abilities(
            AbilitySet::VECTOR,
            [false],
            [element_type.abilities()],
        )
        .map_err(|e| {
            Mode::Error::new_with_source(ExecutionErrorKind::VmInvariantViolation, e.to_string())
        })?;
        Ok(Type::Vector(Rc::new(L::Vector {
            abilities,
            element_type,
        })))
    }

    pub fn read_object(&self, id: &ObjectId) -> Result<&Object, Mode::Error> {
        let Some(obj) = self.state_view.read_object(id) else {
            // protected by transaction input checker
            invariant_violation!("Object {:?} does not exist", id);
        };
        Ok(obj)
    }

    /// Takes an adapter Type and returns a VM runtime Type and the linkage for it.
    pub fn load_vm_type_argument_from_adapter_type(
        &self,
        idx: usize,
        ty: &Type,
    ) -> Result<vm_runtime::Type, Mode::Error> {
        self.load_vm_type_from_adapter_type(Some(idx), ty)
    }

    fn load_vm_type_from_adapter_type(
        &self,
        type_arg_idx: Option<usize>,
        ty: &Type,
    ) -> Result<vm_runtime::Type, Mode::Error> {
        let tag: TypeTag = ty.clone().try_into().map_err(|s| {
            Mode::Error::new_with_source(ExecutionErrorKind::VmInvariantViolation, s)
        })?;
        self.load_vm_type_from_type_tag(type_arg_idx, &tag)
    }

    /// Take a type tag and returns a VM runtime Type and the linkage for it.
    fn load_vm_type_from_type_tag(
        &self,
        type_arg_idx: Option<usize>,
        tag: &TypeTag,
    ) -> Result<vm_runtime::Type, Mode::Error> {
        fn execution_error<Mode: ExecutionMode>(
            env: &Env<Mode>,
            type_arg_idx: Option<usize>,
            e: VMError,
            linkage: &ExecutableLinkage,
        ) -> Mode::Error {
            if let Some(idx) = type_arg_idx {
                env.convert_type_argument_error(idx, e, linkage)
            } else {
                env.convert_linked_vm_error(e, linkage)
            }
        }

        let objects = tag.all_addresses();

        let tag_linkage = ExecutableLinkage::type_linkage::<_, Mode::Error>(
            self.linkage_analysis.config().clone(),
            objects.iter().map(|a| a.to_object_id()),
            self.linkable_store,
        )?;
        let ty = self
            .input_type_resolution_vm
            .load_type(tag)
            .map_err(|e| execution_error(self, type_arg_idx, e, &tag_linkage))?;
        Ok(ty)
    }

    /// Converts a VM runtime Type to an adapter Type.
    pub(crate) fn adapter_type_from_vm_type(
        &self,
        vm: &MoveVM,
        vm_type: &vm_runtime::Type,
    ) -> Result<Type, Mode::Error> {
        use vm_runtime as VRT;

        Ok(match vm_type {
            VRT::Type::Bool => Type::Bool,
            VRT::Type::U8 => Type::U8,
            VRT::Type::U16 => Type::U16,
            VRT::Type::U32 => Type::U32,
            VRT::Type::U64 => Type::U64,
            VRT::Type::U128 => Type::U128,
            VRT::Type::U256 => Type::U256,
            VRT::Type::Address => Type::Address,
            VRT::Type::Signer => Type::Signer,

            VRT::Type::Reference(ref_ty) => {
                let inner_ty = self.adapter_type_from_vm_type(vm, ref_ty)?;
                Type::Reference(false, Rc::new(inner_ty))
            }
            VRT::Type::MutableReference(ref_ty) => {
                let inner_ty = self.adapter_type_from_vm_type(vm, ref_ty)?;
                Type::Reference(true, Rc::new(inner_ty))
            }

            VRT::Type::Vector(inner) => {
                let element_type = self.adapter_type_from_vm_type(vm, inner)?;
                self.vector_type(element_type)?
            }
            VRT::Type::Datatype(_) => {
                let type_information = vm
                    .type_information(vm_type)
                    .map_err(|e| self.convert_vm_error(e))?;
                let Some(data_type_info) = type_information.datatype_info else {
                    invariant_violation!("Expected datatype info for datatype type {:?}", vm_type);
                };
                let datatype = Datatype {
                    abilities: type_information.abilities,
                    module: ModuleId::new(data_type_info.defining_id, data_type_info.module_name),
                    name: data_type_info.type_name,
                    type_arguments: vec![],
                };
                Type::Datatype(Rc::new(datatype))
            }
            ty @ VRT::Type::DatatypeInstantiation(inst) => {
                let (_, type_arguments) = &**inst;
                let type_information = vm
                    .type_information(ty)
                    .map_err(|e| self.convert_vm_error(e))?;
                let Some(data_type_info) = type_information.datatype_info else {
                    invariant_violation!("Expected datatype info for datatype type {:?}", vm_type);
                };

                let abilities = type_information.abilities;
                let module = ModuleId::new(data_type_info.defining_id, data_type_info.module_name);
                let name = data_type_info.type_name;
                let type_arguments = type_arguments
                    .iter()
                    .map(|t| self.adapter_type_from_vm_type(vm, t))
                    .collect::<Result<Vec<_>, _>>()?;

                Type::Datatype(Rc::new(Datatype {
                    abilities,
                    module,
                    name,
                    type_arguments,
                }))
            }

            VRT::Type::TyParam(_) => {
                invariant_violation!(
                    "Unexpected type parameter in VM type: {:?}. This should not happen as we should \
                     have resolved all type parameters before this point.",
                    vm_type
                );
            }
        })
    }

    /// Load a type argument into a VM runtime `Type`. Every struct in the tag is rewritten to its
    /// defining ID via the package's type origin table before loading, so that any adapter type
    /// or type tag that results from this is properly output with defining IDs.
    fn load_vm_type_from_type_input(
        &self,
        type_arg_idx: usize,
        ty: iota_sdk_types::TypeTag,
    ) -> Result<vm_runtime::Type, Mode::Error> {
        fn to_defining_id_type_tag<Mode: ExecutionMode>(
            env: &Env<Mode>,
            type_arg_idx: usize,
            ty: TypeTag,
        ) -> Result<TypeTag, Mode::Error> {
            Ok(match ty {
                TypeTag::Bool
                | TypeTag::U8
                | TypeTag::U16
                | TypeTag::U32
                | TypeTag::U64
                | TypeTag::U128
                | TypeTag::U256
                | TypeTag::Address
                | TypeTag::Signer => ty,
                TypeTag::Vector(inner) => {
                    let inner = to_defining_id_type_tag(env, type_arg_idx, *inner)?;
                    TypeTag::Vector(Box::new(inner))
                }
                TypeTag::Struct(struct_tag) => {
                    let StructTag {
                        address,
                        module,
                        name,
                        type_params,
                    } = *struct_tag;

                    let pkg = env
                        .linkable_store
                        .get_package(&ObjectId::new(address.into_bytes()))
                        .ok()
                        .flatten()
                        .ok_or_else(|| {
                            let argument_idx = match checked_as!(type_arg_idx, u16) {
                                Err(e) => return e.into(),
                                Ok(v) => v,
                            };
                            Mode::Error::from_kind(ExecutionErrorKind::TypeArgumentError {
                                type_argument: argument_idx,
                                kind: TypeArgumentError::TypeNotFound,
                            })
                        })?;
                    let tid = IntraPackageName {
                        module_name: module,
                        type_name: name,
                    };
                    let Some(resolved_address) = pkg.type_origin_table().get(&tid).cloned() else {
                        return Err(Mode::Error::from_kind(
                            ExecutionErrorKind::TypeArgumentError {
                                type_argument: checked_as!(type_arg_idx, u16)?,
                                kind: TypeArgumentError::TypeNotFound,
                            },
                        ));
                    };

                    let tys = type_params
                        .into_iter()
                        .map(|tp| to_defining_id_type_tag(env, type_arg_idx, tp))
                        .collect::<Result<Vec<_>, _>>()?;
                    TypeTag::Struct(Box::new(StructTag {
                        address: resolved_address,
                        module: tid.module_name,
                        name: tid.type_name,
                        type_params: tys,
                    }))
                }
            })
        }
        let tag = to_defining_id_type_tag(self, type_arg_idx, type_tag_sdk_to_core(&ty))?;
        self.load_vm_type_from_type_tag(Some(type_arg_idx), &tag)
    }

    pub fn deserialize_package(
        &self,
        module_bytes: &[Vec<u8>],
        dep_ids: &[ObjectId],
    ) -> Result<DeserializedPackage, Mode::Error> {
        assert_invariant!(
            !module_bytes.is_empty(),
            "empty package is checked in transaction input checker"
        );

        let total_bytes = module_bytes.iter().map(|v| v.len()).sum();

        let binary_config = to_binary_config(self.protocol_config, None);
        let deserialized_modules = module_bytes
            .iter()
            .map(|b| {
                CompiledModule::deserialize_with_config(b, &binary_config)
                    .map_err(|e| e.finish(Location::Undefined))
            })
            .collect::<VMResult<Vec<CompiledModule>>>()
            .map_err(|e| self.convert_vm_error(e))?;
        let computed_digest =
            MovePackage::compute_digest_for_modules_and_deps(module_bytes, dep_ids).into_bytes();
        Ok(DeserializedPackage::new(
            deserialized_modules,
            total_bytes,
            computed_digest,
        ))
    }
}

fn framework_struct_tag(module: &IdentStr, name: &IdentStr) -> StructTag {
    StructTag {
        address: IOTA_FRAMEWORK_ADDRESS,
        module: module.to_owned(),
        name: name.to_owned(),
        type_params: vec![],
    }
}

fn to_identifier<E: ExecutionErrorTrait>(name: String) -> Result<Identifier, E> {
    Identifier::new(name)
        .map_err(|e| E::new_with_source(ExecutionErrorKind::VmInvariantViolation, e.to_string()))
}

fn convert_vm_error<E: ExecutionErrorTrait>(
    error: VMError,
    store: &VerifiedPackageStore<'_>,
    linkage: Option<&ExecutableLinkage>,
) -> E {
    use crate::error::convert_vm_error_impl;
    convert_vm_error_impl(
        error,
        &|id| {
            debug_assert!(
                linkage.is_some(),
                "Linkage should be set anywhere where runtime errors may occur in order to resolve abort locations to package IDs"
            );
            linkage
                .and_then(|linkage| {
                    linkage
                        .0
                        .linkage
                        .get(&id.address().to_object_id())
                        .map(|new_id| ModuleId::new(new_id.to_account_address(), id.name().to_owned()))
                })
                .unwrap_or_else(|| id.clone())
        },
        // NB: the `id` here is the original ID (and hence _not_ relocated).
        &|id, function| {
            debug_assert!(
                linkage.is_some(),
                "Linkage should be set anywhere where runtime errors may occur in order to resolve abort locations to package IDs"
            );
            linkage.and_then(|linkage| {
                let version_id = linkage
                    .0
                    .linkage
                    .get(&id.address().to_object_id())
                    .cloned()
                    .unwrap_or_else(|| id.address().to_object_id());
                store.get_package(&version_id).ok().flatten().and_then(|p| {
                    p.modules().get(id).map(|module| {
                        let module = module.compiled_module();
                        let fdef = module.function_def_at(function);
                        let fhandle = module.function_handle_at(fdef.function);
                        module.identifier_at(fhandle.name).to_string()
                    })
                })
            })
        },
    )
    .into()
}
