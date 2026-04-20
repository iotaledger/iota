// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Warns when a function satisfies view constraints but is not annotated with
//! `#[view]`.

use super::{LINT_WARNING_PREFIX, LinterDiagnosticCategory, LinterDiagnosticCode};
use crate::{
    diag,
    diagnostics::codes::{DiagnosticInfo, Severity, custom},
    expansion::ast::{ModuleIdent, Mutability, Visibility},
    iota_mode::{
        known_attributes::view::ViewAttribute,
        typing::{TxContextKind, contains_object_ty_shallow, tx_context_kind},
    },
    naming::ast::{Type, Type_, Var},
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

        if !is_valid_view_signature(
            &fdef.visibility,
            &fdef.signature.parameters,
            &fdef.signature.return_type,
        ) {
            return false;
        }

        let msg = format!(
            "Function '{}' satisfies view constraints and can be annotated with '#[view]'",
            fname
        );
        let mut d = diag!(VIEW_FUNCTION_DIAG, (fname.loc(), msg));
        d.add_note(format!(
            "Add '#[view]' to make the function explicitly callable as a view function."
        ));
        self.add_diag(d);

        true
    }
);

fn is_valid_view_signature(
    visibility: &Visibility,
    parameters: &[(Mutability, Var, Type)],
    return_ty: &Type,
) -> bool {
    if !matches!(visibility, Visibility::Public(_)) {
        return false;
    }
    if !is_valid_view_return_type(return_ty) {
        return false;
    }

    parameters
        .iter()
        .all(|(_, _, param_ty)| is_valid_view_param_type(param_ty))
}

fn is_valid_view_return_type(return_ty: &Type) -> bool {
    if matches!(return_ty.value, Type_::Unit) {
        return false;
    }
    !contains_object_ty_shallow(return_ty)
}

fn is_valid_view_param_type(param_ty: &Type) -> bool {
    if tx_context_kind(param_ty) == TxContextKind::Mutable {
        return false;
    }

    match &param_ty.value {
        Type_::Ref(is_mut, inner) => {
            let contains_obj = contains_object_ty_shallow(inner);
            if *is_mut && contains_obj {
                return false;
            }
            true
        }
        _ => !contains_object_ty_shallow(param_ty),
    }
}
