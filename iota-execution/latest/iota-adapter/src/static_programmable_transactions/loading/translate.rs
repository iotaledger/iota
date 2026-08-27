// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_types::{
    Command, Input, MakeMoveVector, MergeCoins, MoveCall, ObjectReference, Owner,
    ProgrammableTransaction, Publish, SharedObjectReference, SplitCoins, StructTag,
    TransferObjects, Upgrade,
};
use iota_types::error::ExecutionError;

use crate::static_programmable_transactions::{env::Env, loading::ast as L};

pub fn transaction(
    env: &Env,
    pt: ProgrammableTransaction,
) -> Result<L::Transaction, ExecutionError> {
    let ProgrammableTransaction { inputs, commands } = pt;
    let inputs = inputs
        .into_iter()
        .map(|arg| input(env, arg))
        .collect::<Result<Vec<_>, _>>()?;
    let commands = commands
        .into_iter()
        .map(|cmd| command(env, cmd))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(L::Transaction { inputs, commands })
}

fn object_ref(oref: &ObjectReference) -> L::ObjectRef {
    (*oref.object_id(), oref.version(), *oref.digest())
}

fn input(env: &Env, arg: Input) -> Result<(L::InputArg, L::InputType), ExecutionError> {
    Ok(match arg {
        Input::Pure(bytes) => (L::InputArg::Pure(bytes), L::InputType::Bytes),
        Input::Receiving(oref) => (
            L::InputArg::Receiving(object_ref(&oref)),
            L::InputType::Bytes,
        ),
        Input::ImmutableOrOwned(oref) => {
            let id = oref.object_id();
            let obj = env.read_object(id)?;
            let Some(ty) = obj.type_() else {
                invariant_violation!("Object {:?} has does not have a Move type", id);
            };
            let tag: StructTag = ty.clone().into();
            let ty = env.load_type_from_struct(&tag)?;
            let oref = object_ref(&oref);
            let arg = match obj.owner {
                Owner::Address(_) => L::ObjectArg::OwnedObject(oref),
                Owner::Immutable => L::ObjectArg::ImmObject(oref),
                Owner::Object(_) | Owner::Shared(_) => {
                    invariant_violation!("Unexpected owner for ImmOrOwnedObject: {:?}", obj.owner);
                }
                _ => unimplemented!("a new Owner enum variant was added and needs to be handled"),
            };
            (L::InputArg::Object(arg), L::InputType::Fixed(ty))
        }
        Input::Shared(SharedObjectReference {
            object_id,
            initial_shared_version,
            mutable,
        }) => {
            let obj = env.read_object(&object_id)?;
            let Some(ty) = obj.type_() else {
                invariant_violation!("Object {:?} has does not have a Move type", object_id);
            };
            let tag: StructTag = ty.clone().into();
            let ty = env.load_type_from_struct(&tag)?;
            (
                L::InputArg::Object(L::ObjectArg::SharedObject {
                    id: object_id,
                    initial_shared_version,
                    mutable,
                }),
                L::InputType::Fixed(ty),
            )
        }
        _ => unimplemented!("a new Input enum variant was added and needs to be handled"),
    })
}

fn command(env: &Env, command: Command) -> Result<L::Command, ExecutionError> {
    Ok(match command {
        Command::MoveCall(move_call) => {
            let MoveCall {
                package,
                module,
                function: name,
                type_arguments: ptype_arguments,
                arguments,
            } = move_call;
            let type_arguments = ptype_arguments
                .into_iter()
                .enumerate()
                .map(|(idx, ty)| env.load_type_input(idx, ty))
                .collect::<Result<Vec<_>, _>>()?;
            let function = env.load_function(
                package,
                module.to_string(),
                name.to_string(),
                type_arguments,
            )?;
            L::Command::MoveCall(Box::new(L::MoveCall {
                function,
                arguments,
            }))
        }
        Command::MakeMoveVector(MakeMoveVector { type_tag, elements }) => {
            let type_argument = type_tag.map(|ty| env.load_type_input(0, ty)).transpose()?;
            L::Command::MakeMoveVec(type_argument, elements)
        }
        Command::TransferObjects(TransferObjects { objects, address }) => {
            L::Command::TransferObjects(objects, address)
        }
        Command::SplitCoins(SplitCoins { coin, amounts }) => L::Command::SplitCoins(coin, amounts),
        Command::MergeCoins(MergeCoins {
            coin,
            coins_to_merge,
        }) => L::Command::MergeCoins(coin, coins_to_merge),
        Command::Publish(Publish {
            modules,
            dependencies,
        }) => L::Command::Publish(modules, dependencies),
        Command::Upgrade(Upgrade {
            modules,
            dependencies,
            package,
            ticket,
        }) => L::Command::Upgrade(modules, dependencies, package, ticket),
        _ => unimplemented!("a new Command enum variant was added and needs to be handled"),
    })
}
