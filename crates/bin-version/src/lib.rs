// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Hidden reexports for the bin_version macro
pub mod _hidden {
    pub use const_str::concat;
    pub use git_version::git_version;
}

/// Define constants that hold the git revision and package versions.
///
/// Defines two global `const`s:
///   `GIT_REVISION`: The git revision as specified by the `GIT_REVISION` env
/// variable provided at compile time, or the current git revision as discovered
/// by running `git describe`.
///
///   `VERSION`: The value of the `CARGO_PKG_VERSION` environment variable
/// concatenated with the value of `GIT_REVISION` if it is not empty.
///
/// Note: This macro must only be used from a binary, if used inside a library
/// this will fail to compile.
#[macro_export]
macro_rules! bin_version {
    () => {
        $crate::git_revision!();

        const VERSION: &str = {
            const _VERSION_BASE: &str = if GIT_REVISION.is_empty() {
                env!("CARGO_PKG_VERSION")
            } else {
                $crate::_hidden::concat!(env!("CARGO_PKG_VERSION"), "-", GIT_REVISION)
            };
            if cfg!(debug_assertions) {
                $crate::_hidden::concat!(_VERSION_BASE, "-debug")
            } else {
                _VERSION_BASE
            }
        };
    };
}

/// Defines constant that holds the git revision at build time.
///
///   `GIT_REVISION`: The git revision as specified by the `GIT_REVISION` env
/// variable provided at compile time, or the current git revision as discovered
/// by running `git describe`.
///
/// Note: This macro must only be used from a binary, if used inside a library
/// this will fail to compile.
#[macro_export]
macro_rules! git_revision {
    () => {
        const _ASSERT_IS_BINARY: () = {
            env!(
                "CARGO_BIN_NAME",
                "`bin_version!()` must be used from a binary"
            );
        };

        const GIT_REVISION: &str = {
            if let Some(revision) = option_env!("GIT_REVISION") {
                revision
            } else {
                let version = $crate::_hidden::git_version!(
                    args = ["--always", "--abbrev=12", "--dirty", "--exclude", "*"],
                    fallback = ""
                );

                if version.is_empty() {
                    panic!("unable to query git revision");
                }
                version
            }
        };
    };
}
