// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{cell::RefCell, rc::Rc};

use better_any::{Tid, TidAble};
use iota_types::{
    auth_context::{AuthContext, AuthContextCallArg, AuthContextCommand},
    digests::MoveAuthenticatorDigest,
};
use move_binary_format::errors::{PartialVMError, PartialVMResult};
use move_core_types::vm_status::StatusCode;
use move_vm_runtime::native_extensions::NativeExtensionMarker;

// AuthenticationContext is a wrapper around AuthContext that is exposed to
// NativeContextExtensions in order to provide authentication context
// information to Move native functions. Holds a Rc<RefCell<AuthContext>> to
// allow for mutation of the AuthContext.
#[derive(Tid)]
pub struct AuthenticationContext {
    pub(crate) auth_context: Rc<RefCell<AuthContext>>,
    test_only: bool,
}

impl NativeExtensionMarker<'_> for AuthenticationContext {}

impl AuthenticationContext {
    pub fn new(auth_context: Rc<RefCell<AuthContext>>) -> Self {
        Self {
            auth_context,
            test_only: false,
        }
    }

    pub fn new_for_testing(auth_context: Rc<RefCell<AuthContext>>) -> Self {
        Self {
            auth_context,
            test_only: true,
        }
    }

    pub fn digest(&self) -> MoveAuthenticatorDigest {
        self.auth_context.borrow().digest().to_owned()
    }

    pub fn tx_commands(&self) -> Vec<AuthContextCommand> {
        self.auth_context.borrow().tx_commands().to_owned()
    }

    pub fn tx_inputs(&self) -> Vec<AuthContextCallArg> {
        self.auth_context.borrow().tx_inputs().to_owned()
    }

    // Test only function
    //
    pub fn replace(
        &self,
        auth_digest: MoveAuthenticatorDigest,
        tx_inputs: Vec<AuthContextCallArg>,
        tx_commands: Vec<AuthContextCommand>,
    ) -> PartialVMResult<()> {
        if !self.test_only {
            return Err(
                PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR)
                    .with_message("`replace` called on a non testing scenario".to_string()),
            );
        }
        self.auth_context
            .borrow_mut()
            .replace(auth_digest, tx_inputs, tx_commands);
        Ok(())
    }
}
