// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use move_binary_format::{CompiledModule, file_format::SignatureToken};
use move_bytecode_utils::resolve_struct;
use move_core_types::{account_address::AccountAddress, ident_str, identifier::IdentStr};
use serde::{
    Serialize, Serializer,
    ser::{SerializeStruct, SerializeStructVariant, SerializeTupleVariant},
};

use crate::{
    IOTA_FRAMEWORK_ADDRESS,
    digests::MoveAuthenticatorDigest,
    transaction::{CallArg, Command, ProgrammableTransaction},
    type_input::TypeName,
};

pub const AUTH_CONTEXT_MODULE_NAME: &IdentStr = ident_str!("auth_context");
pub const AUTH_CONTEXT_STRUCT_NAME: &IdentStr = ident_str!("AuthContext");

/// `AuthContext` provides a lightweight execution context used during the
/// authentication phase of a transaction.
///
/// It allows authenticator functions to:
/// - Identify the transaction sender
/// - Inspect the programmable transaction block (PTB) inputs and commands
/// - Perform function-level permission checks
/// - Support OTP, time-locked auth, or regulatory rule enforcement
///
/// This struct is **immutable** during the auth phase and must not allow
/// mutation of state or access to storage beyond what is declared.
///
/// It is guaranteed to be available to all smart accounts implementing a
/// custom authenticator function.
///
/// Typical use:
/// ```move
/// public fun authenticate(tx_hash: vector<u8>, input: &MyAuthInput, ctx: &AuthContext) {
///     assert!(ed25519::ed25519_verify(&input.sig, &input.pk, &tx_hash), 0);
///     assert!(verify_digest(ctx.digest()), 1);
///     ...
/// }
/// ```
// Conceptually similar to `TxContext`, but designed specifically for use in the authentication
// flow.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthContext {
    /// The digest of the MoveAuthenticator
    auth_digest: MoveAuthenticatorDigest,
    /// The authentication input objects or primitive values
    tx_inputs: Vec<CallArg>,
    /// The authentication commands to be executed sequentially.
    tx_commands: Vec<Command>,
}

impl AuthContext {
    pub fn new_from_components(
        auth_digest: MoveAuthenticatorDigest,
        ptb: &ProgrammableTransaction,
    ) -> Self {
        Self {
            auth_digest,
            tx_inputs: ptb.inputs.clone(),
            tx_commands: ptb.commands.clone(),
        }
    }

    pub fn digest(&self) -> &MoveAuthenticatorDigest {
        &self.auth_digest
    }

    pub fn tx_inputs(&self) -> &Vec<CallArg> {
        &self.tx_inputs
    }

    pub fn tx_commands(&self) -> &Vec<Command> {
        &self.tx_commands
    }

    pub fn to_bcs_bytes(&self) -> Vec<u8> {
        bcs::to_bytes(&self).unwrap()
    }

    /// Returns whether the type signature is &mut AuthContext, &AuthContext, or
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

        let (module_addr, module_name, struct_name) = resolve_struct(module, *idx);

        if is_auth_context(module_addr, module_name, struct_name) {
            kind
        } else {
            AuthContextKind::None
        }
    }
}

// TODO: add a deserializer that can handle the Command::MoveCall and
// Command::MakeMoveVec variants properly. For now, we only need serialization
// for inclusion in the tx authenticator input, so we implement Serialize only.
// Alternatively, a custom Command struct could be created for de/serialization
// purposes or to add new functionalities.
impl Serialize for AuthContext {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("AuthContext", 3)?;
        state.serialize_field("auth_digest", &self.auth_digest)?;
        state.serialize_field("tx_inputs", &self.tx_inputs)?;

        // Serialize tx_commands as a Vec of enums, matching the original logic
        struct CommandSer<'a>(&'a Command);

        impl<'a> Serialize for CommandSer<'a> {
            fn serialize<SC>(&self, serializer: SC) -> Result<SC::Ok, SC::Error>
            where
                SC: Serializer,
            {
                match self.0 {
                    Command::MoveCall(m) => {
                        let mut s =
                            serializer.serialize_struct_variant("Command", 0, "MoveCall", 5)?;
                        s.serialize_field("package", &m.package)?;
                        s.serialize_field("module", &m.module)?;
                        s.serialize_field("function", &m.function)?;
                        s.serialize_field(
                            "type_arguments",
                            &m.type_arguments
                                .iter()
                                .map(TypeName::from)
                                .collect::<Vec<_>>(),
                        )?;
                        s.serialize_field("arguments", &m.arguments)?;
                        s.end()
                    }
                    Command::TransferObjects(objects, recipient) => {
                        let mut s = serializer.serialize_struct_variant(
                            "Command",
                            1,
                            "TransferObjects",
                            2,
                        )?;
                        s.serialize_field("objects", objects)?;
                        s.serialize_field("recipient", recipient)?;
                        s.end()
                    }
                    Command::SplitCoins(coin, amounts) => {
                        let mut s =
                            serializer.serialize_struct_variant("Command", 2, "SplitCoins", 2)?;
                        s.serialize_field("coin", coin)?;
                        s.serialize_field("amounts", amounts)?;
                        s.end()
                    }
                    Command::MergeCoins(target_coin, source_coins) => {
                        let mut s =
                            serializer.serialize_struct_variant("Command", 3, "MergeCoins", 2)?;
                        s.serialize_field("target_coin", target_coin)?;
                        s.serialize_field("source_coins", source_coins)?;
                        s.end()
                    }
                    Command::Publish(modules, dependencies) => {
                        let mut s =
                            serializer.serialize_struct_variant("Command", 4, "Publish", 2)?;
                        s.serialize_field("modules", modules)?;
                        s.serialize_field("dependencies", dependencies)?;
                        s.end()
                    }
                    Command::MakeMoveVec(type_arg, elements) => {
                        let mut s =
                            serializer.serialize_tuple_variant("Command", 5, "MakeMoveVec", 2)?;
                        s.serialize_field(&type_arg.as_ref().map(TypeName::from))?;
                        s.serialize_field(elements)?;
                        s.end()
                    }
                    Command::Upgrade(modules, dependencies, package, upgrade_ticket) => {
                        let mut s =
                            serializer.serialize_struct_variant("Command", 6, "Upgrade", 4)?;
                        s.serialize_field("modules", modules)?;
                        s.serialize_field("dependencies", dependencies)?;
                        s.serialize_field("package", package)?;
                        s.serialize_field("upgrade_ticket", upgrade_ticket)?;
                        s.end()
                    }
                }
            }
        }

        state.serialize_field(
            "tx_commands",
            &self.tx_commands.iter().map(CommandSer).collect::<Vec<_>>(),
        )?;

        state.end()
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum AuthContextKind {
    // Not AuthContext
    None,
    // &mut AuthContext
    Mutable,
    // &AuthContext
    Immutable,
}

pub fn is_auth_context(
    module_addr: &AccountAddress,
    module_name: &IdentStr,
    struct_name: &IdentStr,
) -> bool {
    module_addr == &IOTA_FRAMEWORK_ADDRESS
        && module_name == AUTH_CONTEXT_MODULE_NAME
        && struct_name == AUTH_CONTEXT_STRUCT_NAME
}
