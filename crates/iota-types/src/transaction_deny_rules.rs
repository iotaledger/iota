// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::fmt;

use iota_sdk_types::{Address, DenyRuleSet, Identifier, Version};
use move_core_types::{account_address::AccountAddress, ident_str, identifier::IdentStr};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{
    IOTA_FRAMEWORK_ADDRESS, IOTA_TRANSACTION_DENY_RULES_OBJECT_ID, MoveTypeTagTrait,
    collection_types::{LinkedTable, LinkedTableNode},
    dynamic_field::get_dynamic_field_from_store,
    error::{IotaError, IotaResult},
    id::{ID, UID},
    storage::ObjectStore,
    versioned::Versioned,
};

pub const TRANSACTION_DENY_RULES_MODULE_NAME: &IdentStr = ident_str!("transaction_deny_rules");
pub const TRANSACTION_DENY_RULES_MODULE: Identifier =
    Identifier::from_static("transaction_deny_rules");
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

pub const TRANSACTION_DENY_RULES_INNER_V1: u64 = 1;

/// Rust version of the Move `transaction_deny_rules::TransactionDenyRules`
/// type.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TransactionDenyRules {
    pub id: UID,
    pub inner: Versioned,
}

/// Rust version of the Move
/// `transaction_deny_rules::TransactionDenyRulesInnerV1` type.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TransactionDenyRulesInnerV1 {
    pub version: u64,
    pub denied_addresses: LinkedTable<Address>,
    pub denied_objects: LinkedTable<ID>,
    pub denied_packages: LinkedTable<ID>,
    pub package_publish_disabled: bool,
    pub package_upgrade_disabled: bool,
    pub shared_object_disabled: bool,
    pub user_transaction_disabled: bool,
    pub receiving_objects_disabled: bool,
    pub move_authenticator_disabled: bool,
}

/// The full deny rule state read back from the `TransactionDenyRules` object,
/// or `None` while the object has not been created yet.
///
/// Each deny list is reconstructed by walking its `LinkedTable` (`head` →
/// `node.next`) with derived-id child reads, so any node can rebuild the
/// state from its object store alone. Validators seed enforcement and the
/// mirrored on-chain state from this at epoch start.
pub fn get_transaction_deny_rules(
    object_store: &dyn ObjectStore,
) -> IotaResult<Option<DenyRuleSet>> {
    let Some(object) = object_store.try_get_object(&IOTA_TRANSACTION_DENY_RULES_OBJECT_ID)? else {
        return Ok(None);
    };
    let iota_sdk_types::ObjectData::Struct(move_object) = &object.data else {
        return Err(IotaError::ObjectDeserialization {
            error: "TransactionDenyRules object must be a Move object".to_string(),
        });
    };
    let rules: TransactionDenyRules = bcs::from_bytes(move_object.contents()).map_err(|err| {
        IotaError::ObjectDeserialization {
            error: format!("failed to decode TransactionDenyRules: {err}"),
        }
    })?;
    if rules.inner.version != TRANSACTION_DENY_RULES_INNER_V1 {
        return Err(IotaError::ObjectDeserialization {
            error: format!(
                "unsupported TransactionDenyRules inner version {}",
                rules.inner.version
            ),
        });
    }
    let inner: TransactionDenyRulesInnerV1 =
        get_dynamic_field_from_store(object_store, rules.inner.id.id.bytes, &rules.inner.version)?;

    Ok(Some(DenyRuleSet {
        denied_addresses: walk_linked_table(object_store, &inner.denied_addresses)?
            .into_iter()
            .collect(),
        denied_objects: walk_linked_table(object_store, &inner.denied_objects)?
            .into_iter()
            .map(|id| id.bytes)
            .collect(),
        denied_packages: walk_linked_table(object_store, &inner.denied_packages)?
            .into_iter()
            .map(|id| id.bytes)
            .collect(),
        package_publish_disabled: inner.package_publish_disabled,
        package_upgrade_disabled: inner.package_upgrade_disabled,
        shared_object_disabled: inner.shared_object_disabled,
        user_transaction_disabled: inner.user_transaction_disabled,
        receiving_objects_disabled: inner.receiving_objects_disabled,
        move_authenticator_disabled: inner.move_authenticator_disabled,
    }))
}

/// Collects a `LinkedTable`'s keys in list order by following the `next`
/// links, one derived-id child read per entry.
fn walk_linked_table<K>(
    object_store: &dyn ObjectStore,
    table: &LinkedTable<K>,
) -> IotaResult<Vec<K>>
where
    K: MoveTypeTagTrait + Serialize + DeserializeOwned + Clone + fmt::Debug,
{
    let mut keys = Vec::with_capacity(table.size as usize);
    let mut next = table.head.clone();
    while let Some(key) = next {
        // A cycle in the links would otherwise never terminate; the walk can
        // only visit `size` distinct entries.
        if keys.len() as u64 == table.size {
            return Err(IotaError::ObjectDeserialization {
                error: format!(
                    "LinkedTable {} has more linked entries than its size {}",
                    table.id, table.size
                ),
            });
        }
        let node: LinkedTableNode<K, bool> =
            get_dynamic_field_from_store(object_store, table.id, &key)?;
        keys.push(key);
        next = node.next;
    }
    if keys.len() as u64 != table.size {
        return Err(IotaError::ObjectDeserialization {
            error: format!(
                "LinkedTable {} links {} entries but its size is {}",
                table.id,
                keys.len(),
                table.size
            ),
        });
    }
    Ok(keys)
}
