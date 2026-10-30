// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::fmt::{Display, Formatter};

use iota_grpc_types::v1::transaction_execution_service::SimulatedTransaction;
use iota_sdk_types::Transaction;
use iota_types::{effects::TransactionEffectsAPI, transaction::TransactionAPI};
use tabled::{
    builder::Builder as TableBuilder,
    settings::{Panel as TablePanel, Style as TableStyle, style::HorizontalLine},
};

use crate::{client_commands::estimate_gas_budget_from_gas_cost, displays::Pretty};

/// Renders a dry run: the effects, events, and object/balance changes of the
/// simulated transaction, plus the transaction data and gas estimate derived
/// from the locally built transaction.
pub struct DryRunDisplay<'a> {
    pub simulated: &'a SimulatedTransaction,
    pub transaction: &'a Transaction,
}

impl Display for DryRunDisplay<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let DryRunDisplay {
            simulated,
            transaction,
        } = self;

        let executed = simulated.executed_transaction().ok();
        let effects = executed.and_then(|e| e.effects().and_then(|e| e.effects()).ok());

        if let Some(effects) = &effects {
            writeln!(
                f,
                "Dry run completed, execution status: {:?}",
                effects.status()
            )?;
        }

        let mut builder = TableBuilder::default();
        builder.push_record(vec![format!("Sender: {}", transaction.sender())]);
        builder.push_record(vec![format!("Gas Owner: {}", transaction.gas_owner())]);
        builder.push_record(vec![format!("Gas Budget: {}", transaction.gas_budget())]);
        builder.push_record(vec![format!("Gas Price: {}", transaction.gas_price())]);
        let mut table = builder.build();
        table.with(TablePanel::header("Dry Run Transaction Data"));
        table.with(TableStyle::rounded().horizontals([HorizontalLine::new(
            1,
            TableStyle::modern().get_horizontal(),
        )]));
        writeln!(f, "{table}")?;

        if let Some(executed) = executed {
            write!(f, "{}", Pretty(executed))?;
        }

        if let Some(effects) = &effects {
            writeln!(
                f,
                "Estimated gas cost (includes a small buffer): {} NANOS",
                estimate_gas_budget_from_gas_cost(
                    effects.gas_cost_summary(),
                    transaction.gas_price()
                )
            )?;
        }

        if let Some(source) = simulated.execution_error().and_then(|e| e.source.as_ref()) {
            writeln!(f, "Execution error: {source}")?;
        }

        Ok(())
    }
}
