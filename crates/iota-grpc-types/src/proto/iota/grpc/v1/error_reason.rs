// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

include!("../../../generated/iota.grpc.v1.error_reason.rs");

impl AsRef<str> for ErrorReason {
    fn as_ref(&self) -> &str {
        self.as_str_name()
    }
}

impl From<ErrorReason> for String {
    fn from(value: ErrorReason) -> Self {
        value.as_ref().into()
    }
}
