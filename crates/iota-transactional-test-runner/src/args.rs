// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use anyhow::{bail, ensure};
use clap::{self, Args, Parser};
use iota_types::{
    base_types::{IotaAddress, SequenceNumber},
    move_package::UpgradePolicy,
    object::{Object, Owner},
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::{Argument, CallArg, ObjectArg},
};
use move_compiler::editions::Flavor;
use move_core_types::{
    parsing::{
        parser::{Parser as MoveCLParser, parse_u64, parse_u256},
        values::{ParsableValue, ParsedValue, ValueToken},
    },
    runtime_value::{MoveStruct, MoveValue},
    u256::U256,
};
use move_symbol_pool::Symbol;
use move_transactional_test_runner::tasks::{RunCommand, SyntaxChoice};

use crate::test_adapter::{FakeID, IotaTestAdapter};

pub const IOTA_ARGS_LONG: &str = "iota-args";

#[derive(Clone, Debug, clap::Parser)]
pub struct IotaRunArgs {
    #[arg(long)]
    pub sender: Option<String>,
    #[arg(long)]
    pub gas_price: Option<u64>,
    #[arg(long)]
    pub summarize: bool,
}

#[derive(Debug, clap::Parser, Default)]
pub struct IotaPublishArgs {
    #[arg(long)]
    pub sender: Option<String>,
    #[arg(long, action = clap::ArgAction::SetTrue)]
    pub upgradeable: bool,
    #[arg(long, num_args(1..))]
    pub dependencies: Vec<String>,
    #[arg(long)]
    pub gas_price: Option<u64>,
}

#[derive(Debug, clap::Parser)]
pub struct IotaInitArgs {
    #[arg(long, num_args(1..))]
    pub accounts: Option<Vec<String>>,
    #[arg(long)]
    pub protocol_version: Option<u64>,
    #[arg(long)]
    pub max_gas: Option<u64>,
    #[arg(long)]
    pub move_binary_format_version: Option<u32>,
    #[arg(long)]
    pub simulator: bool,
    #[arg(long)]
    pub custom_validator_account: bool,
    #[arg(long)]
    pub reference_gas_price: Option<u64>,
    #[arg(long)]
    pub default_gas_price: Option<u64>,
    #[arg(long)]
    pub object_snapshot_min_checkpoint_lag: Option<usize>,
    #[arg(long)]
    pub flavor: Option<Flavor>,
}

#[derive(Debug, clap::Parser)]
pub struct ViewObjectCommand {
    #[arg(value_parser = parse_fake_id)]
    pub id: FakeID,
}

#[derive(Debug, clap::Parser)]
pub struct TransferObjectCommand {
    #[arg(value_parser = parse_fake_id)]
    pub id: FakeID,
    #[arg(long)]
    pub recipient: String,
    #[arg(long)]
    pub sender: Option<String>,
    #[arg(long)]
    pub gas_budget: Option<u64>,
    #[arg(long)]
    pub gas_price: Option<u64>,
}

#[derive(Debug, clap::Parser)]
pub struct ConsensusCommitPrologueCommand {
    #[arg(long)]
    pub timestamp_ms: u64,
}

#[derive(Debug, clap::Parser)]
pub struct ProgrammableTransactionCommand {
    #[arg(long)]
    pub sender: Option<String>,
    #[arg(long)]
    pub gas_budget: Option<u64>,
    #[arg(long)]
    pub gas_price: Option<u64>,
    #[arg(long)]
    pub dev_inspect: bool,
    #[arg(
        long,
        value_parser = ParsedValue::<IotaExtraValueArgs>::parse,
        num_args(1..),
        action = clap::ArgAction::Append,
    )]
    pub inputs: Vec<ParsedValue<IotaExtraValueArgs>>,
}

#[derive(Debug, clap::Parser)]
pub struct UpgradePackageCommand {
    #[arg(long)]
    pub package: String,
    #[arg(long, value_parser = parse_fake_id)]
    pub upgrade_capability: FakeID,
    #[arg(long, num_args(1..))]
    pub dependencies: Vec<String>,
    #[arg(long)]
    pub sender: String,
    #[arg(long)]
    pub gas_budget: Option<u64>,
    #[arg(long)]
    pub syntax: Option<SyntaxChoice>,
    #[arg(long, default_value="compatible", value_parser = parse_policy)]
    pub policy: u8,
    #[arg(long)]
    pub gas_price: Option<u64>,
}

#[derive(Debug, clap::Parser)]
pub struct StagePackageCommand {
    #[arg(long)]
    pub syntax: Option<SyntaxChoice>,
    #[arg(long, num_args(1..))]
    pub dependencies: Vec<String>,
}

#[derive(Debug, clap::Parser)]
pub struct SetAddressCommand {
    pub address: String,
    #[arg(value_parser = ParsedValue::<IotaExtraValueArgs>::parse)]
    pub input: ParsedValue<IotaExtraValueArgs>,
}

#[derive(Debug, clap::Parser)]
pub struct AdvanceClockCommand {
    #[arg(long)]
    pub duration_ns: u64,
}

#[derive(Debug, clap::Parser)]
pub struct RunGraphqlCommand {
    #[arg(long)]
    pub show_usage: bool,
    #[arg(long)]
    pub show_headers: bool,
    #[arg(long)]
    pub show_service_version: bool,
    #[arg(long, num_args(1..))]
    pub cursors: Vec<String>,
}

#[derive(Debug, clap::Parser)]
pub struct ForceObjectSnapshotCatchup {
    #[arg(long)]
    pub start_cp: u64,
    #[arg(long)]
    pub end_cp: u64,
}

#[derive(Debug, clap::Parser)]
pub struct CreateCheckpointCommand {
    pub count: Option<u64>,
}

#[derive(Debug, clap::Parser)]
pub struct AdvanceEpochCommand {
    pub count: Option<u64>,
}

#[derive(Debug, clap::Parser)]
pub struct SetRandomStateCommand {
    #[arg(long)]
    pub randomness_round: u64,
    #[arg(long)]
    pub random_bytes: String,
    #[arg(long)]
    pub randomness_initial_version: u64,
}

#[derive(Debug)]
pub enum IotaSubcommand<ExtraValueArgs: ParsableValue, ExtraRunArgs: Parser> {
    ViewObject(ViewObjectCommand),
    TransferObject(TransferObjectCommand),
    ConsensusCommitPrologue(ConsensusCommitPrologueCommand),
    ProgrammableTransaction(ProgrammableTransactionCommand),
    UpgradePackage(UpgradePackageCommand),
    StagePackage(StagePackageCommand),
    SetAddress(SetAddressCommand),
    CreateCheckpoint(CreateCheckpointCommand),
    AdvanceEpoch(AdvanceEpochCommand),
    AdvanceClock(AdvanceClockCommand),
    SetRandomState(SetRandomStateCommand),
    ViewCheckpoint,
    RunGraphql(RunGraphqlCommand),
    ForceObjectSnapshotCatchup(ForceObjectSnapshotCatchup),
    Bench(RunCommand<ExtraValueArgs>, ExtraRunArgs),
}

impl<ExtraValueArgs: ParsableValue, ExtraRunArgs: Parser> clap::FromArgMatches
    for IotaSubcommand<ExtraValueArgs, ExtraRunArgs>
{
    fn from_arg_matches(matches: &clap::ArgMatches) -> Result<Self, clap::Error> {
        Ok(match matches.subcommand() {
            Some(("view-object", matches)) => {
                IotaSubcommand::ViewObject(ViewObjectCommand::from_arg_matches(matches)?)
            }
            Some(("transfer-object", matches)) => {
                IotaSubcommand::TransferObject(TransferObjectCommand::from_arg_matches(matches)?)
            }
            Some(("consensus-commit-prologue", matches)) => {
                IotaSubcommand::ConsensusCommitPrologue(
                    ConsensusCommitPrologueCommand::from_arg_matches(matches)?,
                )
            }
            Some(("programmable", matches)) => IotaSubcommand::ProgrammableTransaction(
                ProgrammableTransactionCommand::from_arg_matches(matches)?,
            ),
            Some(("upgrade", matches)) => {
                IotaSubcommand::UpgradePackage(UpgradePackageCommand::from_arg_matches(matches)?)
            }
            Some(("stage-package", matches)) => {
                IotaSubcommand::StagePackage(StagePackageCommand::from_arg_matches(matches)?)
            }
            Some(("set-address", matches)) => {
                IotaSubcommand::SetAddress(SetAddressCommand::from_arg_matches(matches)?)
            }
            Some(("create-checkpoint", matches)) => IotaSubcommand::CreateCheckpoint(
                CreateCheckpointCommand::from_arg_matches(matches)?,
            ),
            Some(("advance-epoch", matches)) => {
                IotaSubcommand::AdvanceEpoch(AdvanceEpochCommand::from_arg_matches(matches)?)
            }
            Some(("advance-clock", matches)) => {
                IotaSubcommand::AdvanceClock(AdvanceClockCommand::from_arg_matches(matches)?)
            }
            Some(("set-random-state", matches)) => {
                IotaSubcommand::SetRandomState(SetRandomStateCommand::from_arg_matches(matches)?)
            }
            Some(("view-checkpoint", _)) => IotaSubcommand::ViewCheckpoint,
            Some(("run-graphql", matches)) => {
                IotaSubcommand::RunGraphql(RunGraphqlCommand::from_arg_matches(matches)?)
            }
            Some(("force-object-snapshot-catchup", matches)) => {
                IotaSubcommand::ForceObjectSnapshotCatchup(
                    ForceObjectSnapshotCatchup::from_arg_matches(matches)?,
                )
            }
            Some(("bench", matches)) => IotaSubcommand::Bench(
                RunCommand::from_arg_matches(matches)?,
                ExtraRunArgs::from_arg_matches(matches)?,
            ),
            _ => {
                return Err(clap::Error::raw(
                    clap::error::ErrorKind::InvalidSubcommand,
                    "Invalid submcommand",
                ));
            }
        })
    }

    fn update_from_arg_matches(&mut self, matches: &clap::ArgMatches) -> Result<(), clap::Error> {
        *self = Self::from_arg_matches(matches)?;
        Ok(())
    }
}

impl<ExtraValueArgs: ParsableValue, ExtraRunArgs: Parser> clap::CommandFactory
    for IotaSubcommand<ExtraValueArgs, ExtraRunArgs>
{
    fn command() -> clap::Command {
        clap::Command::new("iota_sub_command")
            .subcommand(ViewObjectCommand::command().name("view-object"))
            .subcommand(TransferObjectCommand::command().name("transfer-object"))
            .subcommand(ConsensusCommitPrologueCommand::command().name("consensus-commit-prologue"))
            .subcommand(ProgrammableTransactionCommand::command().name("programmable"))
            .subcommand(UpgradePackageCommand::command().name("upgrade"))
            .subcommand(StagePackageCommand::command().name("stage-package"))
            .subcommand(SetAddressCommand::command().name("set-address"))
            .subcommand(CreateCheckpointCommand::command().name("create-checkpoint"))
            .subcommand(AdvanceEpochCommand::command().name("advance-epoch"))
            .subcommand(AdvanceClockCommand::command().name("advance-clock"))
            .subcommand(SetRandomStateCommand::command().name("set-random-state"))
            .subcommand(clap::Command::new("view-checkpoint"))
            .subcommand(RunGraphqlCommand::command().name("run-graphql"))
            .subcommand(ForceObjectSnapshotCatchup::command().name("force-object-snapshot-catchup"))
            .subcommand(
                RunCommand::<ExtraValueArgs>::augment_args(ExtraRunArgs::command()).name("bench"),
            )
    }

    fn command_for_update() -> clap::Command {
        todo!()
    }
}

impl<ExtraValueArgs: ParsableValue, ExtraRunArgs: Parser> clap::Parser
    for IotaSubcommand<ExtraValueArgs, ExtraRunArgs>
{
}

#[derive(Clone, Debug)]
pub enum IotaExtraValueArgs {
    Object(FakeID, Option<SequenceNumber>),
    Digest(String),
    Receiving(FakeID, Option<SequenceNumber>),
    ImmShared(FakeID, Option<SequenceNumber>),
}

#[derive(Clone)]
pub enum IotaValue {
    MoveValue(MoveValue),
    Object(FakeID, Option<SequenceNumber>),
    ObjVec(Vec<(FakeID, Option<SequenceNumber>)>),
    Digest(String),
    Receiving(FakeID, Option<SequenceNumber>),
    ImmShared(FakeID, Option<SequenceNumber>),
}

impl IotaExtraValueArgs {
    fn parse_object_value<'a, I: Iterator<Item = (ValueToken, &'a str)>>(
        parser: &mut MoveCLParser<'a, ValueToken, I>,
    ) -> anyhow::Result<Self> {
        let (fake_id, version) = Self::parse_receiving_or_object_value(parser, "object")?;
        Ok(IotaExtraValueArgs::Object(fake_id, version))
    }

    fn parse_receiving_value<'a, I: Iterator<Item = (ValueToken, &'a str)>>(
        parser: &mut MoveCLParser<'a, ValueToken, I>,
    ) -> anyhow::Result<Self> {
        let (fake_id, version) = Self::parse_receiving_or_object_value(parser, "receiving")?;
        Ok(IotaExtraValueArgs::Receiving(fake_id, version))
    }

    fn parse_read_shared_value<'a, I: Iterator<Item = (ValueToken, &'a str)>>(
        parser: &mut MoveCLParser<'a, ValueToken, I>,
    ) -> anyhow::Result<Self> {
        let (fake_id, version) = Self::parse_receiving_or_object_value(parser, "immshared")?;
        Ok(IotaExtraValueArgs::ImmShared(fake_id, version))
    }

    fn parse_digest_value<'a, I: Iterator<Item = (ValueToken, &'a str)>>(
        parser: &mut MoveCLParser<'a, ValueToken, I>,
    ) -> anyhow::Result<Self> {
        let contents = parser.advance(ValueToken::Ident)?;
        ensure!(contents == "digest");
        parser.advance(ValueToken::LParen)?;
        let package = parser.advance(ValueToken::Ident)?;
        parser.advance(ValueToken::RParen)?;
        Ok(IotaExtraValueArgs::Digest(package.to_owned()))
    }

    fn parse_receiving_or_object_value<'a, I: Iterator<Item = (ValueToken, &'a str)>>(
        parser: &mut MoveCLParser<'a, ValueToken, I>,
        ident_name: &str,
    ) -> anyhow::Result<(FakeID, Option<SequenceNumber>)> {
        let contents = parser.advance(ValueToken::Ident)?;
        ensure!(contents == ident_name);
        parser.advance(ValueToken::LParen)?;
        let i_str = parser.advance(ValueToken::Number)?;
        let (i, _) = parse_u256(i_str)?;
        let fake_id = if let Some(ValueToken::Comma) = parser.peek_tok() {
            parser.advance(ValueToken::Comma)?;
            let j_str = parser.advance(ValueToken::Number)?;
            let (j, _) = parse_u64(j_str)?;
            if i > U256::from(u64::MAX) {
                bail!("Object ID too large")
            }
            FakeID::Enumerated(i.unchecked_as_u64(), j)
        } else {
            let mut u256_bytes = i.to_le_bytes().to_vec();
            u256_bytes.reverse();
            let address: IotaAddress = IotaAddress::from_bytes(&u256_bytes).unwrap();
            FakeID::Known(address.into())
        };
        parser.advance(ValueToken::RParen)?;
        let version = if let Some(ValueToken::AtSign) = parser.peek_tok() {
            parser.advance(ValueToken::AtSign)?;
            let v_str = parser.advance(ValueToken::Number)?;
            let (v, _) = parse_u64(v_str)?;
            Some(SequenceNumber::from_u64(v))
        } else {
            None
        };
        Ok((fake_id, version))
    }
}

impl IotaValue {
    fn assert_move_value(self) -> MoveValue {
        match self {
            IotaValue::MoveValue(v) => v,
            IotaValue::Object(_, _) => panic!("unexpected nested IOTA object in args"),
            IotaValue::ObjVec(_) => panic!("unexpected nested IOTA object vector in args"),
            IotaValue::Digest(_) => panic!("unexpected nested IOTA package digest in args"),
            IotaValue::Receiving(_, _) => panic!("unexpected nested IOTA receiving object in args"),
            IotaValue::ImmShared(_, _) => panic!("unexpected nested IOTA shared object in args"),
        }
    }

    fn assert_object(self) -> (FakeID, Option<SequenceNumber>) {
        match self {
            IotaValue::MoveValue(_) => panic!("unexpected nested non-object value in args"),
            IotaValue::Object(id, version) => (id, version),
            IotaValue::ObjVec(_) => panic!("unexpected nested IOTA object vector in args"),
            IotaValue::Digest(_) => panic!("unexpected nested IOTA package digest in args"),
            IotaValue::Receiving(_, _) => panic!("unexpected nested IOTA receiving object in args"),
            IotaValue::ImmShared(_, _) => panic!("unexpected nested IOTA shared object in args"),
        }
    }

    fn resolve_object(
        fake_id: FakeID,
        version: Option<SequenceNumber>,
        test_adapter: &IotaTestAdapter,
    ) -> anyhow::Result<Object> {
        let id = match test_adapter.fake_to_real_object_id(fake_id) {
            Some(id) => id,
            None => bail!("INVALID TEST. Unknown object, object({})", fake_id),
        };
        let obj_res = if let Some(v) = version {
            iota_types::storage::ObjectStore::get_object_by_key(&*test_adapter.executor, &id, v)
        } else {
            iota_types::storage::ObjectStore::get_object(&*test_adapter.executor, &id)
        };
        let obj = match obj_res {
            Ok(Some(obj)) => obj,
            Err(_) | Ok(None) => bail!("INVALID TEST. Could not load object argument {}", id),
        };
        Ok(obj)
    }

    fn receiving_arg(
        fake_id: FakeID,
        version: Option<SequenceNumber>,
        test_adapter: &IotaTestAdapter,
    ) -> anyhow::Result<ObjectArg> {
        let obj = Self::resolve_object(fake_id, version, test_adapter)?;
        Ok(ObjectArg::Receiving(obj.compute_object_reference()))
    }

    fn read_shared_arg(
        fake_id: FakeID,
        version: Option<SequenceNumber>,
        test_adapter: &IotaTestAdapter,
    ) -> anyhow::Result<ObjectArg> {
        let obj = Self::resolve_object(fake_id, version, test_adapter)?;
        let id = obj.id();
        if let Owner::Shared {
            initial_shared_version,
        } = obj.owner
        {
            Ok(ObjectArg::SharedObject {
                id,
                initial_shared_version,
                mutable: false,
            })
        } else {
            bail!("{fake_id} is not a shared object.")
        }
    }

    fn object_arg(
        fake_id: FakeID,
        version: Option<SequenceNumber>,
        test_adapter: &IotaTestAdapter,
    ) -> anyhow::Result<ObjectArg> {
        let obj = Self::resolve_object(fake_id, version, test_adapter)?;
        let id = obj.id();
        match obj.owner {
            Owner::Shared {
                initial_shared_version,
            } => Ok(ObjectArg::SharedObject {
                id,
                initial_shared_version,
                mutable: true,
            }),
            Owner::AddressOwner(_) | Owner::ObjectOwner(_) | Owner::Immutable => {
                let obj_ref = obj.compute_object_reference();
                Ok(ObjectArg::ImmOrOwnedObject(obj_ref))
            }
        }
    }

    pub(crate) fn into_call_arg(self, test_adapter: &IotaTestAdapter) -> anyhow::Result<CallArg> {
        Ok(match self {
            IotaValue::Object(fake_id, version) => {
                CallArg::Object(Self::object_arg(fake_id, version, test_adapter)?)
            }
            IotaValue::MoveValue(v) => CallArg::Pure(v.simple_serialize().unwrap()),
            IotaValue::Receiving(fake_id, version) => {
                CallArg::Object(Self::receiving_arg(fake_id, version, test_adapter)?)
            }
            IotaValue::ImmShared(fake_id, version) => {
                CallArg::Object(Self::read_shared_arg(fake_id, version, test_adapter)?)
            }
            IotaValue::ObjVec(_) => bail!("obj vec is not supported as an input"),
            IotaValue::Digest(pkg) => {
                let pkg = Symbol::from(pkg);
                let Some(staged) = test_adapter.staged_modules.get(&pkg) else {
                    bail!("Unbound staged package '{pkg}'")
                };
                CallArg::Pure(bcs::to_bytes(&staged.digest).unwrap())
            }
        })
    }

    pub(crate) fn into_argument(
        self,
        builder: &mut ProgrammableTransactionBuilder,
        test_adapter: &IotaTestAdapter,
    ) -> anyhow::Result<Argument> {
        match self {
            IotaValue::ObjVec(vec) => builder.make_obj_vec(
                vec.iter()
                    .map(|(fake_id, version)| Self::object_arg(*fake_id, *version, test_adapter))
                    .collect::<Result<Vec<ObjectArg>, _>>()?,
            ),
            value => {
                let call_arg = value.into_call_arg(test_adapter)?;
                builder.input(call_arg)
            }
        }
    }
}

impl ParsableValue for IotaExtraValueArgs {
    type ConcreteValue = IotaValue;

    fn parse_value<'a, I: Iterator<Item = (ValueToken, &'a str)>>(
        parser: &mut MoveCLParser<'a, ValueToken, I>,
    ) -> Option<anyhow::Result<Self>> {
        match parser.peek()? {
            (ValueToken::Ident, "object") => Some(Self::parse_object_value(parser)),
            (ValueToken::Ident, "digest") => Some(Self::parse_digest_value(parser)),
            (ValueToken::Ident, "receiving") => Some(Self::parse_receiving_value(parser)),
            (ValueToken::Ident, "immshared") => Some(Self::parse_read_shared_value(parser)),
            _ => None,
        }
    }

    fn move_value_into_concrete(v: MoveValue) -> anyhow::Result<Self::ConcreteValue> {
        Ok(IotaValue::MoveValue(v))
    }

    fn concrete_vector(elems: Vec<Self::ConcreteValue>) -> anyhow::Result<Self::ConcreteValue> {
        if !elems.is_empty() && matches!(elems[0], IotaValue::Object(_, _)) {
            Ok(IotaValue::ObjVec(
                elems.into_iter().map(IotaValue::assert_object).collect(),
            ))
        } else {
            Ok(IotaValue::MoveValue(MoveValue::Vector(
                elems
                    .into_iter()
                    .map(IotaValue::assert_move_value)
                    .collect(),
            )))
        }
    }

    fn concrete_struct(values: Vec<Self::ConcreteValue>) -> anyhow::Result<Self::ConcreteValue> {
        Ok(IotaValue::MoveValue(MoveValue::Struct(MoveStruct(
            values.into_iter().map(|v| v.assert_move_value()).collect(),
        ))))
    }

    fn into_concrete_value(
        self,
        _mapping: &impl Fn(&str) -> Option<move_core_types::account_address::AccountAddress>,
    ) -> anyhow::Result<Self::ConcreteValue> {
        match self {
            IotaExtraValueArgs::Object(id, version) => Ok(IotaValue::Object(id, version)),
            IotaExtraValueArgs::Digest(pkg) => Ok(IotaValue::Digest(pkg)),
            IotaExtraValueArgs::Receiving(id, version) => Ok(IotaValue::Receiving(id, version)),
            IotaExtraValueArgs::ImmShared(id, version) => Ok(IotaValue::ImmShared(id, version)),
        }
    }
}

fn parse_fake_id(s: &str) -> anyhow::Result<FakeID> {
    Ok(if let Some((s1, s2)) = s.split_once(',') {
        let (i, _) = parse_u64(s1)?;
        let (j, _) = parse_u64(s2)?;
        FakeID::Enumerated(i, j)
    } else {
        let (i, _) = parse_u256(s)?;
        let mut u256_bytes = i.to_le_bytes().to_vec();
        u256_bytes.reverse();
        let address: IotaAddress = IotaAddress::from_bytes(&u256_bytes).unwrap();
        FakeID::Known(address.into())
    })
}

fn parse_policy(x: &str) -> anyhow::Result<u8> {
    Ok(match x {
        "compatible" => UpgradePolicy::COMPATIBLE,
        "additive" => UpgradePolicy::ADDITIVE,
        "dep_only" => UpgradePolicy::DEP_ONLY,
        _ => bail!(
            "Invalid upgrade policy {x}. Policy must be one of 'compatible', 'additive', or 'dep_only'"
        ),
    })
}
