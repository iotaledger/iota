// Copyright (c) The Diem Core Contributors
// Copyright (c) The Move Contributors
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This module contains verification of usage of dependencies for modules
use std::collections::BTreeSet;

use move_binary_format::{
    errors::{Location, PartialVMError, PartialVMResult, VMResult},
    file_format::CompiledModule,
};
use move_core_types::{language_storage::ModuleId, vm_status::StatusCode};
use move_vm_config::verifier::VerifierConfig;

pub fn verify_module<D>(
    config: &VerifierConfig,
    module: &CompiledModule,
    imm_deps: D,
) -> VMResult<()>
where
    D: Fn(&ModuleId) -> PartialVMResult<Vec<ModuleId>>,
{
    verify_module_impl(config, module, imm_deps)
        .map_err(|e| e.finish(Location::Module(module.self_id())))
}

/// This function performs a depth-first traversal in the module graph, starting
/// at `module` and exploring immediate dependencies.  During the DFS,
/// - If `module.self_id()` is encountered (again), a dependency cycle is
///   detected and an error is returned.
/// - Otherwise terminates without an error.
///
/// Without `config.check_cyclic_dependencies` the traversal stops at the
/// immediate dependencies and no cycle is reported.
fn verify_module_impl<D>(
    config: &VerifierConfig,
    module: &CompiledModule,
    imm_deps: D,
) -> PartialVMResult<()>
where
    D: Fn(&ModuleId) -> PartialVMResult<Vec<ModuleId>>,
{
    let self_id = module.self_id();
    let mut visited = BTreeSet::new();
    // The walk is as deep as the longest dependency chain a publisher can
    // build, so it keeps its own stack rather than recursing.
    let mut stack = vec![module.immediate_dependencies().into_iter()];
    while let Some(frame) = stack.last_mut() {
        let Some(cursor) = frame.next() else {
            stack.pop();
            continue;
        };

        if cursor == self_id {
            return Err(PartialVMError::new(StatusCode::CYCLIC_MODULE_DEPENDENCY));
        }

        // Kept for protocol versions before `check_cyclic_dependencies`, which
        // replay against a traversal that never descends.
        let is_new = if config.check_cyclic_dependencies {
            visited.insert(cursor.clone())
        } else {
            !visited.insert(cursor.clone())
        };

        if is_new {
            stack.push(imm_deps(&cursor)?.into_iter());
        }
    }

    Ok(())
}
