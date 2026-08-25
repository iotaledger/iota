// Copyright (c) The Diem Core Contributors
// Copyright (c) The Move Contributors
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Defines the [Vanilla] implementation of the [MoveFlavor] trait. This
//! implementation supports no flavor-specific resolvers and stores no
//! additional metadata in the lockfile.

use std::{
    collections::{self, BTreeMap},
    iter::empty,
    marker::PhantomData,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use super::MoveFlavor;
use crate::{
    dependency::{DependencySet, Pinned, PinnedDependencyInfo, Unpinned},
    errors::PackageResult,
    package::PackageName,
};

/// The [Vanilla] implementation of the [MoveFlavor] trait. This implementation
/// supports no flavor-specific resolvers and stores no additional metadata in
/// the lockfile.
#[derive(Debug)]
pub struct Vanilla;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum VanillaDep {}

impl MoveFlavor for Vanilla {
    type PublishedMetadata = ();
    type PackageMetadata = ();
    type EnvironmentID = String;
    type AddressInfo = ();

    fn name() -> String {
        "vanilla".to_string()
    }

    fn implicit_deps(
        &self,
        environments: impl Iterator<Item = Self::EnvironmentID>,
    ) -> DependencySet<PinnedDependencyInfo> {
        empty().collect()
    }
}
