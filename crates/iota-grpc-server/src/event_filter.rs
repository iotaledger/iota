// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_metrics::monitored_scope;
use iota_types::{
    base_types::{IotaAddress, ObjectID},
    event::Event,
};
use move_core_types::{identifier::Identifier, language_storage::StructTag};
use serde::{Deserialize, Serialize};

const MAX_FILTER_DEPTH: usize = 10;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum EventFilter {
    // Logical AND of several filters.
    All(Vec<EventFilter>),
    // Logical OR of several filters.
    Any(Vec<EventFilter>),
    // Logical NOT of a filter.
    Not(Box<EventFilter>),

    /// Filter by sender address.
    Sender(IotaAddress),

    /// Return events emitted in a specified Move package + module (optional).
    /// If the event is defined in PackageA::ModuleA but emitted in a tx with
    /// PackageB::ModuleB, filtering `MovePackageAndModule` by PackageB::ModuleB
    /// returns the event. Filtering `MoveEventPackageAndModule` by
    /// PackageA::ModuleA returns the event too.
    MovePackageAndModule {
        /// the Move package ID
        package: ObjectID,
        /// the module name (optional)
        module: Option<Identifier>,
    },
    /// Return events with the given Move package + module (optional) where the
    /// event struct is defined. If the event is defined in
    /// PackageA::ModuleA but emitted in a tx with PackageB::ModuleB, filtering
    /// `MoveEventPackageAndModule` by PackageA::ModuleA returns the
    /// event. Filtering `MovePackageAndModule` by PackageB::ModuleB returns the
    /// event too.
    MoveEventPackageAndModule {
        /// the Move package ID
        package: ObjectID,
        /// the module name (optional)
        module: Option<Identifier>,
    },
    /// Return events with the given Move event struct name (struct tag).
    /// For example, if the event is defined in `0xabcd::MyModule`, and named
    /// `Foo`, then the struct tag is `0xabcd::MyModule::Foo`.
    MoveEventType(StructTag),
}

// Proto-to-internal filter conversion
impl TryFrom<iota_grpc_types::v1::filter::EventFilter> for EventFilter {
    type Error = String;

    fn try_from(proto: iota_grpc_types::v1::filter::EventFilter) -> Result<Self, Self::Error> {
        use iota_grpc_types::v1::filter::event_filter::Filter as ProtoFilter;

        let filter = proto.filter.ok_or("event filter is missing")?;

        match filter {
            ProtoFilter::All(all) => {
                let filters = all
                    .filters
                    .into_iter()
                    .map(EventFilter::try_from)
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(EventFilter::All(filters))
            }
            ProtoFilter::Any(any) => {
                let filters = any
                    .filters
                    .into_iter()
                    .map(EventFilter::try_from)
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(EventFilter::Any(filters))
            }
            ProtoFilter::Negation(not) => {
                let inner = not.filter.ok_or("negation filter is missing")?;
                Ok(EventFilter::Not(Box::new(EventFilter::try_from(*inner)?)))
            }
            ProtoFilter::Sender(addr_filter) => {
                let address = addr_filter
                    .address
                    .ok_or("sender address is missing")?
                    .address;
                let iota_address = IotaAddress::from_bytes(&address)
                    .map_err(|e| format!("invalid sender address: {}", e))?;
                Ok(EventFilter::Sender(iota_address))
            }
            ProtoFilter::MovePackageAndModule(filter) => {
                // TODO: add a function to parse the package and the module name
                let package_bytes = filter.package_id.ok_or("package_id is missing")?.object_id;
                let package = ObjectID::from_bytes(&package_bytes)
                    .map_err(|e| format!("invalid package_id: {}", e))?;
                let module = filter
                    .module
                    .map(|m| {
                        Identifier::new(m.as_str())
                            .map_err(|e| format!("invalid module name: {}", e))
                    })
                    .transpose()?;
                Ok(EventFilter::MovePackageAndModule { package, module })
            }
            ProtoFilter::MoveEventPackageAndModule(filter) => {
                // TODO: add a function to parse the package and the module name
                let package_bytes = filter.package_id.ok_or("package_id is missing")?.object_id;
                let package = ObjectID::from_bytes(&package_bytes)
                    .map_err(|e| format!("invalid package_id: {}", e))?;
                let module = filter
                    .module
                    .map(|m| {
                        Identifier::new(m.as_str())
                            .map_err(|e| format!("invalid module name: {}", e))
                    })
                    .transpose()?;
                Ok(EventFilter::MoveEventPackageAndModule { package, module })
            }
            ProtoFilter::MoveEventType(filter) => {
                let tag: StructTag = filter
                    .struct_tag
                    .parse()
                    .map_err(|e| format!("invalid struct tag: {}", e))?;
                Ok(EventFilter::MoveEventType(tag))
            }
            _ => Err("Unsupported event filter type".to_string()),
        }
    }
}

impl EventFilter {
    pub fn matches_event(&self, item: &Event) -> bool {
        let _scope = monitored_scope("EventFilter::matches_event");

        match self {
            EventFilter::All(filters) => filters.iter().all(|f| f.matches_event(item)),
            EventFilter::Any(filters) => filters.iter().any(|f| f.matches_event(item)),
            EventFilter::Not(filter) => !filter.matches_event(item),

            EventFilter::Sender(sender) => item.sender == *sender,

            EventFilter::MovePackageAndModule { package, module } => {
                item.package_id == *package
                    && (module.is_none()
                        || matches!(module,  Some(m2) if m2 == &item.transaction_module))
            }
            EventFilter::MoveEventPackageAndModule { package, module } => {
                ObjectID::from(item.type_.address) == *package
                    && (module.is_none() || matches!(module,  Some(m2) if m2 == &item.type_.module))
            }
            EventFilter::MoveEventType(event_type) => item.type_ == *event_type,
        }
    }

    pub fn and(self, other_filter: EventFilter) -> Self {
        Self::All(vec![self, other_filter])
    }
    pub fn or(self, other_filter: EventFilter) -> Self {
        Self::Any(vec![self, other_filter])
    }

    /// Validates that the filter depth doesn't exceed the maximum allowed depth
    /// to prevent DoS attacks through deeply nested structures.
    pub fn validate_depth(&self) -> Result<(), String> {
        self.validate_depth_recursive(0)
    }

    pub(crate) fn validate_depth_recursive(&self, current_depth: usize) -> Result<(), String> {
        if current_depth > MAX_FILTER_DEPTH {
            return Err(format!(
                "Event filter depth exceeds maximum allowed depth of {}",
                MAX_FILTER_DEPTH
            ));
        }

        match self {
            EventFilter::All(filters) => {
                for filter in filters {
                    filter.validate_depth_recursive(current_depth + 1)?;
                }
            }
            EventFilter::Any(filters) => {
                for filter in filters {
                    filter.validate_depth_recursive(current_depth + 1)?;
                }
            }
            EventFilter::Not(filter) => {
                filter.validate_depth_recursive(current_depth + 1)?;
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

    pub(crate) fn max_depth_recursive(&self, current_depth: usize) -> usize {
        match self {
            EventFilter::All(filters) | EventFilter::Any(filters) => filters
                .iter()
                .map(|f| f.max_depth_recursive(current_depth + 1))
                .max()
                .unwrap_or(current_depth),
            EventFilter::Not(filter) => filter.max_depth_recursive(current_depth + 1),
            // Atomic filters
            _ => current_depth,
        }
    }

    /// Create a new filter with validation. This should be used when creating
    /// filters from external input (e.g., gRPC requests) to ensure safety.
    pub fn new_validated(filter: EventFilter) -> Result<Self, String> {
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
                "Event filter complexity exceeds maximum allowed nodes: {} > {}",
                node_count, MAX_FILTER_NODES
            ));
        }

        self.validate_depth()
    }

    pub(crate) fn count_nodes(&self) -> usize {
        match self {
            EventFilter::All(filters) | EventFilter::Any(filters) => {
                1 + filters.iter().map(|f| f.count_nodes()).sum::<usize>()
            }
            EventFilter::Not(filter) => 1 + filter.count_nodes(),
            // Atomic filters count as 1 node
            _ => 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_filter_depth_validation() {
        // Simple atomic filter should pass
        let simple_filter = EventFilter::Sender(IotaAddress::random_for_testing_only());
        assert!(simple_filter.validate_depth().is_ok());
        assert_eq!(simple_filter.max_depth(), 0);

        // Nested filter within limits should pass
        let nested_filter = EventFilter::All(vec![
            EventFilter::Sender(IotaAddress::random_for_testing_only()),
            EventFilter::Any(vec![
                EventFilter::MovePackageAndModule {
                    package: ObjectID::random(),
                    module: Some(Identifier::new("MyModule").unwrap()),
                },
                EventFilter::Not(Box::new(EventFilter::MoveEventType(StructTag {
                    address: ObjectID::random().into(),
                    module: Identifier::new("MyModule").unwrap(),
                    name: Identifier::new("MyEvent").unwrap(),
                    type_params: vec![],
                }))),
            ]),
        ]);
        assert!(nested_filter.validate_depth().is_ok());
        assert_eq!(nested_filter.max_depth(), 3); // All -> Any -> Not = 3 levels

        // Deeply nested filter should fail
        let mut deep_filter = EventFilter::Sender(IotaAddress::random_for_testing_only());
        for _ in 0..=10 {
            // MAX_FILTER_DEPTH
            deep_filter = EventFilter::Not(Box::new(deep_filter));
        }
        assert!(deep_filter.validate_depth().is_err());
        assert!(deep_filter.max_depth() > 10);
    }

    #[test]
    fn test_event_filter_empty_logical_filters() {
        // Empty All filter should pass validation
        let empty_all = EventFilter::All(vec![]);
        assert!(empty_all.validate_depth().is_ok());
        assert_eq!(empty_all.max_depth(), 0);

        // Empty Any filter should pass validation
        let empty_any = EventFilter::Any(vec![]);
        assert!(empty_any.validate_depth().is_ok());
        assert_eq!(empty_any.max_depth(), 0);
    }
}
