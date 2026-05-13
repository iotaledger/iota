// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Warns when a function satisfies view constraints but is not annotated with
//! `#[view]`.

use super::{LINT_WARNING_PREFIX, LinterDiagnosticCategory, LinterDiagnosticCode};
use crate::{
    diag,
    diagnostics::codes::{DiagnosticInfo, Severity, custom},
    expansion::ast::ModuleIdent,
    iota_mode::{known_attributes::view::ViewAttribute, typing::is_valid_view_signature},
    parser::ast::FunctionName,
    shared::Identifier,
    typing::{ast as T, visitor::simple_visitor},
};

const VIEW_FUNCTION_DIAG: DiagnosticInfo = custom(
    LINT_WARNING_PREFIX,
    Severity::Warning,
    LinterDiagnosticCategory::Iota as u8,
    LinterDiagnosticCode::ViewFunction as u8,
    "function can be marked '#[view]'",
);

simple_visitor!(
    ViewFunctionVisitor,
    fn visit_function_custom(
        &mut self,
        _module: ModuleIdent,
        fname: FunctionName,
        fdef: &T::Function,
    ) -> bool {
        if fdef.attributes.is_test_or_test_only() {
            return false;
        }

        if fdef.attributes.get_(&ViewAttribute.into()).is_some() {
            return false;
        }

        if !is_valid_view_signature(&fdef.visibility, &fdef.signature) {
            return false;
        }

        let msg = format!(
            "Function '{}' satisfies view constraints and can be annotated with '#[view]'",
            fname
        );
        let mut d = diag!(VIEW_FUNCTION_DIAG, (fname.loc(), msg));
        d.add_note("Add '#[view]' to make the function explicitly callable as a view function.");
        self.add_diag(d);

        true
    }
);
