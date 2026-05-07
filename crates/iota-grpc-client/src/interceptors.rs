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

#[cfg(test)]
mod tests {
    use base64::{Engine as _, prelude::BASE64_STANDARD};
    use tonic::service::Interceptor as _;

    use super::*;

    fn intercepted_authorization(mut interceptor: HeadersInterceptor) -> String {
        let request = interceptor
            .call(tonic::Request::new(()))
            .expect("interceptor should not fail");
        request
            .metadata()
            .get(http::header::AUTHORIZATION.as_str())
            .expect("authorization header should be set")
            .to_str()
            .expect("authorization header should be valid ASCII")
            .to_string()
    }

    #[test]
    fn basic_auth_sets_authorization_header() {
        let mut interceptor = HeadersInterceptor::new();
        interceptor.basic_auth("alice", Some("hunter2"));

        let header = intercepted_authorization(interceptor);
        let encoded = header
            .strip_prefix("Basic ")
            .expect("basic auth header should start with 'Basic '");
        let decoded = BASE64_STANDARD
            .decode(encoded)
            .expect("payload should be valid base64");
        assert_eq!(decoded, b"alice:hunter2");
    }

    #[test]
    fn basic_auth_without_password_emits_trailing_colon() {
        let mut interceptor = HeadersInterceptor::new();
        interceptor.basic_auth("alice", None::<&str>);

        let header = intercepted_authorization(interceptor);
        let encoded = header.strip_prefix("Basic ").unwrap();
        let decoded = BASE64_STANDARD.decode(encoded).unwrap();
        assert_eq!(decoded, b"alice:");
    }

    #[test]
    fn bearer_auth_sets_authorization_header() {
        let mut interceptor = HeadersInterceptor::new();
        interceptor.bearer_auth("my-token");

        let header = intercepted_authorization(interceptor);
        assert_eq!(header, "Bearer my-token");
    }

    #[test]
    fn empty_interceptor_does_not_set_authorization_header() {
        let mut interceptor = HeadersInterceptor::new();
        let request = interceptor.call(tonic::Request::new(())).unwrap();
        assert!(
            request
                .metadata()
                .get(http::header::AUTHORIZATION.as_str())
                .is_none()
        );
    }
}
