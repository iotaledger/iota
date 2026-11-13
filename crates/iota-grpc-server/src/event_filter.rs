// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_metrics::monitored_scope;
use iota_sdk_types::{Address, Event, Identifier, ObjectId, StructTag};
use serde::{Deserialize, Serialize};

use crate::{
    constants::count_filters,
    utils::{parse_filter_identifier, parse_filter_struct_tag},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum EventFilter {
    // Logical AND of several filters.
    All(Vec<EventFilter>),
    // Logical OR of several filters.
    Any(Vec<EventFilter>),
    // Logical NOT of a filter.
    Not(Box<EventFilter>),

    /// Filter by sender address.
    Sender(Address),

    /// Return events emitted in a specified Move package + module (optional).
    /// If the event is defined in PackageA::ModuleA but emitted in a tx with
    /// PackageB::ModuleB, filtering `MovePackageAndModule` by PackageB::ModuleB
    /// returns the event. Filtering `MoveEventPackageAndModule` by
    /// PackageA::ModuleA returns the event too.
    MovePackageAndModule {
        /// the Move package ID
        package: ObjectId,
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
        package: ObjectId,
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

    /// Fails without converting the rest of the filter as soon as it is
    /// deeper than [`MAX_FILTER_DEPTH`] or has more than [`MAX_FILTER_NODES`]
    /// filters.
    ///
    /// [`MAX_FILTER_DEPTH`]: crate::constants::MAX_FILTER_DEPTH
    /// [`MAX_FILTER_NODES`]: crate::constants::MAX_FILTER_NODES
    fn try_from(proto: iota_grpc_types::v1::filter::EventFilter) -> Result<Self, Self::Error> {
        Self::from_proto(proto, 0, &mut 0)
    }
}

impl EventFilter {
    /// Converts `proto`, which is nested `depth` levels deep. `nodes` counts
    /// the filters converted so far, as described for [`MAX_FILTER_NODES`].
    ///
    /// [`MAX_FILTER_NODES`]: crate::constants::MAX_FILTER_NODES
    pub(crate) fn from_proto(
        proto: iota_grpc_types::v1::filter::EventFilter,
        depth: usize,
        nodes: &mut usize,
    ) -> Result<Self, String> {
        use iota_grpc_types::v1::filter::event_filter::Filter as ProtoFilter;

        count_filters(depth, nodes, 1)?;

        let filter = proto.filter.ok_or("event filter is missing")?;

        match filter {
            ProtoFilter::All(all) => {
                let filters = all
                    .filters
                    .into_iter()
                    .map(|f| EventFilter::from_proto(f, depth + 1, nodes))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(EventFilter::All(filters))
            }
            ProtoFilter::Any(any) => {
                let filters = any
                    .filters
                    .into_iter()
                    .map(|f| EventFilter::from_proto(f, depth + 1, nodes))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(EventFilter::Any(filters))
            }
            ProtoFilter::Negation(not) => {
                let inner = not.filter.ok_or("negation filter is missing")?;
                Ok(EventFilter::Not(Box::new(EventFilter::from_proto(
                    *inner,
                    depth + 1,
                    nodes,
                )?)))
            }
            ProtoFilter::Sender(addr_filter) => {
                let address = addr_filter
                    .address
                    .ok_or("sender address is missing")?
                    .address;
                let iota_address = Address::from_bytes(&address)
                    .map_err(|e| format!("invalid sender address: {e}"))?;
                Ok(EventFilter::Sender(iota_address))
            }
            ProtoFilter::MovePackageAndModule(filter) => {
                // TODO: add a function to parse the package and the module name
                let package_bytes = filter.package_id.ok_or("package_id is missing")?.object_id;
                let package = ObjectId::from_bytes(&package_bytes)
                    .map_err(|e| format!("invalid package_id: {e}"))?;
                let module = parse_filter_identifier(filter.module, "module")?;
                Ok(EventFilter::MovePackageAndModule { package, module })
            }
            ProtoFilter::MoveEventPackageAndModule(filter) => {
                // TODO: add a function to parse the package and the module name
                let package_bytes = filter.package_id.ok_or("package_id is missing")?.object_id;
                let package = ObjectId::from_bytes(&package_bytes)
                    .map_err(|e| format!("invalid package_id: {e}"))?;
                let module = parse_filter_identifier(filter.module, "module")?;
                Ok(EventFilter::MoveEventPackageAndModule { package, module })
            }
            ProtoFilter::MoveEventType(filter) => Ok(EventFilter::MoveEventType(
                parse_filter_struct_tag(&filter.struct_tag)?,
            )),
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
                    && (module.is_none() || matches!(module,  Some(m2) if m2 == &item.module))
            }
            EventFilter::MoveEventPackageAndModule { package, module } => {
                &item.struct_tag.address() == package.as_address()
                    && (module.is_none()
                        || matches!(module, Some(m2) if m2 == item.struct_tag.module()))
            }
            EventFilter::MoveEventType(event_type) => item.struct_tag == *event_type,
        }
    }

    pub fn and(self, other_filter: EventFilter) -> Self {
        Self::All(vec![self, other_filter])
    }
    pub fn or(self, other_filter: EventFilter) -> Self {
        Self::Any(vec![self, other_filter])
    }
}

#[cfg(test)]
mod tests {
    use iota_grpc_types::v1::filter::{self as proto_filter, event_filter::Filter as ProtoFilter};
    use prost::Message;

    use super::*;
    use crate::constants::{MAX_FILTER_DEPTH, MAX_FILTER_NODES};

    fn proto(filter: ProtoFilter) -> proto_filter::EventFilter {
        let mut proto = proto_filter::EventFilter::default();
        proto.filter = Some(filter);
        proto
    }

    fn proto_event_type() -> proto_filter::EventFilter {
        let mut event_type = proto_filter::MoveEventTypeFilter::default();
        event_type.struct_tag = "0x2::module::Event".to_string();
        proto(ProtoFilter::MoveEventType(event_type))
    }

    fn proto_not(inner: proto_filter::EventFilter, levels: usize) -> proto_filter::EventFilter {
        (0..levels).fold(inner, |inner, _| {
            let mut not = proto_filter::NotEventFilter::default();
            not.filter = Some(Box::new(inner));
            proto(ProtoFilter::Negation(Box::new(not)))
        })
    }

    fn proto_all(filters: Vec<proto_filter::EventFilter>) -> proto_filter::EventFilter {
        let mut all = proto_filter::AllEventFilter::default();
        all.filters = filters;
        proto(ProtoFilter::All(all))
    }

    fn proto_any(filters: Vec<proto_filter::EventFilter>) -> proto_filter::EventFilter {
        let mut any = proto_filter::AnyEventFilter::default();
        any.filters = filters;
        proto(ProtoFilter::Any(any))
    }

    #[test]
    fn proto_conversion_rejects_filters_deeper_than_the_limit() {
        assert!(EventFilter::try_from(proto_not(proto_event_type(), MAX_FILTER_DEPTH)).is_ok());

        let err =
            EventFilter::try_from(proto_not(proto_event_type(), MAX_FILTER_DEPTH + 1)).unwrap_err();
        assert!(err.contains("levels"), "{err}");

        assert!(EventFilter::try_from(proto_not(proto_event_type(), 1_000)).is_err());
    }

    #[test]
    fn proto_conversion_counts_all_and_any_as_levels() {
        let wraps: [fn(Vec<proto_filter::EventFilter>) -> proto_filter::EventFilter; 2] =
            [proto_all, proto_any];
        for wrap in wraps {
            let nested = |levels| (0..levels).fold(proto_event_type(), |f, _| wrap(vec![f]));
            assert!(EventFilter::try_from(nested(MAX_FILTER_DEPTH)).is_ok());
            assert!(EventFilter::try_from(nested(MAX_FILTER_DEPTH + 1)).is_err());
        }
    }

    #[test]
    fn proto_conversion_rejects_filters_with_too_many_nodes() {
        assert!(
            EventFilter::try_from(proto_all(vec![proto_event_type(); MAX_FILTER_NODES - 1]))
                .is_ok()
        );
        assert!(EventFilter::try_from(proto_all(vec![])).is_ok());
        assert!(EventFilter::try_from(proto_any(vec![])).is_ok());

        let err = EventFilter::try_from(proto_all(vec![proto_event_type(); MAX_FILTER_NODES]))
            .unwrap_err();
        assert!(err.contains("nodes"), "{err}");
    }

    fn struct_tag_error(struct_tag: String) -> String {
        let mut event_type = proto_filter::MoveEventTypeFilter::default();
        event_type.struct_tag = struct_tag;
        EventFilter::try_from(proto(ProtoFilter::MoveEventType(event_type))).unwrap_err()
    }

    #[test]
    fn proto_conversion_rejects_invalid_struct_tags_with_the_reason() {
        let err = struct_tag_error("!".repeat(1_000_000));
        assert!(err.starts_with("invalid struct tag"), "{err}");
        assert!(err.len() < 200, "{} bytes", err.len());

        let address = format!("0x{}", "ab".repeat(32));
        let struct_tag = format!("{address}::pool::Pool<{address}::a::A, {address}::b::B>>");
        let err = struct_tag_error(struct_tag.clone());
        assert!(
            err.ends_with(&format!("at byte {}: \">\"", struct_tag.len() - 1)),
            "{err}"
        );

        let nested = (0..20).fold("0x2::a::A".to_string(), |inner, _| {
            format!("0x2::a::A<{inner}>")
        });
        let err = struct_tag_error(nested);
        assert!(err.contains("nesting exceeded"), "{err}");

        for struct_tag in [
            "0x2::<SELF>::Foo",
            "0x2::coin::<SELF>",
            "0x2::coin::Coin<0x2::<SELF>::A>",
        ] {
            let err = struct_tag_error(struct_tag.to_string());
            assert!(err.starts_with("invalid struct tag"), "{err}");
        }
    }

    #[test]
    fn proto_conversion_rejects_self_as_module_name() {
        let mut package_id = iota_grpc_types::v1::types::ObjectId::default();
        package_id.object_id = ObjectId::random().as_bytes().to_vec().into();
        let mut package_and_module = proto_filter::MovePackageAndModuleFilter::default();
        package_and_module.package_id = Some(package_id);
        package_and_module.module = Some("<SELF>".to_string());
        let err =
            EventFilter::try_from(proto(ProtoFilter::MovePackageAndModule(package_and_module)))
                .unwrap_err();
        assert!(err.contains("invalid module name"), "{err}");
    }

    #[test]
    fn proto_decoding_rejects_deeply_nested_filters() {
        let bytes = proto_not(proto_event_type(), 1_000).encode_to_vec();
        assert!(proto_filter::EventFilter::decode(bytes.as_slice()).is_err());
    }
}
