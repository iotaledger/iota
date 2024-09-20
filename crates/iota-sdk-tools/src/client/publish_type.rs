// Copyright 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;

use iota_move_build::CompiledPackage;

#[derive(Debug)]
pub enum PublishType {
    Path(PathBuf),
    Compiled(CompiledPackage),
}

impl From<&str> for PublishType {
    fn from(value: &str) -> Self {
        Self::from(PathBuf::from(value))
    }
}

impl From<String> for PublishType {
    fn from(value: String) -> Self {
        Self::from(PathBuf::from(value))
    }
}

impl From<PathBuf> for PublishType {
    fn from(value: PathBuf) -> Self {
        Self::Path(value)
    }
}

impl From<CompiledPackage> for PublishType {
    fn from(value: CompiledPackage) -> Self {
        Self::Compiled(value)
    }
}
