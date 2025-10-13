// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;

use move_package_alt::{
    dependency::{self, Pinned, PinnedDependencyInfo, Unpinned},
    errors::PackageResult,
    flavor::MoveFlavor,
    package::PackageName,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename = "kebab-case")]
pub struct OnChainDependency {
    on_chain: bool,
}

#[derive(Debug)]
pub struct IotaFlavor;

impl MoveFlavor for IotaFlavor {
    type FlavorDependency<P: ?Sized> = OnChainDependency;

    fn pin(
        &self,
        deps: BTreeMap<PackageName, Self::FlavorDependency<Unpinned>>,
    ) -> PackageResult<BTreeMap<PackageName, Self::FlavorDependency<Pinned>>> {
        todo!()
    }

    fn fetch(
        &self,
        deps: BTreeMap<PackageName, Self::FlavorDependency<Pinned>>,
    ) -> PackageResult<BTreeMap<PackageName, std::path::PathBuf>> {
        todo!()
    }

    type PublishedMetadata = (); // TODO

    type EnvironmentID = (); // TODO

    type AddressInfo = (); // TODO

    type PackageMetadata = (); // TODO

    fn implicit_deps(&self, id: Self::EnvironmentID) -> Vec<PinnedDependencyInfo<Self>> {
        todo!()
    }
}
