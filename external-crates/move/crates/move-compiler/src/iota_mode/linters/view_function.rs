// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! View Function Analysis
//!
//! This analyzer verifies that functions marked with the `#[view]` attribute
//! meet the requirements for view functions:
//!
//! 1. Must have a non-void return type
//! 2. Must not have object parameters (structs with `key` ability)
//! 3. Must not have mutable references to objects in parameters
//! 4. Must not make any module calls (conservative: no external function calls)
//! 5. Must not call native functions
//!
//! The analyzer also emits warnings for functions that could be marked as view
//! but aren't, and errors for functions incorrectly marked as view.

use crate::shared::Identifier;

use super::{LINT_WARNING_PREFIX, LinterDiagnosticCategory, LinterDiagnosticCode};
use crate::{
    cfgir::{ast as G, visitor::simple_visitor},
    diag,
    diagnostics::codes::{DiagnosticInfo, Severity, custom},
    expansion::ast::ModuleIdent,
    hlir::ast::{BaseType_, SingleType, SingleType_, Type_},
    parser::ast::{Ability_, FunctionName},
};

/// Diagnostic for a function marked #[view] that doesn't qualify
const VIEW_FUNCTION_INVALID_DIAG: DiagnosticInfo = custom(
    LINT_WARNING_PREFIX,
    Severity::NonblockingError,
    LinterDiagnosticCategory::Iota as u8,
    LinterDiagnosticCode::ViewFunctionInvalid as u8,
    "invalid view function",
);

/// Diagnostic for suggesting a function could be marked as view
const VIEW_FUNCTION_SUGGESTION_DIAG: DiagnosticInfo = custom(
    LINT_WARNING_PREFIX,
    Severity::Warning,
    LinterDiagnosticCategory::Iota as u8,
    LinterDiagnosticCode::ViewFunctionSuggestion as u8,
    "function could be marked as view",
);

pub const VIEW_FUNCTION_INVALID_FILTER_NAME: &str = "view_function_invalid";
pub const VIEW_FUNCTION_SUGGESTION_FILTER_NAME: &str = "view_function_suggestion";

//**************************************************************************************************
// Visitor
//**************************************************************************************************

simple_visitor!(
    ViewFunctionVisitor,
    fn visit_function_custom(
        &mut self,
        _module: ModuleIdent,
        function_name: FunctionName,
        fdef: &G::Function,
    ) -> bool {
        // Skip test functions
        if fdef.attributes.is_test_or_test_only() {
            return true;
        }

        let is_marked_view = fdef.attributes.is_view();
        let fn_loc = function_name.loc();
        let fn_name = function_name.value();

        // Analyze the function
        let analysis = analyze_view_function(fdef);

        if is_marked_view {
            // Function is marked as #[view], verify it qualifies
            if let Some(reason) = analysis.disqualification_reason {
                let msg = format!("Function '{}' is marked as #[view] but {}", fn_name, reason);
                let diag = diag!(VIEW_FUNCTION_INVALID_DIAG, (fn_loc, msg));
                self.add_diag(diag);
            }
        } else {
            // Function is not marked as #[view], suggest if it qualifies
            if analysis.disqualification_reason.is_none() {
                let msg = format!(
                    "Function '{}' could be marked with #[view] attribute",
                    fn_name
                );
                let note = "View functions can be called off-chain without a transaction";
                let mut diag = diag!(VIEW_FUNCTION_SUGGESTION_DIAG, (fn_loc, msg));
                diag.add_note(note);
                self.add_diag(diag);
            }
        }

        true // We handled the function
    }
);

//**************************************************************************************************
// Analysis
//**************************************************************************************************

/// Result of analyzing whether a function qualifies as a view function
struct ViewFunctionAnalysis {
    /// If Some, contains the reason why the function doesn't qualify as view
    disqualification_reason: Option<String>,
}

/// Analyze a function to determine if it qualifies as a view function
fn analyze_view_function(fdef: &G::Function) -> ViewFunctionAnalysis {
    // Check 0: Native functions cannot be view
    if matches!(fdef.body.value, G::FunctionBody_::Native) {
        return ViewFunctionAnalysis {
            disqualification_reason: Some(
                "it is a native function (native functions cannot be view)".to_string(),
            ),
        };
    }

    // Check 1: Must have a non-void return type
    if matches!(fdef.signature.return_type.value, Type_::Unit) {
        return ViewFunctionAnalysis {
            disqualification_reason: Some(
                "it has no return type (view functions must return a value)".to_string(),
            ),
        };
    }

    // Check 2: Must not have object parameters or mutable references to objects
    for (_, _, param_type) in &fdef.signature.parameters {
        if let Some(reason) = check_parameter_type(param_type) {
            return ViewFunctionAnalysis {
                disqualification_reason: Some(reason),
            };
        }
    }

    // Check 3: Must not make any module calls
    if let G::FunctionBody_::Defined { blocks, .. } = &fdef.body.value {
        if has_any_module_call(blocks) {
            return ViewFunctionAnalysis {
                disqualification_reason: Some(
                    "view functions cannot make module calls".to_string(),
                ),
            };
        }
    }

    ViewFunctionAnalysis {
        disqualification_reason: None,
    }
}

/// Check if a parameter type disqualifies the function from being a view function
fn check_parameter_type(param_type: &SingleType) -> Option<String> {
    match &param_type.value {
        SingleType_::Base(sp!(_, base_type)) => {
            if is_object_type(base_type) {
                return Some(
                    "it has an object parameter (view functions cannot take objects by value)"
                        .to_string(),
                );
            }
        }
        SingleType_::Ref(is_mut, sp!(_, base_type)) => {
            if *is_mut && is_object_type(base_type) {
                return Some(
                    "it has a mutable reference to an object (view functions cannot mutate objects)"
                        .to_string(),
                );
            }
        }
    }
    None
}

/// Check if a base type is an object type (has key ability)
fn is_object_type(base_type: &BaseType_) -> bool {
    if let BaseType_::Apply(abilities, _, _) = base_type {
        return abilities.has_ability_(Ability_::Key);
    }
    false
}

/// Check if the function body contains any module calls
fn has_any_module_call(
    blocks: &std::collections::BTreeMap<crate::hlir::ast::Label, crate::hlir::ast::BasicBlock>,
) -> bool {
    for block in blocks.values() {
        for cmd in block.iter() {
            if command_has_module_call(cmd) {
                return true;
            }
        }
    }
    false
}

/// Check if a command contains a module call
fn command_has_module_call(cmd: &crate::hlir::ast::Command) -> bool {
    use crate::hlir::ast::Command_ as C;

    let sp!(_, cmd_) = cmd;
    match cmd_ {
        C::Assign(_, _, e)
        | C::Abort(_, e)
        | C::Return { exp: e, .. }
        | C::IgnoreAndPop { exp: e, .. }
        | C::JumpIf { cond: e, .. }
        | C::VariantSwitch { subject: e, .. } => exp_has_module_call(e),
        C::Mutate(el, er) => exp_has_module_call(el) || exp_has_module_call(er),
        C::Jump { .. } => false,
        C::Break(_) | C::Continue(_) => false,
    }
}

/// Check if an expression contains a module call
fn exp_has_module_call(e: &crate::hlir::ast::Exp) -> bool {
    use crate::hlir::ast::UnannotatedExp_ as E;

    match &e.exp.value {
        E::ModuleCall(_) => true,

        E::Unit { .. }
        | E::Move { .. }
        | E::Copy { .. }
        | E::Constant(_)
        | E::ErrorConstant { .. }
        | E::BorrowLocal(_, _)
        | E::Unreachable
        | E::UnresolvedError
        | E::Value(_) => false,

        E::Freeze(inner)
        | E::Dereference(inner)
        | E::UnaryExp(_, inner)
        | E::Borrow(_, inner, _, _)
        | E::Cast(inner, _) => exp_has_module_call(inner),

        E::BinopExp(el, _, er) => exp_has_module_call(el) || exp_has_module_call(er),

        E::Vector(_, _, _, es) | E::Multiple(es) => es.iter().any(exp_has_module_call),

        E::Pack(_, _, fields) | E::PackVariant(_, _, _, fields) => {
            fields.iter().any(|(_, _, e)| exp_has_module_call(e))
        }
    }
}
