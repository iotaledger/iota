// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_types::{
    base_types::ObjectID,
    error::{IotaError, IotaResult},
    execution::TypeLayoutStore,
    layout_resolver::LayoutResolver,
    storage::{BackingPackageStore, PackageObject},
};
use move_core_types::{
    account_address::AccountAddress, annotated_value as A, language_storage::StructTag,
    resolver::ResourceResolver,
};
use move_vm_runtime::move_vm::MoveVM;

use crate::programmable_transactions::{context::load_type_from_struct, linkage_view::LinkageView};

/// Retrieve a `MoveStructLayout` from a `Type`.
/// Invocation into the `Session` to leverage the `LinkageView` implementation
/// common to the runtime.
pub struct TypeLayoutResolver<'state, 'vm> {
    vm: &'vm MoveVM,
    linkage_view: LinkageView<'state>,
}

/// Implements IotaResolver traits by providing null implementations for module
/// and resource resolution and delegating backing package resolution to the
/// trait object.
struct NullIotaResolver<'state>(Box<dyn TypeLayoutStore + 'state>);

impl<'state, 'vm> TypeLayoutResolver<'state, 'vm> {
    pub fn new(vm: &'vm MoveVM, state_view: Box<dyn TypeLayoutStore + 'state>) -> Self {
        let linkage_view = LinkageView::new(Box::new(NullIotaResolver(state_view)));
        Self { vm, linkage_view }
    }
}

impl<'state, 'vm> LayoutResolver for TypeLayoutResolver<'state, 'vm> {
    fn get_annotated_layout(
        &mut self,
        struct_tag: &StructTag,
    ) -> Result<A::MoveDatatypeLayout, IotaError> {
        let Ok(ty) = load_type_from_struct(self.vm, &mut self.linkage_view, &[], struct_tag) else {
            return Err(IotaError::FailObjectLayout {
                st: format!("{}", struct_tag),
            });
        };
        let layout = self.vm.get_runtime().type_to_fully_annotated_layout(&ty);
        let Ok(A::MoveTypeLayout::Struct(layout)) = layout else {
            return Err(IotaError::FailObjectLayout {
                st: format!("{}", struct_tag),
            });
        };
        Ok(A::MoveDatatypeLayout::Struct(layout))
    }
}

impl<'state> BackingPackageStore for NullIotaResolver<'state> {
    fn get_package_object(&self, package_id: &ObjectID) -> IotaResult<Option<PackageObject>> {
        self.0.get_package_object(package_id)
    }
}

impl<'state> ResourceResolver for NullIotaResolver<'state> {
    type Error = IotaError;

    fn get_resource(
        &self,
        _address: &AccountAddress,
        _typ: &StructTag,
    ) -> Result<Option<Vec<u8>>, Self::Error> {
        Ok(None)
    }
}
