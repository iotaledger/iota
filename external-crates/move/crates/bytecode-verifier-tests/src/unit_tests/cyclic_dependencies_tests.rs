// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;

use move_binary_format::{
    errors::PartialVMResult,
    file_format::{
        AddressIdentifierIndex, CompiledModule, IdentifierIndex, ModuleHandle, ModuleHandleIndex,
        empty_module,
    },
};
use move_bytecode_verifier::cyclic_dependencies;
use move_core_types::{
    account_address::AccountAddress, identifier::Identifier, language_storage::ModuleId,
    vm_status::StatusCode,
};
use move_vm_config::verifier::VerifierConfig;

fn id(name: &str) -> ModuleId {
    ModuleId::new(AccountAddress::ZERO, Identifier::new(name).unwrap())
}

/// A module named `self_name` whose immediate dependencies are `deps`.
fn module(self_name: &str, deps: &[&str]) -> CompiledModule {
    let mut m = empty_module();
    m.identifiers = vec![Identifier::new(self_name).unwrap()];
    m.module_handles = vec![ModuleHandle {
        address: AddressIdentifierIndex(0),
        name: IdentifierIndex(0),
    }];
    m.self_module_handle_idx = ModuleHandleIndex(0);

    for (i, dep) in deps.iter().enumerate() {
        m.identifiers.push(Identifier::new(*dep).unwrap());
        m.module_handles.push(ModuleHandle {
            address: AddressIdentifierIndex(0),
            name: IdentifierIndex((i + 1) as u16),
        });
    }

    m
}

/// The rest of the module graph, as a lookup from a module to its immediate
/// dependencies.
fn graph(edges: &[(&str, &[&str])]) -> BTreeMap<ModuleId, Vec<ModuleId>> {
    edges
        .iter()
        .map(|(from, to)| (id(from), to.iter().map(|name| id(name)).collect()))
        .collect()
}

fn verify(
    check_cyclic_dependencies: bool,
    module: &CompiledModule,
    graph: BTreeMap<ModuleId, Vec<ModuleId>>,
) -> Result<(), StatusCode> {
    let config = VerifierConfig {
        check_cyclic_dependencies,
        ..Default::default()
    };

    let deps = |module_id: &ModuleId| -> PartialVMResult<Vec<ModuleId>> {
        Ok(graph.get(module_id).cloned().unwrap_or_default())
    };

    cyclic_dependencies::verify_module(&config, module, deps).map_err(|e| e.major_status())
}

#[test]
fn two_module_cycle_is_rejected() {
    let a = module("A", &["B"]);
    assert_eq!(
        verify(true, &a, graph(&[("B", &["A"])])),
        Err(StatusCode::CYCLIC_MODULE_DEPENDENCY)
    );
}

#[test]
fn cycle_past_the_immediate_dependencies_is_rejected() {
    let a = module("A", &["B"]);
    assert_eq!(
        verify(
            true,
            &a,
            graph(&[("B", &["C"]), ("C", &["D"]), ("D", &["A"])])
        ),
        Err(StatusCode::CYCLIC_MODULE_DEPENDENCY)
    );
}

#[test]
fn acyclic_graph_is_accepted() {
    // A depends on B and C, both of which depend on D. D is reached twice, and
    // the second visit must not be mistaken for a cycle.
    let a = module("A", &["B", "C"]);
    assert_eq!(
        verify(true, &a, graph(&[("B", &["D"]), ("C", &["D"])])),
        Ok(())
    );
}

#[test]
fn cycle_is_not_reported_before_the_flag() {
    // Protocol versions before `check_cyclic_dependencies` never descend past the
    // immediate dependencies, so the same cycle goes unreported. Transactions from
    // those versions replay against this behaviour.
    let a = module("A", &["B"]);
    assert_eq!(verify(false, &a, graph(&[("B", &["A"])])), Ok(()));
}
