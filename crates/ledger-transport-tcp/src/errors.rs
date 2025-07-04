// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use thiserror::Error;

#[derive(Error, Debug)]
pub enum LedgerTCPError {
    #[error("Ledger connect error")]
    ConnectError,
    #[error("TCP response error")]
    ResponseError,
    #[error("Ledger inner error")]
    InnerError,
}
