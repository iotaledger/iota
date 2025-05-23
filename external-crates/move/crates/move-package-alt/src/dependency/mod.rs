// Copyright (c) The Diem Core Contributors
// Copyright (c) The Move Contributors
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod dependency_set;
// TODO: this shouldn't be pub; need to move resolver error into resolver module
pub mod external;
mod git;
mod local;

pub use dependency_set::DependencySet;

use std::{
    collections::BTreeMap,
    fmt::{self, Debug},
    marker::PhantomData,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use derive_where::derive_where;
use serde::{Deserialize, Serialize};

use crate::{
    errors::PackageResult,
    flavor::MoveFlavor,
    package::{EnvironmentName, PackageName, PackagePath},
};

use external::ExternalDependency;
use git::GitDependency;
pub use git::{PinnedGitDependency, UnpinnedGitDependency, fetch_dep};
use local::LocalDependency;

// TODO (potential refactor): consider using objects for manifest dependencies (i.e. `Box<dyn UnpinnedDependency>`).
//      part of the complexity here would be deserialization - probably need a flavor-specific
//      function that converts a toml value to a Box<dyn UnpinnedDependency>
//
//      resolution would also be interesting because of batch resolution. Would probably need a
//      trait method to return a resolver object, and then a method on the resolver object to
//      resolve a bunch of dependencies (resolvers could implement Eq)
//
// TODO: maybe rename ManifestDependencyInfo to UnpinnedDependency

/// Phantom type to represent pinned dependencies (see [PinnedDependency])
#[derive(Debug)]
pub struct Pinned;

/// Phantom type to represent unpinned dependencies (see
/// [ManifestDependencyInfo])
#[derive(Debug)]
pub struct Unpinned;

/// [ManifestDependencyInfo]s contain the dependency-type-specific things that
/// users write in their Move.toml files in the `dependencies` section.
///
/// There are additional general fields in the manifest format (like `override`
/// or `rename-from`) that are not part of the ManifestDependencyInfo. We
/// separate these partly because these things are not serialized to the Lock
/// file. See [crate::package::manifest] for the full representation of an entry
/// in the `dependencies` table.
// TODO: this paragraph will change with upcoming design changes
#[derive(Debug, Serialize, Deserialize)]
#[derive_where(Clone)]
#[serde(untagged)]
pub enum ManifestDependencyInfo<F: MoveFlavor> {
    Git(GitDependency<Unpinned>),
    External(ExternalDependency),
    Local(LocalDependency),
    FlavorSpecific(F::FlavorDependency<Unpinned>),
}

/// Pinned dependencies are guaranteed to always resolve to the same package
/// source. For example, a git dependendency with a branch or tag revision may
/// change over time (and is thus not pinned), whereas a git dependency with a
/// sha revision is always guaranteed to produce the same files.
///
/// Local dependencies are a somewhat special case here - we want to pin them as
/// local deps during development, because the developer would expect to use the
/// latest code without having to explicitly repin, but we need to convert them
/// to persistent dependencies when we publish since we want to retain that
/// information for source verification.
#[derive(Debug, Serialize, Deserialize)]
#[derive_where(Clone)]
#[serde(untagged)]
pub enum PinnedDependencyInfo<F: MoveFlavor + ?Sized> {
    Git(GitDependency<Pinned>),
    Local(LocalDependency),
    FlavorSpecific(F::FlavorDependency<Pinned>),
}

impl<F: MoveFlavor> PinnedDependencyInfo<F> {
    /// Return a dependency representing the root package
    pub fn root_dependency() -> Self {
        Self::Local(LocalDependency { local: PathBuf::from(".") })
    }

    pub async fn fetch(&self) -> PackagePath {
        // TODO: take this from [Package]
        todo!()
    }

    /// Return the absolute path to the directory that this package would be fetched into, without
    /// actually fetching it
    pub fn unfetched_path(&self) -> PathBuf {
        todo!()
    }
}

/// Split up deps into kinds. The union of the output sets is the same as [deps]
#[allow(clippy::type_complexity)]
fn split<F: MoveFlavor>(
    deps: &DependencySet<ManifestDependencyInfo<F>>,
) -> (
    DependencySet<GitDependency<Unpinned>>,
    DependencySet<ExternalDependency>,
    DependencySet<LocalDependency>,
    DependencySet<F::FlavorDependency<Unpinned>>,
) {
    use DependencySet as DS;
    use ManifestDependencyInfo as M;

    let mut gits = DS::new();
    let mut exts = DS::new();
    let mut locs = DS::new();
    let mut flav = DS::new();

    for (env, package_name, dep) in deps.clone().into_iter() {
        match dep {
            M::Git(info) => gits.insert(env, package_name, info),
            M::External(info) => exts.insert(env, package_name, info),
            M::Local(info) => locs.insert(env, package_name, info),
            M::FlavorSpecific(info) => flav.insert(env, package_name, info),
        }
    }

    (gits, exts, locs, flav)
}

// TODO: this will change with upcoming design changes:
/// Replace all dependencies with their pinned versions. The returned set may have a different set
/// of keys than the input, for example if new implicit dependencies are added or if external
/// resolvers resolve default deps to dep-overrides, or if dep-overrides are identical to the
/// default deps.
pub async fn pin<F: MoveFlavor>(
    flavor: &F,
    deps: &DependencySet<ManifestDependencyInfo<F>>, // TODO: maybe take by value?
    envs: &BTreeMap<EnvironmentName, F::EnvironmentID>,
) -> PackageResult<DependencySet<PinnedDependencyInfo<F>>> {
    let (mut gits, mut exts, mut locs, mut flav) = split(deps);

    // TODO: errors!
    let resolved = ExternalDependency::resolve::<F>(exts, envs).await.unwrap();

    let (resolved_gits, resolved_exts, resolved_locs, resolved_flav) = split(&resolved);

    // ensure that there are no more externally resolved deps
    if !resolved_exts.is_empty() {
        // TODO: error!
        panic!("External resolver returned external dependency");
    }

    gits.extend(resolved_gits);
    locs.extend(resolved_locs);
    flav.extend(resolved_flav);

    let pinned_gits: DependencySet<PinnedDependencyInfo<F>> = GitDependency::pin(gits)
        .unwrap() // TODO: error collection!
        .into_iter()
        .map(|(env, package, dep)| (env, package, P::Git::<F>(dep)))
        .collect();

    let pinned_locs = locs
        .into_iter()
        .map(|(env, package, dep)| (env, package, P::Local::<F>(dep)))
        .collect();

    let pinned_flav = flavor
        .pin(flav)
        .unwrap() // TODO: Errors!
        .into_iter()
        .map(|(env, package, dep)| (env, package, P::FlavorSpecific::<F>(dep.clone())))
        .collect();

    Ok(DependencySet::merge([
        pinned_gits,
        pinned_locs,
        pinned_flav,
    ]))
}

// TODO: this will change with the upcoming design changes:
/// For each environment, if none of the implicit dependencies are present in [deps] (or the
/// default environment), then they are all added.
// TODO: what's the notion of identity used here?
fn add_implicit_deps<F: MoveFlavor>(
    flavor: &F,
    deps: &mut DependencySet<PinnedDependencyInfo<F>>,
) -> PackageResult<()> {
    todo!()
}

/// Ensure that all dependencies are stored locally and return the paths to
/// their contents. The returned map is guaranteed to have the same keys as
/// [deps].
fn fetch<F: MoveFlavor>(
    deps: DependencySet<PinnedDependencyInfo<F>>,
) -> PackageResult<DependencySet<PathBuf>> {
    todo!()
}

// TODO: unit tests
#[cfg(test)]
mod tests {}
