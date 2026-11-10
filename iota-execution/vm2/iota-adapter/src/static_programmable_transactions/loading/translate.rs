// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_types as P;
use iota_sdk_types::{
    Command, Input, MakeMoveVector, MergeCoins, MoveCall, Owner, Publish, SharedObjectReference,
    SplitCoins, TransferObjects, Upgrade,
};
use iota_types::{base_types::TxContext, transaction::ProgrammableTransactionExt};

use crate::{
    compat::{ExecutionErrorTrait, ObjectPermissions},
    execution_mode::ExecutionMode,
    gas_charger::GasPayment,
    sdk_conv::core_struct_tag,
    static_programmable_transactions::{
        env::Env,
        linkage,
        loading::{ast as L, ast::PackagePayload},
        metering,
        metering::translation_meter::TranslationMeter,
    },
};

pub fn transaction<Mode: ExecutionMode>(
    meter: &mut TranslationMeter<'_>,
    env: &Env<Mode>,
    _tx_context: &TxContext,
    gas_payment: Option<GasPayment>,
    pt: P::ProgrammableTransaction,
) -> Result<L::Transaction, Mode::Error> {
    if env.protocol_config.validate_ptb_argument_indices()
        && let Err(err) = pt.validate_argument_indices()
    {
        invariant_violation!(
            "PTB argument indices are checked at signing -- this should be impossible: {err}"
        );
    }
    metering::pre_translation::meter::<Mode::Error>(meter, &pt)?;
    let P::ProgrammableTransaction { inputs, commands } = pt;
    let inputs = inputs
        .into_iter()
        .map(|arg| input::<Mode>(env, arg))
        .collect::<Result<Vec<_>, _>>()?;
    let original_command_len = commands.len();
    let commands = commands
        .into_iter()
        .enumerate()
        .map(|(idx, cmd)| command::<Mode>(env, cmd).map_err(|e| e.with_command_index(idx)))
        .collect::<Result<Vec<_>, _>>()?;
    let loaded_tx = L::Transaction {
        gas_payment,
        inputs,
        original_command_len,
        commands,
        // Set by `linkage::refine_linkage` below if unified linkage is enabled.
        unified_linkage: None,
    };
    metering::loading::meter::<Mode::Error>(meter, &loaded_tx)?;
    linkage::refine_linkage::<Mode>(loaded_tx, env.linkage_analysis, env.linkable_store)
}

fn input<Mode: ExecutionMode>(
    env: &Env<Mode>,
    arg: Input,
) -> Result<(L::InputArg, L::InputType), Mode::Error> {
    Ok(match arg {
        Input::Pure(bytes) => (L::InputArg::Pure(bytes), L::InputType::Bytes),
        Input::Receiving(oref) => (L::InputArg::Receiving(oref), L::InputType::Bytes),
        Input::ImmutableOrOwned(oref) => {
            let id = *oref.object_id();
            let obj = env.read_object(&id)?;
            let Some(ty) = obj.data.opt_object_type() else {
                invariant_violation!("Object {:?} has does not have a Move type", id);
            };
            let tag = core_struct_tag(ty);
            let ty = env.load_type_from_struct(&tag)?;
            let arg = match &obj.owner {
                Owner::Address(_) => L::ObjectArg {
                    kind: L::ObjectArgKind::OwnedObject(oref),
                    refined_permissions: ObjectPermissions::ALL,
                },
                Owner::Immutable => L::ObjectArg {
                    kind: L::ObjectArgKind::ImmObject(oref),
                    refined_permissions: ObjectPermissions::IMMUTABLE_USAGE,
                },
                Owner::Object(_) | Owner::Shared(_) => {
                    assert_invariant!(
                        Mode::allow_arbitrary_values(),
                        "Unexpected owner for ImmOrOwnedObject: {:?}",
                        obj.owner,
                    );
                    L::ObjectArg {
                        kind: L::ObjectArgKind::OwnedObject(oref),
                        refined_permissions: ObjectPermissions::ALL,
                    }
                }
                _ => unimplemented!("a new Owner enum variant was added and needs to be handled"),
            };
            (L::InputArg::Object(arg), L::InputType::Fixed(ty))
        }
        Input::Shared(SharedObjectReference {
            object_id: id,
            initial_shared_version,
            mutable,
        }) => {
            let obj = env.read_object(&id)?;
            let Some(ty) = obj.data.opt_object_type() else {
                invariant_violation!("Object {:?} does not have a Move type", id);
            };
            let tag = core_struct_tag(ty);
            let ty = env.load_type_from_struct(&tag)?;
            let owner_permissions = match &obj.owner {
                Owner::Address(_) | Owner::Object(_) | Owner::Immutable => {
                    assert_invariant!(
                        Mode::allow_arbitrary_values(),
                        "Unexpected owner for SharedObject: {:?}",
                        obj.owner
                    );
                    ObjectPermissions::ALL
                }
                Owner::Shared(_) => ObjectPermissions::LEGACY_SHARED_OBJECT,
                _ => unimplemented!("a new Owner enum variant was added and needs to be handled"),
            };
            let refined_permissions = refine_permissions::<Mode>(mutable, owner_permissions)?;
            let kind = L::ObjectArgKind::ConsensusObject {
                id,
                initial_shared_version,
            };
            (
                L::InputArg::Object(L::ObjectArg {
                    kind,
                    refined_permissions,
                }),
                L::InputType::Fixed(ty),
            )
        }
        _ => unimplemented!("a new Input enum variant was added and needs to be handled"),
    })
}

fn refine_permissions<Mode: ExecutionMode>(
    mutable: bool,
    permissions: ObjectPermissions,
) -> Result<ObjectPermissions, Mode::Error> {
    Ok(if mutable {
        assert_invariant!(
            permissions.can_use_mutably(),
            "Mutable shared object usage requires mutable usage permission"
        );
        permissions
    } else {
        assert_invariant!(
            permissions.can_use_immutably(),
            "Immutable shared object usage requires immutable usage permission"
        );
        ObjectPermissions::IMMUTABLE_USAGE
    })
}

fn command<Mode: ExecutionMode>(
    env: &Env<Mode>,
    command: P::Command,
) -> Result<L::Command, Mode::Error> {
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
            modules: items,
            dependencies: dep_ids,
        }) => {
            let resolved_linkage = env
                .linkage_analysis
                .compute_publication_linkage::<Mode::Error>(&dep_ids, env.linkable_store)?;
            let payload = PackagePayload::Deserialized(env.deserialize_package(&items, &dep_ids)?);
            L::Command::Publish(payload, dep_ids, resolved_linkage)
        }
        Command::Upgrade(Upgrade {
            modules: items,
            dependencies: dep_ids,
            package: object_id,
            ticket: argument,
        }) => {
            let resolved_linkage = env
                .linkage_analysis
                .compute_publication_linkage::<Mode::Error>(&dep_ids, env.linkable_store)?;
            let payload = PackagePayload::Deserialized(env.deserialize_package(&items, &dep_ids)?);
            L::Command::Upgrade(payload, dep_ids, object_id, argument, resolved_linkage)
        }
        _ => unimplemented!("a new Command enum variant was added and needs to be handled"),
    })
}
