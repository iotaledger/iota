// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::ObjectId;
use iota_types::{
    error::{ExecutionError, IotaError, IotaResult},
    execution::TypeLayoutStore,
    execution_config_utils::to_binary_config,
    iota_sdk_types_conversions::struct_tag_sdk_to_core,
    layout_resolver::LayoutResolver,
    storage::{BackingPackageStore, PackageObject},
};
use move_core_types::{annotated_value as A, language_storage::TypeTag};
use move_vm_runtime::runtime::MoveRuntime;

use crate::{
    compat::IotaErrorKind,
    data_store::{
        cached_package_store::CachedPackageStore,
        transaction_package_store::TransactionPackageStore,
    },
    sdk_conv::AccountAddressExt,
    static_programmable_transactions::linkage::{
        config::{LinkageConfig, ResolutionConfig},
        resolved_linkage::ExecutableLinkage,
    },
};

/// Retrieve a `MoveStructLayout` from a `Type`.
pub struct TypeLayoutResolver<'state, 'runtime> {
    vm: &'runtime MoveRuntime,
    protocol_config: &'runtime ProtocolConfig,
    state_view: Box<dyn TypeLayoutStore + 'state>,
}

/// Implements BackingPackageStore traits by providing null implementations for module
/// resolution and delegating backing package resolution to the trait object.
struct NullPackageStore<'a, 'state>(&'a (dyn TypeLayoutStore + 'state));

impl<'state, 'runtime> TypeLayoutResolver<'state, 'runtime> {
    pub fn new(
        vm: &'runtime MoveRuntime,
        protocol_config: &'runtime ProtocolConfig,
        state_view: Box<dyn TypeLayoutStore + 'state>,
    ) -> Self {
        Self {
            vm,
            protocol_config,
            state_view,
        }
    }
}

impl LayoutResolver for TypeLayoutResolver<'_, '_> {
    fn get_annotated_layout(
        &mut self,
        struct_tag: &iota_sdk_types::StructTag,
    ) -> Result<A::MoveDatatypeLayout, IotaError> {
        let struct_tag = &struct_tag_sdk_to_core(struct_tag);
        let ids = struct_tag
            .all_addresses()
            .into_iter()
            .map(AccountAddressExt::to_object_id);
        let null_resolver = NullPackageStore(&self.state_view);
        let resolver =
            CachedPackageStore::new(self.vm, TransactionPackageStore::new(&null_resolver));
        let config = ResolutionConfig::new(
            LinkageConfig::new(true),
            to_binary_config(self.protocol_config, None),
        );
        let tag_linkage =
            ExecutableLinkage::type_linkage::<_, ExecutionError>(config, ids, &resolver)?;
        let link_context = tag_linkage.linkage_context::<ExecutionError>()?;
        let data_store = TransactionPackageStore::new(&null_resolver);
        let Ok(vm) = self.vm.make_vm(data_store, link_context) else {
            return Err(IotaErrorKind::FailObjectLayout {
                st: format!("{struct_tag}"),
            });
        };

        let type_tag = TypeTag::Struct(Box::new(struct_tag.clone()));
        match vm.annotated_type_layout(&type_tag) {
            Ok(A::MoveTypeLayout::Struct(s)) => Ok(A::MoveDatatypeLayout::Struct(s)),
            Ok(A::MoveTypeLayout::Enum(e)) => Ok(A::MoveDatatypeLayout::Enum(e)),
            _ => Err(IotaErrorKind::FailObjectLayout {
                st: format!("{struct_tag}"),
            }),
        }
    }
}

impl BackingPackageStore for NullPackageStore<'_, '_> {
    fn get_package_object(&self, package_id: &ObjectId) -> IotaResult<Option<PackageObject>> {
        self.0.get_package_object(package_id)
    }
}
