// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Utility for generating programmable transactions, either by specifying a
//! command or for migrating legacy transactions

use anyhow::Context;
use indexmap::IndexMap;
use iota_sdk_types::{Identifier, TypeTag};
use serde::Serialize;

use crate::{
    base_types::{IotaAddress, ObjectID, ObjectRef},
    transaction::{Argument, CallArg, Command, ProgrammableTransaction, SharedObjectRef},
};

#[derive(PartialEq, Eq, Hash)]
enum BuilderArg {
    Object(ObjectID),
    Pure(Vec<u8>),
    ForcedNonUniquePure(usize),
}

#[derive(Default)]
pub struct ProgrammableTransactionBuilder {
    inputs: IndexMap<BuilderArg, CallArg>,
    commands: Vec<Command>,
}

impl ProgrammableTransactionBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn finish(self) -> ProgrammableTransaction {
        let Self { inputs, commands } = self;
        let inputs = inputs.into_values().collect();
        ProgrammableTransaction { inputs, commands }
    }

    pub fn pure_bytes(&mut self, bytes: Vec<u8>, force_separate: bool) -> Argument {
        let arg = if force_separate {
            BuilderArg::ForcedNonUniquePure(self.inputs.len())
        } else {
            BuilderArg::Pure(bytes.clone())
        };
        let (i, _) = self.inputs.insert_full(arg, CallArg::Pure(bytes));
        Argument::Input(i as u16)
    }

    pub fn pure<T: Serialize>(&mut self, value: T) -> anyhow::Result<Argument> {
        Ok(self.pure_bytes(
            bcs::to_bytes(&value).context("Serializing pure argument.")?,
            // force separate
            false,
        ))
    }

    /// Like pure but forces a separate input entry
    pub fn force_separate_pure<T: Serialize>(&mut self, value: T) -> anyhow::Result<Argument> {
        Ok(self.pure_bytes(
            bcs::to_bytes(&value).context("Serializing pure argument.")?,
            // force separate
            true,
        ))
    }

    pub fn obj(&mut self, obj_arg: impl Into<CallArg>) -> anyhow::Result<Argument> {
        let obj_arg: CallArg = obj_arg.into();
        let id = *obj_arg
            .object_id_opt()
            .ok_or_else(|| anyhow::anyhow!("expected object CallArg, found pure argument"))?;
        let obj_arg = if let Some(old_value) = self.inputs.get(&BuilderArg::Object(id)) {
            match (old_value.as_shared_opt(), obj_arg.as_shared_opt()) {
                (
                    Some(&SharedObjectRef {
                        object_id: id1,
                        initial_shared_version: v1,
                        mutable: mut1,
                    }),
                    Some(&SharedObjectRef {
                        object_id: id2,
                        initial_shared_version: v2,
                        mutable: mut2,
                    }),
                ) if v1 == v2 => {
                    anyhow::ensure!(
                        id1 == id2 && id == id2,
                        "invariant violation! object has id does not match call arg"
                    );
                    CallArg::Shared(SharedObjectRef {
                        object_id: id,
                        initial_shared_version: v2,
                        mutable: mut1 || mut2,
                    })
                }
                _ => {
                    anyhow::ensure!(
                        *old_value == obj_arg,
                        "Mismatched Object argument kind for object {id}. \
                        {old_value:?} is not compatible with {obj_arg:?}"
                    );
                    obj_arg
                }
            }
        } else {
            obj_arg
        };
        let (i, _) = self.inputs.insert_full(BuilderArg::Object(id), obj_arg);
        Ok(Argument::Input(i as u16))
    }

    pub fn input(&mut self, call_arg: CallArg) -> anyhow::Result<Argument> {
        match call_arg {
            CallArg::Pure(value) => Ok(self.pure_bytes(value, /* force separate */ false)),
            CallArg::ImmutableOrOwned(_) | CallArg::Shared(_) | CallArg::Receiving(_) => {
                self.obj(call_arg)
            }
            _ => unimplemented!("a new CallArg variant was added and needs to be handled"),
        }
    }

    pub fn make_obj_vec<T: Into<CallArg>>(
        &mut self,
        objs: impl IntoIterator<Item = T>,
    ) -> anyhow::Result<Argument> {
        let make_vec_args = objs
            .into_iter()
            .map(|obj| self.obj(obj.into()))
            .collect::<Result<_, _>>()?;
        Ok(self.command(Command::new_make_move_vector(None, make_vec_args)))
    }

    pub fn command(&mut self, command: Command) -> Argument {
        let i = self.commands.len();
        self.commands.push(command);
        Argument::Result(i as u16)
    }

    /// Will fail to generate if given an empty ObjVec
    pub fn move_call(
        &mut self,
        package: ObjectID,
        module: Identifier,
        function: Identifier,
        type_arguments: Vec<TypeTag>,
        call_args: Vec<CallArg>,
    ) -> anyhow::Result<()> {
        let arguments = call_args
            .into_iter()
            .map(|a| self.input(a))
            .collect::<Result<_, _>>()?;
        self.command(Command::new_move_call(
            package,
            module,
            function,
            type_arguments,
            arguments,
        ));
        Ok(())
    }

    pub fn programmable_move_call(
        &mut self,
        package: ObjectID,
        module: Identifier,
        function: Identifier,
        type_arguments: Vec<TypeTag>,
        arguments: Vec<Argument>,
    ) -> Argument {
        self.command(Command::new_move_call(
            package,
            module,
            function,
            type_arguments,
            arguments,
        ))
    }

    pub fn publish_upgradeable(
        &mut self,
        modules: Vec<Vec<u8>>,
        dep_ids: Vec<ObjectID>,
    ) -> Argument {
        self.command(Command::new_publish(modules, dep_ids))
    }

    pub fn publish_immutable(&mut self, modules: Vec<Vec<u8>>, dep_ids: Vec<ObjectID>) {
        let cap = self.publish_upgradeable(modules, dep_ids);
        self.commands.push(Command::new_move_call(
            ObjectID::FRAMEWORK,
            Identifier::PACKAGE_MODULE,
            Identifier::from_static("make_immutable"),
            vec![],
            vec![cap],
        ));
    }

    pub fn upgrade(
        &mut self,
        current_package_object_id: ObjectID,
        upgrade_ticket: Argument,
        transitive_deps: Vec<ObjectID>,
        modules: Vec<Vec<u8>>,
    ) -> Argument {
        self.command(Command::new_upgrade(
            modules,
            transitive_deps,
            current_package_object_id,
            upgrade_ticket,
        ))
    }

    pub fn transfer_arg(&mut self, recipient: IotaAddress, arg: Argument) {
        self.transfer_args(recipient, vec![arg])
    }

    pub fn transfer_args(&mut self, recipient: IotaAddress, args: Vec<Argument>) {
        let rec_arg = self.pure(recipient).unwrap();
        self.commands
            .push(Command::new_transfer_objects(args, rec_arg));
    }

    pub fn transfer_object(
        &mut self,
        recipient: IotaAddress,
        object_ref: ObjectRef,
    ) -> anyhow::Result<()> {
        let rec_arg = self.pure(recipient).unwrap();
        let obj_arg = self.obj(CallArg::ImmutableOrOwned(object_ref))?;
        self.commands
            .push(Command::new_transfer_objects(vec![obj_arg], rec_arg));
        Ok(())
    }

    pub fn transfer_iota(&mut self, recipient: IotaAddress, amount: Option<u64>) {
        let rec_arg = self.pure(recipient).unwrap();
        let coin_arg = if let Some(amount) = amount {
            let amt_arg = self.pure(amount).unwrap();
            self.command(Command::new_split_coins(Argument::Gas, vec![amt_arg]))
        } else {
            Argument::Gas
        };
        self.command(Command::new_transfer_objects(vec![coin_arg], rec_arg));
    }

    pub fn pay_all_iota(&mut self, recipient: IotaAddress) {
        let rec_arg = self.pure(recipient).unwrap();
        self.command(Command::new_transfer_objects(vec![Argument::Gas], rec_arg));
    }

    /// Will fail to generate if recipients and amounts do not have the same
    /// lengths
    pub fn pay_iota(
        &mut self,
        recipients: Vec<IotaAddress>,
        amounts: Vec<u64>,
    ) -> anyhow::Result<()> {
        self.pay_impl(recipients, amounts, Argument::Gas)
    }

    pub fn split_coin(&mut self, recipient: IotaAddress, coin: ObjectRef, amounts: Vec<u64>) {
        let coin_arg = self.obj(CallArg::ImmutableOrOwned(coin)).unwrap();
        let amounts_len = amounts.len();
        let amt_args = amounts.into_iter().map(|a| self.pure(a).unwrap()).collect();
        let result = self.command(Command::new_split_coins(coin_arg, amt_args));
        let Argument::Result(result) = result else {
            panic!("self.command should always give a Argument::Result");
        };

        let recipient = self.pure(recipient).unwrap();
        self.command(Command::new_transfer_objects(
            (0..amounts_len)
                .map(|i| Argument::NestedResult(result, i as u16))
                .collect(),
            recipient,
        ));
    }

    /// Will fail to generate if recipients and amounts do not have the same
    /// lengths. Or if coins is empty
    pub fn pay(
        &mut self,
        coins: Vec<ObjectRef>,
        recipients: Vec<IotaAddress>,
        amounts: Vec<u64>,
    ) -> anyhow::Result<()> {
        let mut coins = coins.into_iter();
        let Some(coin) = coins.next() else {
            anyhow::bail!("coins vector is empty");
        };
        let coin_arg = self.obj(CallArg::ImmutableOrOwned(coin))?;
        let merge_args: Vec<_> = coins
            .map(|c| self.obj(CallArg::ImmutableOrOwned(c)))
            .collect::<Result<_, _>>()?;
        if !merge_args.is_empty() {
            self.command(Command::new_merge_coins(coin_arg, merge_args));
        }
        self.pay_impl(recipients, amounts, coin_arg)
    }

    fn pay_impl(
        &mut self,
        recipients: Vec<IotaAddress>,
        amounts: Vec<u64>,
        coin: Argument,
    ) -> anyhow::Result<()> {
        if recipients.len() != amounts.len() {
            anyhow::bail!(
                "Recipients and amounts mismatch. Got {} recipients but {} amounts",
                recipients.len(),
                amounts.len()
            )
        }
        if amounts.is_empty() {
            return Ok(());
        }

        // collect recipients in the case where they are non-unique in order
        // to minimize the number of transfers that must be performed
        let mut recipient_map: IndexMap<IotaAddress, Vec<usize>> = IndexMap::new();
        let mut amt_args = Vec::with_capacity(recipients.len());
        for (i, (recipient, amount)) in recipients.into_iter().zip(amounts).enumerate() {
            recipient_map.entry(recipient).or_default().push(i);
            amt_args.push(self.pure(amount)?);
        }
        let Argument::Result(split_primary) =
            self.command(Command::new_split_coins(coin, amt_args))
        else {
            panic!("self.command should always give a Argument::Result")
        };
        for (recipient, split_secondaries) in recipient_map {
            let rec_arg = self.pure(recipient).unwrap();
            let coins = split_secondaries
                .into_iter()
                .map(|j| Argument::NestedResult(split_primary, j as u16))
                .collect();
            self.command(Command::new_transfer_objects(coins, rec_arg));
        }
        Ok(())
    }
}
