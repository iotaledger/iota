// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::rc::Rc;

use indexmap::{IndexMap, IndexSet};
use iota_sdk_types::ObjectReference;
use iota_types::{
    base_types::TxContextKind,
    coin::RESOLVED_COIN_STRUCT,
    error::{ExecutionError, ExecutionErrorKind, command_argument_error},
};
use move_binary_format::file_format::{Ability, AbilitySet};

use super::{ast as T, env::Env};
use crate::{
    compat::{ExecutionErrorTrait, SafeIndex, execution_status::CommandArgumentError},
    execution_mode::ExecutionMode,
    gas_charger::GasPayment,
    static_programmable_transactions::{
        execution::context::EitherError,
        linkage::resolved_linkage::ExecutableLinkage,
        loading::{ast as L, ast::Type},
        spanned::sp,
        typing::ast::BytesConstraint,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum SplatLocation {
    GasCoin,
    Input(T::InputIndex),
    Result(u16, u16),
}

#[derive(Debug, Clone, Copy)]
enum InputKind {
    Object,
    Pure,
    Receiving,
}

struct Context {
    current_command: u16,
    gas_payment: Option<GasPayment>,
    /// What kind of input is at each original index
    input_resolution: Vec<InputKind>,
    bytes: IndexSet<Vec<u8>>,
    // Mapping from original index to `bytes`
    bytes_idx_remapping: IndexMap<T::InputIndex, T::ByteIndex>,
    receiving_refs: IndexMap<T::InputIndex, ObjectReference>,
    objects: IndexMap<T::InputIndex, T::ObjectInput>,
    pure: IndexMap<(T::InputIndex, Type), T::PureInput>,
    receiving: IndexMap<(T::InputIndex, Type), T::ReceivingInput>,
    original_command_len: usize,
    commands: Vec<T::Command>,
    unified_linkage: Option<ExecutableLinkage>,
}

impl Context {
    fn new(
        gas_payment: Option<GasPayment>,
        original_command_len: usize,
        linputs: L::Inputs,
        unified_linkage: Option<ExecutableLinkage>,
    ) -> Result<Self, ExecutionError> {
        let mut context = Context {
            current_command: 0,
            gas_payment,
            input_resolution: vec![],
            original_command_len,
            bytes: IndexSet::new(),
            bytes_idx_remapping: IndexMap::new(),
            receiving_refs: IndexMap::new(),
            objects: IndexMap::new(),
            pure: IndexMap::new(),
            receiving: IndexMap::new(),
            commands: vec![],
            unified_linkage,
        };
        // clone inputs for debug assertions
        #[cfg(debug_assertions)]
        let cloned_inputs = linputs
            .iter()
            .map(|(arg, _)| arg.clone())
            .collect::<Vec<_>>();
        // - intern the bytes
        // - build maps for object, pure, and receiving inputs
        for (i, (arg, ty)) in linputs.into_iter().enumerate() {
            let idx = T::InputIndex(checked_as!(i, u16)?);
            let kind = match (arg, ty) {
                (L::InputArg::Pure(bytes), L::InputType::Bytes) => {
                    let (byte_index, _) = context.bytes.insert_full(bytes);
                    context.bytes_idx_remapping.insert(idx, byte_index);
                    InputKind::Pure
                }
                (L::InputArg::Receiving(oref), L::InputType::Bytes) => {
                    context.receiving_refs.insert(idx, oref);
                    InputKind::Receiving
                }
                (L::InputArg::Object(arg), L::InputType::Fixed(ty)) => {
                    let o = T::ObjectInput {
                        original_input_index: idx,
                        arg,
                        ty,
                    };
                    context.objects.insert(idx, o);
                    InputKind::Object
                }
                (arg, ty) => invariant_violation!(
                    "Input arg, type mismatch. Unexpected {arg:?} with type {ty:?}"
                ),
            };
            context.input_resolution.push(kind);
        }
        #[cfg(debug_assertions)]
        {
            // iterate to check the correctness of bytes interning
            for (i, arg) in cloned_inputs.iter().enumerate() {
                if let L::InputArg::Pure(bytes) = &arg {
                    let idx = T::InputIndex(checked_as!(i, u16)?);
                    let Some(byte_index) = context.bytes_idx_remapping.get(&idx) else {
                        invariant_violation!("Unbound pure input {}", idx.0);
                    };
                    let Some(interned_bytes) = context.bytes.get_index(*byte_index) else {
                        invariant_violation!("Interned bytes not found for index {}", byte_index);
                    };
                    if interned_bytes != bytes {
                        assert_invariant!(
                            interned_bytes == bytes,
                            "Interned bytes mismatch for input {i}",
                        );
                    }
                }
            }
        }
        Ok(context)
    }

    fn finish(self) -> T::Transaction {
        let Self {
            gas_payment,
            bytes,
            objects,
            pure,
            receiving,
            original_command_len,
            commands,
            unified_linkage,
            ..
        } = self;
        let objects = objects.into_iter().map(|(_, o)| o).collect();
        let pure = pure.into_iter().map(|(_, p)| p).collect();
        let receiving = receiving.into_iter().map(|(_, r)| r).collect();
        T::Transaction {
            gas_payment,
            bytes,
            objects,
            pure,
            receiving,
            original_command_len,
            commands,
            unified_linkage,
        }
    }

    fn push_result(&mut self, command: T::Command_) -> Result<(), ExecutionError> {
        self.commands.push(sp(self.current_command, command));
        Ok(())
    }

    fn result_type(&self, i: u16) -> Option<&T::ResultType> {
        self.commands.get(i as usize).map(|c| &c.value.result_type)
    }

    fn fixed_location_type<Mode: ExecutionMode>(
        &mut self,
        env: &Env<Mode>,
        location: T::Location,
    ) -> Result<Option<Type>, Mode::Error> {
        Ok(Some(match location {
            T::Location::TxContext => env.tx_context_type()?,
            T::Location::AuthContext => env.auth_context_type()?,
            T::Location::GasCoin => env.gas_coin_type()?,
            T::Location::Result(i, j) => {
                let Some(tys) = self.result_type(i) else {
                    invariant_violation!("Result index {i} is out of bounds")
                };
                tys.safe_get(j as usize)?.clone()
            }
            T::Location::ObjectInput(i) => {
                let Some((_, object_input)) = self.objects.get_index(i as usize) else {
                    invariant_violation!("Unbound object input {}", i)
                };
                object_input.ty.clone()
            }
            T::Location::PureInput(_) | T::Location::ReceivingInput(_) => return Ok(None),
        }))
    }

    // Get the fixed type of a location. Returns `None` for Pure and Receiving inputs,
    fn fixed_type<Mode: ExecutionMode>(
        &mut self,
        env: &Env<Mode>,
        splat_location: SplatLocation,
    ) -> Result<Option<(T::Location, Type)>, Mode::Error> {
        let location = match splat_location {
            SplatLocation::GasCoin => T::Location::GasCoin,
            SplatLocation::Result(i, j) => T::Location::Result(i, j),
            SplatLocation::Input(i) => match self.input_resolution.safe_get(i.0 as usize)? {
                InputKind::Object => {
                    let Some(index) = self.objects.get_index_of(&i) else {
                        invariant_violation!("Unbound object input {}", i.0)
                    };
                    T::Location::ObjectInput(checked_as!(index, u16)?)
                }
                InputKind::Pure | InputKind::Receiving => return Ok(None),
            },
        };
        let Some(ty) = self.fixed_location_type(env, location)? else {
            invariant_violation!("Location {location:?} does not have a fixed type")
        };
        Ok(Some((location, ty)))
    }

    fn resolve_location<Mode: ExecutionMode>(
        &mut self,
        env: &Env<Mode>,
        splat_location: SplatLocation,
        expected_ty: &Type,
        bytes_constraint: BytesConstraint,
    ) -> Result<(T::Location, Type), Mode::Error> {
        let location = match splat_location {
            SplatLocation::GasCoin => T::Location::GasCoin,
            SplatLocation::Result(i, j) => T::Location::Result(i, j),
            SplatLocation::Input(i) => match self.input_resolution.safe_get(i.0 as usize)? {
                InputKind::Object => {
                    let Some(index) = self.objects.get_index_of(&i) else {
                        invariant_violation!("Unbound object input {}", i.0)
                    };
                    T::Location::ObjectInput(checked_as!(index, u16)?)
                }
                InputKind::Pure => {
                    let ty = match expected_ty {
                        Type::Reference(_, inner) => (**inner).clone(),
                        ty => ty.clone(),
                    };
                    let k = (i, ty.clone());
                    if !self.pure.contains_key(&k) {
                        let Some(byte_index) = self.bytes_idx_remapping.get(&i).copied() else {
                            invariant_violation!("Unbound pure input {}", i.0);
                        };
                        let pure = T::PureInput {
                            original_input_index: i,
                            byte_index,
                            ty: ty.clone(),
                            constraint: bytes_constraint,
                        };
                        self.pure.insert(k.clone(), pure);
                    }
                    let byte_index = self.pure.get_index_of(&k).unwrap();
                    return Ok((T::Location::PureInput(checked_as!(byte_index, u16)?), ty));
                }
                InputKind::Receiving => {
                    let ty = match expected_ty {
                        Type::Reference(_, inner) => (**inner).clone(),
                        ty => ty.clone(),
                    };
                    let k = (i, ty.clone());
                    if !self.receiving.contains_key(&k) {
                        let Some(object_ref) = self.receiving_refs.get(&i).copied() else {
                            invariant_violation!("Unbound receiving input {}", i.0);
                        };
                        let receiving = T::ReceivingInput {
                            original_input_index: i,
                            object_ref,
                            ty: ty.clone(),
                            constraint: bytes_constraint,
                        };
                        self.receiving.insert(k.clone(), receiving);
                    }
                    let byte_index = self.receiving.get_index_of(&k).unwrap();
                    return Ok((
                        T::Location::ReceivingInput(checked_as!(byte_index, u16)?),
                        ty,
                    ));
                }
            },
        };
        let Some(ty) = self.fixed_location_type(env, location)? else {
            invariant_violation!("Location {location:?} does not have a fixed type")
        };
        Ok((location, ty))
    }
}

pub fn transaction<Mode: ExecutionMode>(
    env: &Env<Mode>,
    lt: L::Transaction,
) -> Result<T::Transaction, Mode::Error> {
    let L::Transaction {
        gas_payment,
        inputs,
        original_command_len,
        commands,
        unified_linkage,
    } = lt;
    let mut context = Context::new(gas_payment, original_command_len, inputs, unified_linkage)?;
    for (i, c) in commands.into_iter().enumerate() {
        let idx = checked_as!(i, u16)?;
        context.current_command = idx;
        let (c_, tys) =
            command::<Mode>(env, &mut context, c).map_err(|e| e.with_command_index(i))?;
        let c = T::Command_ {
            command: c_,
            result_type: tys,
            // computed later
            drop_values: vec![],
            // computed later
            incurs_post_execution_checks: false,
        };
        context.push_result(c)?
    }
    let mut ast = context.finish();
    // mark the last usage of references as Move instead of Copy
    scope_references::transaction(&mut ast);
    // mark unused results to be dropped
    unused_results::transaction(&mut ast)?;
    // track shared object IDs
    post_execution_checks::transaction(&mut ast)?;
    Ok(ast)
}

fn command<Mode: ExecutionMode>(
    env: &Env<Mode>,
    context: &mut Context,
    command: L::Command,
) -> Result<(T::Command__, T::ResultType), Mode::Error> {
    Ok(match command {
        L::Command::MoveCall(lmc) => {
            let L::MoveCall {
                function,
                arguments: largs,
            } = *lmc;
            let arg_locs = locations(context, 0, largs)?;
            let args = move_call_arguments(env, context, &function, arg_locs)?;
            let result = function.signature.return_.clone();
            (
                T::Command__::MoveCall(Box::new(T::MoveCall {
                    function,
                    arguments: args,
                })),
                result,
            )
        }
        L::Command::TransferObjects(lobjects, laddress) => {
            const TRANSFER_OBJECTS_CONSTRAINT: AbilitySet =
                AbilitySet::singleton(Ability::Store).union(AbilitySet::singleton(Ability::Key));
            let object_locs = locations(context, 0, lobjects)?;
            let address_loc = one_location(context, object_locs.len(), laddress)?;
            let objects = constrained_arguments(
                env,
                context,
                0,
                object_locs,
                TRANSFER_OBJECTS_CONSTRAINT,
                CommandArgumentError::InvalidTransferObject,
            )?;
            let address = argument(env, context, objects.len(), address_loc, Type::Address)?;
            (T::Command__::TransferObjects(objects, address), vec![])
        }
        L::Command::SplitCoins(lcoin, lamounts) => {
            let coin_loc = one_location(context, 0, lcoin)?;
            let amount_locs = locations(context, 1, lamounts)?;
            let coin = coin_mut_ref_argument(env, context, 0, coin_loc)?;
            let coin_type = match &coin.value.1 {
                Type::Reference(true, ty) => (**ty).clone(),
                ty => invariant_violation!("coin must be a mutable reference. Found: {ty:?}"),
            };
            let amounts = arguments(
                env,
                context,
                1,
                amount_locs,
                std::iter::repeat_with(|| Type::U64),
            )?;
            let result = vec![coin_type.clone(); amounts.len()];
            (T::Command__::SplitCoins(coin_type, coin, amounts), result)
        }
        L::Command::MergeCoins(ltarget, lcoins) => {
            let target_loc = one_location(context, 0, ltarget)?;
            let coin_locs = locations(context, 1, lcoins)?;
            let target = coin_mut_ref_argument(env, context, 0, target_loc)?;
            let coin_type = match &target.value.1 {
                Type::Reference(true, ty) => (**ty).clone(),
                ty => invariant_violation!("target must be a mutable reference. Found: {ty:?}"),
            };
            let coins = arguments(
                env,
                context,
                1,
                coin_locs,
                std::iter::repeat_with(|| coin_type.clone()),
            )?;
            (T::Command__::MergeCoins(coin_type, target, coins), vec![])
        }
        L::Command::MakeMoveVec(Some(ty), lelems) => {
            let elem_locs = locations(context, 0, lelems)?;
            let elems = arguments(
                env,
                context,
                0,
                elem_locs,
                std::iter::repeat_with(|| ty.clone()),
            )?;
            (
                T::Command__::MakeMoveVec(ty.clone(), elems),
                vec![env.vector_type(ty)?],
            )
        }
        L::Command::MakeMoveVec(None, lelems) => {
            const MAKE_MOVE_VEC_OBJECT_CONSTRAINT: AbilitySet = AbilitySet::singleton(Ability::Key);
            let mut lelems = lelems.into_iter();
            let Some(lfirst) = lelems.next() else {
                // TODO maybe this should be a different errors for CLI usage
                invariant_violation!(
                    "input checker ensures if args are empty, there is a type specified"
                );
            };
            let first_loc = one_location(context, 0, lfirst)?;
            let first_arg = constrained_argument(
                env,
                context,
                0,
                first_loc,
                MAKE_MOVE_VEC_OBJECT_CONSTRAINT,
                CommandArgumentError::InvalidMakeMoveVecNonObjectArgument,
            )?;
            let first_ty = first_arg.value.1.clone();
            let elems_loc = locations(context, 1, lelems)?;
            let mut elems = arguments(
                env,
                context,
                1,
                elems_loc,
                std::iter::repeat_with(|| first_ty.clone()),
            )?;
            elems.insert(0, first_arg);
            (
                T::Command__::MakeMoveVec(first_ty.clone(), elems),
                vec![env.vector_type(first_ty)?],
            )
        }
        L::Command::Publish(items, object_ids, linkage) => {
            let result = if Mode::packages_are_predefined() {
                // If packages are predefined, no upgrade cap is made
                vec![]
            } else {
                vec![env.upgrade_cap_type()?]
            };
            (T::Command__::Publish(items, object_ids, linkage), result)
        }
        L::Command::Upgrade(items, object_ids, object_id, la, linkage) => {
            let location = one_location(context, 0, la)?;
            let expected_ty = env.upgrade_ticket_type()?;
            let a = argument(env, context, 0, location, expected_ty)?;
            let res = env.upgrade_receipt_type()?;
            (
                T::Command__::Upgrade(items, object_ids, object_id, a, linkage),
                vec![res],
            )
        }
    })
}

fn move_call_parameters<'a, Mode: ExecutionMode>(
    _env: &Env<Mode>,
    function: &'a L::LoadedFunction,
) -> Vec<(&'a Type, TxContextKind)> {
    function
        .signature
        .parameters
        .iter()
        .map(|ty| (ty, ty.is_tx_context()))
        .collect()
}

fn move_call_arguments<Mode: ExecutionMode>(
    env: &Env<Mode>,
    context: &mut Context,
    function: &L::LoadedFunction,
    args: Vec<SplatLocation>,
) -> Result<Vec<T::Argument>, Mode::Error> {
    let params = move_call_parameters(env, function);
    assert_invariant!(
        params.len() == function.signature.parameters.len(),
        "Generated parameter types does not match the function signature"
    );
    // An authenticate function takes `&AuthContext` as its second last parameter, before the
    // `TxContext`. The executor supplies it, in authentication mode only.
    let auth_context_idx = params.len().checked_sub(2).filter(|idx| {
        params
            .safe_get(*idx)
            .is_ok_and(|(ty, _)| ty.is_auth_context_ref())
    });
    if auth_context_idx.is_some() {
        if !env.protocol_config.enable_move_authentication() {
            return Err(Mode::Error::new_with_source(
                ExecutionErrorKind::VmInvariantViolation,
                "`iota::auth_context::AuthContext` can't be used as a parameter if the \
                 `move_authentication` feature is disabled",
            ));
        }
        if !Mode::allow_auth_context() {
            return Err(Mode::Error::new_with_source(
                ExecutionErrorKind::VmInvariantViolation,
                "`iota::auth_context::AuthContext` can't be used as a parameter in this execution \
                 mode",
            ));
        }
    }
    // check arity
    let num_tx_contexts = params
        .iter()
        .filter(|(_, k)| matches!(k, TxContextKind::Mutable | TxContextKind::Immutable))
        .count();
    let num_user_args = args.len();
    let Some(num_args) = num_user_args
        .checked_add(num_tx_contexts)
        .and_then(|n| n.checked_add(usize::from(auth_context_idx.is_some())))
    else {
        invariant_violation!("usize overflow when calculating number of arguments");
    };
    let num_parameters = params.len();
    if num_args != num_parameters {
        return Err(Mode::Error::new_with_source(
            ExecutionErrorKind::ArityMismatch,
            format!(
                "Expected {} argument{} calling function '{}::{}', but found {}",
                num_parameters,
                if num_parameters == 1 { "" } else { "s" },
                function.version_mid,
                function.name,
                num_args,
            ),
        ));
    }
    // construct arguments, injecting tx context args as needed
    let mut args = args.into_iter().enumerate();
    let res = params
        .into_iter()
        .enumerate()
        .map(|(param_idx, (expected_ty, tx_context_kind))| {
            if auth_context_idx == Some(param_idx) {
                let idx = checked_as!(param_idx, u16)?;
                let arg__ = T::Argument__::Borrow(false, T::Location::AuthContext);
                let ty = Type::Reference(false, Rc::new(env.auth_context_type()?));
                return Ok(sp(idx, (arg__, ty)));
            }
            Ok(match tx_context_kind {
                TxContextKind::None => {
                    let Some((arg_idx, location)) = args.next() else {
                        invariant_violation!("arguments are empty but arity was already checked");
                    };
                    argument(env, context, arg_idx, location, expected_ty.clone())?
                }
                TxContextKind::Mutable | TxContextKind::Immutable => {
                    let is_mut = match tx_context_kind {
                        TxContextKind::Mutable => true,
                        TxContextKind::Immutable => false,
                        TxContextKind::None => unreachable!(),
                    };
                    // TODO this might overlap or be  out of bounds of the original PTB arguments...
                    // what do we do here?
                    let idx = checked_as!(param_idx, u16)?;
                    let arg__ = T::Argument__::Borrow(is_mut, T::Location::TxContext);
                    let ty = Type::Reference(is_mut, Rc::new(env.tx_context_type()?));
                    sp(idx, (arg__, ty))
                }
            })
        })
        .collect::<Result<Vec<_>, Mode::Error>>()?;

    assert_invariant!(
        args.next().is_none(),
        "some arguments went unused but arity was already checked"
    );
    Ok(res)
}

fn one_location<E: ExecutionErrorTrait>(
    context: &mut Context,
    command_arg_idx: usize,
    arg: L::Argument,
) -> Result<SplatLocation, E> {
    let locs = locations(context, command_arg_idx, vec![arg])?;
    let Ok([loc]): Result<[SplatLocation; 1], _> = locs.try_into() else {
        return Err(command_argument_error(
            CommandArgumentError::InvalidArgumentArity,
            command_arg_idx,
        )
        .into());
    };
    Ok(loc)
}

fn locations<E: ExecutionErrorTrait, Items: IntoIterator<Item = L::Argument>>(
    context: &mut Context,
    start_idx: usize,
    args: Items,
) -> Result<Vec<SplatLocation>, E>
where
    Items::IntoIter: ExactSizeIterator,
{
    fn splat_arg<E: ExecutionErrorTrait>(
        context: &mut Context,
        res: &mut Vec<SplatLocation>,
        arg: L::Argument,
    ) -> Result<(), EitherError<E>> {
        match arg {
            L::Argument::Gas => res.push(SplatLocation::GasCoin),
            L::Argument::Input(i) => {
                if i as usize >= context.input_resolution.len() {
                    return Err(CommandArgumentError::IndexOutOfBounds { index: i }.into());
                }
                res.push(SplatLocation::Input(T::InputIndex(i)))
            }
            L::Argument::NestedResult(i, j) => {
                let Some(command_result) = context.result_type(i) else {
                    return Err(CommandArgumentError::IndexOutOfBounds { index: i }.into());
                };
                if j as usize >= command_result.len() {
                    return Err(CommandArgumentError::SecondaryIndexOutOfBounds {
                        result: i,
                        subresult: j,
                    }
                    .into());
                };
                res.push(SplatLocation::Result(i, j))
            }
            L::Argument::Result(i) => {
                let Some(result) = context.result_type(i) else {
                    return Err(CommandArgumentError::IndexOutOfBounds { index: i }.into());
                };
                let Ok(len): Result<u16, _> = result.len().try_into() else {
                    invariant_violation!("Result of length greater than u16::MAX");
                };
                if len != 1 {
                    // TODO protocol config to allow splatting of args
                    return Err(CommandArgumentError::InvalidResultArity { result: i }.into());
                }
                res.extend((0..len).map(|j| SplatLocation::Result(i, j)))
            }
            _ => unimplemented!("a new Argument enum variant was added and needs to be handled"),
        }
        Ok(())
    }

    let args = args.into_iter();
    let _args_len = args.len();
    let mut res = vec![];
    for (arg_idx, arg) in args.enumerate() {
        splat_arg::<E>(context, &mut res, arg).map_err(|e| {
            let Some(idx) = start_idx.checked_add(arg_idx) else {
                return make_invariant_violation!("usize overflow when calculating argument index")
                    .into();
            };
            e.into_execution_error(idx)
        })?
    }
    debug_assert_eq!(res.len(), _args_len);
    Ok(res)
}

fn arguments<Mode: ExecutionMode>(
    env: &Env<Mode>,
    context: &mut Context,
    start_idx: usize,
    locations: Vec<SplatLocation>,
    expected_tys: impl IntoIterator<Item = Type>,
) -> Result<Vec<T::Argument>, Mode::Error> {
    #[allow(clippy::disallowed_methods)]
    locations
        .into_iter()
        // Intentional zip: expected_tys may be an infinite repeat iterator
        // TODO: Consider fixing callers to not use infinite repeat iterator.
        .zip(expected_tys)
        .enumerate()
        .map(|(i, (location, expected_ty))| {
            let Some(idx) = start_idx.checked_add(i) else {
                invariant_violation!("usize overflow when calculating argument index");
            };
            argument(env, context, idx, location, expected_ty)
        })
        .collect()
}

fn argument<Mode: ExecutionMode>(
    env: &Env<Mode>,
    context: &mut Context,
    command_arg_idx: usize,
    location: SplatLocation,
    expected_ty: Type,
) -> Result<T::Argument, Mode::Error> {
    let arg__ = argument_(env, context, command_arg_idx, location, &expected_ty)
        .map_err(|e| e.into_execution_error(command_arg_idx))?;
    let arg_ = (arg__, expected_ty);
    Ok(sp(checked_as!(command_arg_idx, u16)?, arg_))
}

fn argument_<Mode: ExecutionMode>(
    env: &Env<Mode>,
    context: &mut Context,
    command_arg_idx: usize,
    location: SplatLocation,
    expected_ty: &Type,
) -> Result<T::Argument__, EitherError<Mode::Error>> {
    let current_command = context.current_command;
    let bytes_constraint = BytesConstraint {
        command: current_command,
        argument: checked_as!(command_arg_idx, u16)?,
    };
    let (location, actual_ty) = context
        .resolve_location(env, location, expected_ty, bytes_constraint)
        .map_err(EitherError::Execution)?;
    Ok(match (actual_ty, expected_ty) {
        // Reference location types
        (Type::Reference(a_is_mut, a), Type::Reference(b_is_mut, b)) => {
            let needs_freeze = match (a_is_mut, b_is_mut) {
                // same mutability
                (true, true) | (false, false) => false,
                // mut *can* be used as imm
                (true, false) => true,
                // imm cannot be used as mut
                (false, true) => return Err(CommandArgumentError::TypeMismatch.into()),
            };
            debug_assert!(expected_ty.abilities().has_copy());
            // unused since the type is fixed
            check_type(&a, b)?;
            if needs_freeze {
                T::Argument__::Freeze(T::Usage::new_copy(location))
            } else {
                T::Argument__::new_copy(location)
            }
        }
        (Type::Reference(_, a), b) => {
            check_type(&a, b)?;
            if !b.abilities().has_copy() {
                // TODO this should be a different error for missing copy
                return Err(CommandArgumentError::TypeMismatch.into());
            }
            T::Argument__::Read(T::Usage::new_copy(location))
        }

        // Non reference location types
        (actual_ty, Type::Reference(is_mut, inner)) => {
            check_type(&actual_ty, inner)?;
            T::Argument__::Borrow(/* mut */ *is_mut, location)
        }
        (actual_ty, _) => {
            check_type(&actual_ty, expected_ty)?;
            T::Argument__::Use(if expected_ty.abilities().has_copy() {
                T::Usage::new_copy(location)
            } else {
                T::Usage::new_move(location)
            })
        }
    })
}

fn check_type(actual_ty: &Type, expected_ty: &Type) -> Result<(), CommandArgumentError> {
    if actual_ty == expected_ty {
        Ok(())
    } else {
        Err(CommandArgumentError::TypeMismatch)
    }
}

fn constrained_arguments<Mode: ExecutionMode>(
    env: &Env<Mode>,
    context: &mut Context,
    start_idx: usize,
    locations: Vec<SplatLocation>,
    constraint: AbilitySet,
    err_case: CommandArgumentError,
) -> Result<Vec<T::Argument>, Mode::Error> {
    locations
        .into_iter()
        .enumerate()
        .map(|(i, location)| {
            let Some(idx) = start_idx.checked_add(i) else {
                invariant_violation!("usize overflow when calculating argument index");
            };
            constrained_argument(env, context, idx, location, constraint, err_case.clone())
        })
        .collect()
}

fn constrained_argument<Mode: ExecutionMode>(
    env: &Env<Mode>,
    context: &mut Context,
    command_arg_idx: usize,
    location: SplatLocation,
    constraint: AbilitySet,
    err_case: CommandArgumentError,
) -> Result<T::Argument, Mode::Error> {
    let arg_ = constrained_argument_(
        env,
        context,
        command_arg_idx,
        location,
        constraint,
        err_case,
    )
    .map_err(|e| e.into_execution_error(command_arg_idx))?;
    Ok(sp(checked_as!(command_arg_idx, u16)?, arg_))
}

fn constrained_argument_<Mode: ExecutionMode>(
    env: &Env<Mode>,
    context: &mut Context,
    command_arg_idx: usize,
    location: SplatLocation,
    constraint: AbilitySet,
    err_case: CommandArgumentError,
) -> Result<T::Argument_, EitherError<Mode::Error>> {
    if let Some((location, ty)) =
        constrained_type(env, context, command_arg_idx, location, constraint)
            .map_err(EitherError::Execution)?
    {
        if ty.abilities().has_copy() {
            Ok((T::Argument__::new_copy(location), ty))
        } else {
            Ok((T::Argument__::new_move(location), ty))
        }
    } else {
        Err(err_case.into())
    }
}

fn constrained_type<'a, Mode: ExecutionMode>(
    env: &'a Env<Mode>,
    context: &'a mut Context,
    _command_arg_idx: usize,
    location: SplatLocation,
    constraint: AbilitySet,
) -> Result<Option<(T::Location, Type)>, Mode::Error> {
    let Some((location, ty)) = context.fixed_type(env, location)? else {
        return Ok(None);
    };
    Ok(if constraint.is_subset(ty.abilities()) {
        Some((location, ty))
    } else {
        None
    })
}

fn coin_mut_ref_argument<Mode: ExecutionMode>(
    env: &Env<Mode>,
    context: &mut Context,
    command_arg_idx: usize,
    location: SplatLocation,
) -> Result<T::Argument, Mode::Error> {
    let arg_ = coin_mut_ref_argument_(env, context, command_arg_idx, location)
        .map_err(|e| e.into_execution_error(command_arg_idx))?;
    Ok(sp(checked_as!(command_arg_idx, u16)?, arg_))
}

fn coin_mut_ref_argument_<Mode: ExecutionMode>(
    env: &Env<Mode>,
    context: &mut Context,
    _command_arg_idx: usize,
    location: SplatLocation,
) -> Result<T::Argument_, EitherError<Mode::Error>> {
    let Some((location, actual_ty)) = context
        .fixed_type(env, location)
        .map_err(EitherError::Execution)?
    else {
        // TODO we do not currently bytes in any mode as that would require additional type
        // inference not currently supported
        return Err(CommandArgumentError::TypeMismatch.into());
    };
    Ok(match &actual_ty {
        Type::Reference(is_mut, ty) if *is_mut => {
            check_coin_type(ty)?;
            (
                T::Argument__::new_copy(location),
                Type::Reference(*is_mut, ty.clone()),
            )
        }
        ty => {
            check_coin_type(ty)?;
            (
                T::Argument__::Borrow(/* mut */ true, location),
                Type::Reference(true, Rc::new(ty.clone())),
            )
        }
    })
}

fn check_coin_type<E: ExecutionErrorTrait>(ty: &Type) -> Result<(), EitherError<E>> {
    if coin_inner_type(ty).is_some() {
        Ok(())
    } else {
        Err(CommandArgumentError::TypeMismatch.into())
    }
}

/// Returns the inner type `T` if the type is `iota::coin::Coin<T>`, else `None`
pub(crate) fn coin_inner_type(ty: &Type) -> Option<&Type> {
    if let Type::Datatype(dt) = ty
        && dt.type_arguments.len() == 1
        && dt.qualified_ident() == RESOLVED_COIN_STRUCT
    {
        Some(dt.type_arguments.first().unwrap())
    } else {
        None
    }
}

//**************************************************************************************************
// Reference scoping
//**************************************************************************************************

mod scope_references {
    use std::collections::BTreeSet;

    use crate::{sp, static_programmable_transactions::typing::ast as T};

    struct Context {
        used: BTreeSet<(u16, u16)>,
    }

    /// To mimic proper scoping of references, the last usage of a reference is made a Move instead
    /// of a Copy.
    pub fn transaction(ast: &mut T::Transaction) {
        let mut context = Context {
            used: BTreeSet::new(),
        };
        for c in ast.commands.iter_mut().rev() {
            command(&mut context, c);
        }
    }

    fn command(context: &mut Context, sp!(_, c): &mut T::Command) {
        match &mut c.command {
            T::Command__::MoveCall(mc) => arguments(context, &mut mc.arguments),
            T::Command__::TransferObjects(objects, recipient) => {
                argument(context, recipient);
                arguments(context, objects);
            }
            T::Command__::SplitCoins(_, coin, amounts) => {
                arguments(context, amounts);
                argument(context, coin);
            }
            T::Command__::MergeCoins(_, target, coins) => {
                arguments(context, coins);
                argument(context, target);
            }
            T::Command__::MakeMoveVec(_, xs) => arguments(context, xs),
            T::Command__::Publish(_, _, _) => (),
            T::Command__::Upgrade(_, _, _, x, _) => argument(context, x),
        }
    }

    fn arguments(context: &mut Context, args: &mut [T::Argument]) {
        for arg in args.iter_mut().rev() {
            argument(context, arg)
        }
    }

    fn argument(context: &mut Context, arg: &mut T::Argument) {
        argument_v2(context, arg)
    }

    fn argument_v2(context: &mut Context, sp!(_, (arg_, ty)): &mut T::Argument) {
        use T::Argument__ as TArg;
        let usage = match arg_ {
            // `Read` has the referenced value's type, while its usage location has a reference type
            TArg::Read(u) => u,
            TArg::Use(_) | TArg::Freeze(_) if !ty.is_reference() => return,
            TArg::Use(u) | TArg::Freeze(u) => u,
            // Cannot borrow a reference
            TArg::Borrow(_, _) => return,
        };
        match usage {
            T::Usage::Move(T::Location::Result(i, j)) => {
                debug_assert!(false, "No reference should be moved at this point");
                context.used.insert((*i, *j));
            }
            T::Usage::Copy {
                location: T::Location::Result(i, j),
                ..
            } => {
                // we are at the last usage of a reference result if it was not yet added to the set
                let last_usage = context.used.insert((*i, *j));
                if last_usage {
                    // if it was the last usage, we need to change the Copy to a Move
                    let loc = T::Location::Result(*i, *j);
                    *usage = T::Usage::Move(loc);
                }
            }
            _ => (),
        }
    }
}

//**************************************************************************************************
// Unused results
//**************************************************************************************************

mod unused_results {
    use indexmap::IndexSet;
    use iota_types::error::ExecutionError;

    use crate::{sp, static_programmable_transactions::typing::ast as T};

    /// Finds what `Result` indexes are never used in the transaction.
    /// For each command, marks the indexes of result values with `drop` that are never referred to
    /// via `Result`.
    pub fn transaction(ast: &mut T::Transaction) -> Result<(), ExecutionError> {
        // Collect all used result locations (i, j) across all commands
        let mut used: IndexSet<(u16, u16)> = IndexSet::new();
        for c in &ast.commands {
            command(&mut used, c);
        }

        // For each command, mark unused result indexes with `drop`
        for (i, sp!(_, c)) in ast.commands.iter_mut().enumerate() {
            debug_assert!(c.drop_values.is_empty());
            let i = checked_as!(i, u16)?;
            c.drop_values = c
                .result_type
                .iter()
                .enumerate()
                .map(|(j, ty)| {
                    Ok(ty.abilities().has_drop() && !used.contains(&(i, checked_as!(j, u16)?)))
                })
                .collect::<Result<_, ExecutionError>>()?;
        }
        Ok(())
    }

    fn command(used: &mut IndexSet<(u16, u16)>, sp!(_, c): &T::Command) {
        match &c.command {
            T::Command__::MoveCall(mc) => arguments(used, &mc.arguments),
            T::Command__::TransferObjects(objects, recipient) => {
                argument(used, recipient);
                arguments(used, objects);
            }
            T::Command__::SplitCoins(_, coin, amounts) => {
                arguments(used, amounts);
                argument(used, coin);
            }
            T::Command__::MergeCoins(_, target, coins) => {
                arguments(used, coins);
                argument(used, target);
            }
            T::Command__::MakeMoveVec(_, elements) => arguments(used, elements),
            T::Command__::Publish(_, _, _) => (),
            T::Command__::Upgrade(_, _, _, x, _) => argument(used, x),
        }
    }

    fn arguments(used: &mut IndexSet<(u16, u16)>, args: &[T::Argument]) {
        for arg in args {
            argument(used, arg)
        }
    }

    fn argument(used: &mut IndexSet<(u16, u16)>, sp!(_, (arg_, _)): &T::Argument) {
        if let T::Location::Result(i, j) = arg_.location() {
            used.insert((i, j));
        }
    }
}

//**************************************************************************************************
// Post-execution checked objects
//**************************************************************************************************

mod post_execution_checks {

    use iota_types::error::ExecutionError;

    use crate::{
        compat::{ObjectPermissions, SafeIndex},
        sp,
        static_programmable_transactions::typing::ast as T,
    };

    // Taint context for inputs and results that require/incur post-execution checks
    struct Context {
        // Does the input object require a post-execution check?
        inputs: Vec<bool>,
        // True if the command incurs post-execution checks
        results: Vec<bool>,
        propagate_through_mut_borrow: bool,
    }

    impl Context {
        pub fn new(ast: &T::Transaction) -> Self {
            let T::Transaction {
                gas_payment: _,
                bytes: _,
                objects,
                pure: _,
                receiving: _,
                original_command_len: _,
                commands: _,
                unified_linkage: _,
            } = ast;
            // Find inputs with post execution checks
            let inputs = objects
                .iter()
                .map(|o| {
                    o.arg.refined_permissions.can_use_mutably()
                        && o.arg.refined_permissions != ObjectPermissions::ALL
                })
                .collect::<Vec<_>>();
            Self {
                inputs,
                results: vec![],
                propagate_through_mut_borrow: true,
            }
        }
    }

    /// Find what commands will consume an object and incur a post execution check
    /// MakeMoveVec is the only command that can take objects by-value and propagate them
    /// for another command without directly incurring a check, as such we taint track for
    /// MakeMoveVec
    pub fn transaction(ast: &mut T::Transaction) -> Result<(), ExecutionError> {
        let mut context = Context::new(ast);

        // For each command, find what objects are taken by-value and will incur a post-execution
        // check
        for c in &mut ast.commands {
            debug_assert!(!c.value.incurs_post_execution_checks);
            command(&mut context, c)?;
        }
        Ok(())
    }

    fn command(context: &mut Context, sp!(_, c): &mut T::Command) -> Result<(), ExecutionError> {
        let arg_requires_post_execution_checks = arguments(context, c.command.arguments())?;
        let (incurs_checks, tainted_result) = match &c.command {
            // make move vec does not directly "consume" any objects, and can propagate
            // them to a later command
            T::Command__::MakeMoveVec(_, _) => {
                assert_invariant!(
                    c.result_type.len() == 1,
                    "MakeMoveVec must return a single value"
                );
                (false, arg_requires_post_execution_checks)
            }
            // these commands do not propagate objects, and directly incur checks
            T::Command__::MoveCall(_)
            | T::Command__::TransferObjects(_, _)
            | T::Command__::SplitCoins(_, _, _)
            | T::Command__::MergeCoins(_, _, _)
            | T::Command__::Publish(_, _, _)
            | T::Command__::Upgrade(_, _, _, _, _) => (arg_requires_post_execution_checks, false),
        };
        c.incurs_post_execution_checks |= incurs_checks;
        context.results.push(tainted_result);
        Ok(())
    }

    fn arguments<'a>(
        context: &mut Context,
        args: impl IntoIterator<Item = &'a T::Argument>,
    ) -> Result<bool, ExecutionError> {
        for arg in args {
            if argument(context, arg)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn argument(
        context: &mut Context,
        sp!(_, (arg_, _)): &T::Argument,
    ) -> Result<bool, ExecutionError> {
        Ok(match arg_.location() {
            // no shared/party objects in these locations
            T::Location::TxContext
            | T::Location::AuthContext
            | T::Location::GasCoin
            | T::Location::PureInput(_)
            | T::Location::ReceivingInput(_) => false,
            T::Location::ObjectInput(i) => match arg_ {
                T::Argument__::Use(T::Usage::Move(_)) => {
                    let arg_requires_post_execution_checks = context.inputs.safe_get(i as usize)?;
                    *arg_requires_post_execution_checks
                }
                T::Argument__::Use(T::Usage::Copy { .. })
                | T::Argument__::Borrow(_, _)
                | T::Argument__::Read(_)
                | T::Argument__::Freeze(_) => {
                    // not using the object by-value
                    false
                }
            },

            T::Location::Result(i, _) => {
                match arg_ {
                    T::Argument__::Use(T::Usage::Move(_)) => {
                        let tainted = context.results.safe_get(i as usize)?;
                        *tainted
                    }
                    T::Argument__::Borrow(/* mut */ true, _) => {
                        if context.propagate_through_mut_borrow {
                            let tainted = context.results.safe_get(i as usize)?;
                            *tainted
                        } else {
                            false
                        }
                    }
                    T::Argument__::Use(T::Usage::Copy { .. })
                    | T::Argument__::Borrow(false, _)
                    | T::Argument__::Read(_)
                    | T::Argument__::Freeze(_) => {
                        // not using the object by-value
                        false
                    }
                }
            }
        })
    }
}
