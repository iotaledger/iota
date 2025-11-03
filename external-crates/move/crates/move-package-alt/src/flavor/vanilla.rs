// Copyright (c) The Diem Core Contributors
// Copyright (c) The Move Contributors
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Defines the [Vanilla] implementation of the [MoveFlavor] trait. This
//! implementation supports no flavor-specific resolvers and stores no
//! additional metadata in the lockfile.

use std::{
    collections::{self, BTreeMap},
    marker::PhantomData,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use super::MoveFlavor;
use crate::{
    dependency::{Pinned, PinnedDependencyInfo, Unpinned},
    errors::PackageResult,
    package::PackageName,
};

/// The [Vanilla] implementation of the [MoveFlavor] trait. This implementation
/// supports no flavor-specific resolvers and stores no additional metadata in
/// the lockfile.
#[derive(Debug)]
pub struct Vanilla;

impl MoveFlavor for Vanilla {
    type PublishedMetadata = ();
    type PackageMetadata = ();
    type EnvironmentID = String;
    type AddressInfo = ();

    fn implicit_deps(&self, environment: Self::EnvironmentID) -> Vec<PinnedDependencyInfo<Self>> {
        vec![]
    }

    // TODO: should be !, but that's not supported; instead
    // should be some type that always gives an error during
    // deserialization
    type FlavorDependency<P: ?Sized> = ();

    fn pin(
        &self,
        deps: BTreeMap<PackageName, Self::FlavorDependency<Unpinned>>,
    ) -> PackageResult<BTreeMap<PackageName, Self::FlavorDependency<Pinned>>> {
        // always an error
        todo!()
    }

    fn fetch(
        &self,
        deps: BTreeMap<PackageName, Self::FlavorDependency<Pinned>>,
    ) -> PackageResult<BTreeMap<PackageName, PathBuf>> {
        // always an error
        todo!()
    }
}
