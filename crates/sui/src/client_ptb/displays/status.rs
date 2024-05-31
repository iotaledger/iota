// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use std::fmt::{Display, Formatter};

use sui_json_rpc_types::SuiExecutionStatus::{self, Failure, Success};

use crate::client_ptb::displays::Pretty;

impl<'a> Display for Pretty<'a, SuiExecutionStatus> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let Pretty(status) = self;

        let output = match status {
            Success => "success".to_string(),
            Failure { error } => format!("failed due to {error}"),
        };

        write!(f, "{}", output)
    }
}
