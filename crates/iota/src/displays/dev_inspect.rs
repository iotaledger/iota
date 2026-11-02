// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::fmt::{Display, Formatter};

use iota_grpc_types::v1::{
    command::CommandOutput, transaction_execution_service::SimulatedTransaction,
};
use iota_types::effects::TransactionEffectsAPI;

use crate::displays::Pretty;

impl Display for Pretty<'_, SimulatedTransaction> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let Pretty(simulated) = self;

        if let Some(error) = simulated.execution_error() {
            match &error.source {
                Some(source) => writeln!(f, "Dev inspect failed: {source}")?,
                None => writeln!(f, "Dev inspect failed")?,
            }
            return Ok(());
        }

        let executed = simulated.executed_transaction().ok();

        if let Some(effects) = executed.and_then(|e| e.effects().and_then(|e| e.effects()).ok()) {
            writeln!(
                f,
                "Dev inspect completed, execution status: {:?}",
                effects.status()
            )?;
        }

        if let Some(executed) = executed {
            write!(f, "{}", Pretty(executed))?;
        }

        let Some(command_results) = simulated.command_results() else {
            return Ok(());
        };

        if command_results.results.is_empty() {
            writeln!(f, "No execution results")?;
            return Ok(());
        }

        writeln!(f, "Execution Result")?;
        for (index, result) in command_results.results.iter().enumerate() {
            writeln!(f, "  Command {index}:")?;

            if let Ok(mutated) = result.mutated_by_ref() {
                if !mutated.outputs.is_empty() {
                    writeln!(f, "    Mutable Reference Outputs")?;
                    for output in &mutated.outputs {
                        write_command_output(f, output, true)?;
                    }
                }
            }

            if let Ok(returned) = result.return_values() {
                if !returned.outputs.is_empty() {
                    writeln!(f, "    Return values")?;
                    for output in &returned.outputs {
                        write_command_output(f, output, false)?;
                    }
                }
            }
        }

        Ok(())
    }
}

fn write_command_output(
    f: &mut Formatter<'_>,
    output: &CommandOutput,
    with_argument: bool,
) -> std::fmt::Result {
    if with_argument {
        if let Ok(argument) = output.argument() {
            writeln!(f, "      Argument: {argument:?}")?;
        }
    }
    if let Ok(type_tag) = output.type_tag() {
        writeln!(f, "      TypeTag: {type_tag}")?;
    }
    if let Ok(bytes) = output.output_bcs() {
        writeln!(f, "      Bytes: {bytes:?}")?;
    }
    Ok(())
}
