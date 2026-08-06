// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{fmt, sync::Arc};

use anyhow::Result;
use async_trait::async_trait;
use bytes::Bytes;
use iota_config::object_storage_config::{CONNECT_TIMEOUT, TRANSFER_STALL_TIMEOUT};
use object_store::{GetResult, path::Path};
use percent_encoding::{PercentEncode, utf8_percent_encode};
use reqwest::{Client, ClientBuilder};

use crate::object_store::{
    ObjectStoreGetExt, collect_get_result_with_progress,
    http::{DEFAULT_USER_AGENT, STRICT_PATH_ENCODE_SET, exists, get, size},
};

#[derive(Debug)]
pub(crate) struct S3Client {
    endpoint: String,
    client: Client,
}

impl S3Client {
    pub fn new(endpoint: &str) -> Result<Self> {
        let mut builder = ClientBuilder::new();
        builder = builder
            .user_agent(DEFAULT_USER_AGENT)
            .pool_idle_timeout(None)
            .connect_timeout(CONNECT_TIMEOUT)
            .read_timeout(TRANSFER_STALL_TIMEOUT);
        let client = builder.https_only(false).build()?;

        Ok(Self {
            endpoint: endpoint.to_string(),
            client,
        })
    }
    async fn get(&self, location: &Path) -> Result<GetResult> {
        let url = self.path_url(location);
        get(&url, "s3", location, &self.client).await
    }
    async fn exists(&self, location: &Path) -> Result<bool> {
        let url = self.path_url(location);
        exists(&url, "s3", location, &self.client).await
    }
    async fn size(&self, location: &Path) -> Result<u64> {
        let url = self.path_url(location);
        size(&url, "s3", location, &self.client).await
    }
    fn path_url(&self, path: &Path) -> String {
        format!("{}/{}", self.endpoint, Self::encode_path(path))
    }
    fn encode_path(path: &Path) -> PercentEncode<'_> {
        utf8_percent_encode(path.as_ref(), &STRICT_PATH_ENCODE_SET)
    }
}

/// Interface for [Amazon S3](https://aws.amazon.com/s3/).
#[derive(Debug)]
pub struct AmazonS3 {
    client: Arc<S3Client>,
}

impl AmazonS3 {
    pub fn new(endpoint: &str) -> Result<Self> {
        let s3_client = S3Client::new(endpoint)?;
        Ok(AmazonS3 {
            client: Arc::new(s3_client),
        })
    }
}

impl fmt::Display for AmazonS3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "s3:{}", self.client.endpoint)
    }
}

#[async_trait]
impl ObjectStoreGetExt for AmazonS3 {
    async fn get_bytes(&self, location: &Path) -> Result<Bytes> {
        let result = self.client.get(location).await?;
        let bytes = result.bytes().await?;
        Ok(bytes)
    }

    async fn get_bytes_with_progress(
        &self,
        location: &Path,
        on_bytes: &(dyn Fn(u64) + Send + Sync),
    ) -> Result<Bytes> {
        let result = self.client.get(location).await?;
        collect_get_result_with_progress(result, location, on_bytes).await
    }

    async fn exists(&self, location: &Path) -> Result<bool> {
        self.client.exists(location).await
    }

    async fn object_size(&self, location: &Path) -> Result<u64> {
        self.client.size(location).await
    }
}
