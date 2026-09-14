// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_client::GrpcError;

/// Check if error is a gRPC error with the specified status code.
///
/// Matches both transport-level `Error::Grpc` and per-item `Error::Server`
/// errors, since batch APIs return per-item failures as `Error::Server`.
pub fn is_grpc_error(err: &GrpcError, code: tonic::Code) -> bool {
    match err {
        GrpcError::Grpc(status) => status.code() == code,
        GrpcError::Server(status) => tonic::Code::from_i32(status.code) == code,
        _ => false,
    }
}

/// Check if error is a gRPC NotFound error.
pub fn is_grpc_not_found(err: &GrpcError) -> bool {
    is_grpc_error(err, tonic::Code::NotFound)
}

/// Assert that a result is a gRPC error with the specified status code.
pub fn assert_grpc_error<T: std::fmt::Debug>(result: Result<T, GrpcError>, code: tonic::Code) {
    match result {
        Err(ref err) if is_grpc_error(err, code) => {}
        Err(err) => panic!("Expected gRPC {code:?} error, got: {err:?}"),
        Ok(val) => panic!("Expected gRPC {code:?} error, got success: {val:?}"),
    }
}

/// Assert that a result is a gRPC NotFound error.
pub fn assert_grpc_not_found<T: std::fmt::Debug>(result: Result<T, GrpcError>) {
    match result {
        Err(ref err) if is_grpc_not_found(err) => {}
        Err(err) => panic!("Expected gRPC NotFound error, got: {err:?}"),
        Ok(val) => panic!("Expected gRPC NotFound error, got success: {val:?}"),
    }
}

/// Check if error is a Server error with NOT_FOUND status code.
pub fn is_server_not_found(err: &GrpcError) -> bool {
    matches!(err, GrpcError::Server(status) if tonic::Code::from_i32(status.code) == tonic::Code::NotFound)
}

/// Assert that a result is a Server "not found" error.
pub fn assert_server_not_found<T: std::fmt::Debug>(result: Result<T, GrpcError>) {
    match result {
        Err(ref err) if is_server_not_found(err) => {}
        Err(err) => panic!("Expected Server not-found error, got: {err:?}"),
        Ok(val) => panic!("Expected Server not-found error, got success: {val:?}"),
    }
}

/// Check if error is a proto conversion error.
pub fn is_proto_conversion_error(err: &GrpcError) -> bool {
    matches!(err, GrpcError::ProtoConversion(_))
}

/// Assert that a result is a proto conversion error.
pub fn assert_proto_conversion_error<T: std::fmt::Debug>(result: Result<T, GrpcError>) {
    match result {
        Err(ref err) if is_proto_conversion_error(err) => {}
        Err(err) => panic!("Expected ProtoConversion error, got: {err:?}"),
        Ok(val) => panic!("Expected ProtoConversion error, got success: {val:?}"),
    }
}
