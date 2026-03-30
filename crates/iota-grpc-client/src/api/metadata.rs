// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! MetadataEnvelope wrapper that preserves gRPC metadata headers alongside the
//! body.

use crate::ResponseExt;

/// A response from the gRPC API that carries both the response body and
/// metadata headers.
///
/// The metadata contains IOTA-specific headers such as chain ID, epoch,
/// checkpoint height, and timestamps. Access them via the [`ResponseExt`]
/// trait methods.
///
/// Use [`body()`](Self::body) / [`body_mut()`](Self::body_mut) to access the
/// response body, or [`into_inner()`](Self::into_inner) to consume the
/// envelope and extract the body.
///
/// # Example
///
/// ```ignore
/// let response = client.get_health(None).await?;
///
/// // Access body fields
/// println!("{:?}", response.body().executed_checkpoint_height);
///
/// // Access metadata headers via ResponseExt
/// println!("epoch: {:?}", response.epoch());
/// println!("chain: {:?}", response.chain());
///
/// // Extract just the body if you don't need metadata
/// let body = response.into_inner();
/// ```
#[derive(Debug, Clone)]
pub struct MetadataEnvelope<T> {
    inner: T,
    metadata: tonic::metadata::MetadataMap,
}

impl<T> MetadataEnvelope<T> {
    /// Create a new response from a body and metadata.
    pub fn new(inner: T, metadata: tonic::metadata::MetadataMap) -> Self {
        Self { inner, metadata }
    }

    /// Consume the response and return the body, discarding metadata.
    pub fn into_inner(self) -> T {
        self.inner
    }

    /// Get a reference to the response body.
    pub fn body(&self) -> &T {
        &self.inner
    }

    /// Get a mutable reference to the response body.
    pub fn body_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    /// Consume the response and return both the body and metadata.
    pub fn into_parts(self) -> (T, tonic::metadata::MetadataMap) {
        (self.inner, self.metadata)
    }

    /// Get a reference to the metadata.
    pub fn metadata(&self) -> &tonic::metadata::MetadataMap {
        &self.metadata
    }

    /// Transform the body, preserving metadata.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> MetadataEnvelope<U> {
        MetadataEnvelope {
            inner: f(self.inner),
            metadata: self.metadata,
        }
    }

    /// Transform the body with a fallible function, preserving metadata on
    /// success.
    pub fn try_map<U, E>(
        self,
        f: impl FnOnce(T) -> Result<U, E>,
    ) -> Result<MetadataEnvelope<U>, E> {
        Ok(MetadataEnvelope {
            inner: f(self.inner)?,
            metadata: self.metadata,
        })
    }
}

impl<T> From<tonic::Response<T>> for MetadataEnvelope<T> {
    fn from(response: tonic::Response<T>) -> Self {
        let metadata = response.metadata().clone();
        Self {
            inner: response.into_inner(),
            metadata,
        }
    }
}

impl<T> ResponseExt for MetadataEnvelope<T> {
    fn chain_id(&self) -> Option<iota_sdk_types::Digest> {
        self.metadata.chain_id()
    }

    fn chain(&self) -> Option<&str> {
        self.metadata.chain()
    }

    fn epoch(&self) -> Option<u64> {
        self.metadata.epoch()
    }

    fn checkpoint_height(&self) -> Option<u64> {
        self.metadata.checkpoint_height()
    }

    fn timestamp_ms(&self) -> Option<u64> {
        self.metadata.timestamp_ms()
    }

    fn timestamp(&self) -> Option<&str> {
        self.metadata.timestamp()
    }

    fn lowest_available_checkpoint(&self) -> Option<u64> {
        self.metadata.lowest_available_checkpoint()
    }

    fn lowest_available_checkpoint_objects(&self) -> Option<u64> {
        self.metadata.lowest_available_checkpoint_objects()
    }

    fn server_version(&self) -> Option<&str> {
        self.metadata.server_version()
    }
}
