// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::time::Duration;

use iota_types::{
    base_types::{ObjectRef, StructTag},
    iota_sdk_types_conversions::struct_tag_core_to_sdk,
    transaction::{CallArg, SharedObjectRef},
};
use move_binary_format::normalized;
use rand::{Rng, seq::SliceRandom};
use tokio::time::Instant;
use tracing::debug;

use crate::surfer_state::{EntryFunction, SurferState};

enum InputObjectPassKind {
    Value,
    ByRef,
    MutRef,
}

type Type = normalized::Type<normalized::ArcIdentifier>;

#[derive(Clone, Default)]
pub struct SurfStrategy {
    min_tx_interval: Duration,
}

impl SurfStrategy {
    pub fn new(min_tx_interval: Duration) -> Self {
        Self { min_tx_interval }
    }

    /// Given a state and a list of callable Move entry functions,
    /// explore them for a while, and eventually return. This function may
    /// not return in some situations, so its important to call it with a
    /// timeout or select! to ensure the task doesn't block forever.
    pub async fn surf_for_a_while(
        &mut self,
        state: &mut SurferState,
        mut entry_functions: Vec<EntryFunction>,
    ) {
        entry_functions.shuffle(&mut state.rng);
        for entry in entry_functions {
            let next_tx_time = Instant::now() + self.min_tx_interval;
            let Some(args) = Self::choose_function_call_args(state, entry.parameters).await else {
                debug!(
                    "Failed to choose arguments for Move function {:?}::{:?}",
                    entry.module, entry.function
                );
                continue;
            };
            state
                .execute_move_transaction(entry.package, entry.module, entry.function, args)
                .await;
            tokio::time::sleep_until(next_tx_time).await;
        }
    }

    async fn choose_function_call_args(
        state: &mut SurferState,
        params: Vec<Type>,
    ) -> Option<Vec<CallArg>> {
        let mut args = vec![];
        let mut chosen_owned_objects = vec![];
        let mut failed = false;
        for param in params {
            let arg = match param {
                Type::Bool => CallArg::pure(&state.rng.gen::<bool>()),
                Type::U8 => CallArg::pure(&state.rng.gen::<u8>()),
                Type::U16 => CallArg::pure(&state.rng.gen::<u16>()),
                Type::U32 => CallArg::pure(&state.rng.gen::<u32>()),
                Type::U64 => CallArg::pure(&state.rng.gen::<u64>()),
                Type::U128 => CallArg::pure(&state.rng.gen::<u128>()),
                Type::Address => {
                    CallArg::pure(&state.cluster.get_addresses().choose(&mut state.rng))
                }
                ty @ Type::Datatype(_) => {
                    match Self::choose_object_call_arg(
                        state,
                        InputObjectPassKind::Value,
                        ty,
                        &mut chosen_owned_objects,
                    )
                    .await
                    {
                        Some(arg) => arg,
                        None => {
                            failed = true;
                            break;
                        }
                    }
                }
                Type::Reference(mut_, ty) => {
                    let kind = if mut_ {
                        InputObjectPassKind::MutRef
                    } else {
                        InputObjectPassKind::ByRef
                    };
                    match Self::choose_object_call_arg(state, kind, *ty, &mut chosen_owned_objects)
                        .await
                    {
                        Some(arg) => arg,
                        None => {
                            failed = true;
                            break;
                        }
                    }
                }
                Type::U256 | Type::Signer | Type::Vector(_) | Type::TypeParameter(_) => {
                    failed = true;
                    break;
                }
            };
            args.push(arg);
        }
        if failed {
            for (struct_tag, obj_ref) in chosen_owned_objects {
                state
                    .owned_objects
                    .get_mut(&struct_tag)
                    .unwrap()
                    .insert(obj_ref);
            }
            None
        } else {
            Some(args)
        }
    }

    async fn choose_object_call_arg(
        state: &mut SurferState,
        kind: InputObjectPassKind,
        arg_type: Type,
        chosen_owned_objects: &mut Vec<(StructTag, ObjectRef)>,
    ) -> Option<CallArg> {
        let pool = state.pool.read().await;
        let type_tag = match arg_type {
            Type::Datatype(dt) => dt.to_struct_tag(&*pool),
            _ => {
                return None;
            }
        };
        drop(pool);
        let type_tag = struct_tag_core_to_sdk(&type_tag);
        let owned = state.matching_owned_objects_count(&type_tag);
        let shared = state.matching_shared_objects_count(&type_tag).await;
        let immutable = state.matching_immutable_objects_count(&type_tag).await;

        let total_matching_count = match kind {
            InputObjectPassKind::Value => owned,
            InputObjectPassKind::MutRef => owned + shared,
            InputObjectPassKind::ByRef => owned + shared + immutable,
        };
        if total_matching_count == 0 {
            return None;
        }
        let mut n = state.rng.gen_range(0..total_matching_count);
        if n < owned {
            let obj_ref = state.choose_nth_owned_object(&type_tag, n);
            chosen_owned_objects.push((type_tag, obj_ref));
            return Some(CallArg::ImmutableOrOwned(obj_ref));
        }
        n -= owned;
        if n < shared {
            let (id, initial_shared_version) = state.choose_nth_shared_object(&type_tag, n).await;
            return Some(CallArg::Shared(SharedObjectRef {
                object_id: id,
                initial_shared_version,
                mutable: matches!(kind, InputObjectPassKind::MutRef),
            }));
        }
        n -= shared;
        let obj_ref = state.choose_nth_immutable_object(&type_tag, n).await;
        Some(CallArg::ImmutableOrOwned(obj_ref))
    }
}
