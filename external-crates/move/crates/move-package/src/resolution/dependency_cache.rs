// Copyright (c) The Move Contributors
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsStr,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use anyhow::Result;
use colored::Colorize;

use super::repository_path;
use crate::{
    package_hooks,
    source_package::parsed_manifest::{DependencyKind, GitInfo, PackageName},
};

/// Fetches remote dependencies and caches information about those already
/// fetched when building a given package.
#[derive(Debug, Clone)]
pub struct DependencyCache {
    /// Subdirs already materialized for each fetched git repository, keyed by
    /// the repository's local cache path. A single clone is shared by every
    /// subdir of the same `url + rev`, so we track the subdirs individually:
    /// the first one triggers the clone and later ones are added to the
    /// repository's sparse-checkout set.
    fetched_deps: BTreeMap<PathBuf, BTreeSet<PathBuf>>,

    /// Should a dependency fetched when building a different package be
    /// refreshed to the newest version when building a new package
    skip_fetch_latest_git_deps: bool,
}

impl DependencyCache {
    pub fn new(skip_fetch_latest_git_deps: bool) -> DependencyCache {
        let fetched_deps = BTreeMap::new();
        DependencyCache {
            fetched_deps,
            skip_fetch_latest_git_deps,
        }
    }

    pub fn download_and_update_if_remote<Progress: Write>(
        &mut self,
        dep_name: PackageName,
        kind: &DependencyKind,
        progress_output: &mut Progress,
    ) -> Result<()> {
        match kind {
            DependencyKind::Local(_) => Ok(()),

            DependencyKind::OnChain(info) => {
                // check if a given dependency type has already been fetched
                if self
                    .fetched_deps
                    .insert(repository_path(kind), BTreeSet::new())
                    .is_some()
                {
                    return Ok(());
                }
                package_hooks::resolve_on_chain_dependency(dep_name, info)
            }

            DependencyKind::Git(GitInfo {
                git_url,
                git_rev,
                subdir,
            }) => {
                let repository_path = repository_path(kind);

                // A single clone serves every subdir of the same `url + rev`. Skip if this
                // exact subdir is already materialized this run; otherwise note whether this
                // is the first subdir of the repository we touch (only then is it refreshed).
                let first_touch = match self.fetched_deps.get(&repository_path) {
                    Some(subdirs) if subdirs.contains(subdir) => return Ok(()),
                    Some(_) => false,
                    None => true,
                };

                ensure_git_available(progress_output)?;

                let git_path = repository_path.as_path();

                if !git_path.exists() {
                    writeln!(
                        progress_output,
                        "{} {}",
                        "FETCHING GIT DEPENDENCY".bold().green(),
                        git_url,
                    )?;
                    clone_dependency(
                        git_path,
                        git_url.as_str(),
                        git_rev.as_str(),
                        subdir,
                        dep_name,
                        progress_output,
                    )?;
                } else {
                    // The repository is already cached. Make sure this subdir is checked
                    // out (a no-op for full clones and for subdirs already present) before
                    // optionally refreshing the repository. The refresh is per-repository,
                    // so only the first subdir we touch this run performs it.
                    ensure_subdir_present(git_path, subdir);

                    if first_touch && !self.skip_fetch_latest_git_deps {
                        update_dependency(
                            git_path,
                            git_url.as_str(),
                            git_rev.as_str(),
                            dep_name,
                            progress_output,
                        )?;
                    }
                }

                self.fetched_deps
                    .entry(repository_path)
                    .or_default()
                    .insert(subdir.clone());

                Ok(())
            }
        }
    }
}

/// Errors out if `git` is not callable.
fn ensure_git_available<Progress: Write>(progress_output: &mut Progress) -> Result<()> {
    if Command::new("git")
        .arg("--version")
        .stdin(Stdio::null())
        .output()
        .is_err()
    {
        writeln!(progress_output, "Git is not installed or not in the PATH.")?;
        return Err(anyhow::anyhow!("Git is not installed or not in the PATH."));
    }
    Ok(())
}

/// Clones a git dependency into `git_path` and checks out `git_rev`.
///
/// Prefers a blobless, sparse clone restricted to `subdir`: this downloads only
/// the commit graph plus the blobs reachable from `subdir`, rather than the
/// whole repository and its history. Falls back to a full clone when sparse
/// cloning is unavailable (e.g. an old `git`) or when the package lives at the
/// repository root (`subdir` is empty), where there is nothing to narrow to.
fn clone_dependency<Progress: Write>(
    git_path: &Path,
    git_url: &str,
    git_rev: &str,
    subdir: &Path,
    dep_name: PackageName,
    progress_output: &mut Progress,
) -> Result<()> {
    if !subdir.as_os_str().is_empty() && try_sparse_clone(git_path, git_url, git_rev, subdir) {
        return Ok(());
    }

    full_clone(git_path, git_url, git_rev, dep_name, progress_output)
}

/// Attempts a blobless sparse clone of `subdir` at `git_rev`. Returns `true` on
/// success. On any failure it removes the (possibly partial) clone and returns
/// `false` so the caller can fall back to a full clone.
fn try_sparse_clone(git_path: &Path, git_url: &str, git_rev: &str, subdir: &Path) -> bool {
    // `--filter=blob:none` keeps the full commit graph so any revision -- tag,
    // branch, or commit SHA -- still resolves, while deferring blob downloads;
    // `--sparse --no-checkout` leaves the working tree empty until we narrow it.
    let ok = git_status(&[
        OsStr::new("clone"),
        OsStr::new("--filter=blob:none"),
        OsStr::new("--sparse"),
        OsStr::new("--no-checkout"),
        OsStr::new(git_url),
        git_path.as_os_str(),
    ]) && git_status(&[
        OsStr::new("-C"),
        git_path.as_os_str(),
        OsStr::new("sparse-checkout"),
        OsStr::new("set"),
        OsStr::new("--cone"),
        subdir.as_os_str(),
    ]) && git_status(&[
        OsStr::new("-C"),
        git_path.as_os_str(),
        OsStr::new("checkout"),
        OsStr::new(git_rev),
    ]);

    if ok {
        return true;
    }

    // Leave no partial clone behind for the full-clone fallback.
    if git_path.exists() {
        let _ = std::fs::remove_dir_all(git_path);
    }
    false
}

/// Performs a full clone (history and all files) followed by a checkout of
/// `git_rev`. This is the original fetch behaviour, kept as a fallback.
fn full_clone<Progress: Write>(
    git_path: &Path,
    git_url: &str,
    git_rev: &str,
    dep_name: PackageName,
    progress_output: &mut Progress,
) -> Result<()> {
    if let Ok(mut output) = Command::new("git")
        .args([
            OsStr::new("clone"),
            OsStr::new(git_url),
            git_path.as_os_str(),
        ])
        .stdin(Stdio::null())
        .spawn()
    {
        output.wait().map_err(|_| {
            anyhow::anyhow!("Failed to clone Git repository for package '{}'", dep_name)
        })?;
        if output.stdout.is_some() {
            writeln!(progress_output, "{:?}", output)?;
        }
    } else {
        return Err(anyhow::anyhow!(
            "Failed to clone Git repository for package '{}'",
            dep_name
        ));
    }

    Command::new("git")
        .args([
            OsStr::new("-C"),
            git_path.as_os_str(),
            OsStr::new("checkout"),
            OsStr::new(git_rev),
        ])
        .stdin(Stdio::null())
        .output()
        .map_err(|_| {
            anyhow::anyhow!(
                "Failed to checkout Git reference '{}' for package '{}'",
                git_rev,
                dep_name
            )
        })?;

    Ok(())
}

/// Ensures `subdir` is checked out in an already-cached repository.
///
/// Only repositories we cloned sparsely are touched: a `sparse-checkout add` is
/// idempotent for subdirs already present, and pulls in new ones. Full clones
/// from older tool versions already contain every subdir, so they are left
/// alone, and root packages (empty `subdir`) need no narrowing.
fn ensure_subdir_present(git_path: &Path, subdir: &Path) {
    if subdir.as_os_str().is_empty() || !is_sparse_repo(git_path) {
        return;
    }

    git_status(&[
        OsStr::new("-C"),
        git_path.as_os_str(),
        OsStr::new("sparse-checkout"),
        OsStr::new("add"),
        subdir.as_os_str(),
    ]);
}

/// Whether `git_path` is a sparse-checkout repository (i.e. one we created with
/// [`try_sparse_clone`]). Full clones report `false` and are left untouched.
fn is_sparse_repo(git_path: &Path) -> bool {
    Command::new("git")
        .args([
            OsStr::new("-C"),
            git_path.as_os_str(),
            OsStr::new("config"),
            OsStr::new("--get"),
            OsStr::new("core.sparseCheckout"),
        ])
        .stdin(Stdio::null())
        .output()
        .map(|out| String::from_utf8_lossy(&out.stdout).trim() == "true")
        .unwrap_or(false)
}

/// Refreshes a cached repository to the latest state of `git_rev`.
///
/// Revisions pinned to a tag or a commit SHA are already immutable and return
/// early; only branch-pinned dependencies are fetched and hard-reset.
fn update_dependency<Progress: Write>(
    git_path: &Path,
    git_url: &str,
    git_rev: &str,
    dep_name: PackageName,
    progress_output: &mut Progress,
) -> Result<()> {
    let os_git_rev = OsStr::new(git_rev);

    // Check first that it isn't a git rev (if it doesn't work, just continue with
    // the fetch)
    if let Ok(rev) = Command::new("git")
        .args([
            OsStr::new("-C"),
            git_path.as_os_str(),
            OsStr::new("rev-parse"),
            OsStr::new("--verify"),
            os_git_rev,
        ])
        .stdin(Stdio::null())
        .output()
    {
        if let Ok(parsable_version) = String::from_utf8(rev.stdout) {
            // If it's exactly the same, then it's a git rev
            if parsable_version.trim().starts_with(git_rev) {
                return Ok(());
            }
        }
    }

    let tag = Command::new("git")
        .args([
            OsStr::new("-C"),
            git_path.as_os_str(),
            OsStr::new("tag"),
            OsStr::new("--list"),
            os_git_rev,
        ])
        .stdin(Stdio::null())
        .output();

    if let Ok(tag) = tag {
        if let Ok(parsable_version) = String::from_utf8(tag.stdout) {
            // If it's exactly the same, then it's a git tag, for now tags won't be
            // updated Tags don't easily update locally
            // and you can't use reset --hard to cleanup
            // any extra files
            if parsable_version.trim().starts_with(git_rev) {
                return Ok(());
            }
        }
    }

    writeln!(
        progress_output,
        "{} {}",
        "UPDATING GIT DEPENDENCY".bold().green(),
        git_url,
    )?;

    // If the current folder exists, do a fetch and reset to ensure that the branch
    // is up to date.
    //
    // NOTE: this means that you must run the package system with a working network
    // connection.

    if let Ok(mut output) = Command::new("git")
        .args([
            OsStr::new("-C"),
            git_path.as_os_str(),
            OsStr::new("fetch"),
            OsStr::new("origin"),
        ])
        .stdin(Stdio::null())
        .spawn()
    {
        output.wait().map_err(|_| {
            anyhow::anyhow!(
                "Failed to fetch latest Git state for package '{}', to skip set \
                 --skip-fetch-latest-git-deps",
                dep_name
            )
        })?;
        if output.stdout.is_some() {
            writeln!(progress_output, "{:?}", output)?;
        }
    } else {
        return Err(anyhow::anyhow!(
            "Failed to fetch latest Git state for package '{}', to skip set \
             --skip-fetch-latest-git-deps",
            dep_name
        ));
    }

    let status = Command::new("git")
        .args([
            OsStr::new("-C"),
            git_path.as_os_str(),
            OsStr::new("reset"),
            OsStr::new("--hard"),
            OsStr::new(&format!("origin/{}", git_rev)),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|_| {
            anyhow::anyhow!(
                "Failed to reset to latest Git state '{}' for package '{}', to skip \
                 set --skip-fetch-latest-git-deps",
                git_rev,
                dep_name
            )
        })?;

    if !status.success() {
        return Err(anyhow::anyhow!(
            "Failed to reset to latest Git state '{}' for package '{}', to skip set \
             --skip-fetch-latest-git-deps | Exit status: {}",
            git_rev,
            dep_name,
            status
        ));
    }

    Ok(())
}

/// Runs `git` with `args`, inheriting stdio so progress is shown, and reports
/// whether it exited successfully. A spawn failure (e.g. `git` missing) is
/// treated as a non-successful run.
fn git_status(args: &[&OsStr]) -> bool {
    Command::new("git")
        .args(args)
        .stdin(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use tempfile::TempDir;

    use super::*;

    /// Runs `git -C <dir> <args>`, asserting success.
    fn run_git(dir: &Path, args: &[&str]) {
        let success = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .status()
            .expect("failed to spawn git")
            .success();
        assert!(success, "git {args:?} failed in {}", dir.display());
    }

    /// Builds a throwaway git repository containing one package per entry in
    /// `subdirs` (each with a minimal `Move.toml`), committed and tagged `v1`.
    /// Returns the repository and its `HEAD` commit SHA.
    fn make_repo(subdirs: &[&str]) -> (TempDir, String) {
        let repo = tempfile::tempdir().unwrap();
        let root = repo.path();
        run_git(root, &["init", "--quiet"]);
        run_git(root, &["config", "user.email", "test@example.com"]);
        run_git(root, &["config", "user.name", "test"]);
        run_git(root, &["config", "commit.gpgsign", "false"]);
        // Let this repo serve partial (`--filter`) clones and fetches of arbitrary
        // object ids over `file://`. Without these a blobless clone is silently
        // downgraded to a full one, so the tests could not observe blob deferral.
        run_git(root, &["config", "uploadpack.allowFilter", "true"]);
        run_git(root, &["config", "uploadpack.allowAnySHA1InWant", "true"]);
        for sub in subdirs {
            let dir = root.join(sub);
            fs::create_dir_all(&dir).unwrap();
            fs::write(
                dir.join("Move.toml"),
                format!("[package]\nname = \"{sub}\"\n"),
            )
            .unwrap();
        }
        run_git(root, &["add", "-A"]);
        run_git(root, &["commit", "--quiet", "-m", "init"]);
        run_git(root, &["tag", "v1"]);
        let sha = String::from_utf8(
            Command::new("git")
                .arg("-C")
                .arg(root)
                .args(["rev-parse", "HEAD"])
                .output()
                .unwrap()
                .stdout,
        )
        .unwrap()
        .trim()
        .to_string();
        (repo, sha)
    }

    /// A `file://` URL for the fixture repo. The scheme matters: git treats a
    /// bare local path as a "local clone" and silently ignores `--filter`, but
    /// over `file://` it goes through the real fetch protocol that honours it.
    fn repo_url(repo: &TempDir) -> String {
        format!("file://{}", repo.path().display())
    }

    /// Whether the package at `subdir` has been checked out in `dest`.
    fn has_pkg(dest: &Path, subdir: &str) -> bool {
        dest.join(subdir).join("Move.toml").exists()
    }

    /// Number of objects the clone left un-downloaded (fetched lazily on
    /// demand) -- direct proof that `--filter=blob:none` actually took
    /// effect. A full clone, or one whose filter was silently ignored,
    /// reports 0.
    fn deferred_object_count(dest: &Path) -> usize {
        let output = Command::new("git")
            .args([
                OsStr::new("-C"),
                dest.as_os_str(),
                OsStr::new("rev-list"),
                OsStr::new("--objects"),
                OsStr::new("--all"),
                OsStr::new("--missing=print"),
            ])
            .output()
            .expect("failed to spawn git");
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter(|line| line.starts_with('?'))
            .count()
    }

    #[test]
    fn sparse_clone_narrows_to_requested_subdir() {
        let (repo, _) = make_repo(&["a", "b", "c"]);
        let dest_dir = tempfile::tempdir().unwrap();
        let dest = dest_dir.path().join("clone");

        assert!(try_sparse_clone(
            &dest,
            &repo_url(&repo),
            "v1",
            Path::new("a")
        ));
        assert!(is_sparse_repo(&dest));
        assert!(has_pkg(&dest, "a"), "requested subdir is checked out");
        assert!(
            !has_pkg(&dest, "b"),
            "unrequested subdir is not checked out"
        );
        assert!(
            !has_pkg(&dest, "c"),
            "unrequested subdir is not checked out"
        );
        // The point of the whole change: blobs outside the checked-out subdir are
        // deferred, not downloaded. This fails if `--filter=blob:none` is dropped or
        // silently ignored (e.g. a bare-path clone source).
        assert!(
            deferred_object_count(&dest) > 0,
            "blobless clone should defer blob downloads",
        );
    }

    #[test]
    fn ensure_subdir_present_adds_sibling_to_sparse_clone() {
        let (repo, _) = make_repo(&["a", "b", "c"]);
        let dest_dir = tempfile::tempdir().unwrap();
        let dest = dest_dir.path().join("clone");
        assert!(try_sparse_clone(
            &dest,
            &repo_url(&repo),
            "v1",
            Path::new("a")
        ));

        ensure_subdir_present(&dest, Path::new("b"));

        assert!(has_pkg(&dest, "b"), "sibling subdir is added");
        assert!(has_pkg(&dest, "a"), "original subdir remains");
        assert!(!has_pkg(&dest, "c"), "untouched subdir stays absent");
    }

    #[test]
    fn sparse_clone_resolves_a_raw_commit_sha() {
        // A commit SHA must resolve -- a shallow clone (`--depth 1`) could not do this.
        let (repo, sha) = make_repo(&["a", "b"]);
        let dest_dir = tempfile::tempdir().unwrap();
        let dest = dest_dir.path().join("clone");

        assert!(try_sparse_clone(
            &dest,
            &repo_url(&repo),
            &sha,
            Path::new("a")
        ));
        assert!(has_pkg(&dest, "a"));
    }

    #[test]
    fn full_clone_checks_out_everything_and_is_not_sparse() {
        let (repo, _) = make_repo(&["a", "b", "c"]);
        let dest_dir = tempfile::tempdir().unwrap();
        let dest = dest_dir.path().join("clone");

        full_clone(
            &dest,
            &repo_url(&repo),
            "v1",
            PackageName::from("pkg"),
            &mut std::io::sink(),
        )
        .unwrap();

        assert!(!is_sparse_repo(&dest), "a full clone is not a sparse repo");
        assert_eq!(
            deferred_object_count(&dest),
            0,
            "a full clone downloads every object, it defers nothing",
        );
        assert!(
            has_pkg(&dest, "a") && has_pkg(&dest, "b") && has_pkg(&dest, "c"),
            "a full clone contains every subdir",
        );
    }

    #[test]
    fn ensure_subdir_present_leaves_full_clone_untouched() {
        // Backward compatibility: a full clone created by an older tool version must
        // not be narrowed -- doing so would hide files an existing build relies on.
        let (repo, _) = make_repo(&["a", "b", "c"]);
        let dest_dir = tempfile::tempdir().unwrap();
        let dest = dest_dir.path().join("clone");
        full_clone(
            &dest,
            &repo_url(&repo),
            "v1",
            PackageName::from("pkg"),
            &mut std::io::sink(),
        )
        .unwrap();

        ensure_subdir_present(&dest, Path::new("a"));

        assert!(!is_sparse_repo(&dest), "still a full clone");
        assert!(
            has_pkg(&dest, "a") && has_pkg(&dest, "b") && has_pkg(&dest, "c"),
            "every subdir is still present",
        );
    }
}
