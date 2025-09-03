// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use better_any::{Tid, TidAble};
use iota_types::{base_types::ObjectID, storage::BackingPackageStore};
use move_binary_format::CompiledModule;
use move_core_types::identifier::IdentStr;

/// A raw module loader extension for native functions
///
/// This extension enables native functions the loading of
/// packages and modules, without any linkage or other restrictions.
///
/// For the time being it does not provide caching of any kind. In the current
/// use case of `move authentication` it only needs to load a module
/// only once for each use.
#[derive(Tid)]
pub struct RawModuleLoader<'package_store> {
    package_store: &'package_store dyn BackingPackageStore,
}

impl<'package_store> RawModuleLoader<'package_store> {
    pub fn new(package_store: &'package_store dyn BackingPackageStore) -> Self {
        RawModuleLoader { package_store }
    }

    /// Attempt to load the [CompiledModule] from global storage
    ///
    /// It requires the `Storage ID` of the given package and the name of the
    /// module associated with the package.
    ///
    /// On success the [CompiledModule] is returned otherwise None.
    pub fn get_module(
        &self,
        package_id: &ObjectID,
        module_name: &IdentStr,
    ) -> Option<CompiledModule> {
        // Errors are not propagated as in this scenario only the DB can fail, in which
        // case there is absolutely nothing that we can do.
        let Ok(package_object) = self.package_store.get_package_object(package_id) else {
            return None;
        };
        let module_bytes = package_object.and_then(|package| {
            package
                .move_package()
                .serialized_module_map()
                .get(module_name.as_str())
                .cloned()
        })?;
        CompiledModule::deserialize_with_defaults(&module_bytes).ok()
    }
}
