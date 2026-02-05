// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use iota_grpc_types::v0::filter as proto_filter;
use iota_metrics::monitored_scope;
use iota_types::{
    base_types::{IotaAddress, ObjectID},
    effects::TransactionEffectsAPI,
    full_checkpoint_content::CheckpointTransaction,
    object::Owner,
    transaction::TransactionDataAPI,
};
use serde::{Deserialize, Serialize};

use crate::{GrpcStateReader, event_filter::EventFilter};

/// Maximum allowed depth for nested filters to prevent DoS attacks
const MAX_FILTER_DEPTH: usize = 10;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum TransactionKind {
    /// The `SystemTransaction` variant can be used to filter for all types of
    /// system transactions.
    SystemTransaction = 0,
    ProgrammableTransaction = 1,
    Genesis = 2,
    ConsensusCommitPrologueV1 = 3,
    AuthenticatorStateUpdateV1 = 4,
    EndOfEpochTransaction = 5,
    RandomnessStateUpdate = 6,
}

impl From<&iota_types::transaction::TransactionKind> for TransactionKind {
    fn from(kind: &iota_types::transaction::TransactionKind) -> Self {
        match kind {
            iota_types::transaction::TransactionKind::ProgrammableTransaction(_) => {
                TransactionKind::ProgrammableTransaction
            }
            iota_types::transaction::TransactionKind::Genesis(_) => TransactionKind::Genesis,
            iota_types::transaction::TransactionKind::ConsensusCommitPrologueV1(_) => {
                TransactionKind::ConsensusCommitPrologueV1
            }
            iota_types::transaction::TransactionKind::AuthenticatorStateUpdateV1(_) => {
                TransactionKind::AuthenticatorStateUpdateV1
            }
            iota_types::transaction::TransactionKind::EndOfEpochTransaction(_) => {
                TransactionKind::EndOfEpochTransaction
            }
            iota_types::transaction::TransactionKind::RandomnessStateUpdate(_) => {
                TransactionKind::RandomnessStateUpdate
            }
        }
    }
}

impl TryFrom<proto_filter::TransactionKind> for TransactionKind {
    type Error = String;

    fn try_from(kind: proto_filter::TransactionKind) -> Result<Self, String> {
        match kind {
            proto_filter::TransactionKind::SystemTransaction => {
                Ok(TransactionKind::SystemTransaction)
            }
            proto_filter::TransactionKind::ProgrammableTransaction => {
                Ok(TransactionKind::ProgrammableTransaction)
            }
            proto_filter::TransactionKind::Genesis => Ok(TransactionKind::Genesis),
            proto_filter::TransactionKind::ConsensusCommitPrologueV1 => {
                Ok(TransactionKind::ConsensusCommitPrologueV1)
            }
            proto_filter::TransactionKind::AuthenticatorStateUpdateV1 => {
                Ok(TransactionKind::AuthenticatorStateUpdateV1)
            }
            proto_filter::TransactionKind::EndOfEpochTransaction => {
                Ok(TransactionKind::EndOfEpochTransaction)
            }
            proto_filter::TransactionKind::RandomnessStateUpdate => {
                Ok(TransactionKind::RandomnessStateUpdate)
            }
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
    Sender(IotaAddress),
    /// Filter by recipient address. The recipient is determined by
    /// checking the owners of mutated and unwrapped objects.
    Receiver(IotaAddress),

    /// Filter for transactions that touch this object.
    AffectedObject(ObjectID),

    /// Filter by move package, module (optional) and function (optional).
    MoveCall {
        /// the Move package ID
        package: ObjectID,
        /// the module name
        module: Option<String>,
        /// the function name
        function: Option<String>,
    },

    /// Filter transactions that contain events matching the given event filter.
    Events(EventFilter),
}

// Proto-to-internal filter conversion
impl TryFrom<proto_filter::TransactionFilter> for TransactionFilter {
    type Error = String;

    fn try_from(proto: proto_filter::TransactionFilter) -> Result<Self, Self::Error> {
        use proto_filter::transaction_filter::Filter as ProtoFilter;

        let filter = proto.filter.ok_or("transaction filter is missing")?;

        match filter {
            ProtoFilter::All(all) => {
                let filters = all
                    .filters
                    .into_iter()
                    .map(TransactionFilter::try_from)
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(TransactionFilter::All(filters))
            }
            ProtoFilter::Any(any) => {
                let filters = any
                    .filters
                    .into_iter()
                    .map(TransactionFilter::try_from)
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(TransactionFilter::Any(filters))
            }
            ProtoFilter::Negation(not) => {
                let inner = not.filter.ok_or("negation filter is missing")?;
                Ok(TransactionFilter::Not(Box::new(
                    TransactionFilter::try_from(*inner)?,
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
                let iota_address = IotaAddress::from_bytes(&address)
                    .map_err(|e| format!("invalid sender address: {}", e))?;
                Ok(TransactionFilter::Sender(iota_address))
            }
            ProtoFilter::Receiver(addr_filter) => {
                let address = addr_filter
                    .address
                    .ok_or("receiver address is missing")?
                    .address;
                let iota_address = IotaAddress::from_bytes(&address)
                    .map_err(|e| format!("invalid receiver address: {}", e))?;
                Ok(TransactionFilter::Receiver(iota_address))
            }
            ProtoFilter::AffectedObject(obj_filter) => {
                // TODO: add a function to convert ObjectReference to ObjectID
                let object_ref = obj_filter.object_ref.ok_or("object_ref is missing")?;
                let object_id_str = object_ref.object_id.ok_or("object_id is missing")?;
                let object_id: ObjectID = object_id_str
                    .parse()
                    .map_err(|e| format!("invalid object_id: {}", e))?;
                Ok(TransactionFilter::AffectedObject(object_id))
            }
            ProtoFilter::MoveCall(call_filter) => {
                // TODO: is this correct?
                let package_bytes = call_filter
                    .package_id
                    .ok_or("package_id is missing")?
                    .address;
                let package = ObjectID::from_bytes(&package_bytes)
                    .map_err(|e| format!("invalid package_id: {}", e))?;
                Ok(TransactionFilter::MoveCall {
                    package,
                    module: call_filter.module,
                    function: call_filter.function,
                })
            }
            ProtoFilter::Event(event_filter) => {
                let internal_event_filter = EventFilter::try_from(event_filter)?;
                Ok(TransactionFilter::Events(internal_event_filter))
            }
        }
    }
}

fn is_system_transaction(transaction_kind: &TransactionKind) -> bool {
    match transaction_kind {
        TransactionKind::Genesis
        | TransactionKind::ConsensusCommitPrologueV1
        | TransactionKind::AuthenticatorStateUpdateV1
        | TransactionKind::EndOfEpochTransaction
        | TransactionKind::RandomnessStateUpdate => true,
        TransactionKind::ProgrammableTransaction => false,
        _ => panic!("Unhandled transaction kind"),
    }
}

impl TransactionFilter {
    pub fn matches_transaction(
        &self,
        state_reader: Arc<dyn GrpcStateReader>,
        item: &CheckpointTransaction,
    ) -> bool {
        let _scope = monitored_scope("TransactionFilter::matches_transaction");
        let tx_data = item.transaction.data().transaction_data();

        match self {
            TransactionFilter::All(filters) => filters
                .iter()
                .all(|f| f.matches_transaction(state_reader.clone(), item)),

            TransactionFilter::Any(filters) => filters
                .iter()
                .any(|f| f.matches_transaction(state_reader.clone(), item)),

            TransactionFilter::Not(filter) => {
                !filter.matches_transaction(state_reader.clone(), item)
            }

            TransactionFilter::TransactionKind(kinds) => {
                let actual_kind = TransactionKind::from(tx_data.kind());
                kinds.iter().any(|kind| match kind {
                    TransactionKind::SystemTransaction => is_system_transaction(&actual_kind),
                    _ => kind == &actual_kind,
                })
            }

            TransactionFilter::Sender(a) => &tx_data.sender() == a,

            TransactionFilter::Receiver(a) => item
                .effects
                .mutated()
                .iter()
                .chain(item.effects.unwrapped().iter())
                .any(|(_, owner)| matches!(owner, Owner::AddressOwner(addr) if *addr == *a)),

            TransactionFilter::AffectedObject(o) => item
                .effects
                .all_affected_objects()
                .iter()
                .any(|obj_ref| &obj_ref.0 == o),

            TransactionFilter::MoveCall {
                package,
                module,
                function,
            } => tx_data.move_calls().into_iter().any(|(p, m, f)| {
                p == package
                    && (module.is_none() || matches!(module,  Some(m2) if m2 == &m.to_string()))
                    && (function.is_none() || matches!(function, Some(f2) if f2 == &f.to_string()))
            }),

            TransactionFilter::Events(event_filter) => item.events.as_ref().is_some_and(|evts| {
                evts.data
                    .iter()
                    .any(|event| event_filter.matches_event(state_reader.clone(), event))
            }),
        }
    }

    /// Validates that the filter depth doesn't exceed the maximum allowed depth
    /// to prevent DoS attacks through deeply nested structures.
    pub fn validate_depth(&self) -> Result<(), String> {
        self.validate_depth_recursive(0)
    }

    fn validate_depth_recursive(&self, current_depth: usize) -> Result<(), String> {
        if current_depth > MAX_FILTER_DEPTH {
            return Err(format!(
                "Filter depth exceeds maximum allowed depth of {}",
                MAX_FILTER_DEPTH
            ));
        }

        match self {
            TransactionFilter::All(filters) => {
                for filter in filters {
                    filter.validate_depth_recursive(current_depth + 1)?;
                }
            }
            TransactionFilter::Any(filters) => {
                for filter in filters {
                    filter.validate_depth_recursive(current_depth + 1)?;
                }
            }
            TransactionFilter::Not(filter) => {
                filter.validate_depth_recursive(current_depth + 1)?;
            }
            TransactionFilter::Events(event_filter) => {
                // Also validate the event filter depth
                event_filter.validate_depth_recursive(current_depth + 1)?;
            }
            // Atomic filters don't add to depth
            _ => {}
        }

        Ok(())
    }

    /// Returns the maximum depth of this filter tree
    pub fn max_depth(&self) -> usize {
        self.max_depth_recursive(0)
    }

    fn max_depth_recursive(&self, current_depth: usize) -> usize {
        match self {
            TransactionFilter::All(filters) | TransactionFilter::Any(filters) => filters
                .iter()
                .map(|f| f.max_depth_recursive(current_depth + 1))
                .max()
                .unwrap_or(current_depth),
            TransactionFilter::Not(filter) => filter.max_depth_recursive(current_depth + 1),
            TransactionFilter::Events(event_filter) => {
                event_filter.max_depth_recursive(current_depth + 1)
            }
            // Atomic filters
            _ => current_depth,
        }
    }

    /// Create a new filter with validation. This should be used when creating
    /// filters from external input (e.g., gRPC requests) to ensure safety.
    pub fn new_validated(filter: TransactionFilter) -> Result<Self, String> {
        filter.validate_depth()?;
        Ok(filter)
    }

    /// Validates the total complexity of the filter including counting the
    /// number of total filter nodes to prevent resource exhaustion.
    pub fn validate_complexity(&self) -> Result<(), String> {
        const MAX_FILTER_NODES: usize = 1000; // Maximum number of filter nodes

        let node_count = self.count_nodes();
        if node_count > MAX_FILTER_NODES {
            return Err(format!(
                "Filter complexity exceeds maximum allowed nodes: {} > {}",
                node_count, MAX_FILTER_NODES
            ));
        }

        self.validate_depth()
    }

    fn count_nodes(&self) -> usize {
        match self {
            TransactionFilter::All(filters) | TransactionFilter::Any(filters) => {
                1 + filters.iter().map(|f| f.count_nodes()).sum::<usize>()
            }
            TransactionFilter::Not(filter) => 1 + filter.count_nodes(),
            TransactionFilter::Events(event_filter) => 1 + event_filter.count_nodes(),
            // Atomic filters count as 1 node
            _ => 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use iota_types::base_types::ObjectID;

    use super::*;

    #[test]
    fn test_filter_depth_validation() {
        // Simple atomic filter should pass
        let simple_filter = TransactionFilter::Sender(IotaAddress::random_for_testing_only());
        assert!(simple_filter.validate_depth().is_ok());
        assert_eq!(simple_filter.max_depth(), 0);

        // Nested filter within limits should pass
        let nested_filter = TransactionFilter::All(vec![
            TransactionFilter::Sender(IotaAddress::random_for_testing_only()),
            TransactionFilter::Any(vec![
                TransactionFilter::AffectedObject(ObjectID::random()),
                TransactionFilter::Not(Box::new(TransactionFilter::AffectedObject(
                    ObjectID::random(),
                ))),
            ]),
        ]);
        assert!(nested_filter.validate_depth().is_ok());
        assert_eq!(nested_filter.max_depth(), 3); // All -> Any -> Not = 3 levels

        // Deeply nested filter should fail
        let mut deep_filter = TransactionFilter::Sender(IotaAddress::random_for_testing_only());
        for _ in 0..=MAX_FILTER_DEPTH {
            deep_filter = TransactionFilter::Not(Box::new(deep_filter));
        }
        assert!(deep_filter.validate_depth().is_err());
        assert!(deep_filter.max_depth() > MAX_FILTER_DEPTH);
    }

    #[test]
    fn test_filter_complexity_validation() {
        // Simple filter should pass complexity validation
        let simple_filter = TransactionFilter::Sender(IotaAddress::random_for_testing_only());
        assert!(simple_filter.validate_complexity().is_ok());
        assert_eq!(simple_filter.count_nodes(), 1);

        // Moderately complex filter should pass
        let complex_filter = TransactionFilter::All(vec![
            TransactionFilter::Sender(IotaAddress::random_for_testing_only()),
            TransactionFilter::Any(vec![
                TransactionFilter::Receiver(IotaAddress::random_for_testing_only()),
                TransactionFilter::AffectedObject(ObjectID::random()),
            ]),
        ]);
        assert!(complex_filter.validate_complexity().is_ok());
        assert_eq!(complex_filter.count_nodes(), 5); // All + Sender + Any + Receiver + AffectedObject = 5 nodes
    }

    #[test]
    fn test_new_validated() {
        let valid_filter = TransactionFilter::Sender(IotaAddress::random_for_testing_only());
        assert!(TransactionFilter::new_validated(valid_filter).is_ok());

        // Create an invalid deeply nested filter
        let mut invalid_filter = TransactionFilter::Sender(IotaAddress::random_for_testing_only());
        for _ in 0..=MAX_FILTER_DEPTH {
            invalid_filter = TransactionFilter::Not(Box::new(invalid_filter));
        }
        assert!(TransactionFilter::new_validated(invalid_filter).is_err());
    }

    #[test]
    fn test_empty_logical_filters() {
        // Empty All filter should pass validation
        let empty_all = TransactionFilter::All(vec![]);
        assert!(empty_all.validate_depth().is_ok());
        assert_eq!(empty_all.max_depth(), 0);

        // Empty Any filter should pass validation
        let empty_any = TransactionFilter::Any(vec![]);
        assert!(empty_any.validate_depth().is_ok());
        assert_eq!(empty_any.max_depth(), 0);
    }

    #[test]
    fn test_complex_nested_structure() {
        // Create a complex but valid nested structure
        let complex_filter = TransactionFilter::All(vec![
            TransactionFilter::Any(vec![
                TransactionFilter::Sender(IotaAddress::random_for_testing_only()),
                TransactionFilter::Receiver(IotaAddress::random_for_testing_only()),
            ]),
            TransactionFilter::Not(Box::new(TransactionFilter::All(vec![
                TransactionFilter::Sender(IotaAddress::random_for_testing_only()),
                TransactionFilter::AffectedObject(ObjectID::random()),
            ]))),
        ]);

        assert!(complex_filter.validate_depth().is_ok());
        assert_eq!(complex_filter.max_depth(), 3);
    }
}
