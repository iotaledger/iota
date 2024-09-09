// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This analysis flags uses of random::Random and random::RandomGenerator in
//! public functions.

use super::{
    LinterDiagCategory, IOTA_PKG_NAME, LINTER_DEFAULT_DIAG_CODE, LINT_WARNING_PREFIX,
    RANDOM_GENERATOR_STRUCT_NAME, RANDOM_MOD_NAME, RANDOM_STRUCT_NAME,
};
use crate::{
    diag,
    diagnostics::{
        codes::{custom, DiagnosticInfo, Severity},
        WarningFilters,
    },
    expansion::ast::{ModuleIdent, Visibility},
    iota_mode::IOTA_ADDR_NAME,
    naming::ast as N,
    parser::ast::FunctionName,
    shared::{program_info::TypingProgramInfo, CompilationEnv},
    typing::{
        ast as T,
        visitor::{TypingVisitorConstructor, TypingVisitorContext},
    },
};

const PUBLIC_RANDOM_DIAG: DiagnosticInfo = custom(
    LINT_WARNING_PREFIX,
    Severity::Warning,
    LinterDiagCategory::PublicRandom as u8,
    LINTER_DEFAULT_DIAG_CODE,
    "Risky use of 'iota::random'",
);

pub struct PublicRandomVisitor;
pub struct Context<'a> {
    env: &'a mut CompilationEnv,
}

impl TypingVisitorConstructor for PublicRandomVisitor {
    type Context<'a> = Context<'a>;

    fn context<'a>(
        env: &'a mut CompilationEnv,
        _program_info: &'a TypingProgramInfo,
        _program: &T::Program_,
    ) -> Self::Context<'a> {
        Context { env }
    }
}

impl TypingVisitorContext for Context<'_> {
    fn add_warning_filter_scope(&mut self, filter: WarningFilters) {
        self.env.add_warning_filter_scope(filter)
    }

    fn pop_warning_filter_scope(&mut self) {
        self.env.pop_warning_filter_scope()
    }

    fn visit_module_custom(&mut self, ident: ModuleIdent, mdef: &mut T::ModuleDefinition) -> bool {
        // skips if true
        mdef.attributes.is_test_or_test_only() || ident.value.address.is(IOTA_ADDR_NAME)
    }

    fn visit_function_custom(
        &mut self,
        _module: ModuleIdent,
        fname: FunctionName,
        fdef: &mut T::Function,
    ) -> bool {
        if fdef.attributes.is_test_or_test_only()
            || !matches!(fdef.visibility, Visibility::Public(_))
        {
            return true;
        }
        for (_, _, t) in &fdef.signature.parameters {
            if let Some(struct_name) = is_random_or_random_generator(t) {
                let tloc = t.loc;
                let msg =
                    format!("'public' function '{fname}' accepts '{struct_name}' as a parameter");
                let mut d = diag!(PUBLIC_RANDOM_DIAG, (tloc, msg));
                let note = format!(
                    "Functions that accept '{}::{}::{}' as a parameter might be abused by attackers by inspecting the results of randomness",
                    IOTA_PKG_NAME, RANDOM_MOD_NAME, struct_name
                );
                d.add_note(note);
                d.add_note("Non-public functions are preferred");
                self.env.add_diag(d);
            }
        }
        true
    }
}

fn is_random_or_random_generator(sp!(_, t): &N::Type) -> Option<&str> {
    use N::Type_ as T;
    match t {
        T::Ref(_, inner_t) => is_random_or_random_generator(inner_t),
        T::Apply(_, sp!(_, tname), _) => {
            if tname.is(IOTA_PKG_NAME, RANDOM_MOD_NAME, RANDOM_STRUCT_NAME) {
                Some(RANDOM_STRUCT_NAME)
            } else if tname.is(IOTA_PKG_NAME, RANDOM_MOD_NAME, RANDOM_GENERATOR_STRUCT_NAME) {
                Some(RANDOM_GENERATOR_STRUCT_NAME)
            } else {
                None
            }
        }
        T::Unit | T::Param(_) | T::Var(_) | T::Anything | T::UnresolvedError | T::Fun(_, _) => None,
    }
}
