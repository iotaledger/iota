// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::{BTreeMap, BTreeSet};

use itertools::{Either, Itertools};
use petgraph::{Graph, algo::has_path_connecting, graph::NodeIndex};
use prost_types::{
    DescriptorProto, EnumDescriptorProto, EnumValueDescriptorProto, FieldDescriptorProto,
    FileDescriptorProto, OneofDescriptorProto, ServiceDescriptorProto, SourceCodeInfo,
    field_descriptor_proto::{Label, Type},
};

use super::comments::Comments;
use crate::ident::{to_snake, to_upper_camel};

/// `MessageGraph` builds a graph of messages whose edges correspond to nesting.
/// The goal is to recognize when message types are recursively nested, so
/// that fields can be boxed when necessary.
pub struct DescriptorGraph {
    index: BTreeMap<String, NodeIndex>,
    graph: Graph<String, ()>,

    pub packages: BTreeSet<String>,
    pub messages: BTreeMap<String, Message>,
    enums: BTreeMap<String, Enum>,
    services: BTreeMap<String, Service>,
}

impl DescriptorGraph {
    pub(crate) fn new<'a>(files: impl Iterator<Item = &'a FileDescriptorProto>) -> DescriptorGraph {
        let mut graph = DescriptorGraph {
            index: BTreeMap::new(),
            graph: Graph::new(),
            messages: BTreeMap::new(),
            enums: BTreeMap::new(),
            services: BTreeMap::new(),
            packages: BTreeSet::new(),
        };

        for file in files {
            let package = format!(
                "{}{}",
                if file.package.is_some() { "." } else { "" },
                file.package.as_deref().unwrap_or("")
            );
            for msg in &file.message_type {
                graph.add_message_edges(&package, msg);
            }

            graph.packages.insert(package);
            FileParser::parse(&mut graph, file.clone());
        }

        graph
    }

    fn get_or_insert_index(&mut self, msg_name: String) -> NodeIndex {
        assert_eq!(b'.', msg_name.as_bytes()[0]);
        *self
            .index
            .entry(msg_name.clone())
            .or_insert_with(|| self.graph.add_node(msg_name))
    }

    /// Adds message to graph IFF it contains a non-repeated field containing
    /// another message. The purpose of the message graph is detecting
    /// recursively nested messages and co-recursively nested messages.
    /// Because prost does not box message fields, recursively nested messages
    /// would not compile in Rust. To allow recursive messages, the message
    /// graph is used to detect recursion and automatically box the recursive
    /// field. Since repeated messages are already put in a Vec, boxing them
    /// isn’t necessary even if the reference is recursive.
    fn add_message_edges(&mut self, package: &str, msg: &DescriptorProto) {
        let msg_name = format!("{}.{}", package, msg.name.as_ref().unwrap());
        let msg_index = self.get_or_insert_index(msg_name.clone());

        for field in &msg.field {
            if field.r#type() == Type::Message && field.label() != Label::Repeated {
                let field_index = self.get_or_insert_index(field.type_name.clone().unwrap());
                self.graph.add_edge(msg_index, field_index, ());
            }
        }

        for msg in &msg.nested_type {
            self.add_message_edges(&msg_name, msg);
        }
    }

    /// Returns true if message type `inner` is nested in message type `outer`.
    #[allow(unused)]
    pub fn is_nested(&self, outer: &str, inner: &str) -> bool {
        let outer = match self.index.get(outer) {
            Some(outer) => *outer,
            None => return false,
        };
        let inner = match self.index.get(inner) {
            Some(inner) => *inner,
            None => return false,
        };

        has_path_connecting(&self.graph, outer, inner, None)
    }
}

struct FileParser<'a> {
    graph: &'a mut DescriptorGraph,
    package: String,
    type_path: Vec<String>,
    source_info: Option<SourceCodeInfo>,
    path: Vec<i32>,
}

impl<'a> FileParser<'a> {
    fn parse(graph: &'a mut DescriptorGraph, file: FileDescriptorProto) {
        let source_info = file.source_code_info.map(|mut s| {
            s.location.retain(|loc| {
                let len = loc.path.len();
                len > 0 && len % 2 == 0
            });
            s.location.sort_by(|a, b| a.path.cmp(&b.path));
            s
        });

        let package = format!(
            "{}{}",
            if file.package.is_some() { "." } else { "" },
            file.package.as_deref().unwrap_or("")
        );

        let mut parser = Self {
            graph,
            package,
            type_path: Vec::new(),
            path: Vec::new(),
            source_info,
        };

        // Messages
        parser.path.push(4);
        for (idx, message) in file.message_type.into_iter().enumerate() {
            parser.path.push(idx as i32);
            parser.process_message(message);
            parser.path.pop();
        }
        parser.path.pop();

        // Enums
        parser.path.push(5);
        for (idx, desc) in file.enum_type.into_iter().enumerate() {
            parser.path.push(idx as i32);
            parser.process_enum(desc);
            parser.path.pop();
        }
        parser.path.pop();

        // Services
        parser.path.push(6);
        for (idx, service) in file.service.into_iter().enumerate() {
            parser.path.push(idx as i32);
            parser.process_service(service);
            parser.path.pop();
        }

        parser.path.pop();
    }

    fn process_service(&mut self, service: ServiceDescriptorProto) {
        let name = service.name().to_owned();
        // debug!("  service: {:?}", name);
        let comments = self.comments();
        self.path.push(2);
        let methods = service
            .method
            .into_iter()
            .enumerate()
            .map(|(idx, mut method)| {
                // debug!("  method: {:?}", method.name());

                self.path.push(idx as i32);
                let comments = self.comments();
                self.path.pop();

                let name = method.name.take().unwrap();
                let input_proto_type = method.input_type.take().unwrap();
                let output_proto_type = method.output_type.take().unwrap();
                // TODO FIX
                // let input_type = self.resolve_ident(&input_proto_type);
                // let output_type = self.resolve_ident(&output_proto_type);
                let input_type = String::new();
                let output_type = String::new();
                let client_streaming = method.client_streaming();
                let server_streaming = method.server_streaming();

                Method {
                    name: to_snake(&name),
                    proto_name: name,
                    comments,
                    input_type,
                    output_type,
                    input_proto_type,
                    output_proto_type,
                    options: method.options.unwrap_or_default(),
                    client_streaming,
                    server_streaming,
                }
            })
            .collect();
        self.path.pop();

        let service = Service {
            name: to_upper_camel(&name),
            proto_name: name,
            package: self.package.clone(),
            comments,
            methods,
            options: service.options.unwrap_or_default(),
        };
        let full_name = format!("{}{}", service.package, service.proto_name);
        self.graph.services.insert(full_name, service);
    }

    fn process_message(&mut self, descriptor: DescriptorProto) {
        let type_name = self.fq_name(descriptor.name());

        // Split the nested message types into a vector of normal nested message types,
        // and a map of the map field entry types. The path index of the nested
        // message types is preserved so that comments can be retrieved.
        type NestedTypes = Vec<(DescriptorProto, usize)>;
        type MapTypes = BTreeMap<String, (FieldDescriptorProto, FieldDescriptorProto)>;
        let (nested_types, map_types): (NestedTypes, MapTypes) = descriptor
            .nested_type
            .iter()
            .enumerate()
            .partition_map(|(idx, nested_type)| {
                if nested_type
                    .options
                    .as_ref()
                    .and_then(|options| options.map_entry)
                    .unwrap_or(false)
                {
                    let key = nested_type.field[0].clone();
                    let value = nested_type.field[1].clone();
                    assert_eq!("key", key.name());
                    assert_eq!("value", value.name());

                    let name = format!("{}.{}", &type_name, nested_type.name());
                    Either::Right((name, (key, value)))
                } else {
                    Either::Left((nested_type.clone(), idx))
                }
            });

        // Split the fields into a vector of the normal fields, and oneof fields.
        // Path indexes are preserved so that comments can be retrieved.
        let mut fields: Vec<Field> = Vec::new();
        let mut oneof_map: BTreeMap<i32, Vec<Field>> = Default::default();
        self.path.push(2);
        for (idx, proto) in descriptor.field.iter().enumerate() {
            self.path.push(idx as i32);
            let comments = self.comments();
            let map = map_types.get(proto.type_name()).cloned();

            if let Some(oneof_index) = proto.oneof_index {
                if !proto.proto3_optional() {
                    // oneof
                    oneof_map.entry(oneof_index).or_default().push(Field {
                        inner: proto.clone(),
                        map,
                        comments,
                    });
                } else {
                    // normal field
                    fields.push(Field {
                        inner: proto.clone(),
                        map,
                        comments,
                    });
                }
            } else {
                // normal field
                fields.push(Field {
                    inner: proto.clone(),
                    map,
                    comments,
                });
            }
            self.path.pop();
        }
        self.path.pop();

        self.path.push(8);
        let oneof_fields: Vec<OneofField> = descriptor
            .oneof_decl
            .iter()
            .enumerate()
            .filter_map(|(idx, proto)| {
                let idx = idx as i32;
                self.path.push(idx);
                let oneof = if let Some(fields) = oneof_map.remove(&idx) {
                    let comments = self.comments();
                    Some(OneofField::new(
                        &descriptor,
                        proto.clone(),
                        fields,
                        comments,
                    ))
                } else {
                    None
                };
                self.path.pop();
                oneof
            })
            .collect();
        self.path.pop();

        // Handle Nested Messages and Enums
        self.type_path.push(descriptor.name().to_owned());

        self.path.push(3);
        for (nested_type, idx) in nested_types {
            self.path.push(idx as i32);
            self.process_message(nested_type);
            self.path.pop();
        }
        self.path.pop();

        self.path.push(4);
        for (idx, nested_enum) in descriptor.enum_type.clone().into_iter().enumerate() {
            self.path.push(idx as i32);
            self.process_enum(nested_enum);
            self.path.pop();
        }
        self.path.pop();

        self.type_path.pop();
        // End Handling Nested Messages and Enums

        let message = Message {
            descriptor,
            package: self.package.clone(),
            type_name,
            fields,
            oneof_fields,
            comments: self.comments(),
        };
        self.graph
            .messages
            .insert(message.type_name.clone(), message);
    }

    fn process_enum(&mut self, desc: EnumDescriptorProto) {
        let type_name = self.fq_name(desc.name());
        let mut values = Vec::new();

        self.path.push(2);
        for (idx, value) in desc.value.clone().into_iter().enumerate() {
            self.path.push(idx as i32);
            values.push(EnumValue {
                inner: value,
                comments: self.comments(),
            });
            self.path.pop();
        }
        self.path.pop();

        let e = Enum {
            type_name,
            package: self.package.clone(),
            inner: desc,
            comments: self.comments(),
            values,
        };
        self.graph.enums.insert(e.type_name.clone(), e);
    }

    fn comments_opt(&self) -> Option<Comments> {
        let source_info = self.source_info.as_ref()?;
        let idx = source_info
            .location
            .binary_search_by_key(&self.path.as_slice(), |location| &location.path[..])
            .unwrap();
        let location = source_info.location.get(idx)?;
        Some(Comments::from_location(location))
    }

    fn comments(&self) -> Comments {
        self.comments_opt().unwrap_or_default()
    }

    /// Returns the fully-qualified name, starting with a dot
    fn fq_name(&self, message_name: &str) -> String {
        format!(
            "{}{}{}{}.{}",
            if self.package.is_empty() { "" } else { "." },
            self.package.trim_matches('.'),
            if self.type_path.is_empty() { "" } else { "." },
            self.type_path.join("."),
            message_name,
        )
    }
}

#[derive(Debug)]
#[allow(unused)]
pub struct Message {
    pub descriptor: prost_types::DescriptorProto,
    // Fully qualified type name
    pub type_name: String,
    pub package: String,
    pub fields: Vec<Field>,
    pub oneof_fields: Vec<OneofField>,

    pub comments: Comments,
}

// impl Message {
//     pub fn descriptor(&self) -> &DescriptorProto {
//     }
// }

#[derive(Debug)]
#[allow(unused)]
pub struct Field {
    pub inner: FieldDescriptorProto,
    // label: FieldLabel,
    pub map: Option<(FieldDescriptorProto, FieldDescriptorProto)>,

    pub comments: Comments,
}

impl Field {
    pub fn rust_struct_field_name(&self) -> String {
        crate::ident::sanitize_identifier(self.inner.name())
    }

    pub fn is_optional(&self) -> bool {
        if self.inner.proto3_optional.unwrap_or(false) {
            return true;
        }

        if self.inner.label() != Label::Optional {
            return false;
        }

        match self.inner.r#type() {
            Type::Message => true,
            _ => false,
            // _ => self.syntax == Syntax::Proto2,
        }
    }

    pub fn is_map(&self) -> bool {
        self.map.is_some()
    }

    pub fn is_repeated(&self) -> bool {
        self.inner.label() == Label::Repeated
    }

    pub fn resolve_rust_type_path(
        &self,
        context: &crate::context::Context,
        package: &str,
    ) -> String {
        match self.inner.r#type() {
            Type::Float => String::from("f32"),
            Type::Double => String::from("f64"),
            Type::Uint32 | Type::Fixed32 => String::from("u32"),
            Type::Uint64 | Type::Fixed64 => String::from("u64"),
            Type::Int32 | Type::Sfixed32 | Type::Sint32 => String::from("i32"),
            Type::Int64 | Type::Sfixed64 | Type::Sint64 => String::from("i64"),
            Type::Bool => String::from("bool"),
            Type::String => String::from("String"),
            Type::Bytes => String::from("::prost::bytes::Bytes"),
            Type::Group | Type::Message | Type::Enum => {
                context.resolve_ident(package, self.inner.type_name())
            }
        }
    }

    pub fn is_message(&self) -> bool {
        matches!(self.inner.r#type(), Type::Message)
    }

    pub fn is_well_known_type(&self) -> bool {
        self.inner.type_name().starts_with(".google.protobuf")
    }

    pub fn is_enum(&self) -> bool {
        matches!(self.inner.r#type(), Type::Enum)
    }
}

#[derive(Debug)]
#[allow(unused)]
pub struct OneofField {
    pub descriptor: OneofDescriptorProto,
    pub fields: Vec<Field>,

    pub comments: Comments,

    // This type has the same name as another nested type at the same level
    pub has_type_name_conflict: bool,
}

impl OneofField {
    fn new(
        parent: &DescriptorProto,
        descriptor: OneofDescriptorProto,
        fields: Vec<Field>,
        comments: Comments,
    ) -> Self {
        let has_type_name_conflict = parent
            .nested_type
            .iter()
            .any(|nested| to_snake(nested.name()) == descriptor.name());

        Self {
            descriptor,
            fields,
            comments,
            has_type_name_conflict,
        }
    }

    pub fn rust_struct_field_name(&self) -> String {
        crate::ident::sanitize_identifier(self.descriptor.name())
    }
}

#[derive(Debug)]
#[allow(unused)]
pub struct Enum {
    // Fully qualified type name
    type_name: String,
    package: String,
    inner: EnumDescriptorProto,
    values: Vec<EnumValue>,

    comments: Comments,
}

#[derive(Debug)]
#[allow(unused)]
pub struct EnumValue {
    inner: EnumValueDescriptorProto,
    comments: Comments,
}

/// A service descriptor.
#[derive(Debug, Clone)]
#[allow(unused)]
pub struct Service {
    /// The service name in Rust style.
    pub name: String,
    /// The service name as it appears in the .proto file.
    pub proto_name: String,
    /// The package name as it appears in the .proto file.
    pub package: String,
    /// The service comments.
    pub comments: Comments,
    /// The service methods.
    pub methods: Vec<Method>,
    /// The service options.
    pub options: prost_types::ServiceOptions,
}

/// A service method descriptor.
#[derive(Debug, Clone)]
#[allow(unused)]
pub struct Method {
    /// The name of the method in Rust style.
    pub name: String,
    /// The name of the method as it appears in the .proto file.
    pub proto_name: String,
    /// The method comments.
    pub comments: Comments,
    /// The input Rust type.
    pub input_type: String,
    /// The output Rust type.
    pub output_type: String,
    /// The input Protobuf type.
    pub input_proto_type: String,
    /// The output Protobuf type.
    pub output_proto_type: String,
    /// The method options.
    pub options: prost_types::MethodOptions,
    /// Identifies if client streams multiple client messages.
    pub client_streaming: bool,
    /// Identifies if server streams multiple server messages.
    pub server_streaming: bool,
}
