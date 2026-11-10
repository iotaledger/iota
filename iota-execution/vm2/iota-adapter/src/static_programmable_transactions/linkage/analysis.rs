// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::{ObjectId, ProgrammableTransaction};
use iota_types::{error::ExecutionErrorKind, execution_config_utils::to_binary_config};
use move_binary_format::file_format::Visibility;
use move_core_types::identifier::IdentStr;
use move_vm_runtime::validation::verification::ast::Package as VerifiedPackage;

use crate::{
    compat::ExecutionErrorTrait,
    data_store::VerifiedPackageStore,
    execution_mode::ExecutionMode,
    execution_value::ExecutionState,
    sdk_conv::AccountAddressExt,
    static_programmable_transactions::{
        linkage::{
            config::{LinkageConfig, ResolutionConfig},
            resolution::{ResolutionTable, VersionConstraint, add_and_unify, get_package},
            resolved_linkage::{ExecutableLinkage, ResolvedLinkage},
        },
        loading::ast::Type,
    },
};

#[derive(Debug)]
pub struct LinkageAnalyzer {
    internal: ResolutionConfig,
}

impl LinkageAnalyzer {
    pub fn new<Mode: ExecutionMode>(protocol_config: &ProtocolConfig) -> Result<Self, Mode::Error> {
        let always_include_system_packages = !Mode::packages_are_predefined();
        let linkage_config = LinkageConfig::new(always_include_system_packages);
        let binary_config = to_binary_config(protocol_config, None);
        Ok(Self {
            internal: ResolutionConfig::new(linkage_config, binary_config),
        })
    }

    pub fn compute_call_linkage<E: ExecutionErrorTrait>(
        &self,
        package: &ObjectId,
        module_name: &IdentStr,
        function_name: &IdentStr,
        type_args: &[Type],
        store: &VerifiedPackageStore<'_>,
    ) -> Result<ExecutableLinkage, E> {
        Ok(ExecutableLinkage::new(
            ResolvedLinkage::from_resolution_table(self.compute_call_linkage_(
                package,
                module_name,
                function_name,
                type_args,
                store,
            )?),
        ))
    }

    pub fn compute_publication_linkage<E: ExecutionErrorTrait>(
        &self,
        deps: &[ObjectId],
        store: &VerifiedPackageStore<'_>,
    ) -> Result<ResolvedLinkage, E> {
        Ok(ResolvedLinkage::from_resolution_table(
            self.compute_publication_linkage_(deps, store)?,
        ))
    }

    pub fn config(&self) -> &ResolutionConfig {
        &self.internal
    }

    pub fn compute_input_type_resolution_linkage<E: ExecutionErrorTrait>(
        &self,
        tx: &ProgrammableTransaction,
        package_store: &VerifiedPackageStore<'_>,
        object_store: &dyn ExecutionState,
    ) -> Result<ExecutableLinkage, E> {
        input_type_resolution_analysis::compute_resolution_linkage(
            self,
            tx,
            package_store,
            object_store,
        )
    }

    fn compute_call_linkage_<E: ExecutionErrorTrait>(
        &self,
        package: &ObjectId,
        module_name: &IdentStr,
        function_name: &IdentStr,
        type_args: &[Type],
        store: &VerifiedPackageStore<'_>,
    ) -> Result<ResolutionTable, E> {
        let mut resolution_table = self.internal.resolution_table_with_native_packages(store)?;

        fn add_package<E: ExecutionErrorTrait>(
            object_id: &ObjectId,
            store: &VerifiedPackageStore<'_>,
            resolution_table: &mut ResolutionTable,
            self_resolution_fn: fn(&Arc<VerifiedPackage>) -> Option<VersionConstraint>,
            dep_resolution_fn: fn(&Arc<VerifiedPackage>) -> Option<VersionConstraint>,
        ) -> Result<(), E> {
            let pkg = get_package(object_id, store)?;
            let transitive_deps = resolution_table
                .config
                .linkage_table(&pkg)
                .into_values()
                .map(AccountAddressExt::to_object_id);
            for object_id in transitive_deps {
                add_and_unify(&object_id, store, resolution_table, dep_resolution_fn)?;
            }
            add_and_unify(object_id, store, resolution_table, self_resolution_fn)?;
            Ok(())
        }

        let pkg = get_package(package, store)?;
        let fn_not_found_err = || -> E {
            E::new_with_source(
                ExecutionErrorKind::FunctionNotFound,
                format!(
                    "Could not resolve function '{function_name}' in module '{package}::{module_name}'"
                ),
            )
        };
        let fdef = pkg
            .modules()
            .iter()
            .find(|m| m.0.name() == module_name)
            .ok_or_else(fn_not_found_err)?
            .1
            .compiled_module()
            .find_function_def_by_name(function_name.as_str())
            .ok_or_else(fn_not_found_err)?;

        let dep_resolution_fn = match fdef.1.visibility {
            Visibility::Public => VersionConstraint::at_least,
            Visibility::Private | Visibility::Friend => VersionConstraint::exact,
        };

        add_package(
            package,
            store,
            &mut resolution_table,
            VersionConstraint::exact,
            dep_resolution_fn,
        )?;

        for type_defining_id in type_args.iter().flat_map(|ty| ty.all_addresses()) {
            // Type arguments are "at least" constraints
            add_package(
                &type_defining_id.to_object_id(),
                store,
                &mut resolution_table,
                VersionConstraint::at_least,
                VersionConstraint::at_least,
            )?;
        }

        Ok(resolution_table)
    }

    /// Compute the linkage for a publish or upgrade command. This is a special case because
    fn compute_publication_linkage_<E: ExecutionErrorTrait>(
        &self,
        deps: &[ObjectId],
        store: &VerifiedPackageStore<'_>,
    ) -> Result<ResolutionTable, E> {
        let mut resolution_table = self.internal.resolution_table_with_native_packages(store)?;
        for id in deps {
            add_and_unify(id, store, &mut resolution_table, VersionConstraint::exact)?;
        }
        Ok(resolution_table)
    }
}

mod input_type_resolution_analysis {
    use iota_sdk_types::{
        Command, Input, MakeMoveVector, MoveCall, ProgrammableTransaction, Publish, Upgrade,
    };
    use iota_types::iota_sdk_types_conversions::type_tag_sdk_to_core;

    use crate::{
        compat::ExecutionErrorTrait,
        data_store::VerifiedPackageStore,
        execution_value::ExecutionState,
        sdk_conv::{AccountAddressExt, core_struct_tag},
        static_programmable_transactions::linkage::{
            analysis::LinkageAnalyzer,
            resolution::ResolutionTable,
            resolved_linkage::{ExecutableLinkage, ResolvedLinkage},
        },
    };

    pub(super) fn compute_resolution_linkage<E: ExecutionErrorTrait>(
        analyzer: &LinkageAnalyzer,
        tx: &ProgrammableTransaction,
        package_store: &VerifiedPackageStore<'_>,
        object_store: &dyn ExecutionState,
    ) -> Result<ExecutableLinkage, E> {
        let ProgrammableTransaction { inputs, commands } = tx;

        let mut resolution_table = analyzer
            .internal
            .resolution_table_with_native_packages(package_store)?;
        for arg in inputs.iter() {
            input(&mut resolution_table, arg, package_store, object_store)?;
        }

        for cmd in commands.iter() {
            command(&mut resolution_table, cmd, package_store)?;
        }

        Ok(ExecutableLinkage::new(
            ResolvedLinkage::from_resolution_table(resolution_table),
        ))
    }

    fn input<E: ExecutionErrorTrait>(
        resolution_table: &mut ResolutionTable,
        arg: &Input,
        package_store: &VerifiedPackageStore<'_>,
        object_store: &dyn ExecutionState,
    ) -> Result<(), E> {
        let id = match arg {
            Input::Pure(_) | Input::Receiving(_) => return Ok(()),
            Input::ImmutableOrOwned(oref) => *oref.object_id(),
            Input::Shared(shared) => shared.object_id,
            _ => unimplemented!("a new Input enum variant was added and needs to be handled"),
        };
        let Some(obj) = object_store.read_object(&id) else {
            invariant_violation!("Object {:?} not found in object store", id);
        };
        let Some(ty) = obj.data.opt_object_type() else {
            invariant_violation!("Object {:?} has does not have a Move type", id);
        };

        // invariant: the addresses in the type are defining addresses for the types since
        // these are the types of the objects as stored on-chain.
        let tag = core_struct_tag(ty);
        let ids = tag
            .all_addresses()
            .into_iter()
            .map(AccountAddressExt::to_object_id);
        resolution_table.add_type_linkages_to_table(ids, package_store)?;

        Ok(())
    }

    fn command<E: ExecutionErrorTrait>(
        resolution_table: &mut ResolutionTable,
        command: &Command,
        package_store: &VerifiedPackageStore<'_>,
    ) -> Result<(), E> {
        let mut add_ty_input = |ty: &iota_sdk_types::TypeTag| -> Result<(), E> {
            let tag = type_tag_sdk_to_core(ty);
            let ids = tag
                .all_addresses()
                .into_iter()
                .map(AccountAddressExt::to_object_id);
            resolution_table.add_type_linkages_to_table(ids, package_store)
        };
        match command {
            Command::MoveCall(MoveCall {
                package,
                type_arguments,
                ..
            }) => {
                type_arguments.iter().try_for_each(add_ty_input)?;
                resolution_table.add_type_linkages_to_table([*package], package_store)?;
            }
            Command::MakeMoveVector(MakeMoveVector {
                type_tag: Some(ty), ..
            }) => {
                add_ty_input(ty)?;
            }
            Command::MakeMoveVector(MakeMoveVector { type_tag: None, .. })
            | Command::TransferObjects(_)
            | Command::SplitCoins(_)
            | Command::MergeCoins(_) => (),
            Command::Publish(Publish { dependencies, .. }) => {
                resolution_table.add_type_linkages_to_table(dependencies, package_store)?;
            }
            Command::Upgrade(Upgrade {
                dependencies,
                package,
                ..
            }) => {
                resolution_table.add_type_linkages_to_table([*package], package_store)?;
                resolution_table.add_type_linkages_to_table(dependencies, package_store)?;
            }
            _ => unimplemented!("a new Command enum variant was added and needs to be handled"),
        }

        Ok(())
    }
}
