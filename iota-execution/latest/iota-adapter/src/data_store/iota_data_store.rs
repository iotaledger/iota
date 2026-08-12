// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::rc::Rc;

use iota_sdk_types::{ObjectId, move_package::MovePackage};
use iota_types::{error::IotaResult, iota_sdk_types_conversions::identifier_core_to_sdk};
use move_binary_format::errors::{Location, PartialVMError, PartialVMResult, VMResult};
use move_core_types::{
    account_address::AccountAddress, identifier::IdentStr, language_storage::ModuleId,
    resolver::ModuleResolver, vm_status::StatusCode,
};
use move_vm_types::data_store::DataStore;

use crate::data_store::{PackageStore, linkage_view::LinkageView};

// Implementation of the `DataStore` trait for the Move VM.
// When used during execution it may have a list of new packages that have
// just been published in the current context. Those are used for module/type
// resolution when executing module init.
// It may be created with an empty slice of packages either when no
// publish/upgrade are performed or when a type is requested not during
// execution.
pub(crate) struct IotaDataStore<'state, 'a> {
    linkage_view: &'a LinkageView<'state>,
    new_packages: &'a [MovePackage],
}

impl<'state, 'a> IotaDataStore<'state, 'a> {
    pub(crate) fn new(
        linkage_view: &'a LinkageView<'state>,
        new_packages: &'a [MovePackage],
    ) -> Self {
        Self {
            linkage_view,
            new_packages,
        }
    }

    fn get_module(&self, module_id: &ModuleId) -> Option<&Vec<u8>> {
        for package in self.new_packages {
            if package.id != ObjectId::from(module_id.address().into_bytes()) {
                continue;
            }

            let module = package.get_module(&identifier_core_to_sdk(module_id.name()));

            if module.is_some() {
                return module;
            }
        }
        None
    }
}

// TODO: `DataStore` will be reworked and this is likely to disappear.
//       Leaving this comment around until then as testament to better days to
// come...
impl DataStore for IotaDataStore<'_, '_> {
    fn link_context(&self) -> AccountAddress {
        self.linkage_view.link_context()
    }

    fn relocate(&self, module_id: &ModuleId) -> PartialVMResult<ModuleId> {
        self.linkage_view.relocate(module_id).map_err(|err| {
            PartialVMError::new(StatusCode::LINKER_ERROR)
                .with_message(format!("Error relocating {module_id}: {err:?}"))
        })
    }

    fn defining_module(
        &self,
        runtime_id: &ModuleId,
        struct_: &IdentStr,
    ) -> PartialVMResult<ModuleId> {
        self.linkage_view
            .defining_module(runtime_id, struct_)
            .map_err(|err| {
                PartialVMError::new(StatusCode::LINKER_ERROR).with_message(format!(
                    "Error finding defining module for {runtime_id}::{struct_}: {err:?}"
                ))
            })
    }

    fn load_module(&self, module_id: &ModuleId) -> VMResult<Vec<u8>> {
        if let Some(bytes) = self.get_module(module_id) {
            return Ok(bytes.clone());
        }
        match self.linkage_view.get_module(module_id) {
            Ok(Some(bytes)) => Ok(bytes),
            Ok(None) => Err(PartialVMError::new(StatusCode::LINKER_ERROR)
                .with_message(format!("Cannot find {module_id:?} in data cache"))
                .finish(Location::Undefined)),
            Err(err) => {
                let msg = format!("Unexpected storage error: {err:?}");
                Err(
                    PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR)
                        .with_message(msg)
                        .finish(Location::Undefined),
                )
            }
        }
    }

    fn publish_module(&mut self, _module_id: &ModuleId, _blob: Vec<u8>) -> VMResult<()> {
        // we cannot panic here because during execution and publishing this is
        // currently called from the publish flow in the Move runtime
        Ok(())
    }
}

impl PackageStore for IotaDataStore<'_, '_> {
    fn get_package(&self, id: &ObjectId) -> IotaResult<Option<Rc<MovePackage>>> {
        for package in self.new_packages {
            if package.id() == *id {
                return Ok(Some(Rc::new(package.clone())));
            }
        }
        self.linkage_view.get_package(id)
    }
}
