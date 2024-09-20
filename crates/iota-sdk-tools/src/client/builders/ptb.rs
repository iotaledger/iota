// Copyright 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Builder for Programmable Transactions.

use core::marker::PhantomData;
use std::collections::HashMap;

use async_trait::async_trait;
use iota_json_rpc_types::IotaObjectDataOptions;
use iota_move_build::BuildConfig;
use iota_types::{
    base_types::{IotaAddress, ObjectID},
    move_package::PACKAGE_MODULE_NAME,
    object::Owner,
    programmable_transaction_builder::ProgrammableTransactionBuilder as IotaPTB,
    transaction::{Argument, Command, ObjectArg},
    Identifier, IOTA_FRAMEWORK_PACKAGE_ID,
};

use crate::{
    client::{publish_type::PublishType, response::TransactionResponse},
    types::{GasCoin, MoveParam, MoveTypes, ParamType},
    Client, ClientError,
};

/// A builder for a programmable transaction which provides a better API vs.
/// IOTAs [`ProgrammableTransactionBuilder`](IotaPTB).
///
/// Additional Features:
/// - Command results can be bound by a static name, and referenced by later
///   calls with that name using [`Res`].
/// - Parameters can be passed in without fiddling with inputs, and will be
///   automatically handled.
/// - Mixed parameters (new inputs and named references) can be defined in a
///   single tuple.
/// - The builder is 100% chainable, start-to-finish.
pub struct ProgrammableTransactionBuilder {
    builder: IotaPTB,
    // This is only necessary because the IOTA PTB does not have any way to publicly access this
    // property :c
    num_commands: usize,
    gas_budget: Option<u64>,
    gas: Option<GasCoin>,
    wait_for_finalization: bool,
    named_commands: HashMap<&'static str, Argument>,
    client: Client,
}

impl ProgrammableTransactionBuilder {
    /// Instantiate a new PTB.
    pub fn new(client: &Client) -> Self {
        Self {
            builder: Default::default(),
            num_commands: 0,
            gas_budget: None,
            gas: None,
            wait_for_finalization: true,
            named_commands: Default::default(),
            client: client.clone(),
        }
    }

    /// Begin building a move call.
    pub fn move_call(
        &mut self,
        package_id: ObjectID,
        module: &str,
        function: &str,
    ) -> MoveCallCommandBuilder {
        MoveCallCommandBuilder::new(self, package_id, module, function)
    }

    /// Transfer objects to a recipient address.
    pub async fn transfer_objects<U: PTBArguments>(
        &mut self,
        recipient: IotaAddress,
        objects: U,
    ) -> Result<&mut Self, ClientError> {
        let objects = objects.args(self).await?;
        self.builder.transfer_args(recipient, objects);
        self.num_commands += 1;
        Ok(self)
    }

    /// Transfer IOTA to a recipient address.
    pub async fn transfer_iota(
        &mut self,
        recipient: IotaAddress,
        amount: impl Into<Option<u64>> + Send,
    ) -> Result<&mut Self, ClientError> {
        self.builder.transfer_iota(recipient, amount.into());
        self.num_commands += 1;
        Ok(self)
    }

    /// Merge multiple coins into one.
    pub async fn merge_coins(
        &mut self,
        primary_coin: ObjectID,
        consumed_coins: impl IntoIterator<Item = ObjectID> + Send,
    ) -> Result<&mut Self, ClientError> {
        let primary_coin = self.resolve_obj(primary_coin, true, false).await?;
        let mut consumed = Vec::new();
        for coin in consumed_coins {
            consumed.push(self.resolve_obj(coin, true, false).await?);
        }
        self.builder
            .command(Command::MergeCoins(primary_coin, consumed));
        self.num_commands += 1;
        Ok(self)
    }

    /// Split a coin into many.
    pub async fn split_coins(
        &mut self,
        coin: ObjectID,
        split_amounts: impl IntoIterator<Item = u64> + Send,
    ) -> Result<&mut Self, ClientError> {
        let coin = self.resolve_obj(coin, true, false).await?;
        let split_amounts = split_amounts
            .into_iter()
            .map(|v| self.builder.pure(v))
            .collect::<Result<_, _>>()?;
        self.builder
            .command(Command::SplitCoins(coin, split_amounts));
        self.num_commands += 1;
        Ok(self)
    }

    /// Publish a move package. Returns the upgrade capability, if there is one.
    pub fn publish(
        &mut self,
        kind: impl Into<PublishType> + Send,
    ) -> Result<PublishBuilder, ClientError> {
        PublishBuilder::new(self, kind)
    }

    /// Set the gas budget. Optional.
    pub fn gas_budget(&mut self, gas_budget: u64) -> &mut Self {
        self.gas_budget = Some(gas_budget);
        self
    }

    /// Set the gas coins that will be consumed. Optional.
    pub fn gas(&mut self, gas: GasCoin) -> &mut Self {
        self.gas = Some(gas);
        self
    }

    /// Set the flag that determines whether the execution will wait for
    /// finalization of this transaction. Default: true
    pub fn wait_for_finalization(mut self, wait_for_finalization: bool) -> Self {
        self.wait_for_finalization = wait_for_finalization;
        self
    }

    /// Execute the publish with the given data.
    pub async fn execute(self) -> Result<TransactionResponse, ClientError> {
        let txn = self.builder.finish();
        tracing::debug!("{txn}");
        self.client
            .data_mut()
            .await
            .execute_programmable_txn(txn, self.gas_budget, self.gas, self.wait_for_finalization)
            .await
    }

    async fn resolve_obj(
        &mut self,
        object_id: ObjectID,
        mutable: bool,
        receiving: bool,
    ) -> Result<Argument, ClientError> {
        let obj = self
            .client
            .data()
            .await
            .read_api()
            .get_object_with_options(object_id, IotaObjectDataOptions::new().with_owner())
            .await?;
        let obj_ref = obj
            .object_ref_if_exists()
            .ok_or_else(|| ClientError::MissingField("object ref"))?;

        Ok(
            match obj
                .owner()
                .ok_or_else(|| ClientError::MissingField("owner"))?
            {
                Owner::AddressOwner(_) | Owner::ObjectOwner(_) | Owner::Immutable => {
                    if receiving {
                        self.builder.obj(ObjectArg::Receiving(obj_ref))?
                    } else {
                        self.builder.obj(ObjectArg::ImmOrOwnedObject(obj_ref))?
                    }
                }
                Owner::Shared {
                    initial_shared_version,
                } => self.builder.obj(ObjectArg::SharedObject {
                    id: object_id,
                    initial_shared_version,
                    mutable,
                })?,
            },
        )
    }
}

/// A builder for a move call command within a programmable transaction.
#[derive(Debug)]
pub struct MoveCallCommandBuilder<'a, G: MoveTypes = (), A: PTBArguments = ()> {
    package: ObjectID,
    module: String,
    function: String,
    args: Option<A>,
    generics: PhantomData<G>,
    ptb: &'a mut ProgrammableTransactionBuilder,
}

impl<'a, G: MoveTypes, A: PTBArguments> MoveCallCommandBuilder<'a, G, A> {
    /// Instantiate a move call command builder.
    pub fn new(
        ptb: &'a mut ProgrammableTransactionBuilder,
        package_id: ObjectID,
        module: &str,
        function: &str,
    ) -> Self {
        Self {
            package: package_id,
            module: module.to_owned(),
            function: function.to_owned(),
            args: None,
            generics: PhantomData,
            ptb,
        }
    }

    /// Set the call params. Optional.
    pub fn params<U: PTBArguments>(self, params: U) -> MoveCallCommandBuilder<'a, G, U> {
        MoveCallCommandBuilder {
            package: self.package,
            module: self.module,
            function: self.function,
            args: Some(params),
            generics: self.generics,
            ptb: self.ptb,
        }
    }

    /// Set the generic type arguments. Optional.
    pub fn generics<U: MoveTypes>(self) -> MoveCallCommandBuilder<'a, U, A> {
        MoveCallCommandBuilder {
            package: self.package,
            module: self.module,
            function: self.function,
            args: self.args,
            generics: PhantomData,
            ptb: self.ptb,
        }
    }

    /// Finish the move call and return the PTB.
    pub async fn finish(
        self,
        name: impl NamedCommands,
    ) -> Result<&'a mut ProgrammableTransactionBuilder, ClientError> {
        let args = if let Some(a) = self.args {
            a.args(self.ptb).await?
        } else {
            Vec::new()
        };

        name.push_named_commands(self.ptb);
        self.ptb.builder.command(Command::move_call(
            self.package,
            Identifier::new(self.module)?,
            Identifier::new(self.function)?,
            G::type_tags(self.package),
            args,
        ));
        self.ptb.num_commands += 1;

        Ok(self.ptb)
    }
}

/// A builder for a move call command within a programmable transaction.
#[derive(Debug)]
pub struct PublishBuilder<'a> {
    ptb: &'a mut ProgrammableTransactionBuilder,
    cap: Argument,
}

impl<'a> PublishBuilder<'a> {
    /// Instantiate a publish call builder.
    pub fn new(
        ptb: &'a mut ProgrammableTransactionBuilder,
        kind: impl Into<PublishType>,
    ) -> Result<Self, ClientError> {
        let module = match kind.into() {
            PublishType::Path(path) => BuildConfig::default().build(path)?,
            PublishType::Compiled(m) => m,
        };
        let cap = ptb.builder.publish_upgradeable(
            module.get_package_bytes(false),
            module.published_dependency_ids(),
        );
        ptb.num_commands += 1;
        Ok(Self { ptb, cap })
    }

    /// Get the package ID from the UpgradeCap so that it can be used for future
    /// commands.
    ///
    /// **NOTE:** This is currently not usable for move calls because the IOTA
    /// PTB does not support using an argument for the package ID.
    pub async fn package_id(
        self,
        name: impl NamedCommand,
    ) -> Result<&'a mut ProgrammableTransactionBuilder, ClientError> {
        self.ptb
            .move_call(
                IOTA_FRAMEWORK_PACKAGE_ID,
                PACKAGE_MODULE_NAME.as_str(),
                "upgrade_package",
            )
            .params(self.cap)
            .finish(name)
            .await?;
        self.ptb.num_commands += 1;
        Ok(self.ptb)
    }

    /// Finish the move call and return the UpgradeCap.
    pub fn upgrade_cap(
        self,
        name: impl NamedCommand,
    ) -> Result<&'a mut ProgrammableTransactionBuilder, ClientError> {
        name.push_named_commands(self.ptb);

        Ok(self.ptb)
    }
}

impl core::fmt::Debug for ProgrammableTransactionBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProgrammableTransactionBuilder")
            .field("gas_budget", &self.gas_budget)
            .field("gas", &self.gas)
            .field("named_commands", &self.named_commands)
            .finish()
    }
}

/// A trait which defines arguments for a [`ProgrammableTransactionBuilder`].
#[async_trait]
pub trait PTBArguments: Send + Sync {
    /// Get the arguments.
    async fn args(
        &self,
        ptb: &mut ProgrammableTransactionBuilder,
    ) -> anyhow::Result<Vec<Argument>> {
        let mut args = Vec::new();
        self.push_args(ptb, &mut args).await?;
        Ok(args)
    }

    /// Push the args onto the list.
    async fn push_args(
        &self,
        ptb: &mut ProgrammableTransactionBuilder,
        args: &mut Vec<Argument>,
    ) -> anyhow::Result<()>;
}

macro_rules! impl_ptb_args_tuple {
    ($($tup:ident.$idx:tt),+$(,)?) => {
        #[async_trait]
        impl<$($tup),+> PTBArguments for ($($tup),+)
        where $($tup: PTBArguments),+
        {
            async fn push_args(&self, ptb: &mut ProgrammableTransactionBuilder, args: &mut Vec<Argument>) -> anyhow::Result<()> {
                $(
                    self.$idx.push_args(ptb, args).await?;
                )+
                Ok(())
            }
        }
    };
}
impl_ptb_args_tuple!(T1.0, T2.1);
impl_ptb_args_tuple!(T1.0, T2.1, T3.2);
impl_ptb_args_tuple!(T1.0, T2.1, T3.2, T4.3);
impl_ptb_args_tuple!(T1.0, T2.1, T3.2, T4.3, T5.4);

#[async_trait]
impl<T: MoveParam + Send + Sync> PTBArguments for T {
    async fn push_args(
        &self,
        ptb: &mut ProgrammableTransactionBuilder,
        args: &mut Vec<Argument>,
    ) -> anyhow::Result<()> {
        let arg = match self.param()? {
            ParamType::Object(id) => ptb.resolve_obj(id, false, false).await?,
            ParamType::Pure(v) => ptb.builder.pure_bytes(v, false),
        };
        args.push(arg);
        Ok(())
    }
}

#[async_trait]
impl PTBArguments for Argument {
    async fn push_args(
        &self,
        _: &mut ProgrammableTransactionBuilder,
        args: &mut Vec<Argument>,
    ) -> anyhow::Result<()> {
        args.push(*self);
        Ok(())
    }
}

/// Allows specifying mutable parameters.
pub struct Mut<T>(pub T);

#[async_trait]
impl<T: MoveParam + Send + Sync> PTBArguments for Mut<T> {
    async fn push_args(
        &self,
        ptb: &mut ProgrammableTransactionBuilder,
        args: &mut Vec<Argument>,
    ) -> anyhow::Result<()> {
        let arg = match self.0.param()? {
            ParamType::Object(id) => ptb.resolve_obj(id, true, false).await?,
            ParamType::Pure(v) => ptb.builder.pure_bytes(v, false),
        };
        args.push(arg);
        Ok(())
    }
}

/// Allows specifying receiving parameters.
pub struct Receiving<T>(pub T);

#[async_trait]
impl<T: MoveParam + Send + Sync> PTBArguments for Receiving<T> {
    async fn push_args(
        &self,
        ptb: &mut ProgrammableTransactionBuilder,
        args: &mut Vec<Argument>,
    ) -> anyhow::Result<()> {
        let arg = match self.0.param()? {
            ParamType::Object(id) => ptb.resolve_obj(id, false, true).await?,
            ParamType::Pure(v) => ptb.builder.pure_bytes(v, false),
        };
        args.push(arg);
        Ok(())
    }
}

/// The result of a previous command by name.
pub struct Res(pub &'static str);

#[async_trait]
impl PTBArguments for Res {
    async fn push_args(
        &self,
        ptb: &mut ProgrammableTransactionBuilder,
        args: &mut Vec<iota_types::transaction::Argument>,
    ) -> anyhow::Result<()> {
        if let Some(arg) = ptb.named_commands.get(self.0) {
            args.push(*arg);
        } else {
            anyhow::bail!("no command named `{}` exists", self.0)
        }
        Ok(())
    }
}

/// A trait that defines a named command, either a string or nothing.
pub trait NamedCommand {
    /// Get the named command argument.
    fn named_command(&self, ptb: &mut ProgrammableTransactionBuilder) -> Argument;

    /// Push the named command to the PTB.
    fn push_named_command(self, arg: Argument, ptb: &mut ProgrammableTransactionBuilder);
}

impl NamedCommand for &'static str {
    fn named_command(&self, ptb: &mut ProgrammableTransactionBuilder) -> Argument {
        Argument::Result(ptb.num_commands as _)
    }

    fn push_named_command(self, arg: Argument, ptb: &mut ProgrammableTransactionBuilder) {
        ptb.named_commands.insert(self, arg);
    }
}

impl NamedCommand for Option<&'static str> {
    fn named_command(&self, ptb: &mut ProgrammableTransactionBuilder) -> Argument {
        Argument::Result(ptb.num_commands as _)
    }

    fn push_named_command(self, arg: Argument, ptb: &mut ProgrammableTransactionBuilder) {
        if let Some(s) = self {
            s.push_named_command(arg, ptb)
        }
    }
}

/// A trait that allows tuples to be used to bind nested named commands.
pub trait NamedCommands {
    /// Push the named commands to the PTB.
    fn push_named_commands(self, ptb: &mut ProgrammableTransactionBuilder);
}

impl<T: NamedCommand> NamedCommands for T {
    fn push_named_commands(self, ptb: &mut ProgrammableTransactionBuilder) {
        let arg = Argument::Result(ptb.num_commands as _);
        self.push_named_command(arg, ptb)
    }
}

macro_rules! impl_named_command_tuple {
    ($($tup:ident.$idx:tt),+$(,)?) => {
        impl<$($tup),+> NamedCommands for ($($tup),+)
        where $($tup: NamedCommand),+
        {
            fn push_named_commands(self, ptb: &mut ProgrammableTransactionBuilder) {
                $(
                    let arg = Argument::NestedResult(ptb.num_commands as _, $idx);
                    self.$idx.push_named_command(arg, ptb);
                )+
            }
        }
    };
}
impl_named_command_tuple!(T1.0, T2.1);
impl_named_command_tuple!(T1.0, T2.1, T3.2);
impl_named_command_tuple!(T1.0, T2.1, T3.2, T4.3);
impl_named_command_tuple!(T1.0, T2.1, T3.2, T4.3, T5.4);
impl_named_command_tuple!(T1.0, T2.1, T3.2, T4.3, T5.4, T6.5);

impl NamedCommands for () {
    fn push_named_commands(self, _: &mut ProgrammableTransactionBuilder) {}
}
