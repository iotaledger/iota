// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Typed error surface for the local VM SDK.
//!
//! Every fallible entry point returns [`VmSdkError`], whose variants partition
//! the failure space by phase. The validation and signature-verification phases
//! carry the underlying [`IotaError`] as a [`std::error::Error::source`] for
//! matching on the concrete cause; the remaining phases wrap a message because
//! their causes (BCS / layout / VM invariant failures) are not a single
//! matchable type.

use iota_sdk_types::{ObjectId, Version};
use iota_types::error::IotaError;

/// Top-level error for every fallible operation in the SDK.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum VmSdkError {
    /// Pre-execution validation of the transaction failed — malformed data,
    /// gas budget below minimum, denied object, failed input check, etc.
    #[error(transparent)]
    Validation(#[from] ValidationError),
    /// Cryptographic signature verification failed before execution. Match on
    /// the wrapped [`IotaError`] for the concrete cause.
    #[error("signature verification failed: {0}")]
    SignatureVerification(#[source] IotaError),
    /// A networked store (`GrpcStore` / `GraphQLStore`) failed to fetch or
    /// decode data from a node.
    #[error(transparent)]
    Store(#[from] StoreError),
    /// A required object was not present in the store.
    #[error("missing object {id}{}", .version.map(|v| format!(" at version {v}")).unwrap_or_default())]
    MissingObject {
        id: ObjectId,
        version: Option<Version>,
    },
    /// Failed to decode a value or event against its Move type layout (layout
    /// resolution or BCS deserialization). A Move-level abort is *not* reported
    /// here — it surfaces on
    /// [`ExecutionResult::status`](crate::ExecutionResult).
    #[error(transparent)]
    Execution(#[from] ExecutionError),
    /// The Move VM itself faulted in a way that prevented execution (invariant
    /// violation, executor construction failure, authenticator resolution, …).
    #[error(transparent)]
    Vm(#[from] VmError),
    /// The requested protocol version cannot serve the request: either this
    /// build of the SDK does not know the version (typically a node running a
    /// newer protocol than this binary supports), or the version predates a
    /// feature the request requires (carried in `feature`).
    #[error("unsupported protocol version {}{}", version.as_u64(), feature.map(|f| format!(": {f} is not available")).unwrap_or_default())]
    UnsupportedProtocolVersion {
        version: iota_protocol_config::ProtocolVersion,
        /// The feature whose absence makes the version unsupported (e.g.
        /// `"MoveAuthenticator signatures"`), or `None` when the version itself
        /// is simply unknown to this build.
        feature: Option<&'static str>,
    },
}

/// Pre-execution validation failed. `source` is the underlying node error to
/// match on; `context` names the check that produced it.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
#[error("{context}: {source}")]
pub struct ValidationError {
    pub context: String,
    #[source]
    pub source: IotaError,
}

impl ValidationError {
    pub fn new(context: impl Into<String>, source: impl Into<IotaError>) -> Self {
        Self {
            context: context.into(),
            source: source.into(),
        }
    }
}

/// A `MoveAuthenticator` rejected the transaction (the authenticator function
/// aborted). Carried by [`SignatureStatus::Failed`](crate::SignatureStatus).
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
#[error("signature verification failed: {source}")]
pub struct SignatureError {
    /// The authenticator's typed rejection cause, for matching on the concrete
    /// `ExecutionErrorKind`.
    #[source]
    pub source: iota_types::error::ExecutionError,
}

impl SignatureError {
    pub fn new(source: iota_types::error::ExecutionError) -> Self {
        Self { source }
    }
}

/// A networked store failed to fetch or decode data from a node.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
#[error("{context}: {message}")]
pub struct StoreError {
    pub context: String,
    pub message: String,
}

impl StoreError {
    /// Build a [`StoreError`]; `context` names the fetch/decode step that
    /// failed and `message` carries the underlying cause.
    pub fn new(context: impl Into<String>, message: impl std::fmt::Display) -> Self {
        Self {
            context: context.into(),
            message: message.to_string(),
        }
    }
}

/// Failed to decode a value or event against its Move type layout.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
#[error("execution error: {message}")]
pub struct ExecutionError {
    pub message: String,
}

impl ExecutionError {
    pub fn new(message: impl std::fmt::Display) -> Self {
        Self {
            message: message.to_string(),
        }
    }
}

/// The Move VM faulted before or during execution.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
#[error("vm error: {message}")]
pub struct VmError {
    pub message: String,
}

impl VmError {
    pub fn new(message: impl std::fmt::Display) -> Self {
        Self {
            message: message.to_string(),
        }
    }
}
