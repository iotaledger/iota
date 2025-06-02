// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This analysis flags uses of the iota::coin::Coin struct in fields of other structs. In most cases
//! it's preferable to use iota::balance::Balance instead to save space.

use crate::{
    diag,
    diagnostics::codes::{custom, DiagnosticInfo, Severity},
    expansion::ast::ModuleIdent,
    naming::ast as N,
    parser::ast::DatatypeName,
    iota_mode::IOTA_ADDR_VALUE,
    typing::{ast as T, visitor::simple_visitor},
};

use super::{
    LinterDiagnosticCategory, LinterDiagnosticCode, COIN_MOD_NAME, COIN_STRUCT_NAME,
    LINT_WARNING_PREFIX,
};

const COIN_FIELD_DIAG: DiagnosticInfo = custom(
    LINT_WARNING_PREFIX,
    Severity::Warning,
    LinterDiagnosticCategory::Iota as u8,
    LinterDiagnosticCode::CoinField as u8,
    "sub-optimal 'iota::coin::Coin' field type",
);

simple_visitor!(
    CoinFieldVisitor,
    fn visit_module_custom(&mut self, _ident: ModuleIdent, mdef: &T::ModuleDefinition) -> bool {
        // skip if test only
        mdef.attributes.is_test_or_test_only()
    },
    // TODO enums
    fn visit_struct_custom(
        &mut self,
        _module: ModuleIdent,
        _sname: DatatypeName,
        sdef: &N::StructDefinition,
    ) -> bool {
        if sdef.attributes.is_test_or_test_only() {
            return false;
        }

        if let N::StructFields::Defined(_, sfields) = &sdef.fields {
            for (_floc, _fname, (_, (_, ftype))) in sfields {
                if is_field_coin_type(ftype) {
                    let msg = "Sub-optimal 'iota::coin::Coin' field type. Using \
                        'iota::balance::Balance' instead will be more space efficient";
                    self.add_diag(diag!(COIN_FIELD_DIAG, (ftype.loc, msg)));
                }
            }
        }
        false
    }
);

fn is_field_coin_type(sp!(_, t): &N::Type) -> bool {
    use N::Type_ as T;
    match t {
        T::Ref(_, inner_t) => is_field_coin_type(inner_t),
        T::Apply(_, tname, _) => {
            let sp!(_, tname) = tname;
            tname.is(&IOTA_ADDR_VALUE, COIN_MOD_NAME, COIN_STRUCT_NAME)
        }
        T::Unit | T::Param(_) | T::Var(_) | T::Anything | T::UnresolvedError | T::Fun(_, _) => {
            false
        }
    }
}
