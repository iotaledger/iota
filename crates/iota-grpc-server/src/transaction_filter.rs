// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_json_rpc_types::{
    Filter, IotaTransactionBlockEffects, IotaTransactionBlockEffectsAPI,
    IotaTransactionBlockEvents, IotaTransactionKind, OwnedObjectRef,
};
use iota_metrics::monitored_scope;
use iota_types::{
    base_types::{IotaAddress, ObjectID},
    object::Owner,
    transaction::{TransactionData, TransactionDataAPI},
};
use serde::{Deserialize, Serialize};

use crate::event_filter::EventFilter;

/// Maximum allowed depth for nested filters to prevent DoS attacks
const MAX_FILTER_DEPTH: usize = 10;

#[derive(Clone)]
pub struct TransactionDataWithEffectsAndEvents {
    pub tx_data: TransactionData,
    pub effects: IotaTransactionBlockEffects,
    pub events: IotaTransactionBlockEvents,
}

impl From<TransactionDataWithEffectsAndEvents> for IotaTransactionBlockEffects {
    fn from(e: TransactionDataWithEffectsAndEvents) -> Self {
        e.effects
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
    TransactionKind(Vec<IotaTransactionKind>),

    /// Filter by sender address.
    Sender(IotaAddress),
    /// Filter by recipient address. The recipient is determined by
    /// checking the owners of mutated and unwrapped objects.
    Receiver(IotaAddress),

    /// Filter by input object.
    InputObject(ObjectID),
    /// Filter by changed object, including created, mutated and unwrapped
    /// objects.
    ChangedObject(ObjectID),
    /// Filter transactions that wrapped or deleted the specified object.
    /// Includes transactions that either created and immediately wrapped
    /// the object or unwrapped and immediately deleted it.
    /// TODO: @infra: do we need that now that we have the AffectedObject
    /// filter?
    WrappedOrDeletedObject(ObjectID),
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

impl TransactionFilter {
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

impl Filter<TransactionDataWithEffectsAndEvents> for TransactionFilter {
    fn matches(&self, item: &TransactionDataWithEffectsAndEvents) -> bool {
        let _scope = monitored_scope("TransactionFilter::matches");
        match self {
            TransactionFilter::All(filters) => filters.iter().all(|f| f.matches(item)),
            TransactionFilter::Any(filters) => filters.iter().any(|f| f.matches(item)),
            TransactionFilter::Not(filter) => !filter.matches(item),

            TransactionFilter::TransactionKind(kinds) => kinds
                .iter()
                .any(|kind| kind == &IotaTransactionKind::from(item.tx_data.kind())),

            TransactionFilter::Sender(a) => &item.tx_data.sender() == a,
            TransactionFilter::Receiver(a) => {
                let mutated: &[OwnedObjectRef] = item.effects.mutated();
                mutated.iter().chain(item.effects.unwrapped().iter()).any(|oref: &OwnedObjectRef| {
                    matches!(oref.owner, Owner::AddressOwner(owner) if owner == *a)
                })
            }

            TransactionFilter::InputObject(o) => {
                let Ok(input_objects) = item.tx_data.input_objects() else {
                    return false;
                };
                input_objects.iter().any(|object| object.object_id() == *o)
            }
            TransactionFilter::ChangedObject(o) => item
                .effects
                .mutated()
                .iter()
                .any(|oref: &OwnedObjectRef| &oref.reference.object_id == o),
            TransactionFilter::WrappedOrDeletedObject(o) => item
                .effects
                .wrapped()
                .iter()
                .chain(item.effects.deleted().iter())
                .chain(item.effects.unwrapped_then_deleted().iter())
                .any(|oref| &oref.object_id == o),
            TransactionFilter::AffectedObject(o) => item
                .effects
                .created()
                .iter()
                .chain(item.effects.mutated().iter())
                .chain(item.effects.unwrapped().iter())
                .map(|oref: &OwnedObjectRef| &oref.reference)
                .chain(item.effects.shared_objects().iter())
                .chain(item.effects.deleted().iter())
                .chain(item.effects.unwrapped_then_deleted().iter())
                .chain(item.effects.wrapped().iter())
                .any(|oref| &oref.object_id == o),

            TransactionFilter::MoveCall {
                package,
                module,
                function,
            } => item.tx_data.move_calls().into_iter().any(|(p, m, f)| {
                p == package
                    && (module.is_none() || matches!(module,  Some(m2) if m2 == &m.to_string()))
                    && (function.is_none() || matches!(function, Some(f2) if f2 == &f.to_string()))
            }),

            TransactionFilter::Events(event_filter) => item
                .events
                .data
                .iter()
                .any(|event| event_filter.matches(event)),
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
                TransactionFilter::InputObject(ObjectID::random()),
                TransactionFilter::Not(Box::new(TransactionFilter::ChangedObject(
                    ObjectID::random(),
                ))),
            ]),
        ]);
        assert!(nested_filter.validate_depth().is_ok());
        assert_eq!(nested_filter.max_depth(), 2);

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
                TransactionFilter::InputObject(ObjectID::random()),
                TransactionFilter::ChangedObject(ObjectID::random()),
            ]),
        ]);
        assert!(complex_filter.validate_complexity().is_ok());
        assert_eq!(complex_filter.count_nodes(), 4); // All + Sender + Any + InputObject + ChangedObject = 5 nodes
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
                TransactionFilter::InputObject(ObjectID::random()),
                TransactionFilter::ChangedObject(ObjectID::random()),
            ]))),
        ]);

        assert!(complex_filter.validate_depth().is_ok());
        assert_eq!(complex_filter.max_depth(), 3);
    }
}
