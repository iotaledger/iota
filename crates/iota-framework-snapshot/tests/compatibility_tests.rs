// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod compatibility_tests {
    use std::{
        collections::BTreeMap,
        path::{Path, PathBuf},
    };

    use iota_framework::{BuiltInFramework, compare_system_package};
    use iota_framework_snapshot::{load_bytecode_snapshot, load_bytecode_snapshot_manifest};
    use iota_move_build::published_at_property;
    use iota_protocol_config::{Chain, ProtocolConfig, ProtocolVersion};
    use iota_types::execution_config_utils::to_binary_config;
    use move_package::source_package::{
        manifest_parser::parse_move_manifest_from_file, parsed_manifest::SourceManifest,
    };
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_framework_compatibility() {
        // This test checks that the current framework is compatible with all previous
        // framework bytecode snapshots.
        for (version, _snapshots) in load_bytecode_snapshot_manifest() {
            let config =
                ProtocolConfig::get_for_version(ProtocolVersion::new(version), Chain::Unknown);
            let binary_config = to_binary_config(&config);
            let framework = load_bytecode_snapshot(version).unwrap();
            let old_framework_store: BTreeMap<_, _> = framework
                .into_iter()
                .map(|package| (package.id, package.genesis_object()))
                .collect();
            for cur_package in BuiltInFramework::iter_system_packages() {
                if compare_system_package(
                    &old_framework_store,
                    &cur_package.id,
                    &cur_package.modules(),
                    cur_package.dependencies.to_vec(),
                    &binary_config,
                )
                .await
                .is_none()
                {
                    panic!(
                        "The current IOTA framework {:?} is not compatible with version {:?}",
                        cur_package.id, version
                    );
                }
            }
        }
    }

    #[test]
    fn check_framework_change_with_protocol_upgrade() {
        // This test checks that if we ever update the framework, the current protocol
        // version must differ the latest bytecode snapshot in each network.
        let snapshots = load_bytecode_snapshot_manifest();
        let latest_snapshot_version = *snapshots.keys().max().unwrap();
        if latest_snapshot_version != ProtocolVersion::MAX.as_u64() {
            // If we have already incremented the protocol version, then we are fine and we
            // don't care if the framework has changed.
            return;
        }
        let latest_snapshot = load_bytecode_snapshot(*snapshots.keys().max().unwrap()).unwrap();
        // Turn them into BTreeMap for deterministic comparison.
        let latest_snapshot_ref: BTreeMap<_, _> =
            latest_snapshot.iter().map(|p| (&p.id, p)).collect();
        let current_framework: BTreeMap<_, _> = BuiltInFramework::iter_system_packages()
            .map(|p| (&p.id, p))
            .collect();
        assert_eq!(
            latest_snapshot_ref, current_framework,
            "The current framework differs the latest bytecode snapshot. Did you forget to upgrade protocol version?"
        );
    }

    /// This test checks that the `SinglePackage` entries in `manifest.json`
    /// match the metadata in the `Move.toml` files in the repo for each
    /// revision.
    #[test]
    fn check_manifest_against_tomls() {
        // Only execute the test when the manifest is modified
        if check_if_manifest_was_modified() {
            let manifest = load_bytecode_snapshot_manifest();

            // Clone the current repo into a temp directory
            let (_temp_dir, clone_repo_path) = clone_repo_to_temp();

            for entry in manifest.values() {
                // Checkout each specified revision in the cloned repo
                checkout_revision(&clone_repo_path, &entry.git_revision);

                for package in entry.packages.iter() {
                    // parse package.path/Move.toml
                    let toml_path = clone_repo_path.join(&package.path);
                    let package_toml: SourceManifest =
                        parse_move_manifest_from_file(&toml_path).expect("Move.toml exists");
                    // check manifest name field is package.name
                    assert_eq!(package_toml.package.name.to_string(), package.name);
                    // check manifest published-at field is package.id
                    let published_at_field = published_at_property(&package_toml)
                        .expect("Move.toml file has published-at field");
                    assert_eq!(published_at_field, package.id);
                }
            }
        }
    }

    fn clone_repo_to_temp() -> (TempDir, PathBuf) {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let repo_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let clone_path = temp_dir.path().join("repo");

        let status = std::process::Command::new("git")
            .args([
                "clone",
                repo_path.to_str().unwrap(),
                clone_path.to_str().unwrap(),
            ])
            .status()
            .expect("Failed to clone repository");

        assert!(status.success(), "Git clone failed");

        (temp_dir, clone_path)
    }

    fn check_if_manifest_was_modified() -> bool {
        // Run `git diff --name-only HEAD`
        let output = std::process::Command::new("git")
            .args(["diff", "--name-only", "HEAD"])
            .output()
            .expect("failed to run git diff");

        let changed_files = String::from_utf8_lossy(&output.stdout);

        changed_files.contains("iota-framework-snapshot/manifest.json")
    }

    fn checkout_revision(repo_path: &Path, rev: &str) {
        let fetch_status = std::process::Command::new("git")
            .args(["fetch", "--depth=2147483647", "origin", rev])
            .current_dir(repo_path)
            .status()
            .expect("Failed to execute git fetch");

        assert!(fetch_status.success(), "Git fetch failed");

        let checkout_status = std::process::Command::new("git")
            .args(["checkout", rev])
            .current_dir(repo_path)
            .status()
            .expect("Failed to execute git checkout");

        assert!(checkout_status.success(), "Git checkout failed");
    }

    #[test]
    fn check_no_dirty_manifest_commit() {
        let snapshots = load_bytecode_snapshot_manifest();
        for snapshot in snapshots.values() {
            assert!(
                !snapshot.git_revision.contains("dirty"),
                "If you are trying to regenerate the bytecode snapshot after cherry-picking, please do so in a standalone PR after the cherry-pick is merged on the release branch.",
            );
        }
    }
}
