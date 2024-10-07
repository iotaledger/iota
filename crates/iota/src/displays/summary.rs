// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::fmt::{Display, Formatter};

use tabled::{
    builder::Builder as TableBuilder,
    settings::{Panel as TablePanel, Style as TableStyle, style::HorizontalLine},
};

use crate::{client_ptb::ptb::Summary, displays::Pretty};

impl<'a> Display for Pretty<'a, Summary> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut builder = TableBuilder::default();
        let Pretty(summary) = self;
        let Summary {
            digest,
            status,
            gas_cost,
        } = summary;

        builder.push_record(vec![format!("Digest: {}", digest)]);
        builder.push_record(vec![format!("Status: {}", Pretty(status))]);
        builder.push_record(vec![format!("{}", Pretty(gas_cost))]);
        let mut table = builder.build();
        table.with(TablePanel::header("PTB Execution Summary"));
        table.with(TableStyle::rounded().horizontals([HorizontalLine::new(
            1,
            TableStyle::modern().get_horizontal(),
        )]));
        write!(f, "{}", table)
    }
}
