// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{str::FromStr, time::Duration};

use anyhow::Result;
use object_store::{
    ClientOptions, ObjectStore, RetryConfig, aws::AmazonS3ConfigKey, gcp::GoogleConfigKey,
};
use url::Url;

pub fn create_remote_store_client(
    url: String,
    remote_store_options: Vec<(String, String)>,
    request_timeout: u64,
) -> Result<Box<dyn ObjectStore>> {
    let retry_config = RetryConfig {
        max_retries: 0,
        retry_timeout: Duration::from_secs(request_timeout + 1),
        ..Default::default()
    };

    create_remote_store_client_with_ops(url, remote_store_options, request_timeout, retry_config)
}

pub fn create_remote_store_client_with_ops(
    url: String,
    remote_store_options: Vec<(String, String)>,
    request_timeout: u64,
    retry_config: RetryConfig,
) -> Result<Box<dyn ObjectStore>> {
    let client_options = ClientOptions::new()
        .with_timeout(Duration::from_secs(request_timeout))
        .with_allow_http(true);
    if remote_store_options.is_empty() {
        let http_store = object_store::http::HttpBuilder::new()
            .with_url(url)
            .with_client_options(client_options)
            .with_retry(retry_config)
            .build()?;
        Ok(Box::new(http_store))
    } else if Url::parse(&url)?.scheme() == "gs" {
        let url = Url::parse(&url)?;
        let mut builder = object_store::gcp::GoogleCloudStorageBuilder::new()
            .with_url(url.as_str())
            .with_retry(retry_config)
            .with_client_options(client_options);
        for (key, value) in remote_store_options {
            builder = builder.with_config(GoogleConfigKey::from_str(&key)?, value);
        }
        Ok(Box::new(builder.build()?))
    } else {
        let url = Url::parse(&url)?;
        let mut builder = object_store::aws::AmazonS3Builder::new()
            .with_url(url.as_str())
            .with_retry(retry_config)
            .with_client_options(client_options);
        for (key, value) in remote_store_options {
            builder = builder.with_config(AmazonS3ConfigKey::from_str(&key)?, value);
        }
        Ok(Box::new(builder.build()?))
    }
}
