// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#![allow(clippy::large_enum_variant)]
#![allow(clippy::doc_overindented_list_items)]

use google::rpc::bad_request::FieldViolation;
use iota::grpc::v0::error_reason::ErrorReason;

pub(crate) mod google;
pub(crate) mod iota;

pub type BoxError = Box<dyn std::error::Error + Send + Sync + 'static>;

#[derive(Debug)]
pub struct TryFromProtoError {
    field_violation: FieldViolation,
    source: Option<BoxError>,
}

impl std::fmt::Display for TryFromProtoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "error converting from protobuf: ")?;

        write!(f, "field: {}", self.field_violation.field)?;

        if !self.field_violation.reason.is_empty() {
            write!(f, " reason: {}", self.field_violation.reason)?;
        }

        if !self.field_violation.description.is_empty() {
            write!(f, " description: {}", self.field_violation.description)?;
        }

        Ok(())
    }
}

impl std::error::Error for TryFromProtoError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source.as_deref().map(|s| s as _)
    }
}

impl TryFromProtoError {
    pub fn nested<T: AsRef<str>>(mut self, field: T) -> Self {
        let field = field.as_ref();
        self.field_violation = self.field_violation.nested(field);
        self
    }

    pub fn nested_at<T: AsRef<str>>(mut self, field: T, index: usize) -> Self {
        let field = field.as_ref();
        self.field_violation = self.field_violation.nested_at(field, index);
        self
    }

    pub fn missing<T: AsRef<str>>(field: T) -> Self {
        let field = field.as_ref();

        Self {
            field_violation: FieldViolation::new(field).with_reason(ErrorReason::FieldMissing),
            source: None,
        }
    }

    pub fn invalid<T: AsRef<str>, E: Into<BoxError>>(field: T, error: E) -> Self {
        let field = field.as_ref();
        let error = error.into();

        Self {
            field_violation: FieldViolation::new(field)
                .with_reason(ErrorReason::FieldInvalid)
                .with_description(error.to_string()),
            source: Some(error),
        }
    }

    pub fn field_violation(&self) -> &FieldViolation {
        &self.field_violation
    }
}

#[derive(Debug)]
pub enum GrpcConversionError {
    UnsupportedArgumentType { arg_type: String },
    BcsSerializationFailed { message: String },
}

impl std::fmt::Display for GrpcConversionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedArgumentType { arg_type } => {
                write!(
                    f,
                    "Unsupported argument type for gRPC conversion: {}",
                    arg_type
                )
            }
            Self::BcsSerializationFailed { message } => {
                write!(f, "Failed to serialize BCS data: {}", message)
            }
        }
    }
}

impl std::error::Error for GrpcConversionError {}

// TimeStamp
//

pub fn timestamp_ms_to_proto(timestamp_ms: u64) -> prost_types::Timestamp {
    let timestamp = std::time::Duration::from_millis(timestamp_ms);
    prost_types::Timestamp {
        seconds: timestamp.as_secs() as i64,
        nanos: timestamp.subsec_nanos() as i32,
    }
}

#[allow(clippy::result_large_err)]
pub fn proto_to_timestamp_ms(timestamp: prost_types::Timestamp) -> Result<u64, TryFromProtoError> {
    let seconds = std::time::Duration::from_secs(
        timestamp
            .seconds
            .try_into()
            .map_err(|e| TryFromProtoError::invalid("seconds", e))?,
    );
    let nanos = std::time::Duration::from_nanos(
        timestamp
            .nanos
            .try_into()
            .map_err(|e| TryFromProtoError::invalid("nanos", e))?,
    );

    (seconds + nanos)
        .as_millis()
        .try_into()
        .map_err(|e| TryFromProtoError::invalid("seconds + nanos", e))
}
