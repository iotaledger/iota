// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{fmt, fs, fs::File, io::Read, path::PathBuf};

use anyhow::{Context, Result, anyhow};
use async_trait::async_trait;
use bytes::Bytes;
use object_store::path::Path;

use crate::object_store::{ObjectStoreGetExt, util::path_to_filesystem};

pub struct LocalStorage {
    root: PathBuf,
}

impl LocalStorage {
    pub fn new(directory: &std::path::Path) -> Result<Self> {
        let path = fs::canonicalize(directory).context(anyhow!("Unable to canonicalize"))?;
        fs::create_dir_all(&path).context(anyhow!(
            "Failed to create local directory: {}",
            path.display()
        ))?;
        Ok(LocalStorage { root: path })
    }
}

impl fmt::Display for LocalStorage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "local:{}", self.root.display())
    }
}

#[async_trait]
impl ObjectStoreGetExt for LocalStorage {
    async fn get_bytes(&self, location: &Path) -> Result<Bytes> {
        let path_to_filesystem = path_to_filesystem(self.root.clone(), location)?;
        let handle = tokio::task::spawn_blocking(move || {
            let mut f = File::open(path_to_filesystem)
                .map_err(|e| anyhow!("Failed to open file with error: {}", e.to_string()))?;
            let mut buf = vec![];
            f.read_to_end(&mut buf)
                .context(anyhow!("Failed to read file"))?;
            Ok(buf.into())
        });
        handle.await?
    }
}
