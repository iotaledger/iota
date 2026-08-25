// Copyright (c) The Diem Core Contributors
// Copyright (c) The Move Contributors
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Git dependencies are cached in `~/.move`. Each dependency has a sparse,
//! shallow checkout in the directory `~/.move/<remote>_<sha>` (see
//! [crate::git::format_repo_to_fs_path])

use super::Dependency;
use crate::package::paths::PackagePath;

/// Once a dependency has been fetched, it is simply represented by a
/// [PackagePath]
type Fetched = PackagePath;

pub struct FetchedDependency(pub(super) Dependency<Fetched>);
