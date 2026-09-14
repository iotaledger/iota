// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap, HashSet, hash_map::Entry},
    rc::Rc,
    str::FromStr,
};

use iota_sdk_types::{
    ObjectId,
    move_package::{MovePackage, TypeOrigin, UpgradeInfo},
};
use iota_types::{
    error::{ExecutionError, IotaError, IotaResult},
    iota_sdk_types_conversions::identifier_core_to_sdk,
    move_package::MovePackageExt,
};
use move_core_types::{
    account_address::AccountAddress,
    identifier::{IdentStr, Identifier},
    language_storage::ModuleId,
    resolver::{LinkageResolver, ModuleResolver},
};

use crate::data_store::PackageStore;

/// Exposes module and linkage resolution to the Move runtime.  The first by
/// delegating to `resolver` and the second via linkage information that is
/// loaded from a move package.
pub struct LinkageView<'state> {
    /// Interface to resolve packages, modules and resources directly from the
    /// store, and possibly from other sources (e.g., packages just
    /// published).
    resolver: Box<dyn PackageStore + 'state>,
    /// Information used to change module and type identities during linkage.
    linkage_info: RefCell<Option<LinkageInfo>>,
    /// Cache containing the type origin information from every package that has
    /// been set as the link context, and every other type that has been
    /// requested by the loader in this session. It's okay to retain entries
    /// in this cache between different link contexts because a type's
    /// Runtime ID and Defining ID are invariant between across link contexts.
    ///
    /// Cache is keyed first by the Runtime ID of the type's module, and then
    /// the type's identifier. The value is the ObjectId/Address of the
    /// package that introduced the type.
    type_origin_cache: RefCell<HashMap<ModuleId, HashMap<Identifier, AccountAddress>>>,
    /// Cache of past package addresses that have been the link context -- if a
    /// package is in this set, then we will not try to load its type origin
    /// table when setting it as a context (again).
    past_contexts: RefCell<HashSet<ObjectId>>,
}

#[derive(Debug)]
pub struct LinkageInfo {
    storage_id: AccountAddress,
    runtime_id: AccountAddress,
    link_table: BTreeMap<ObjectId, UpgradeInfo>,
}

pub struct SavedLinkage(LinkageInfo);

impl<'state> LinkageView<'state> {
    /// Creates a new `LinkageView` instance with the provided `PackageStore`.
    /// This instance is responsible for resolving and linking types across
    /// different contexts. It initializes internal caches for type origins
    /// and past contexts.
    pub fn new(resolver: Box<dyn PackageStore + 'state>) -> Self {
        Self {
            resolver,
            linkage_info: RefCell::new(None),
            type_origin_cache: RefCell::new(HashMap::new()),
            past_contexts: RefCell::new(HashSet::new()),
        }
    }

    /// Reset the `LinkageInfo`.
    pub fn reset_linkage(&self) -> Result<(), ExecutionError> {
        let Ok(mut linkage_info) = self.linkage_info.try_borrow_mut() else {
            invariant_violation!("Unable to to reset linkage")
        };
        *linkage_info = None;
        Ok(())
    }

    /// Indicates whether this `LinkageView` has had its context set to match
    /// the linkage in `context`.
    pub fn has_linkage(&self, context: ObjectId) -> Result<bool, ExecutionError> {
        let Ok(linkage_info) = self.linkage_info.try_borrow() else {
            invariant_violation!("Unable to borrow linkage info")
        };
        Ok(linkage_info
            .as_ref()
            .is_some_and(|l| l.storage_id.as_ref() == context.as_bytes()))
    }

    /// Reset the linkage, but save the context that existed before, if there
    /// was one.
    pub fn steal_linkage(&self) -> Option<SavedLinkage> {
        Some(SavedLinkage(self.linkage_info.take()?))
    }

    /// Restore a previously saved linkage context.  Fails if there is already a
    /// context set.
    pub fn restore_linkage(&self, saved: Option<SavedLinkage>) -> Result<(), ExecutionError> {
        let Some(SavedLinkage(saved)) = saved else {
            return Ok(());
        };

        let Ok(mut linkage_info) = self.linkage_info.try_borrow_mut() else {
            invariant_violation!("Unable to to borrow linkage while restoring")
        };
        if let Some(existing) = &*linkage_info {
            invariant_violation!(
                "Attempt to overwrite linkage by restoring: {saved:#?} \
                 Existing linkage: {existing:#?}",
            )
        }

        // No need to populate type origin cache, because a saved context must have been
        // set as a linkage before, and the cache would have been populated at
        // that time.
        *linkage_info = Some(saved);
        Ok(())
    }

    /// Set the linkage context to the information based on the linkage and type
    /// origin tables from the `context` package.  Returns the original
    /// package ID (aka the runtime ID) of the context package on success.
    pub fn set_linkage(&self, context: &MovePackage) -> Result<AccountAddress, ExecutionError> {
        let Ok(mut linkage_info) = self.linkage_info.try_borrow_mut() else {
            invariant_violation!("Unable to to borrow linkage to set")
        };
        if let Some(existing) = &*linkage_info {
            invariant_violation!(
                "Attempt to overwrite linkage info with context from {}. \
                    Existing linkage: {existing:#?}",
                context.id(),
            )
        }

        let linkage = LinkageInfo::from(context);
        let storage_id = context.id();
        let runtime_id = linkage.runtime_id;
        *linkage_info = Some(linkage);

        if !self.past_contexts.borrow_mut().insert(storage_id) {
            return Ok(runtime_id);
        }

        // Pre-populate the type origin cache with entries from the current package --
        // this is necessary to serve "defining module" requests for unpublished
        // packages, but will also speed up other requests.
        for TypeOrigin {
            module_name,
            datatype_name,
            package: defining_id,
        } in context.type_origin_table()
        {
            let Ok(module_name) = Identifier::from_str(module_name) else {
                invariant_violation!("Module name isn't an identifier: {module_name}");
            };

            let Ok(datatype_name) = Identifier::from_str(datatype_name) else {
                invariant_violation!("Struct name isn't an identifier: {datatype_name}");
            };

            let runtime_id = ModuleId::new(runtime_id, module_name);
            self.add_type_origin(runtime_id, datatype_name, *defining_id)?;
        }

        Ok(runtime_id)
    }

    /// Retrieves the original package ID (as an `AccountAddress`) from the
    /// linkage information, if available.
    pub fn original_package_id(&self) -> Result<Option<AccountAddress>, ExecutionError> {
        let Ok(linkage_info) = self.linkage_info.try_borrow() else {
            invariant_violation!("Unable to borrow linkage info")
        };
        Ok(linkage_info.as_ref().map(|info| info.runtime_id))
    }

    /// Retrieves the cached type origin for the given `ModuleId` and struct
    /// identifier (`IdentStr`). This method uses the internal
    /// `type_origin_cache` to provide fast lookups for previously resolved
    /// types.
    fn get_cached_type_origin(
        &self,
        runtime_id: &ModuleId,
        struct_: &IdentStr,
    ) -> Option<AccountAddress> {
        self.type_origin_cache
            .borrow()
            .get(runtime_id)?
            .get(struct_)
            .cloned()
    }

    /// Adds a type origin to the cache, associating the given `ModuleId` and
    /// struct identifier (`Identifier`) with the provided defining `ObjectId`.
    fn add_type_origin(
        &self,
        runtime_id: ModuleId,
        struct_: Identifier,
        defining_id: ObjectId,
    ) -> Result<(), ExecutionError> {
        let mut cache = self.type_origin_cache.borrow_mut();
        let module_cache = cache.entry(runtime_id.clone()).or_default();

        match module_cache.entry(struct_) {
            Entry::Vacant(entry) => {
                entry.insert(AccountAddress::new(defining_id.into_bytes()));
            }

            Entry::Occupied(entry) => {
                if entry.get().as_ref() != defining_id.as_bytes() {
                    invariant_violation!(
                        "Conflicting defining ID for {}::{}: {} and {}",
                        runtime_id,
                        entry.key(),
                        defining_id,
                        entry.get(),
                    );
                }
            }
        }

        Ok(())
    }

    /// Retrieves the current link context's storage ID as an `AccountAddress`.
    pub(crate) fn link_context(&self) -> Result<AccountAddress, ExecutionError> {
        let Ok(linkage_info) = self.linkage_info.try_borrow() else {
            invariant_violation!("Unable to borrow linkage info")
        };
        Ok(linkage_info
            .as_ref()
            .map_or(AccountAddress::ZERO, |l| l.storage_id))
    }

    /// Relocates a given `ModuleId` based on the current linkage context.
    pub(crate) fn relocate(&self, module_id: &ModuleId) -> Result<ModuleId, IotaError> {
        let Ok(linkage_info) = self.linkage_info.try_borrow() else {
            invariant_violation!("Unable to borrow linkage info")
        };
        let Some(linkage) = &*linkage_info else {
            invariant_violation!("No linkage context set while relocating {module_id}.")
        };

        // The request is to relocate a module in the package that the link context is
        // from.  This entry will not be stored in the linkage table, so must be
        // handled specially.
        if module_id.address() == &linkage.runtime_id {
            return Ok(ModuleId::new(
                linkage.storage_id,
                module_id.name().to_owned(),
            ));
        }

        let runtime_id = ObjectId::new(module_id.address().into_bytes());
        let Some(upgrade) = linkage.link_table.get(&runtime_id) else {
            invariant_violation!(
                "Missing linkage for {runtime_id} in context {}, runtime_id is {}",
                linkage.storage_id,
                linkage.runtime_id
            );
        };

        Ok(ModuleId::new(
            AccountAddress::new(upgrade.upgraded_id.into_bytes()),
            module_id.name().to_owned(),
        ))
    }

    /// Determines the defining module for a given struct within a `ModuleId`.
    /// The function first checks the cached type origin and returns the
    /// corresponding `ModuleId` if found. If not, it relocates the
    /// module and queries the type origin table from the associated package. If
    /// the defining module is found, it caches the result and returns the
    /// `ModuleId`.
    pub(crate) fn defining_module(
        &self,
        runtime_id: &ModuleId,
        struct_: &IdentStr,
    ) -> Result<ModuleId, IotaError> {
        let Ok(linkage_info) = self.linkage_info.try_borrow() else {
            invariant_violation!("Unable to borrow linkage info")
        };
        if linkage_info.is_none() {
            invariant_violation!(
                "No linkage context set for defining module query on {runtime_id}::{struct_}."
            )
        }

        if let Some(cached) = self.get_cached_type_origin(runtime_id, struct_) {
            return Ok(ModuleId::new(cached, runtime_id.name().to_owned()));
        }

        let storage_id = ObjectId::new(self.relocate(runtime_id)?.address().into_bytes());
        let Some(package) = self.resolver.get_package(&storage_id)? else {
            invariant_violation!("Missing dependent package in store: {storage_id}",)
        };

        for TypeOrigin {
            module_name,
            datatype_name,
            package,
        } in package.type_origin_table()
        {
            if module_name == runtime_id.name().as_str() && datatype_name == struct_.as_str() {
                self.add_type_origin(runtime_id.clone(), struct_.to_owned(), *package)?;
                return Ok(ModuleId::new(
                    AccountAddress::new(package.into_bytes()),
                    runtime_id.name().to_owned(),
                ));
            }
        }

        invariant_violation!(
            "{runtime_id}::{struct_} not found in type origin table in {storage_id} (v{})",
            package.version(),
        )
    }
}

impl From<&MovePackage> for LinkageInfo {
    fn from(package: &MovePackage) -> Self {
        Self {
            storage_id: AccountAddress::new(package.id().into_bytes()),
            runtime_id: AccountAddress::new(package.original_package_id().into_bytes()),
            link_table: package.linkage_table().clone(),
        }
    }
}

impl LinkageResolver for LinkageView<'_> {
    type Error = IotaError;

    fn link_context(&self) -> AccountAddress {
        // TODO should we propagate the error
        LinkageView::link_context(self).unwrap()
    }

    fn relocate(&self, module_id: &ModuleId) -> Result<ModuleId, Self::Error> {
        LinkageView::relocate(self, module_id)
    }

    fn defining_module(
        &self,
        runtime_id: &ModuleId,
        struct_: &IdentStr,
    ) -> Result<ModuleId, Self::Error> {
        LinkageView::defining_module(self, runtime_id, struct_)
    }
}

impl ModuleResolver for LinkageView<'_> {
    type Error = IotaError;

    fn get_module(&self, id: &ModuleId) -> Result<Option<Vec<u8>>, Self::Error> {
        Ok(self
            .get_package(&ObjectId::new(id.address().into_bytes()))?
            .and_then(|package| {
                package
                    .serialized_module_map()
                    .get(&identifier_core_to_sdk(id.name()))
                    .cloned()
            }))
    }
}

impl PackageStore for LinkageView<'_> {
    fn get_package(&self, package_id: &ObjectId) -> IotaResult<Option<Rc<MovePackage>>> {
        self.resolver.get_package(package_id)
    }
}
