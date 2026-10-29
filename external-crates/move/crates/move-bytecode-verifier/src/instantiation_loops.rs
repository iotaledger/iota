// Copyright (c) The Diem Core Contributors
// Copyright (c) The Move Contributors
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This implements an algorithm that detects loops during the instantiation of
//! generics.
//!
//! It builds a graph from the given `CompiledModule` and converts the original
//! problem into finding strongly connected components in the graph with certain
//! properties. Read the documentation of the types/functions below for details
//! of how it works.
//!
//! Note: We're doing generics only up to specialization, and are doing a
//! conservative check of generic call sites to eliminate those which could lead
//! to an infinite number of specialized instances. We do reject recursive
//! functions that create a new type upon each call but do terminate eventually.

use std::{
    collections::{HashMap, HashSet, hash_map},
    rc::Rc,
};

use move_binary_format::{
    errors::{Location, PartialVMError, PartialVMResult, VMResult},
    file_format::{
        Bytecode, CompiledModule, FunctionDefinition, FunctionDefinitionIndex, FunctionHandleIndex,
        SignatureIndex, SignatureToken, TypeParameterIndex,
    },
};
use move_bytecode_verifier_meter::{Meter, Scope};
use move_core_types::vm_status::StatusCode;
use move_vm_config::verifier::VerifierConfig;
use petgraph::{
    Graph,
    algo::tarjan_scc,
    graph::{EdgeIndex, NodeIndex},
    visit::EdgeRef,
};

/// Meter units charged for each node added to the graph and each edge
/// considered for it, when metering is enabled.
const INSTANTIATION_GRAPH_NODE_COST: u128 = 1;
const INSTANTIATION_GRAPH_EDGE_COST: u128 = 1;

/// Most nodes and constructor edges listed in a rejection diagnostic; any
/// beyond this are summarized by a count. Keeps the message a fixed size no
/// matter how large the offending component is.
const MAX_DIAGNOSTIC_ITEMS: usize = 10;

/// Joins up to `MAX_DIAGNOSTIC_ITEMS` rendered items and notes how many more
/// were left out, so the rejection message never grows with the loop's size.
fn bounded_join(items: impl Iterator<Item = String>, total: usize) -> String {
    let shown: Vec<String> = items.take(MAX_DIAGNOSTIC_ITEMS).collect();
    let omitted = total.saturating_sub(shown.len());
    if omitted > 0 {
        format!("{}, and {} more", shown.join(", "), omitted)
    } else {
        shown.join(", ")
    }
}

/// Data attached to each node.
/// Each node corresponds to a type formal of a generic function in the module.
#[derive(Eq, PartialEq, Hash, Copy, Clone)]
struct Node(FunctionDefinitionIndex, TypeParameterIndex);

/// Data attached to each edge. Indicating the type of the edge.
enum Edge<'a> {
    /// This type of edge from type formal T1 to T2 means the type bound to T1
    /// is used to instantiate T2 unmodified, thus the name `Identity`.
    ///
    /// Example:
    /// ```
    /// //    foo<T>() { bar<T>(); return; }
    /// //
    /// //    edge: foo_T --Id--> bar_T
    /// ```
    Identity,
    /// This type of edge from type formal T1 to T2 means T2 is instantiated
    /// with a type resulted by applying one or more type constructors to T1
    /// (potentially with other types).
    ///
    /// This is interesting to us as it creates a new (and bigger) type.
    ///
    /// Example:
    /// ```
    /// //    struct Baz<T> {}
    /// //    foo<T>() { bar<Baz<T>>(); return; }
    /// //
    /// //    edge: foo_T --TyConApp(Baz<T>)--> bar_T
    /// ```
    TyConApp(&'a SignatureToken),
}

/// Discriminant of `Edge`. Two edges with the same endpoints and discriminant
/// are interchangeable for loop detection, so this is enough to deduplicate
/// them regardless of the constructor type a `TyConApp` carries.
#[derive(Eq, PartialEq, Hash, Copy, Clone)]
enum EdgeKind {
    Identity,
    TyConApp,
}

impl Edge<'_> {
    fn kind(&self) -> EdgeKind {
        match self {
            Edge::Identity => EdgeKind::Identity,
            Edge::TyConApp(_) => EdgeKind::TyConApp,
        }
    }
}

/// The caller type parameters that one type argument of a signature mentions.
enum ArgumentParams {
    /// The argument is a type parameter itself.
    Identity(TypeParameterIndex),
    /// The argument applies type constructors to the parameters listed, which
    /// may be none.
    TyConApp(Vec<TypeParameterIndex>),
}

/// Adds every type parameter mentioned in `ty` to `found`.
fn collect_type_parameters(ty: &SignatureToken, found: &mut HashSet<TypeParameterIndex>) {
    use SignatureToken::*;
    match ty {
        Bool | Address | U8 | U16 | U32 | U64 | U128 | U256 | Signer | Datatype(_) => {}
        TypeParameter(idx) => {
            found.insert(*idx);
        }
        Vector(ty) | Reference(ty) | MutableReference(ty) => collect_type_parameters(ty, found),
        DatatypeInstantiation(inst) => {
            let (_, tys) = &**inst;
            for ty in tys {
                collect_type_parameters(ty, found);
            }
        }
    }
}

pub struct InstantiationLoopChecker<'a, 'b, M: Meter + ?Sized> {
    module: &'a CompiledModule,

    /// Charged for every node and edge when `meter_enabled`.
    meter: &'b mut M,
    meter_enabled: bool,
    /// Hard ceiling on the number of graph edges, or `None` for no ceiling.
    max_graph_edges: Option<usize>,

    graph: Graph<Node, Edge<'a>>,
    node_map: HashMap<Node, NodeIndex>,
    func_handle_def_map: HashMap<FunctionHandleIndex, FunctionDefinitionIndex>,
    /// Endpoints and kind of every edge already in `graph`, so an edge reached
    /// again through another instantiation is added only once.
    edge_set: HashSet<(Node, Node, EdgeKind)>,
    /// What each argument of a signature mentions, computed once per
    /// signature: walking the arguments costs as much as they are long, and
    /// every caller of every instantiation that uses the signature would
    /// otherwise repeat it.
    argument_params: HashMap<SignatureIndex, Rc<[ArgumentParams]>>,
}

impl<'a, 'b, M: Meter + ?Sized> InstantiationLoopChecker<'a, 'b, M> {
    fn new(
        module: &'a CompiledModule,
        meter: &'b mut M,
        meter_enabled: bool,
        max_graph_edges: Option<usize>,
    ) -> Self {
        Self {
            module,
            meter,
            meter_enabled,
            max_graph_edges,
            graph: Graph::new(),
            node_map: HashMap::new(),
            func_handle_def_map: module
                .function_defs()
                .iter()
                .enumerate()
                .map(|(def_idx, def)| (def.function, FunctionDefinitionIndex::new(def_idx as u16)))
                .collect(),
            edge_set: HashSet::new(),
            argument_params: HashMap::new(),
        }
    }

    pub fn verify_module(
        config: &VerifierConfig,
        module: &'a CompiledModule,
        meter: &'b mut M,
    ) -> VMResult<()> {
        Self::verify_module_impl(config, module, meter)
            .map_err(|e| e.finish(Location::Module(module.self_id())))
    }

    fn verify_module_impl(
        config: &VerifierConfig,
        module: &'a CompiledModule,
        meter: &'b mut M,
    ) -> PartialVMResult<()> {
        let mut checker = Self::new(
            module,
            meter,
            config.meter_instantiation_loops,
            config.max_instantiation_loop_graph_edges,
        );
        checker.build_graph()?;
        let mut components = checker.find_non_trivial_components();

        match components.pop() {
            None => Ok(()),
            Some((nodes, edges)) => {
                let constructor_edges: Vec<EdgeIndex> = edges
                    .into_iter()
                    .filter(|edge_idx| {
                        matches!(
                            checker.graph.edge_weight(*edge_idx).unwrap(),
                            Edge::TyConApp(_)
                        )
                    })
                    .collect();
                let msg_edges = bounded_join(
                    constructor_edges
                        .iter()
                        .map(|idx| checker.format_edge(*idx)),
                    constructor_edges.len(),
                );
                let msg_nodes = bounded_join(
                    nodes.iter().map(|idx| checker.format_node(*idx)),
                    nodes.len(),
                );
                let msg = format!(
                    "edges with constructors: [{}], nodes: [{}]",
                    msg_edges, msg_nodes
                );
                Err(PartialVMError::new(StatusCode::LOOP_IN_INSTANTIATION_GRAPH).with_message(msg))
            }
        }
    }

    /// Charges `units` to the meter when metering is enabled.
    fn charge(&mut self, units: u128) -> PartialVMResult<()> {
        if self.meter_enabled {
            self.meter.add(Scope::Module, units)?;
        }
        Ok(())
    }

    /// Retrieves the node corresponding to the specified type formal.
    /// If none exists in the graph yet, create one.
    fn get_or_add_node(&mut self, node: Node) -> PartialVMResult<NodeIndex> {
        let (idx, added) = match self.node_map.entry(node) {
            hash_map::Entry::Occupied(entry) => (*entry.get(), false),
            hash_map::Entry::Vacant(entry) => {
                let idx = self.graph.add_node(node);
                entry.insert(idx);
                (idx, true)
            }
        };
        if added {
            self.charge(INSTANTIATION_GRAPH_NODE_COST)?;
        }
        Ok(idx)
    }

    /// What each argument of the signature `idx` mentions, walked on the
    /// first request and reused after that.
    fn argument_params(&mut self, idx: SignatureIndex) -> Rc<[ArgumentParams]> {
        if let Some(params) = self.argument_params.get(&idx) {
            return params.clone();
        }
        let params: Rc<[ArgumentParams]> = self
            .module
            .signature_at(idx)
            .0
            .iter()
            .map(|ty| match ty {
                SignatureToken::TypeParameter(actual_idx) => ArgumentParams::Identity(*actual_idx),
                _ => {
                    let mut found = HashSet::new();
                    collect_type_parameters(ty, &mut found);
                    let mut found: Vec<_> = found.into_iter().collect();
                    found.sort_unstable();
                    ArgumentParams::TyConApp(found)
                }
            })
            .collect();
        self.argument_params.insert(idx, params.clone());
        params
    }

    /// Helper function that creates an edge from one given node to the other.
    /// If a node does not exist, create one.
    fn add_edge(&mut self, node_from: Node, node_to: Node, edge: Edge<'a>) -> PartialVMResult<()> {
        // Charged before deduplication: different instantiations of one callee
        // can reach the same edge again, and that work costs as much as a new
        // edge.
        self.charge(INSTANTIATION_GRAPH_EDGE_COST)?;
        // Duplicates leave the strongly connected components, and whether they
        // contain a type-constructor edge, unchanged, so each
        // (source, target, kind) is recorded once.
        if !self.edge_set.insert((node_from, node_to, edge.kind())) {
            return Ok(());
        }
        // Fail fast once the graph reaches its configured ceiling, before
        // allocating another node or edge.
        if let Some(max) = self.max_graph_edges {
            if self.graph.edge_count() >= max {
                return Err(
                    PartialVMError::new(StatusCode::PROGRAM_TOO_COMPLEX).with_message(format!(
                        "instantiation loop graph exceeded the {max}-edge limit"
                    )),
                );
            }
        }
        let node_from_idx = self.get_or_add_node(node_from)?;
        let node_to_idx = self.get_or_add_node(node_to)?;
        self.graph.add_edge(node_from_idx, node_to_idx, edge);
        Ok(())
    }

    /// Helper of 'fn build_graph' that inspects a function call. If type
    /// parameters of the caller appear in the type actuals to the callee,
    /// nodes and edges are added to the graph.
    fn build_graph_call(
        &mut self,
        caller_idx: FunctionDefinitionIndex,
        callee_idx: FunctionDefinitionIndex,
        type_actuals_idx: SignatureIndex,
    ) -> PartialVMResult<()> {
        let type_actuals = &self.module.signature_at(type_actuals_idx).0;
        let params = self.argument_params(type_actuals_idx);

        for (formal_idx, arg) in params.iter().enumerate() {
            let formal = Node(callee_idx, formal_idx as TypeParameterIndex);
            match arg {
                ArgumentParams::Identity(actual_idx) => {
                    self.add_edge(Node(caller_idx, *actual_idx), formal, Edge::Identity)?
                }
                ArgumentParams::TyConApp(type_params) => {
                    for type_param in type_params {
                        self.add_edge(
                            Node(caller_idx, *type_param),
                            formal,
                            Edge::TyConApp(&type_actuals[formal_idx]),
                        )?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Helper of `fn build_graph` that inspects a function definition for calls
    /// between two generic functions defined in the current module.
    fn build_graph_function_def(
        &mut self,
        caller_idx: FunctionDefinitionIndex,
        caller_def: &FunctionDefinition,
    ) -> PartialVMResult<()> {
        if let Some(code) = &caller_def.code {
            // Every call to the same instantiation from this caller adds the
            // same edges, so only the first one needs to be inspected.
            let mut seen = HashSet::new();
            for instr in &code.code {
                if let Bytecode::CallGeneric(callee_inst_idx) = instr {
                    if !seen.insert(*callee_inst_idx) {
                        continue;
                    }
                    // Get the id of the definition of the function being called.
                    // Skip if the function is not defined in the current module, as we do not
                    // have mutual recursions across module boundaries.
                    let callee_si = self.module.function_instantiation_at(*callee_inst_idx);
                    if let Some(callee_idx) = self.func_handle_def_map.get(&callee_si.handle) {
                        let callee_idx = *callee_idx;
                        self.build_graph_call(caller_idx, callee_idx, callee_si.type_parameters)?
                    }
                }
            }
        }
        Ok(())
    }

    /// Builds a graph G such that
    ///   - Each type formal of a generic function is a node in G.
    ///   - There is an edge from type formal f_T to g_T if f_T is used to
    ///     instantiate g_T in a call.
    ///     - Each edge is labeled either `Identity` or `TyConApp`. See `Edge`
    ///       for details.
    fn build_graph(&mut self) -> PartialVMResult<()> {
        for (def_idx, func_def) in self
            .module
            .function_defs()
            .iter()
            .enumerate()
            .filter(|(_, def)| !def.is_native())
        {
            self.build_graph_function_def(FunctionDefinitionIndex::new(def_idx as u16), func_def)?
        }
        Ok(())
    }

    /// Computes the strongly connected components of the graph built and keep
    /// the ones that contain at least one `TyConApp` edge. Such components
    /// indicate there exists a loop such that an input type can get
    /// "bigger" infinitely many times along the loop, also creating
    /// infinitely many types. This is precisely the kind of constructs we want
    /// to forbid.
    fn find_non_trivial_components(&self) -> Vec<(Vec<NodeIndex>, Vec<EdgeIndex>)> {
        tarjan_scc(&self.graph)
            .into_iter()
            .filter_map(move |nodes| {
                let node_set: HashSet<_> = nodes.iter().cloned().collect();

                let edges: Vec<_> = nodes
                    .iter()
                    .flat_map(|node_idx| {
                        self.graph.edges(*node_idx).filter_map(|edge| {
                            if node_set.contains(&edge.target()) {
                                Some(edge.id())
                            } else {
                                None
                            }
                        })
                    })
                    .collect();

                if edges.iter().any(
                    |edge_idx| match self.graph.edge_weight(*edge_idx).unwrap() {
                        Edge::Identity => false,
                        Edge::TyConApp(_) => true,
                    },
                ) {
                    Some((nodes, edges))
                } else {
                    None
                }
            })
            .collect()
    }

    fn format_node(&self, node_idx: NodeIndex) -> String {
        let Node(def_idx, param_idx) = self.graph.node_weight(node_idx).unwrap();
        format!("f{}#{}", def_idx, param_idx)
    }

    fn format_edge(&self, edge_idx: EdgeIndex) -> String {
        let (node_idx_1, node_idx_2) = self.graph.edge_endpoints(edge_idx).unwrap();
        let node_1 = self.format_node(node_idx_1);
        let node_2 = self.format_node(node_idx_2);

        match self.graph.edge_weight(edge_idx).unwrap() {
            Edge::TyConApp(ty) => format!("{} --{:?}--> {}", node_1, ty, node_2,),
            Edge::Identity => format!("{} ----> {}", node_1, node_2),
        }
    }
}

#[cfg(test)]
mod tests {
    use move_binary_format::file_format::{
        AbilitySet, Bytecode, CodeUnit, DatatypeHandle, DatatypeHandleIndex, DatatypeTyParameter,
        FunctionDefinition, FunctionHandle, FunctionHandleIndex, FunctionInstantiation,
        FunctionInstantiationIndex, IdentifierIndex, ModuleHandleIndex, Signature, SignatureIndex,
        SignatureToken, Visibility, empty_module,
    };
    use move_bytecode_verifier_meter::{bound::BoundMeter, dummy::DummyMeter};
    use move_core_types::{identifier::Identifier, vm_status::StatusCode};
    use move_vm_config::verifier::MeterConfig;

    use super::*;

    /// A verifier config with instantiation-loop metering turned on.
    fn metered_config() -> VerifierConfig {
        VerifierConfig {
            meter_instantiation_loops: true,
            ..VerifierConfig::default()
        }
    }

    /// A meter whose module limit is `max` units.
    fn meter_with_module_limit(max: u128) -> BoundMeter {
        BoundMeter::new(MeterConfig {
            max_per_fun_meter_units: Some(max),
            max_per_mod_meter_units: Some(max),
            max_per_pkg_meter_units: Some(max),
        })
    }

    /// Builds a module with a single generic function of `arity` type
    /// parameters that calls itself `call_count` times. Every call wraps all
    /// `arity` caller parameters into each of the callee's `arity` type
    /// arguments, so one `CallGeneric` adds `arity * arity` edges.
    fn repeated_generic_self_call(arity: u16, call_count: usize) -> CompiledModule {
        let mut module = empty_module();
        module.identifiers.push(Identifier::new("f").unwrap());
        module.identifiers.push(Identifier::new("T").unwrap());
        module.datatype_handles.push(DatatypeHandle {
            module: ModuleHandleIndex(0),
            name: IdentifierIndex(2),
            abilities: AbilitySet::EMPTY,
            type_parameters: vec![
                DatatypeTyParameter {
                    constraints: AbilitySet::EMPTY,
                    is_phantom: false,
                };
                arity as usize
            ],
        });
        let actual = SignatureToken::DatatypeInstantiation(Box::new((
            DatatypeHandleIndex(0),
            (0..arity).map(SignatureToken::TypeParameter).collect(),
        )));
        module
            .signatures
            .push(Signature(vec![actual; arity as usize]));
        module.function_handles.push(FunctionHandle {
            module: ModuleHandleIndex(0),
            name: IdentifierIndex(1),
            parameters: SignatureIndex(0),
            return_: SignatureIndex(0),
            type_parameters: vec![AbilitySet::EMPTY; arity as usize],
        });
        module.function_instantiations.push(FunctionInstantiation {
            handle: FunctionHandleIndex(0),
            type_parameters: SignatureIndex(1),
        });
        module.function_defs.push(FunctionDefinition {
            function: FunctionHandleIndex(0),
            visibility: Visibility::Private,
            is_entry: false,
            acquires_global_resources: vec![],
            code: Some(CodeUnit {
                locals: SignatureIndex(0),
                code: vec![Bytecode::CallGeneric(FunctionInstantiationIndex(0)); call_count],
                jump_tables: vec![],
            }),
        });
        module
    }

    fn graph_size(arity: u16, call_count: usize) -> (usize, usize) {
        let module = repeated_generic_self_call(arity, call_count);
        let mut meter = DummyMeter;
        let mut checker = InstantiationLoopChecker::new(&module, &mut meter, false, None);
        checker.build_graph().unwrap();
        (checker.graph.node_count(), checker.graph.edge_count())
    }

    #[test]
    fn repeated_identical_call_does_not_grow_the_graph() {
        let arity = 4;
        let (few_nodes, few_edges) = graph_size(arity, 2);
        let (many_nodes, many_edges) = graph_size(arity, 1_000);

        // The graph is the same however many times the identical call is
        // repeated.
        assert_eq!(few_nodes, arity as usize);
        assert_eq!(many_nodes, arity as usize);
        assert_eq!(few_edges, (arity as usize) * (arity as usize));
        assert_eq!(few_edges, many_edges);
    }

    #[test]
    fn repeated_identical_call_still_reports_the_loop() {
        let module = repeated_generic_self_call(4, 1_000);
        let status = InstantiationLoopChecker::verify_module(
            &VerifierConfig::default(),
            &module,
            &mut DummyMeter,
        )
        .unwrap_err()
        .major_status();
        assert_eq!(status, StatusCode::LOOP_IN_INSTANTIATION_GRAPH);
    }

    #[test]
    fn metering_bounds_graph_construction_when_enabled() {
        // 8 type parameters means 64 edges per call, so a tight meter is
        // exhausted while the graph is still being built.
        let module = repeated_generic_self_call(8, 50);
        let mut meter = meter_with_module_limit(10);
        let status =
            InstantiationLoopChecker::verify_module(&metered_config(), &module, &mut meter)
                .unwrap_err()
                .major_status();
        // `BoundMeter` reports an exceeded limit as CONSTRAINT_NOT_SATISFIED.
        assert_eq!(status, StatusCode::CONSTRAINT_NOT_SATISFIED);
    }

    #[test]
    fn metering_off_by_default_leaves_the_meter_untouched() {
        // The same tight meter is never charged while metering is off, so the
        // loop is still reported.
        let module = repeated_generic_self_call(8, 50);
        let mut meter = meter_with_module_limit(10);
        let status = InstantiationLoopChecker::verify_module(
            &VerifierConfig::default(),
            &module,
            &mut meter,
        )
        .unwrap_err()
        .major_status();
        assert_eq!(status, StatusCode::LOOP_IN_INSTANTIATION_GRAPH);
    }

    #[test]
    fn repeated_call_is_inspected_once() {
        // One call with 8 type parameters costs 64 edge charges and 8 node
        // charges. Inspecting all 50 repeats would cost 3,200 edge charges.
        let module = repeated_generic_self_call(8, 50);
        let mut meter = meter_with_module_limit(100);
        let status =
            InstantiationLoopChecker::verify_module(&metered_config(), &module, &mut meter)
                .unwrap_err()
                .major_status();
        assert_eq!(status, StatusCode::LOOP_IN_INSTANTIATION_GRAPH);
        assert_eq!(meter.get_usage(Scope::Module), 72);
    }

    #[test]
    fn rejection_diagnostic_is_bounded() {
        // 8 formals in a self-call form a complete graph: 8 nodes and 64
        // constructor edges, well past the diagnostic sample size.
        let module = repeated_generic_self_call(8, 2);
        let err = InstantiationLoopChecker::verify_module(
            &VerifierConfig::default(),
            &module,
            &mut DummyMeter,
        )
        .unwrap_err();
        assert_eq!(err.major_status(), StatusCode::LOOP_IN_INSTANTIATION_GRAPH);

        let msg = err.message().expect("rejection carries a message");
        // Only a fixed sample of the 64 edges is listed (one "-->" each), the
        // remaining 54 summarized by a count rather than spelled out.
        assert_eq!(msg.matches("-->").count(), MAX_DIAGNOSTIC_ITEMS);
        assert!(msg.contains(&format!("and {} more", 64 - MAX_DIAGNOSTIC_ITEMS)));
        // Listing all 64 edges would be several times longer; the sample keeps
        // the message size tied to MAX_DIAGNOSTIC_ITEMS, not the edge count.
        assert!(
            msg.len() < 4096,
            "diagnostic unexpectedly large: {}",
            msg.len()
        );
    }

    #[test]
    fn graph_edge_cap_fails_fast() {
        // 8 formals build 64 edges; a 20-edge cap stops construction well
        // before that.
        let module = repeated_generic_self_call(8, 2);
        let config = VerifierConfig {
            max_instantiation_loop_graph_edges: Some(20),
            ..VerifierConfig::default()
        };
        let status = InstantiationLoopChecker::verify_module(&config, &module, &mut DummyMeter)
            .unwrap_err()
            .major_status();
        assert_eq!(status, StatusCode::PROGRAM_TOO_COMPLEX);
    }

    #[test]
    fn graph_edge_cap_allows_graphs_within_the_limit() {
        // The same module builds exactly 64 edges, so a 64-edge cap admits it
        // and the loop is still reported.
        let module = repeated_generic_self_call(8, 2);
        let config = VerifierConfig {
            max_instantiation_loop_graph_edges: Some(64),
            ..VerifierConfig::default()
        };
        let status = InstantiationLoopChecker::verify_module(&config, &module, &mut DummyMeter)
            .unwrap_err()
            .major_status();
        assert_eq!(status, StatusCode::LOOP_IN_INSTANTIATION_GRAPH);
    }

    fn push_function(module: &mut CompiledModule, name: &str, arity: usize, code: Vec<Bytecode>) {
        module.identifiers.push(Identifier::new(name).unwrap());
        let handle = module.function_handles.len() as u16;
        module.function_handles.push(FunctionHandle {
            module: ModuleHandleIndex(0),
            name: IdentifierIndex((module.identifiers.len() - 1) as u16),
            parameters: SignatureIndex(0),
            return_: SignatureIndex(0),
            type_parameters: vec![AbilitySet::EMPTY; arity],
        });
        module.function_defs.push(FunctionDefinition {
            function: FunctionHandleIndex(handle),
            visibility: Visibility::Private,
            is_entry: false,
            acquires_global_resources: vec![],
            code: Some(CodeUnit {
                locals: SignatureIndex(0),
                code,
                jump_tables: vec![],
            }),
        });
    }

    /// Builds a module where each of `callers` functions calls, once, every
    /// one of `k` generic functions, all instantiated with one signature of 32
    /// type arguments, each a `depth`-deep `vector` of `u8`. The arguments
    /// mention no type parameter, so no edge comes from them.
    fn callers_sharing_parameter_free_arguments(
        callers: usize,
        k: usize,
        depth: usize,
    ) -> CompiledModule {
        let mut module = empty_module();
        let mut ty = SignatureToken::U8;
        for _ in 0..depth {
            ty = SignatureToken::Vector(Box::new(ty));
        }
        module.signatures.push(Signature(vec![ty; 32]));
        for i in 0..k {
            push_function(&mut module, &format!("g{i}"), 32, vec![Bytecode::Ret]);
            module.function_instantiations.push(FunctionInstantiation {
                handle: FunctionHandleIndex(i as u16),
                type_parameters: SignatureIndex(1),
            });
        }
        for c in 0..callers {
            let mut code: Vec<Bytecode> = (0..k)
                .map(|i| Bytecode::CallGeneric(FunctionInstantiationIndex(i as u16)))
                .collect();
            code.push(Bytecode::Ret);
            push_function(&mut module, &format!("f{c}"), 0, code);
        }
        module
    }

    #[test]
    fn type_arguments_are_walked_once_per_signature() {
        // 50 callers of 20 instantiations sharing one signature of large,
        // parameter-free arguments: no edge, so nothing is charged, and the
        // arguments are walked once rather than once per call.
        let module = callers_sharing_parameter_free_arguments(50, 20, 255);
        let mut meter = meter_with_module_limit(1);
        let mut checker = InstantiationLoopChecker::new(&module, &mut meter, true, None);
        checker.build_graph().unwrap();
        assert_eq!(checker.graph.edge_count(), 0);
        assert_eq!(checker.argument_params.len(), 1);
    }
}
