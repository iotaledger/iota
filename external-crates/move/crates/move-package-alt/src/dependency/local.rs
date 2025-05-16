// Copyright (c) The Diem Core Contributors
// Copyright (c) The Move Contributors
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Types and methods related to local dependencies (of the form `{ local =
//! "<path>" }`)

use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::errors::PackageResult;
use serde::{Deserialize, Serialize};

// TODO: PinnedLocalDependencies should be different from UnpinnedLocalDependency - the former also
// needs an absolute filesystem path (which doesn't get serialized to the lockfile)
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct LocalDependency {
    /// The path on the filesystem, relative to the location of the containing file (which is
    /// stored in the `Located` wrapper)
    pub(crate) local: PathBuf,
}

impl LocalDependency {
    /// The path on the filesystem, relative to the location of the containing file
    pub fn path(&self) -> PackageResult<PathBuf> {
        // TODO incorrect, we need a base path
        self.local.canonicalize().map_err(|e| {
            crate::errors::PackageError::Generic(format!(
                "Failed to canonicalize path {}: {}",
                self.local.display(),
                e
            ))
        })
    }

    /// The path on the filesystem, relative to the location of the containing file
    pub fn root_dependency() -> Self {
        Self {
            local: PathBuf::from("."),
        }
    }
}
