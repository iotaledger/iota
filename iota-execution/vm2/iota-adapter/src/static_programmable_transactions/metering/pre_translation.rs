// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_types::{
    Command, Input, MakeMoveVector, MergeCoins, ProgrammableTransaction, Publish, SplitCoins,
    TransferObjects, Upgrade,
};

use crate::{
    compat::ExecutionErrorTrait,
    static_programmable_transactions::metering::translation_meter::TranslationMeter,
};

/// Before loading and type checking, we do a first pass over the transaction to charge for basic
/// properties:
/// - number of inputs and pure input bytes
/// - number of commands and their arguments
/// - for Move calls, count arguments to the function (both value and type arguments) as the
///   "arguments" to charge for for the command.
pub fn meter<E: ExecutionErrorTrait>(
    meter: &mut TranslationMeter,
    transaction: &ProgrammableTransaction,
) -> Result<(), E> {
    meter.charge_base_inputs(transaction.inputs.len())?;

    for input in &transaction.inputs {
        match input {
            Input::Pure(bytes) => {
                meter.charge_pure_input_bytes(bytes.len())?;
            }
            Input::ImmutableOrOwned(_) | Input::Shared(_) | Input::Receiving(_) => (),
            _ => unimplemented!("a new Input enum variant was added and needs to be handled"),
        }
    }

    for command in &transaction.commands {
        meter.charge_base_command(arguments_len(command))?;
    }

    Ok(())
}

fn arguments_len(cmd: &Command) -> usize {
    match cmd {
        Command::MoveCall(call) => call
            .type_arguments
            .len()
            .saturating_add(call.arguments.len()),
        Command::TransferObjects(TransferObjects { objects: args, .. })
        | Command::SplitCoins(SplitCoins { amounts: args, .. })
        | Command::MergeCoins(MergeCoins {
            coins_to_merge: args,
            ..
        }) => args.len().saturating_add(1),
        Command::Publish(Publish {
            modules,
            dependencies,
        }) => modules.len().saturating_add(dependencies.len()),
        Command::MakeMoveVector(MakeMoveVector { elements, .. }) => {
            elements.len().saturating_add(1)
        }
        Command::Upgrade(Upgrade {
            modules,
            dependencies,
            ..
        }) => modules
            .len()
            .saturating_add(dependencies.len())
            .saturating_add(2),
        _ => unimplemented!("a new Command enum variant was added and needs to be handled"),
    }
}
