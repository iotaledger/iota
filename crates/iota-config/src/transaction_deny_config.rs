// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashSet;

use iota_sdk_types::{Address, ObjectId};
use iota_types::deny_rule_governance::DenyRuleConfig;
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct TransactionDenyConfig {
    /// A list of object IDs that are not allowed to be accessed/used in
    /// transactions. Note that since this is checked during transaction
    /// signing, only root object ids are supported here (i.e. no
    /// child-objects). Similarly this does not apply to wrapped objects as
    /// they are not directly accessible.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    object_deny_list: Vec<ObjectId>,

    /// A list of package object IDs that are not allowed to be called into in
    /// transactions, either directly or indirectly through transitive
    /// dependencies. Note that this does not apply to type arguments.
    /// Also since we only compare the deny list against the upgraded package ID
    /// of each dependency in the used package, when a package ID is denied,
    /// newer versions of that package are still allowed. If we want to deny
    /// the entire upgrade family of a package, we need to explicitly
    /// specify all the package IDs in the deny list. TODO: We could
    /// consider making this more flexible, e.g. whether to check in type args,
    /// whether to block entire upgrade family, whether to allow upgrade and
    /// etc.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    package_deny_list: Vec<ObjectId>,

    /// A list of iota addresses that are not allowed to be used as the sender
    /// or sponsor.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    address_deny_list: Vec<Address>,

    /// Whether publishing new packages is disabled.
    #[serde(default)]
    package_publish_disabled: bool,

    /// Whether upgrading existing packages is disabled.
    #[serde(default)]
    package_upgrade_disabled: bool,

    /// Whether usage of shared objects is disabled.
    #[serde(default)]
    shared_object_disabled: bool,

    /// Whether user transactions are disabled (i.e. only system transactions
    /// are allowed). This is essentially a kill switch for transactions
    /// processing to a degree.
    #[serde(default)]
    user_transaction_disabled: bool,

    /// In-memory maps for faster lookup of various lists.
    #[serde(skip)]
    object_deny_set: OnceCell<HashSet<ObjectId>>,

    #[serde(skip)]
    package_deny_set: OnceCell<HashSet<ObjectId>>,

    #[serde(skip)]
    address_deny_set: OnceCell<HashSet<Address>>,

    /// Whether receiving objects transferred to other objects is allowed
    #[serde(default)]
    receiving_objects_disabled: bool,

    /// Whether `MoveAuthenticator` is disabled
    #[serde(default)]
    move_authenticator_disabled: bool,
    // TODO: We could consider add a deny list for types that we want to disable public transfer.
    // TODO: We could also consider disable more types of commands, such as transfer, split and
    // etc.
}

impl TransactionDenyConfig {
    pub fn get_object_deny_set(&self) -> &HashSet<ObjectId> {
        self.object_deny_set
            .get_or_init(|| self.object_deny_list.iter().cloned().collect())
    }

    pub fn get_package_deny_set(&self) -> &HashSet<ObjectId> {
        self.package_deny_set
            .get_or_init(|| self.package_deny_list.iter().cloned().collect())
    }

    pub fn get_address_deny_set(&self) -> &HashSet<Address> {
        self.address_deny_set
            .get_or_init(|| self.address_deny_list.iter().cloned().collect())
    }

    pub fn package_publish_disabled(&self) -> bool {
        self.package_publish_disabled
    }

    pub fn package_upgrade_disabled(&self) -> bool {
        self.package_upgrade_disabled
    }

    pub fn shared_object_disabled(&self) -> bool {
        self.shared_object_disabled
    }

    pub fn user_transaction_disabled(&self) -> bool {
        self.user_transaction_disabled
    }

    pub fn receiving_objects_disabled(&self) -> bool {
        self.receiving_objects_disabled
    }

    pub fn move_authenticator_disabled(&self) -> bool {
        self.move_authenticator_disabled
    }
}

#[derive(Default)]
pub struct TransactionDenyConfigBuilder {
    config: TransactionDenyConfig,
}

impl TransactionDenyConfigBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn build(self) -> TransactionDenyConfig {
        self.config
    }

    pub fn disable_user_transaction(mut self) -> Self {
        self.config.user_transaction_disabled = true;
        self
    }

    pub fn disable_shared_object_transaction(mut self) -> Self {
        self.config.shared_object_disabled = true;
        self
    }

    pub fn disable_package_publish(mut self) -> Self {
        self.config.package_publish_disabled = true;
        self
    }

    pub fn disable_package_upgrade(mut self) -> Self {
        self.config.package_upgrade_disabled = true;
        self
    }

    pub fn disable_receiving_objects(mut self) -> Self {
        self.config.receiving_objects_disabled = true;
        self
    }

    pub fn add_denied_object(mut self, id: ObjectId) -> Self {
        self.config.object_deny_list.push(id);
        self
    }

    pub fn add_denied_address(mut self, address: Address) -> Self {
        self.config.address_deny_list.push(address);
        self
    }

    pub fn add_denied_package(mut self, id: ObjectId) -> Self {
        self.config.package_deny_list.push(id);
        self
    }

    pub fn disable_move_authenticator(mut self) -> Self {
        self.config.move_authenticator_disabled = true;
        self
    }
}

impl DenyRuleConfig for TransactionDenyConfig {
    fn is_address_denied(&self, address: &Address) -> bool {
        self.get_address_deny_set().contains(address)
    }

    fn is_object_denied(&self, id: &ObjectId) -> bool {
        self.get_object_deny_set().contains(id)
    }

    fn is_package_denied(&self, id: &ObjectId) -> bool {
        self.get_package_deny_set().contains(id)
    }

    fn has_denied_addresses(&self) -> bool {
        !self.address_deny_list.is_empty()
    }

    fn has_denied_objects(&self) -> bool {
        !self.object_deny_list.is_empty()
    }

    fn has_denied_packages(&self) -> bool {
        !self.package_deny_list.is_empty()
    }

    fn package_publish_disabled(&self) -> bool {
        self.package_publish_disabled
    }

    fn package_upgrade_disabled(&self) -> bool {
        self.package_upgrade_disabled
    }

    fn shared_object_disabled(&self) -> bool {
        self.shared_object_disabled
    }

    fn user_transaction_disabled(&self) -> bool {
        self.user_transaction_disabled
    }

    fn receiving_objects_disabled(&self) -> bool {
        self.receiving_objects_disabled
    }

    fn move_authenticator_disabled(&self) -> bool {
        self.move_authenticator_disabled
    }
}

#[cfg(test)]
mod tests {
    use iota_sdk_types::{Address, ObjectId};

    use super::{DenyRuleConfig, TransactionDenyConfig, TransactionDenyConfigBuilder};

    #[test]
    fn trait_impl_reflects_config() {
        let addr = Address::new([1u8; 32]);
        let obj = ObjectId::new([2u8; 32]);
        let pkg = ObjectId::new([3u8; 32]);
        let config = TransactionDenyConfigBuilder::new()
            .add_denied_address(addr)
            .add_denied_object(obj)
            .add_denied_package(pkg)
            .disable_user_transaction()
            .disable_shared_object_transaction()
            .disable_move_authenticator()
            .build();

        // Exercise the trait via dynamic dispatch, exactly as the deny checks do.
        let deny: &dyn DenyRuleConfig = &config;
        assert!(deny.is_address_denied(&addr));
        assert!(!deny.is_address_denied(&Address::new([9u8; 32])));
        assert!(deny.is_object_denied(&obj));
        assert!(!deny.is_object_denied(&ObjectId::new([9u8; 32])));
        assert!(deny.is_package_denied(&pkg));
        assert!(deny.has_denied_addresses());
        assert!(deny.has_denied_objects());
        assert!(deny.has_denied_packages());
        assert!(deny.user_transaction_disabled());
        assert!(deny.shared_object_disabled());
        assert!(deny.move_authenticator_disabled());
        assert!(!deny.package_publish_disabled());
        assert!(!deny.package_upgrade_disabled());
        assert!(!deny.receiving_objects_disabled());

        let empty: &dyn DenyRuleConfig = &TransactionDenyConfig::default();
        assert!(!empty.has_denied_addresses());
        assert!(!empty.has_denied_objects());
        assert!(!empty.has_denied_packages());
    }
}
