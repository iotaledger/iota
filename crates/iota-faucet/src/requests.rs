// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_types::Address;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum FaucetRequest {
    FixedAmountRequest(FixedAmountRequest),
    GetBatchSendStatusRequest(GetBatchSendStatusRequest),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FixedAmountRequest {
    pub recipient: Address,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GetBatchSendStatusRequest {
    pub task_id: String,
}

impl FaucetRequest {
    pub fn new_fixed_amount_request(recipient: impl Into<Address>) -> Self {
        Self::FixedAmountRequest(FixedAmountRequest {
            recipient: recipient.into(),
        })
    }

    pub fn new_get_batch_send_status_request(task_id: impl Into<String>) -> Self {
        Self::GetBatchSendStatusRequest(GetBatchSendStatusRequest {
            task_id: task_id.into(),
        })
    }
}
