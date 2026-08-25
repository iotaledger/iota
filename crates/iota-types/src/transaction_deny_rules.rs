// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_types::{Identifier, Version};
use move_core_types::{account_address::AccountAddress, ident_str, identifier::IdentStr};

use crate::{
    IOTA_FRAMEWORK_ADDRESS, IOTA_TRANSACTION_DENY_RULES_OBJECT_ID, error::IotaResult,
    storage::ObjectStore,
};

pub const TRANSACTION_DENY_RULES_MODULE_NAME: &IdentStr = ident_str!("transaction_deny_rules");
pub const TRANSACTION_DENY_RULES_UPDATE_FUNCTION_NAME: Identifier =
    Identifier::from_static("update");
pub const TRANSACTION_DENY_RULES_CREATE_FUNCTION_NAME: Identifier =
    Identifier::from_static("create");
pub const RESOLVED_IOTA_TRANSACTION_DENY_RULES: (&AccountAddress, &IdentStr, &IdentStr) = (
    &IOTA_FRAMEWORK_ADDRESS,
    ident_str!("transaction_deny_rules"),
    ident_str!("TransactionDenyRules"),
);

/// The initial shared version of the `TransactionDenyRules` object, or `None`
/// while the object has not been created yet (the
/// `TransactionDenyRulesCreate` end-of-epoch transaction has not run).
///
/// # Panics
///
/// Panics if the object exists but is not shared, which would mean the
/// system invariant on the reserved object is broken.
pub fn get_transaction_deny_rules_obj_initial_shared_version(
    object_store: &dyn ObjectStore,
) -> IotaResult<Option<Version>> {
    Ok(object_store
        .try_get_object(&IOTA_TRANSACTION_DENY_RULES_OBJECT_ID)?
        .map(|obj| {
            obj.owner
                .into_opt_shared()
                .expect("TransactionDenyRules object must be shared")
        }))
}
