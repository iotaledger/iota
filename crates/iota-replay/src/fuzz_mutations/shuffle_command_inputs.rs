// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_types::transaction::{
    Command, MakeMoveVector, MergeCoins, ProgrammableMoveCall, SplitCoins, TransactionKind,
    TransferObjects,
};
use rand::seq::SliceRandom;
use tracing::info;

use crate::fuzz::TransactionKindMutator;

pub struct ShuffleCommandInputs {
    pub rng: rand::rngs::StdRng,
    pub num_mutations_per_base_left: u64,
}

impl ShuffleCommandInputs {
    fn shuffle_command(&mut self, command: &mut Command) {
        match command {
            Command::MakeMoveVector(MakeMoveVector { elements: args, .. })
            | Command::MergeCoins(MergeCoins {
                coins_to_merge: args,
                ..
            })
            | Command::SplitCoins(SplitCoins { amounts: args, .. })
            | Command::TransferObjects(TransferObjects { objects: args, .. })
            | Command::MoveCall(ProgrammableMoveCall {
                arguments: args, ..
            }) => {
                args.shuffle(&mut self.rng);
            }
            Command::Publish(_) | Command::Upgrade(_) => (),
            _ => unimplemented!("a new Command enum variant was added and needs to be handled"),
        }
    }
}

impl TransactionKindMutator for ShuffleCommandInputs {
    fn mutate(&mut self, transaction_kind: &TransactionKind) -> Option<TransactionKind> {
        if self.num_mutations_per_base_left == 0 {
            // Nothing else to do
            return None;
        }

        self.num_mutations_per_base_left -= 1;
        if let TransactionKind::ProgrammableTransaction(mut p) = transaction_kind.clone() {
            for command in &mut p.commands {
                self.shuffle_command(command);
            }
            info!("Mutation: Shuffling command inputs");
            Some(TransactionKind::ProgrammableTransaction(p))
        } else {
            // Other types not supported yet
            None
        }
    }

    fn reset(&mut self, mutations_per_base: u64) {
        self.num_mutations_per_base_left = mutations_per_base;
    }
}
