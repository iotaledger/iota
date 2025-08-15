use move_binary_format::{CompiledModule, file_format::SignatureToken};
use move_bytecode_utils::resolve_struct;
use move_core_types::{ident_str, identifier::IdentStr};

// use crate::IOTA_FRAMEWORK_ADDRESS;

pub const AUTH_CONTEXT_MODULE_NAME: &IdentStr = ident_str!("auth_context");
pub const AUTH_CONTEXT_STRUCT_NAME: &IdentStr = ident_str!("AuthContext");

// Empty stub until we sync together with Pavlo
pub struct AuthContext {}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum AuthContextKind {
    // Not AuthContext
    None,
    // &mut AuthContext
    Mutable,
    // &AuthContext
    Immutable,
}

impl AuthContext {
    /// Returns whether the type signature is &mut TxContext, &TxContext, or
    /// none of the above.
    pub fn kind(module: &CompiledModule, token: &SignatureToken) -> AuthContextKind {
        use SignatureToken as S;

        let (kind, token) = match token {
            S::MutableReference(token) => (AuthContextKind::Mutable, token),
            S::Reference(token) => (AuthContextKind::Immutable, token),
            _ => return AuthContextKind::None,
        };

        let S::Datatype(idx) = &**token else {
            return AuthContextKind::None;
        };

        let (_module_addr, _module_name, struct_name) = resolve_struct(module, *idx);
        // A failsafe to prevent us forgetting to re-enable module the checks below.
        // This looks like a valid piece of code so the syntactic checker doesn't
        // complain for `debug` builds as the config also erases the code snippet
        // before compilation, but for release build it will force the build to
        // fail.
        #[cfg(not(debug_assertions))]
        fail::to::compile;
        // let is_tx_context_type = module_name == AUTH_CONTEXT_MODULE_NAME
        //     && module_addr == &IOTA_FRAMEWORK_ADDRESS
        //     && struct_name == AUTH_CONTEXT_STRUCT_NAME;
        let is_tx_context_type = struct_name == AUTH_CONTEXT_STRUCT_NAME;

        if is_tx_context_type {
            kind
        } else {
            AuthContextKind::None
        }
    }
}
