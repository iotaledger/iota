// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_types::{ObjectId, StructTag};
use iota_types::{
    error::{IotaError, IotaResult},
    execution::TypeLayoutStore,
    layout_resolver::LayoutResolver,
    storage::{BackingPackageStore, PackageObject},
};
use move_core_types::annotated_value as A;
use move_vm_runtime::move_vm::MoveVM;

use crate::{
    data_store::{cached_data_store::CachedPackageStore, linkage_view::LinkageView},
    programmable_transactions::context::load_type_from_struct,
};

/// Retrieve a `MoveStructLayout` from a `Type`.
/// Invocation into the `Session` to leverage the `LinkageView` implementation
/// common to the runtime.
pub struct TypeLayoutResolver<'state, 'vm> {
    vm: &'vm MoveVM,
    linkage_view: LinkageView<'state>,
}

/// Implements IotaResolver traits by providing a null implementation for module
/// resolution and delegating backing package resolution to the trait object.
struct NullIotaResolver<'state>(Box<dyn TypeLayoutStore + 'state>);

impl<'state, 'vm> TypeLayoutResolver<'state, 'vm> {
    pub fn new(vm: &'vm MoveVM, state_view: Box<dyn TypeLayoutStore + 'state>) -> Self {
        let linkage_view = LinkageView::new(Box::new(CachedPackageStore::new(Box::new(
            NullIotaResolver(state_view),
        ))));
        Self { vm, linkage_view }
    }
}

impl LayoutResolver for TypeLayoutResolver<'_, '_> {
    fn get_annotated_layout(
        &mut self,
        struct_tag: &StructTag,
    ) -> Result<A::MoveDatatypeLayout, IotaError> {
        let Ok(ty) = load_type_from_struct(self.vm, &self.linkage_view, &[], struct_tag) else {
            return Err(IotaError::FailObjectLayout {
                st: format!("{struct_tag}"),
            });
        };
        let layout = self.vm.get_runtime().type_to_fully_annotated_layout(&ty);
        match layout {
            Ok(A::MoveTypeLayout::Struct(s)) => Ok(A::MoveDatatypeLayout::Struct(s)),
            Ok(A::MoveTypeLayout::Enum(e)) => Ok(A::MoveDatatypeLayout::Enum(e)),
            _ => Err(IotaError::FailObjectLayout {
                st: format!("{struct_tag}"),
            }),
        }
    }
}

impl BackingPackageStore for NullIotaResolver<'_> {
    fn get_package_object(&self, package_id: &ObjectId) -> IotaResult<Option<PackageObject>> {
        self.0.get_package_object(package_id)
    }
}
