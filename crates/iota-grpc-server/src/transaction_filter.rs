// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_types::v1::filter as proto_filter;
use iota_metrics::monitored_scope;
use iota_sdk_types::{Address, Command, ExecutionStatus, ObjectId, Owner};
use iota_types::{
    effects::{TransactionEffectsAPI, TransactionEffectsExt},
    full_checkpoint_content::CheckpointTransaction,
    transaction::TransactionAPI,
};
use serde::{Deserialize, Serialize};

use crate::{constants::count_filters, event_filter::EventFilter};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum TransactionKind {
    /// The `System` variant can be used to filter for all types of system
    /// transactions.
    System = 0,
    Programmable = 1,
    Genesis = 2,
    ConsensusCommitPrologueV1 = 3,
    EndOfEpoch = 5,
    RandomnessStateUpdate = 6,
    ClaimAccount = 7,
}

impl From<&iota_sdk_types::TransactionKind> for TransactionKind {
    fn from(kind: &iota_sdk_types::TransactionKind) -> Self {
        match kind {
            iota_sdk_types::TransactionKind::Programmable(_) => TransactionKind::Programmable,
            iota_sdk_types::TransactionKind::Genesis(_) => TransactionKind::Genesis,
            iota_sdk_types::TransactionKind::ConsensusCommitPrologueV1(_) => {
                TransactionKind::ConsensusCommitPrologueV1
            }
            #[allow(deprecated)]
            iota_sdk_types::TransactionKind::AuthenticatorStateUpdateV1Deprecated => {
                TransactionKind::System
            }
            iota_sdk_types::TransactionKind::EndOfEpoch(_) => TransactionKind::EndOfEpoch,
            iota_sdk_types::TransactionKind::RandomnessStateUpdate(_) => {
                TransactionKind::RandomnessStateUpdate
            }
            // The filter proto has no dedicated value for this kind yet; the
            // `System` filter still matches it.
            iota_sdk_types::TransactionKind::TransactionDenyRulesUpdate(_) => {
                TransactionKind::System
            }
            iota_sdk_types::TransactionKind::ClaimAccount(_) => TransactionKind::ClaimAccount,
            _ => unimplemented!(
                "a new TransactionKind enum variant was added and needs to be handled"
            ),
        }
    }
}

impl TryFrom<proto_filter::TransactionKind> for TransactionKind {
    type Error = String;

    fn try_from(kind: proto_filter::TransactionKind) -> Result<Self, String> {
        match kind {
            proto_filter::TransactionKind::System => Ok(TransactionKind::System),
            proto_filter::TransactionKind::Programmable => Ok(TransactionKind::Programmable),
            proto_filter::TransactionKind::Genesis => Ok(TransactionKind::Genesis),
            proto_filter::TransactionKind::ConsensusCommitPrologueV1 => {
                Ok(TransactionKind::ConsensusCommitPrologueV1)
            }
            proto_filter::TransactionKind::EndOfEpoch => Ok(TransactionKind::EndOfEpoch),
            proto_filter::TransactionKind::RandomnessStateUpdate => {
                Ok(TransactionKind::RandomnessStateUpdate)
            }
            _ => Err("Unsupported transaction kind".to_string()),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CommandFilter {
    /// Match a MoveCall command.
    /// Package is required; module and function are optional.
    MoveCall {
        package: ObjectId,
        module: Option<String>,
        function: Option<String>,
    },
    /// Match a TransferObjects command.
    TransferObjects,
    /// Match a SplitCoins command.
    SplitCoins,
    /// Match a MergeCoins command.
    MergeCoins,
    /// Match a Publish command.
    Publish,
    /// Match a MakeMoveVec command.
    MakeMoveVec,
    /// Match an Upgrade command.
    /// Optionally filter by the specific package being upgraded.
    Upgrade { package: Option<ObjectId> },
}

impl CommandFilter {
    /// Returns true if any of the given commands match this filter.
    pub fn matches_commands(&self, commands: &[Command]) -> bool {
        commands.iter().any(|cmd| match (self, cmd) {
            (
                CommandFilter::MoveCall {
                    package,
                    module,
                    function,
                },
                Command::MoveCall(call),
            ) => {
                call.package == *package
                    && (module.is_none() || matches!(module, Some(m) if m == &call.module))
                    && (function.is_none() || matches!(function, Some(f) if f == &call.function))
            }
            (CommandFilter::TransferObjects, Command::TransferObjects(..)) => true,
            (CommandFilter::SplitCoins, Command::SplitCoins(..)) => true,
            (CommandFilter::MergeCoins, Command::MergeCoins(..)) => true,
            (CommandFilter::Publish, Command::Publish(..)) => true,
            (CommandFilter::MakeMoveVec, Command::MakeMoveVector(..)) => true,
            (CommandFilter::Upgrade { package }, Command::Upgrade(cmd)) => {
                package.is_none() || matches!(package, Some(p) if p == &cmd.package)
            }
            _ => false,
        })
    }
}

impl TryFrom<proto_filter::CommandFilter> for CommandFilter {
    type Error = String;

    fn try_from(proto: proto_filter::CommandFilter) -> Result<Self, Self::Error> {
        use proto_filter::command_filter::Filter as ProtoCommandFilter;

        let filter = proto.filter.ok_or("command filter is missing")?;

        match filter {
            ProtoCommandFilter::MoveCall(call_filter) => {
                let package_bytes = call_filter
                    .package_id
                    .ok_or("package_id is missing")?
                    .object_id;
                let package = ObjectId::from_bytes(&package_bytes)
                    .map_err(|e| format!("invalid package_id: {e}"))?;
                Ok(CommandFilter::MoveCall {
                    package,
                    module: call_filter.module,
                    function: call_filter.function,
                })
            }
            ProtoCommandFilter::TransferObjects(_) => Ok(CommandFilter::TransferObjects),
            ProtoCommandFilter::SplitCoins(_) => Ok(CommandFilter::SplitCoins),
            ProtoCommandFilter::MergeCoins(_) => Ok(CommandFilter::MergeCoins),
            ProtoCommandFilter::Publish(_) => Ok(CommandFilter::Publish),
            ProtoCommandFilter::MakeMoveVec(_) => Ok(CommandFilter::MakeMoveVec),
            ProtoCommandFilter::Upgrade(upgrade_filter) => {
                let package = upgrade_filter
                    .package_id
                    .map(|addr| {
                        ObjectId::from_bytes(&addr.object_id)
                            .map_err(|e| format!("invalid package_id: {e}"))
                    })
                    .transpose()?;
                Ok(CommandFilter::Upgrade { package })
            }
            _ => Err("Unsupported command filter type".to_string()),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ExecutionStatusFilter {
    /// Match successful transactions.
    Success,
    /// Match failed transactions (cancelled, execution error, or any other
    /// failure).
    Failure,
}

impl ExecutionStatusFilter {
    /// Returns true if the given execution status matches this filter.
    pub fn matches_status(&self, status: &ExecutionStatus) -> bool {
        match self {
            ExecutionStatusFilter::Success => status.is_success(),
            ExecutionStatusFilter::Failure => status.is_failure(),
        }
    }
}

impl From<proto_filter::ExecutionStatusFilter> for ExecutionStatusFilter {
    fn from(proto: proto_filter::ExecutionStatusFilter) -> Self {
        if proto.success {
            ExecutionStatusFilter::Success
        } else {
            ExecutionStatusFilter::Failure
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TransactionFilter {
    // Logical AND of several filters.
    All(Vec<TransactionFilter>),
    // Logical OR of several filters.
    Any(Vec<TransactionFilter>),
    // Logical NOT of a filter.
    Not(Box<TransactionFilter>),

    /// Filter transactions of any given kind in the input.
    TransactionKind(Vec<TransactionKind>),

    /// Filter by sender address.
    Sender(Address),
    /// Filter by recipient address. The recipient is determined by
    /// checking the owners of mutated and unwrapped objects.
    Receiver(Address),

    /// Filter for transactions that touch this object.
    AffectedObject(ObjectId),

    /// Filter by command type with optional criteria.
    Command(CommandFilter),

    /// Filter transactions that contain events matching the given event filter.
    Events(EventFilter),

    /// Filter by transaction execution status.
    ExecutionStatus(ExecutionStatusFilter),
}

// Proto-to-internal filter conversion
impl TryFrom<proto_filter::TransactionFilter> for TransactionFilter {
    type Error = String;

    /// Fails without converting the rest of the filter as soon as it is
    /// deeper than [`MAX_FILTER_DEPTH`] or has more than [`MAX_FILTER_NODES`]
    /// filters.
    ///
    /// [`MAX_FILTER_DEPTH`]: crate::constants::MAX_FILTER_DEPTH
    /// [`MAX_FILTER_NODES`]: crate::constants::MAX_FILTER_NODES
    fn try_from(proto: proto_filter::TransactionFilter) -> Result<Self, Self::Error> {
        Self::from_proto(proto, 0, &mut 0)
    }
}

impl TransactionFilter {
    /// Converts `proto`, which is nested `depth` levels deep. `nodes` counts
    /// the filters converted so far, as described for [`MAX_FILTER_NODES`].
    ///
    /// [`MAX_FILTER_NODES`]: crate::constants::MAX_FILTER_NODES
    fn from_proto(
        proto: proto_filter::TransactionFilter,
        depth: usize,
        nodes: &mut usize,
    ) -> Result<Self, String> {
        use proto_filter::transaction_filter::Filter as ProtoFilter;

        let kinds = match &proto.filter {
            Some(ProtoFilter::TransactionKinds(kinds_filter)) => kinds_filter.kinds.len(),
            _ => 0,
        };
        count_filters(depth, nodes, 1 + kinds)?;

        let filter = proto.filter.ok_or("transaction filter is missing")?;

        match filter {
            ProtoFilter::All(all) => {
                let filters = all
                    .filters
                    .into_iter()
                    .map(|f| TransactionFilter::from_proto(f, depth + 1, nodes))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(TransactionFilter::All(filters))
            }
            ProtoFilter::Any(any) => {
                let filters = any
                    .filters
                    .into_iter()
                    .map(|f| TransactionFilter::from_proto(f, depth + 1, nodes))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(TransactionFilter::Any(filters))
            }
            ProtoFilter::Negation(not) => {
                let inner = not.filter.ok_or("negation filter is missing")?;
                Ok(TransactionFilter::Not(Box::new(
                    TransactionFilter::from_proto(*inner, depth + 1, nodes)?,
                )))
            }
            ProtoFilter::TransactionKinds(kinds_filter) => {
                let kinds = kinds_filter
                    .kinds
                    .into_iter()
                    .map(|k| {
                        proto_filter::TransactionKind::try_from(k)
                            .map_err(|e| format!("Unknown transaction kind: {e}"))
                            .and_then(TransactionKind::try_from)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(TransactionFilter::TransactionKind(kinds))
            }
            ProtoFilter::Sender(addr_filter) => {
                let address = addr_filter
                    .address
                    .ok_or("sender address is missing")?
                    .address;
                let iota_address = Address::from_bytes(&address)
                    .map_err(|e| format!("invalid sender address: {e}"))?;
                Ok(TransactionFilter::Sender(iota_address))
            }
            ProtoFilter::Receiver(addr_filter) => {
                let address = addr_filter
                    .address
                    .ok_or("receiver address is missing")?
                    .address;
                let iota_address = Address::from_bytes(&address)
                    .map_err(|e| format!("invalid receiver address: {e}"))?;
                Ok(TransactionFilter::Receiver(iota_address))
            }
            ProtoFilter::AffectedObject(obj_filter) => {
                let object_ref = obj_filter.object_ref.ok_or("object_ref is missing")?;
                let object_id: ObjectId = object_ref
                    .object_id
                    .as_ref()
                    .ok_or("object_id is missing")?
                    .object_id()
                    .map_err(|e| format!("invalid object_id: {e}"))?;
                Ok(TransactionFilter::AffectedObject(object_id))
            }
            ProtoFilter::Command(command_filter) => {
                let internal_command_filter = CommandFilter::try_from(command_filter)?;
                Ok(TransactionFilter::Command(internal_command_filter))
            }
            ProtoFilter::Event(event_filter) => {
                let internal_event_filter =
                    EventFilter::from_proto(event_filter, depth + 1, nodes)?;
                Ok(TransactionFilter::Events(internal_event_filter))
            }
            ProtoFilter::ExecutionStatus(status_filter) => {
                Ok(TransactionFilter::ExecutionStatus(status_filter.into()))
            }
            _ => Err("Unsupported transaction filter type".to_string()),
        }
    }
}

fn is_system_transaction(transaction_kind: &TransactionKind) -> bool {
    match transaction_kind {
        TransactionKind::System
        | TransactionKind::Genesis
        | TransactionKind::ConsensusCommitPrologueV1
        | TransactionKind::EndOfEpoch
        | TransactionKind::RandomnessStateUpdate => true,
        TransactionKind::Programmable | TransactionKind::ClaimAccount => false,
    }
}

impl TransactionFilter {
    pub fn matches_transaction(&self, item: &CheckpointTransaction) -> bool {
        let _scope = monitored_scope("TransactionFilter::matches_transaction");
        let tx = item.transaction.data().transaction();

        match self {
            TransactionFilter::All(filters) => filters.iter().all(|f| f.matches_transaction(item)),

            TransactionFilter::Any(filters) => filters.iter().any(|f| f.matches_transaction(item)),

            TransactionFilter::Not(filter) => !filter.matches_transaction(item),

            TransactionFilter::TransactionKind(kinds) => {
                let actual_kind = TransactionKind::from(tx.kind());
                kinds.iter().any(|kind| match kind {
                    TransactionKind::System => is_system_transaction(&actual_kind),
                    _ => kind == &actual_kind,
                })
            }

            TransactionFilter::Sender(a) => &tx.sender() == a,

            TransactionFilter::Receiver(a) => item
                .effects
                .mutated()
                .iter()
                .chain(item.effects.unwrapped().iter())
                .any(|changed| matches!(changed.owner(), Owner::Address(addr) if addr == a)),

            TransactionFilter::AffectedObject(o) => item
                .effects
                .all_affected_objects()
                .iter()
                .any(|obj_ref| &obj_ref.object_id == o),

            TransactionFilter::Command(cmd_filter) => match tx.kind() {
                iota_sdk_types::TransactionKind::Programmable(pt) => {
                    cmd_filter.matches_commands(&pt.commands)
                }
                _ => false,
            },

            TransactionFilter::Events(event_filter) => item
                .events
                .as_ref()
                .is_some_and(|evts| evts.iter().any(|event| event_filter.matches_event(event))),

            TransactionFilter::ExecutionStatus(status_filter) => {
                status_filter.matches_status(item.effects.status())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use iota_sdk_types::{Argument, Command, Identifier};
    use proto_filter::transaction_filter::Filter as ProtoFilter;

    use super::*;
    use crate::constants::{MAX_FILTER_DEPTH, MAX_FILTER_NODES};

    fn proto(filter: ProtoFilter) -> proto_filter::TransactionFilter {
        let mut proto = proto_filter::TransactionFilter::default();
        proto.filter = Some(filter);
        proto
    }

    fn proto_kinds(n: usize) -> proto_filter::TransactionFilter {
        let mut kinds = proto_filter::TransactionKindsFilter::default();
        kinds.kinds = vec![proto_filter::TransactionKind::Programmable as i32; n];
        proto(ProtoFilter::TransactionKinds(kinds))
    }

    fn proto_events() -> proto_filter::TransactionFilter {
        let mut event_type = proto_filter::MoveEventTypeFilter::default();
        event_type.struct_tag = "0x2::module::Event".to_string();
        let mut events = proto_filter::EventFilter::default();
        events.filter = Some(proto_filter::event_filter::Filter::MoveEventType(
            event_type,
        ));
        proto(ProtoFilter::Event(events))
    }

    fn proto_not(
        inner: proto_filter::TransactionFilter,
        levels: usize,
    ) -> proto_filter::TransactionFilter {
        (0..levels).fold(inner, |inner, _| {
            let mut not = proto_filter::NotTransactionFilter::default();
            not.filter = Some(Box::new(inner));
            proto(ProtoFilter::Negation(Box::new(not)))
        })
    }

    #[test]
    fn proto_conversion_rejects_filters_deeper_than_the_limit() {
        assert!(TransactionFilter::try_from(proto_not(proto_kinds(1), MAX_FILTER_DEPTH)).is_ok());
        assert!(
            TransactionFilter::try_from(proto_not(proto_kinds(1), MAX_FILTER_DEPTH + 1)).is_err()
        );

        // The event filter of a transaction filter adds one level.
        assert!(
            TransactionFilter::try_from(proto_not(proto_events(), MAX_FILTER_DEPTH - 1)).is_ok()
        );
        assert!(TransactionFilter::try_from(proto_not(proto_events(), MAX_FILTER_DEPTH)).is_err());

        assert!(TransactionFilter::try_from(proto_not(proto_kinds(1), 1_000)).is_err());
    }

    fn proto_all(filters: Vec<proto_filter::TransactionFilter>) -> proto_filter::TransactionFilter {
        let mut all = proto_filter::AllTransactionFilter::default();
        all.filters = filters;
        proto(ProtoFilter::All(all))
    }

    fn proto_any(filters: Vec<proto_filter::TransactionFilter>) -> proto_filter::TransactionFilter {
        let mut any = proto_filter::AnyTransactionFilter::default();
        any.filters = filters;
        proto(ProtoFilter::Any(any))
    }

    #[test]
    fn proto_conversion_counts_all_and_any_as_levels() {
        let wraps: [fn(Vec<proto_filter::TransactionFilter>) -> proto_filter::TransactionFilter;
            2] = [proto_all, proto_any];
        for wrap in wraps {
            let nested = |levels| (0..levels).fold(proto_kinds(1), |f, _| wrap(vec![f]));
            assert!(TransactionFilter::try_from(nested(MAX_FILTER_DEPTH)).is_ok());
            assert!(TransactionFilter::try_from(nested(MAX_FILTER_DEPTH + 1)).is_err());
        }
        assert!(TransactionFilter::try_from(proto_all(vec![])).is_ok());
        assert!(TransactionFilter::try_from(proto_any(vec![])).is_ok());
    }

    #[test]
    fn proto_conversion_rejects_filters_with_too_many_nodes() {
        // Every transaction kind counts as a filter.
        assert!(TransactionFilter::try_from(proto_kinds(MAX_FILTER_NODES - 1)).is_ok());
        assert!(TransactionFilter::try_from(proto_kinds(MAX_FILTER_NODES)).is_err());

        // The filter count covers the event filters inside the transaction filter.
        let err =
            TransactionFilter::try_from(proto_any(vec![proto_events(); MAX_FILTER_NODES / 2]))
                .unwrap_err();
        assert!(err.contains("nodes"), "{err}");
    }

    // --- CommandFilter matching tests ---

    fn make_move_call_cmd(package: ObjectId, module: &str, function: &str) -> Command {
        Command::new_move_call(
            package,
            Identifier::new_unchecked(module),
            Identifier::new_unchecked(function),
            vec![],
            vec![],
        )
    }

    #[test]
    fn test_command_filter_move_call_exact() {
        let pkg = ObjectId::random();
        let commands = vec![make_move_call_cmd(pkg, "my_module", "my_func")];

        // Exact match
        let filter = CommandFilter::MoveCall {
            package: pkg,
            module: Some("my_module".into()),
            function: Some("my_func".into()),
        };
        assert!(filter.matches_commands(&commands));

        // Wrong function
        let filter = CommandFilter::MoveCall {
            package: pkg,
            module: Some("my_module".into()),
            function: Some("other_func".into()),
        };
        assert!(!filter.matches_commands(&commands));

        // Wrong module
        let filter = CommandFilter::MoveCall {
            package: pkg,
            module: Some("other_module".into()),
            function: None,
        };
        assert!(!filter.matches_commands(&commands));

        // Wrong package
        let filter = CommandFilter::MoveCall {
            package: ObjectId::random(),
            module: None,
            function: None,
        };
        assert!(!filter.matches_commands(&commands));
    }

    #[test]
    fn test_command_filter_move_call_optional_fields() {
        let pkg = ObjectId::random();
        let commands = vec![make_move_call_cmd(pkg, "my_module", "my_func")];

        // Package only — matches any module/function
        let filter = CommandFilter::MoveCall {
            package: pkg,
            module: None,
            function: None,
        };
        assert!(filter.matches_commands(&commands));

        // Package + module — matches any function
        let filter = CommandFilter::MoveCall {
            package: pkg,
            module: Some("my_module".into()),
            function: None,
        };
        assert!(filter.matches_commands(&commands));
    }

    #[test]
    fn test_command_filter_transfer_objects() {
        let commands = vec![Command::new_transfer_objects(
            vec![Argument::Input(0)],
            Argument::Input(1),
        )];

        assert!(CommandFilter::TransferObjects.matches_commands(&commands));
        assert!(!CommandFilter::SplitCoins.matches_commands(&commands));
    }

    #[test]
    fn test_command_filter_split_coins() {
        let commands = vec![Command::new_split_coins(
            Argument::Input(0),
            vec![Argument::Input(1)],
        )];

        assert!(CommandFilter::SplitCoins.matches_commands(&commands));
        assert!(!CommandFilter::MergeCoins.matches_commands(&commands));
    }

    #[test]
    fn test_command_filter_merge_coins() {
        let commands = vec![Command::new_merge_coins(
            Argument::Input(0),
            vec![Argument::Input(1)],
        )];

        assert!(CommandFilter::MergeCoins.matches_commands(&commands));
        assert!(!CommandFilter::SplitCoins.matches_commands(&commands));
    }

    #[test]
    fn test_command_filter_publish() {
        let commands = vec![Command::new_publish(vec![vec![1, 2, 3]], vec![])];

        assert!(CommandFilter::Publish.matches_commands(&commands));
        assert!(!CommandFilter::TransferObjects.matches_commands(&commands));
    }

    #[test]
    fn test_command_filter_make_move_vec() {
        let commands = vec![Command::new_make_move_vector(
            None,
            vec![Argument::Input(0)],
        )];

        assert!(CommandFilter::MakeMoveVec.matches_commands(&commands));
        assert!(!CommandFilter::Publish.matches_commands(&commands));
    }

    #[test]
    fn test_command_filter_upgrade_any() {
        let pkg = ObjectId::random();
        let commands = vec![Command::new_upgrade(
            vec![vec![1, 2, 3]],
            vec![],
            pkg,
            Argument::Input(0),
        )];

        // Match any upgrade
        let filter = CommandFilter::Upgrade { package: None };
        assert!(filter.matches_commands(&commands));
    }

    #[test]
    fn test_command_filter_upgrade_specific_package() {
        let pkg = ObjectId::random();
        let other_pkg = ObjectId::random();
        let commands = vec![Command::new_upgrade(
            vec![vec![1, 2, 3]],
            vec![],
            pkg,
            Argument::Input(0),
        )];

        // Match specific package
        let filter = CommandFilter::Upgrade { package: Some(pkg) };
        assert!(filter.matches_commands(&commands));

        // Wrong package
        let filter = CommandFilter::Upgrade {
            package: Some(other_pkg),
        };
        assert!(!filter.matches_commands(&commands));
    }

    #[test]
    fn test_command_filter_empty_commands() {
        let commands: Vec<Command> = vec![];

        assert!(!CommandFilter::Publish.matches_commands(&commands));
        assert!(!CommandFilter::TransferObjects.matches_commands(&commands));
        assert!(
            !CommandFilter::MoveCall {
                package: ObjectId::random(),
                module: None,
                function: None,
            }
            .matches_commands(&commands)
        );
    }

    #[test]
    fn test_command_filter_multiple_commands() {
        let pkg = ObjectId::random();
        let commands = vec![
            Command::new_split_coins(Argument::Input(0), vec![Argument::Input(1)]),
            make_move_call_cmd(pkg, "m", "f"),
            Command::new_transfer_objects(vec![Argument::Result(0)], Argument::Input(2)),
        ];

        // All three types should match
        assert!(CommandFilter::SplitCoins.matches_commands(&commands));
        assert!(
            CommandFilter::MoveCall {
                package: pkg,
                module: Some("m".into()),
                function: Some("f".into()),
            }
            .matches_commands(&commands)
        );
        assert!(CommandFilter::TransferObjects.matches_commands(&commands));

        // But Publish doesn't
        assert!(!CommandFilter::Publish.matches_commands(&commands));
    }

    // --- ExecutionStatusFilter matching tests ---

    #[test]
    fn test_execution_status_filter_success() {
        let filter = ExecutionStatusFilter::Success;

        assert!(filter.matches_status(&ExecutionStatus::Success));
        assert!(!filter.matches_status(&ExecutionStatus::Failure {
            error: iota_sdk_types::ExecutionError::InsufficientGas,
            command: None,
        }));
    }

    #[test]
    fn test_execution_status_filter_failure() {
        let filter = ExecutionStatusFilter::Failure;

        assert!(!filter.matches_status(&ExecutionStatus::Success));

        // Regular failure
        assert!(filter.matches_status(&ExecutionStatus::Failure {
            error: iota_sdk_types::ExecutionError::InsufficientGas,
            command: None,
        }));

        // Cancelled due to congestion
        assert!(filter.matches_status(&ExecutionStatus::Failure {
            error: iota_sdk_types::ExecutionError::ExecutionCanceledDueToSharedObjectCongestion {
                congested_objects: vec![],
            },
            command: None,
        }));

        // Cancelled due to randomness
        assert!(filter.matches_status(&ExecutionStatus::Failure {
            error: iota_sdk_types::ExecutionError::ExecutionCanceledDueToRandomnessUnavailable,
            command: None,
        }));
    }

    #[test]
    fn test_is_system_transaction_classifies_every_kind() {
        // The abstract `System` marker is produced by `From` for concrete
        // system kinds (e.g. AuthenticatorStateUpdateV1), so it must classify
        // as a system transaction rather than fall through to a panic.
        assert!(is_system_transaction(&TransactionKind::System));
        assert!(is_system_transaction(&TransactionKind::Genesis));
        assert!(is_system_transaction(
            &TransactionKind::ConsensusCommitPrologueV1
        ));
        assert!(is_system_transaction(&TransactionKind::EndOfEpoch));
        assert!(is_system_transaction(
            &TransactionKind::RandomnessStateUpdate
        ));
        assert!(!is_system_transaction(&TransactionKind::Programmable));
    }
}
