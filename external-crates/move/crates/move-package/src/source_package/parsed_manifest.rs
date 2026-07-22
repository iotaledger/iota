// Copyright (c) The Diem Core Contributors
// Copyright (c) The Move Contributors
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::BTreeMap,
    path::{Component, Path, PathBuf},
};

use anyhow::{Result, bail};
use move_compiler::editions::{Edition, Flavor};
use move_core_types::account_address::AccountAddress;
use move_symbol_pool::symbol::Symbol;
use serde::{Deserialize, Serialize};

pub type NamedAddress = Symbol;
pub type PackageName = Symbol;
pub type FileName = Symbol;
pub type PackageDigest = Symbol;
pub type DepOverride = bool;

pub type AddressDeclarations = BTreeMap<NamedAddress, Option<AccountAddress>>;
pub type DevAddressDeclarations = BTreeMap<NamedAddress, AccountAddress>;
pub type Version = (u64, u64, u64);
pub type Dependencies = BTreeMap<PackageName, Dependency>;
pub type Substitution = BTreeMap<NamedAddress, SubstOrRename>;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SourceManifest {
    pub package: PackageInfo,
    pub addresses: Option<AddressDeclarations>,
    pub dev_address_assignments: Option<DevAddressDeclarations>,
    pub build: Option<BuildInfo>,
    pub dependencies: Dependencies,
    pub dev_dependencies: Dependencies,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct PackageInfo {
    pub name: PackageName,
    pub authors: Vec<Symbol>,
    pub license: Option<Symbol>,
    pub edition: Option<Edition>,
    pub flavor: Option<Flavor>,
    pub custom_properties: BTreeMap<Symbol, String>,
}

#[derive(Debug, Clone, Eq, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum Dependency {
    /// Parametrised by the binary that will resolve packages for this
    /// dependency.
    External(Symbol),
    Internal(InternalDependency),
}

#[derive(Debug, Clone, Eq, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct InternalDependency {
    pub kind: DependencyKind,
    pub subst: Option<Substitution>,
    pub digest: Option<PackageDigest>,
    pub dep_override: DepOverride,
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum DependencyKind {
    Local(PathBuf),
    Git(GitInfo),
    OnChain(OnChainInfo),
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct GitInfo {
    /// The git clone url to download from
    pub git_url: Symbol,
    /// The git revision, AKA, a commit SHA
    pub git_rev: Symbol,
    /// The path under this repo where the move package can be found -- e.g.,
    /// 'language/move-stdlib`
    pub subdir: PathBuf,
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct OnChainInfo {
    pub id: Symbol,
}

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct BuildInfo {
    pub language_version: Option<Version>,
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum SubstOrRename {
    RenameFrom(NamedAddress),
    Assign(AccountAddress),
}

impl DependencyKind {
    /// Given a dependency `self` assumed to be defined relative to a `parent`
    /// dependency which can itself be defined in terms of some grandparent
    /// dependency (not provided), update `self` to be defined relative to
    /// its grandparent.
    ///
    /// Fails if the resulting dependency cannot be described relative to the
    /// grandparent, because its path is not valid (does not point to a
    /// valid location in the filesystem for local dependencies, or within
    /// the repository for remote dependencies).
    pub fn reroot(&mut self, parent: &DependencyKind) -> Result<()> {
        let mut parent = parent.clone();

        match (&mut parent, &self) {
            // If `self` is a git or custom dependency kind, it does not need to be re-rooted
            // because its URI is already absolute. (i.e. the location of an absolute URI does not
            // change if referenced relative to some other URI).
            (_, DependencyKind::Git(_) | DependencyKind::OnChain(_)) => return Ok(()),

            (DependencyKind::Local(parent), DependencyKind::Local(subdir)) => {
                parent.push(subdir);
                *parent = normalize_path(&parent, /* allow_cwd_parent */ true)?;
            }

            (DependencyKind::Git(git), DependencyKind::Local(subdir)) => {
                git.subdir.push(subdir);
                git.subdir = normalize_path(&git.subdir, /* allow_cwd_parent */ false)?;
            }

            (DependencyKind::OnChain(_), _) => return Ok(()),
        };

        *self = parent;
        Ok(())
    }
}

/// Default `DependencyKind` is the one that acts as the left and right identity
/// to `DependencyKind::rerooted` (modulo path normalization).
impl Default for DependencyKind {
    fn default() -> Self {
        DependencyKind::Local(PathBuf::new())
    }
}

/// Normalize the representation of `path` by eliminating redundant `.`
/// components and applying `..` component.  Does not access the filesystem
/// (e.g. to resolve symlinks or test for file existence), unlike
/// `std::fs::canonicalize`.
///
/// Fails if the normalized path attempts to access the parent of a root
/// directory or volume prefix, or is prefixed by accesses to parent directories
/// when `allow_cwd_parent` is false.
///
/// Returns the normalized path on success.
pub fn normalize_path(path: impl AsRef<Path>, allow_cwd_parent: bool) -> Result<PathBuf> {
    use Component::*;

    let mut stack = Vec::new();
    for component in path.as_ref().components() {
        match component {
            // Components that contribute to the path as-is.
            verbatim @ (Prefix(_) | RootDir | Normal(_)) => stack.push(verbatim),

            // Equivalent of a `.` path component -- can be ignored.
            CurDir => { /* nop */ }

            // Going up in the directory hierarchy, which may fail if that's not possible.
            ParentDir => match stack.last() {
                None | Some(ParentDir) => {
                    stack.push(ParentDir);
                }

                Some(Normal(_)) => {
                    stack.pop();
                }

                Some(CurDir) => {
                    unreachable!("Component::CurDir never added to the stack");
                }

                Some(RootDir | Prefix(_)) => bail!(
                    "Invalid path accessing parent of root directory: {}",
                    path.as_ref().to_string_lossy(),
                ),
            },
        }
    }

    let normalized: PathBuf = stack.iter().collect();
    if !allow_cwd_parent && stack.first() == Some(&ParentDir) {
        bail!(
            "Path cannot access parent of current directory: {}",
            normalized.to_string_lossy()
        );
    }

    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git(url: &str, rev: &str, subdir: &str) -> DependencyKind {
        DependencyKind::Git(GitInfo {
            git_url: Symbol::from(url),
            git_rev: Symbol::from(rev),
            subdir: PathBuf::from(subdir),
        })
    }

    fn local(path: &str) -> DependencyKind {
        DependencyKind::Local(PathBuf::from(path))
    }

    fn onchain(id: &str) -> DependencyKind {
        DependencyKind::OnChain(OnChainInfo {
            id: Symbol::from(id),
        })
    }

    /// Reroot `child` against `parent`, asserting success, and return the
    /// result.
    fn rerooted(mut child: DependencyKind, parent: &DependencyKind) -> DependencyKind {
        child.reroot(parent).expect("reroot should succeed");
        child
    }

    const URL: &str = "https://github.com/iotaledger/iota.git";
    const REV: &str = "v1.23.2-rc";
    const FW: &str = "crates/iota-framework/packages/iota-framework";
    const SYS: &str = "crates/iota-framework/packages/iota-system";
    const STD: &str = "crates/iota-framework/packages/move-stdlib";

    #[test]
    fn git_child_is_absolute_and_unchanged() {
        // A git dependency's location is absolute; the parent must not affect it.
        let child = git(URL, REV, FW);
        assert_eq!(rerooted(child.clone(), &local("some/dir")), child);
        assert_eq!(
            rerooted(child.clone(), &git("other.git", "main", "x")),
            child
        );
        assert_eq!(rerooted(child.clone(), &DependencyKind::default()), child);
    }

    #[test]
    fn onchain_child_is_unchanged() {
        let child = onchain("0x2");
        assert_eq!(rerooted(child.clone(), &git(URL, REV, FW)), child);
        assert_eq!(rerooted(child.clone(), &local("a/b")), child);
    }

    #[test]
    fn onchain_parent_leaves_child_unchanged() {
        let child = local("../move-stdlib");
        assert_eq!(rerooted(child.clone(), &onchain("0x2")), child);
    }

    #[test]
    fn local_under_local_joins_and_normalizes() {
        assert_eq!(rerooted(local("../C"), &local("deps/B")), local("deps/C"));
        assert_eq!(rerooted(local("./sub"), &local("root")), local("root/sub"));
        // A top-level local dependency: parent is the default (empty) root.
        assert_eq!(
            rerooted(local("packages/foo"), &DependencyKind::default()),
            local("packages/foo"),
        );
    }

    #[test]
    fn local_under_git_becomes_a_sibling_git_subdir() {
        // The core of the fix: `{ local = "../move-stdlib" }` declared inside
        // iota-framework resolves into the same repo at the sibling subdir.
        assert_eq!(
            rerooted(local("../move-stdlib"), &git(URL, REV, FW)),
            git(URL, REV, STD),
        );
    }

    #[test]
    fn multiple_local_siblings_under_one_git_parent() {
        // iota-system depends on both move-stdlib and iota-framework via `../`.
        let parent = git(URL, REV, SYS);
        assert_eq!(
            rerooted(local("../move-stdlib"), &parent),
            git(URL, REV, STD)
        );
        assert_eq!(
            rerooted(local("../iota-framework"), &parent),
            git(URL, REV, FW)
        );
    }

    #[test]
    fn deep_chain_git_local_local_stays_in_repo() {
        // iota-system -> iota-framework -> move-stdlib, every hop a `../` local.
        // Rerooting against the resolved git kind at each level keeps the chain
        // inside the repository (this is what the recursion threads through).
        let sys = git(URL, REV, SYS);
        let fw = rerooted(local("../iota-framework"), &sys);
        assert_eq!(fw, git(URL, REV, FW));
        let std = rerooted(local("../move-stdlib"), &fw);
        assert_eq!(std, git(URL, REV, STD));
    }

    #[test]
    fn git_child_under_git_parent_keeps_its_own_repo() {
        // git -> git into a different repository: the git child is absolute and
        // does not inherit the parent's url/rev/subdir.
        let parent = git(URL, REV, FW);
        let other = git("https://example.com/other.git", "main", "pkgs/x");
        assert_eq!(rerooted(other.clone(), &parent), other);
    }

    #[test]
    fn git_subdir_normalizes_dotdot_components() {
        let parent = git(URL, REV, "a/b/c");
        assert_eq!(rerooted(local("../../x"), &parent), git(URL, REV, "a/x"));
    }

    #[test]
    fn local_escaping_git_repo_root_is_rejected() {
        // A subdir that climbs above the repository root is invalid.
        let parent = git(URL, REV, "a");
        let mut child = local("../../x");
        assert!(child.reroot(&parent).is_err());
    }
}
