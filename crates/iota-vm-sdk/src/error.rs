// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Typed error surface for the local VM SDK.
//!
//! Every fallible entry point returns [`VmSdkError`]. The variants partition
//! the failure space into the four phases of the SDK — decode, validate,
//! verify-signature, execute — plus a catch-all for Move VM faults and missing
//! objects, so callers can branch on the failure phase.
//!
//! The sub-error types ([`DecodeError`], [`ValidationError`], …) wrap the
//! underlying cause as a string.

use iota_sdk_types::{ObjectId, Version};

/// Top-level error for every fallible operation in the SDK.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum VmSdkError {
    /// A static decode step failed (BCS / signature / type-tag parsing).
    #[error(transparent)]
    Decode(#[from] DecodeError),
    /// Pre-execution validation of the transaction failed — malformed data,
    /// gas budget below minimum, denied object, failed input check, etc.
    #[error(transparent)]
    Validation(#[from] ValidationError),
    /// Cryptographic signature verification failed before execution.
    #[error(transparent)]
    SignatureVerification(#[from] SignatureError),
    /// A required object was not present in the store.
    #[error("missing object {id}{}", .version.map(|v| format!(" at version {v}")).unwrap_or_default())]
    MissingObject {
        id: ObjectId,
        version: Option<Version>,
    },
    /// Execution produced a recoverable Move-level error (an abort, a runtime
    /// limit, etc.). The transaction ran but did not succeed.
    #[error(transparent)]
    Execution(#[from] ExecutionError),
    /// The Move VM itself faulted in a way that prevented execution (invariant
    /// violation, executor construction failure, …).
    #[error(transparent)]
    Vm(#[from] VmError),
    /// This build of the SDK does not know the requested protocol version
    /// (typically a node running a newer protocol than this binary supports).
    #[error("unsupported protocol version {}", version.as_u64())]
    UnsupportedProtocolVersion {
        version: iota_protocol_config::ProtocolVersion,
    },
}

impl VmSdkError {
    /// Construct a [`VmSdkError::MissingObject`].
    pub fn missing_object(id: ObjectId, version: Option<Version>) -> Self {
        Self::MissingObject { id, version }
    }
}

/// A static decode step failed.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
#[error("{context}: {message}")]
pub struct DecodeError {
    pub context: String,
    pub message: String,
}

impl DecodeError {
    pub fn new(context: impl Into<String>, message: impl std::fmt::Display) -> Self {
        Self {
            context: context.into(),
            message: message.to_string(),
        }
    }
}

/// Pre-execution validation failed.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
#[error("{context}: {message}")]
pub struct ValidationError {
    pub context: String,
    pub message: String,
}

impl ValidationError {
    pub fn new(context: impl Into<String>, message: impl std::fmt::Display) -> Self {
        Self {
            context: context.into(),
            message: message.to_string(),
        }
    }
}

/// Cryptographic signature verification failed.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
#[error("signature verification failed: {message}")]
pub struct SignatureError {
    pub message: String,
}

impl SignatureError {
    pub fn new(message: impl std::fmt::Display) -> Self {
        Self {
            message: message.to_string(),
        }
    }
}

/// A recoverable Move-level execution error.
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
