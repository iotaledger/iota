// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
use iota_grpc_types::headers;
use iota_sdk_types::Digest;

/// Extension trait used to facilitate retrieval of IOTA specific data from
/// responses
pub trait ResponseExt: sealed::Sealed {
    fn chain_id(&self) -> Option<Digest>;
    fn chain(&self) -> Option<&str>;
    fn epoch(&self) -> Option<u64>;
    fn checkpoint_height(&self) -> Option<u64>;
    fn timestamp_ms(&self) -> Option<u64>;
    fn timestamp(&self) -> Option<&str>;
    fn lowest_available_checkpoint(&self) -> Option<u64>;
    fn lowest_available_checkpoint_objects(&self) -> Option<u64>;
    fn server_version(&self) -> Option<&str>;
}

impl ResponseExt for http::header::HeaderMap {
    fn chain_id(&self) -> Option<Digest> {
        self.get(headers::X_IOTA_CHAIN_ID)
            .map(http::header::HeaderValue::as_bytes)
            .and_then(|s| Digest::from_base58(s).ok())
    }

    fn chain(&self) -> Option<&str> {
        self.get(headers::X_IOTA_CHAIN)
            .and_then(|h| h.to_str().ok())
    }

    fn epoch(&self) -> Option<u64> {
        self.get(headers::X_IOTA_EPOCH)
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.parse().ok())
    }

    fn checkpoint_height(&self) -> Option<u64> {
        self.get(headers::X_IOTA_CHECKPOINT_HEIGHT)
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.parse().ok())
    }

    fn timestamp_ms(&self) -> Option<u64> {
        self.get(headers::X_IOTA_TIMESTAMP_MS)
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.parse().ok())
    }

    fn timestamp(&self) -> Option<&str> {
        self.get(headers::X_IOTA_TIMESTAMP)
            .and_then(|h| h.to_str().ok())
    }

    fn lowest_available_checkpoint(&self) -> Option<u64> {
        self.get(headers::X_IOTA_LOWEST_AVAILABLE_CHECKPOINT)
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.parse().ok())
    }

    fn lowest_available_checkpoint_objects(&self) -> Option<u64> {
        self.get(headers::X_IOTA_LOWEST_AVAILABLE_CHECKPOINT_OBJECTS)
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.parse().ok())
    }

    fn server_version(&self) -> Option<&str> {
        self.get(headers::X_IOTA_SERVER)
            .and_then(|h| h.to_str().ok())
    }
}

impl ResponseExt for tonic::metadata::MetadataMap {
    fn chain_id(&self) -> Option<Digest> {
        self.as_ref().chain_id()
    }

    fn chain(&self) -> Option<&str> {
        self.as_ref().chain()
    }

    fn epoch(&self) -> Option<u64> {
        self.as_ref().epoch()
    }

    fn checkpoint_height(&self) -> Option<u64> {
        self.as_ref().checkpoint_height()
    }

    fn timestamp_ms(&self) -> Option<u64> {
        self.as_ref().timestamp_ms()
    }

    fn timestamp(&self) -> Option<&str> {
        self.as_ref().timestamp()
    }

    fn lowest_available_checkpoint(&self) -> Option<u64> {
        self.as_ref().lowest_available_checkpoint()
    }

    fn lowest_available_checkpoint_objects(&self) -> Option<u64> {
        self.as_ref().lowest_available_checkpoint_objects()
    }

    fn server_version(&self) -> Option<&str> {
        self.as_ref().server_version()
    }
}

impl<T> ResponseExt for tonic::Response<T> {
    fn chain_id(&self) -> Option<Digest> {
        self.metadata().chain_id()
    }

    fn chain(&self) -> Option<&str> {
        self.metadata().chain()
    }

    fn epoch(&self) -> Option<u64> {
        self.metadata().epoch()
    }

    fn checkpoint_height(&self) -> Option<u64> {
        self.metadata().checkpoint_height()
    }

    fn timestamp_ms(&self) -> Option<u64> {
        self.metadata().timestamp_ms()
    }

    fn timestamp(&self) -> Option<&str> {
        self.metadata().timestamp()
    }

    fn lowest_available_checkpoint(&self) -> Option<u64> {
        self.metadata().lowest_available_checkpoint()
    }

    fn lowest_available_checkpoint_objects(&self) -> Option<u64> {
        self.metadata().lowest_available_checkpoint_objects()
    }

    fn server_version(&self) -> Option<&str> {
        self.metadata().server_version()
    }
}

impl ResponseExt for tonic::Status {
    fn chain_id(&self) -> Option<Digest> {
        self.metadata().chain_id()
    }

    fn chain(&self) -> Option<&str> {
        self.metadata().chain()
    }

    fn epoch(&self) -> Option<u64> {
        self.metadata().epoch()
    }

    fn checkpoint_height(&self) -> Option<u64> {
        self.metadata().checkpoint_height()
    }

    fn timestamp_ms(&self) -> Option<u64> {
        self.metadata().timestamp_ms()
    }

    fn timestamp(&self) -> Option<&str> {
        self.metadata().timestamp()
    }

    fn lowest_available_checkpoint(&self) -> Option<u64> {
        self.metadata().lowest_available_checkpoint()
    }

    fn lowest_available_checkpoint_objects(&self) -> Option<u64> {
        self.metadata().lowest_available_checkpoint_objects()
    }

    fn server_version(&self) -> Option<&str> {
        self.metadata().server_version()
    }
}

mod sealed {
    pub trait Sealed {}

    impl Sealed for tonic::Status {}
    impl<T> Sealed for tonic::Response<T> {}
    impl Sealed for http::header::HeaderMap {}
    impl Sealed for tonic::metadata::MetadataMap {}
    impl<T> Sealed for crate::api::MetadataEnvelope<T> {}
}
