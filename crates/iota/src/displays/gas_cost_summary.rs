// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::fmt::{Display, Formatter};

use iota_types::gas::GasCostSummary;

use crate::displays::Pretty;

impl Display for Pretty<'_, GasCostSummary> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let Pretty(gcs) = self;
        let GasCostSummary {
            computation_cost,
            computation_cost_burned,
            storage_cost,
            storage_rebate,
            non_refundable_storage_fee,
        } = gcs;
        let output = format!(
            "Gas Cost Summary:\n   \
                 Computation Cost: {computation_cost}\n   \
                 Computation Cost Burned: {computation_cost_burned}\n   \
                 Storage Cost: {storage_cost}\n   \
                 Storage Rebate: {storage_rebate}\n   \
                 Non-refundable Storage Fee: {non_refundable_storage_fee}",
        );
        write!(f, "{}", output)
    }
}
