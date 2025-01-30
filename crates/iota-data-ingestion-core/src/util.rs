// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{str::FromStr, time::Duration};

use anyhow::Result;
use object_store::{
    ClientOptions, ObjectStore, RetryConfig, aws::AmazonS3ConfigKey, gcp::GoogleConfigKey,
};
use url::Url;

/// Creates a remote store client *without* any retry mechanism.
///
/// This function constructs a remote store client configured to *not* retry
/// failed requests. All requests will fail immediately if the underlying
/// operation encounters an error.  This is a convenience wrapper around
/// `create_remote_store_client_with_ops` that sets the retry configuration
/// to disable retries.
///
/// # Arguments
///
/// * `url`: The URL of the remote store. The scheme of the URL determines the
///   storage provider:
///     * `http://` or `https://`: HTTP-based store.
///     * `gs://`: Google Cloud Storage.
///     * `s3://` or other AWS S3-compatible URL: Amazon S3.
/// * `remote_store_options`: A vector of key-value pairs representing
///   provider-specific options.
///     * For GCS: See [`object_store::gcp::GoogleConfigKey`] for valid keys.
///     * For S3: See [`object_store::aws::AmazonS3ConfigKey`] for valid keys.
///     * For HTTP: No options are currently supported. This parameter should be
///       empty.
/// * `request_timeout_secs`: The timeout duration (in seconds) for individual
///   requests. This timeout is used to set a slightly longer retry timeout
///   (request_timeout_secs + 1) internally, even though retries are disabled.
///   This is done to ensure that the overall operation doesn't hang
///   indefinitely.
///
/// # Examples
///
/// Creating an S3 client without retries:
///
/// ```rust
/// # use iota_data_ingestion_core::create_remote_store_client;
/// use object_store::aws::AmazonS3ConfigKey;
///
/// let url = "s3://my-bucket";
/// let options = vec![(
///     AmazonS3ConfigKey::Region.as_ref().to_owned(),
///     "us-east-1".to_string(),
/// )];
/// let client = create_remote_store_client(url.to_string(), options, 30).unwrap();
/// ```
///
/// Creating a GCS client without retries:
///
/// ```text
/// # use iota_data_ingestion_core::create_remote_store_client;
/// use object_store::gcp::GoogleConfigKey;
///
/// let url = "gs://my-bucket";
/// let options = vec![(
///     GoogleConfigKey::ServiceAccount.as_ref().to_owned(),
///     "my-service-account".to_string(),
/// )];
/// let client = create_remote_store_client(url.to_string(), options, 30).unwrap();
/// ```
///
/// Creating an HTTP client without retries (no options supported):
///
/// ```text
/// # use iota_data_ingestion_core::create_remote_store_client;
///
/// let url = "http://example.bucket.com";
/// let options = vec![]; // No options for HTTP
/// let client = create_remote_store_client(url.to_string(), options, 30).unwrap();
/// ```
pub fn create_remote_store_client(
    url: String,
    remote_store_options: Vec<(String, String)>,
    request_timeout_secs: u64,
) -> Result<Box<dyn ObjectStore>> {
    let retry_config = RetryConfig {
        max_retries: 0,
        retry_timeout: Duration::from_secs(request_timeout_secs + 1),
        ..Default::default()
    };

    create_remote_store_client_with_ops(
        url,
        remote_store_options,
        request_timeout_secs,
        retry_config,
    )
}

/// Creates a remote store client with configurable retry behavior and options.
///
/// This function constructs a remote store client for various cloud storage
/// providers (HTTP, Google Cloud Storage, Amazon S3) based on the provided URL
/// and options. It allows configuring retry behavior through the `retry_config`
/// argument.
///
/// # Arguments
///
/// * `url`: The URL of the remote store.  The scheme of the URL determines the
///   storage provider:
///     * `http://` or `https://`:  HTTP-based store.
///     * `gs://`: Google Cloud Storage.
///     * `s3://` or other AWS S3-compatible URL: Amazon S3.
/// * `remote_store_options`: A vector of key-value pairs representing
///   provider-specific options.
///     * For GCS:  See [`object_store::gcp::GoogleConfigKey`] for valid keys.
///     * For S3: See [`object_store::aws::AmazonS3ConfigKey`] for valid keys.
///     * For HTTP: No options are currently supported. This parameter should be
///       empty.
/// * `request_timeout_secs`: The timeout duration (in seconds) for individual
///   requests.
/// * `retry_config`: A [`RetryConfig`] struct defining the retry strategy. This
///   allows fine-grained control over the number of retries, backoff behavior,
///   and retry timeouts.  See the documentation for
///   [`object_store::RetryConfig`] for details.
///
/// # Examples
///
/// Creating an S3 client with specific options and a retry configuration:
///
/// ```text
/// # use iota_data_ingestion_core::create_remote_store_client_with_ops;
/// use object_store::{RetryConfig, aws::AmazonS3ConfigKey};
///
/// let url = "s3://my-bucket";
/// let options = vec![(
///     AmazonS3ConfigKey::Region.as_ref().to_owned(),
///     "us-east-1".to_string(),
/// )];
/// let retry_config = RetryConfig::default(); // Use default retry settings
/// let client =
///     create_remote_store_client_with_ops(url.to_string(), options, 30, retry_config).unwrap();
/// ```
///
/// Creating a GCS client:
///
/// ```text
/// # use iota_data_ingestion_core::create_remote_store_client_with_ops;
/// use object_store::{RetryConfig, gcp::GoogleConfigKey};
///
/// let url = "gs://my-bucket";
/// let options = vec![(
///     GoogleConfigKey::ServiceAccount.as_ref().to_owned(),
///     "my-service-account".to_string(),
/// )];
/// let retry_config = RetryConfig::default();
/// let client =
///     create_remote_store_client_with_ops(url.to_string(), options, 30, retry_config).unwrap();
/// ```
///
/// Creating an HTTP client (no options supported):
///
/// ```text
/// # use iota_data_ingestion_core::create_remote_store_client_with_ops;
/// use object_store::RetryConfig;
///
/// let url = "http://example.bucket.com";
/// let options = vec![]; // No options for HTTP
/// let retry_config = RetryConfig::default();
/// let client =
///     create_remote_store_client_with_ops(url.to_string(), options, 30, retry_config).unwrap();
/// ```
pub fn create_remote_store_client_with_ops(
    url: String,
    remote_store_options: Vec<(String, String)>,
    request_timeout_secs: u64,
    retry_config: RetryConfig,
) -> Result<Box<dyn ObjectStore>> {
    let client_options = ClientOptions::new()
        .with_timeout(Duration::from_secs(request_timeout_secs))
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
