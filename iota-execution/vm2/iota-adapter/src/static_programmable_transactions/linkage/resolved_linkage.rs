// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{borrow::Borrow, collections::BTreeMap, rc::Rc};

use iota_sdk_types::ObjectId;
use move_vm_runtime::shared::linkage_context::LinkageContext;

use crate::{
    compat::ExecutionErrorTrait,
    data_store::VerifiedPackageStore,
    sdk_conv::ToAccountAddress,
    static_programmable_transactions::linkage::{
        config::ResolutionConfig,
        resolution::{PackageResolution, ResolutionTable, VersionConstraint},
    },
};

#[derive(Clone, Debug)]
pub struct ExecutableLinkage(pub Rc<ResolvedLinkage>);

impl ExecutableLinkage {
    pub fn new(resolved_linkage: ResolvedLinkage) -> Self {
        Self(Rc::new(resolved_linkage))
    }

    /// Given a list of object IDs, generate a `ResolvedLinkage` for them.
    /// Since this linkage analysis should only be used for types, all packages are resolved
    /// "upwards" (i.e., later versions of the package are preferred).
    pub fn type_linkage<I, E>(
        config: ResolutionConfig,
        ids: I,
        store: &VerifiedPackageStore<'_>,
    ) -> Result<Self, E>
    where
        E: ExecutionErrorTrait,
        I: IntoIterator,
        I::Item: Borrow<ObjectId>,
    {
        let mut resolution_table = ResolutionTable::empty(config);
        resolution_table.add_type_linkages_to_table(ids, store)?;
        Ok(Self::new(ResolvedLinkage::from_resolution_table(
            resolution_table,
        )))
    }

    pub fn linkage_context<E: ExecutionErrorTrait>(&self) -> Result<LinkageContext, E> {
        LinkageContext::new(
            self.0
                .linkage
                .iter()
                .map(|(k, v)| (k.to_account_address(), v.to_account_address()))
                .collect(),
        )
        .map_err(|e| {
            make_invariant_violation!(
                "Failed to create linkage context from resolved linkage: {:?}",
                e
            )
            .into()
        })
    }
}

#[derive(Debug)]
pub struct ResolvedLinkage {
    // A mapping of original package ID to its resolved version ID for that linkage.
    pub linkage: BTreeMap<ObjectId, ObjectId>,
    // A mapping of every package ID to its runtime ID.
    // Note: Multiple packages can have the same runtime ID in this mapping, and domain of this map
    // is a superset of range of `linkage`.
    pub linkage_resolution: BTreeMap<ObjectId, PackageResolution>,
}

impl ResolvedLinkage {
    /// In the current linkage resolve an object ID to its original package ID.
    pub fn resolve_to_original_id(&self, object_id: &ObjectId) -> Option<ObjectId> {
        self.linkage_resolution
            .get(object_id)
            .map(|resolution| resolution.original_id)
    }

    /// The version of the `object_id`. `None` if this linkage never resolved that package version,
    /// or if the entry was late-bound for publication or upgrade.
    pub fn resolved_version(&self, object_id: &ObjectId) -> Option<u64> {
        self.linkage_resolution
            .get(object_id)
            .and_then(|resolution| resolution.version)
    }

    /// Create a `ResolvedLinkage` from a `ResolutionTable`.
    pub(crate) fn from_resolution_table(resolution_table: ResolutionTable) -> Self {
        let mut linkage = BTreeMap::new();
        for (original_id, resolution) in resolution_table.resolution_table {
            match resolution {
                VersionConstraint::Exact(_version, object_id)
                | VersionConstraint::AtLeast(_version, object_id) => {
                    linkage.insert(original_id, object_id);
                }
            }
        }
        Self {
            linkage,
            linkage_resolution: resolution_table.all_versions_resolution_table,
        }
    }

    /// We need to late-bind the "self" resolution since for publication and upgrade we don't know
    /// this a priori when loading the PTB.
    pub fn update_for_publication(
        package_version_id: ObjectId,
        original_package_id: ObjectId,
        mut resolved_linkage: ResolvedLinkage,
    ) -> ExecutableLinkage {
        // original package ID maps to the link context (new package ID) in this context
        resolved_linkage
            .linkage
            .insert(original_package_id, package_version_id);
        // Add resolution from the new package ID to the original package ID.
        resolved_linkage.linkage_resolution.insert(
            package_version_id,
            PackageResolution {
                original_id: original_package_id,
                version: None,
            },
        );
        ExecutableLinkage::new(resolved_linkage)
    }
}
