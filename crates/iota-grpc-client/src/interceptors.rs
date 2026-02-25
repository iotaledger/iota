// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Interceptor used to add additional headers to a Request
#[derive(Clone, Debug, Default)]
pub struct HeadersInterceptor {
    headers: tonic::metadata::MetadataMap,
}

impl HeadersInterceptor {
    /// Create a new, empty `HeadersInterceptor`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Return reference to the internal `MetadataMap`.
    pub fn headers(&self) -> &tonic::metadata::MetadataMap {
        &self.headers
    }

    /// Get mutable access to the internal `MetadataMap` for modification.
    pub fn headers_mut(&mut self) -> &mut tonic::metadata::MetadataMap {
        &mut self.headers
    }

    /// Enable HTTP basic authentication with a username and optional password.
    pub fn basic_auth<U, P>(&mut self, username: U, password: Option<P>)
    where
        U: std::fmt::Display,
        P: std::fmt::Display,
    {
        use std::io::Write;

        use base64::{prelude::BASE64_STANDARD, write::EncoderWriter};

        let mut buf = b"Basic ".to_vec();
        {
            let mut encoder = EncoderWriter::new(&mut buf, &BASE64_STANDARD);
            let _ = write!(encoder, "{username}:");
            if let Some(password) = password {
                let _ = write!(encoder, "{password}");
            }
        }
        let mut header = tonic::metadata::MetadataValue::try_from(buf)
            .expect("base64 is always valid HeaderValue");
        header.set_sensitive(true);

        self.headers
            .insert(http::header::AUTHORIZATION.as_str(), header);
    }

    /// Enable HTTP bearer authentication.
    pub fn bearer_auth<T>(&mut self, token: T)
    where
        T: std::fmt::Display,
    {
        let header_value = format!("Bearer {token}");
        let mut header = tonic::metadata::MetadataValue::try_from(header_value)
            .expect("token is always valid HeaderValue");
        header.set_sensitive(true);

        self.headers
            .insert(http::header::AUTHORIZATION.as_str(), header);
    }
}

impl tonic::service::Interceptor for &HeadersInterceptor {
    fn call(
        &mut self,
        mut request: tonic::Request<()>,
    ) -> std::result::Result<tonic::Request<()>, tonic::Status> {
        if !self.headers.is_empty() {
            request
                .metadata_mut()
                .as_mut()
                .extend(self.headers.clone().into_headers());
        }
        Ok(request)
    }
}

impl tonic::service::Interceptor for HeadersInterceptor {
    fn call(
        &mut self,
        request: tonic::Request<()>,
    ) -> std::result::Result<tonic::Request<()>, tonic::Status> {
        (&*self).call(request)
    }
}
