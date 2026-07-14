// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeSet;

use iota_sdk_types::{Address, ObjectId};
use serde::{Deserialize, Serialize};

use crate::base_types::AuthorityName;

/// Read access to a set of transaction deny rules.
///
/// Implemented by both a validator's local `TransactionDenyConfig` and the
/// consensus-governed [`DenyRuleSet`], so the deny checks can run against
/// either source without knowing which one is in effect.
pub trait DenyRuleConfig {
    /// Whether `address` is denied as a transaction sender or gas sponsor.
    fn is_address_denied(&self, address: &Address) -> bool;
    /// Whether the object `id` is denied as an input or receiving object.
    fn is_object_denied(&self, id: &ObjectId) -> bool;
    /// Whether the package `id` is denied as a (transitive) dependency.
    fn is_package_denied(&self, id: &ObjectId) -> bool;
    /// Whether any address is denied; lets checks skip scanning signers when
    /// there are none.
    fn has_denied_addresses(&self) -> bool;
    /// Whether any object is denied; lets checks skip scanning input and
    /// receiving objects when there are none.
    fn has_denied_objects(&self) -> bool;
    /// Whether any package is denied; lets checks skip resolving package
    /// dependencies (which loads packages from the store) when there are none.
    fn has_denied_packages(&self) -> bool;
    fn package_publish_disabled(&self) -> bool;
    fn package_upgrade_disabled(&self) -> bool;
    fn shared_object_disabled(&self) -> bool;
    fn user_transaction_disabled(&self) -> bool;
    fn receiving_objects_disabled(&self) -> bool;
    fn move_authenticator_disabled(&self) -> bool;
}

/// A complete set of deny rules.
///
/// Deny lists use `BTreeSet` so the BCS encoding is deterministic across
/// validators (a requirement for the consensus messages that carry this type).
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DenyRuleSet {
    /// Addresses denied as transaction sender or gas sponsor. A denied
    /// address can still receive objects.
    pub denied_addresses: BTreeSet<Address>,
    /// Objects denied as transaction inputs or receiving objects.
    pub denied_objects: BTreeSet<ObjectId>,
    /// Packages denied as a (transitive) dependency of any command; upgrading
    /// a denied package is denied too.
    pub denied_packages: BTreeSet<ObjectId>,
    /// Denies all package publishing.
    pub package_publish_disabled: bool,
    /// Denies all package upgrades.
    pub package_upgrade_disabled: bool,
    /// Denies transactions that use shared objects as inputs.
    pub shared_object_disabled: bool,
    /// Denies all user transactions (kill switch).
    pub user_transaction_disabled: bool,
    /// Denies transactions that contain receiving objects.
    pub receiving_objects_disabled: bool,
    /// Denies transactions signed with a Move authenticator.
    pub move_authenticator_disabled: bool,
}

impl DenyRuleConfig for DenyRuleSet {
    fn is_address_denied(&self, address: &Address) -> bool {
        self.denied_addresses.contains(address)
    }

    fn is_object_denied(&self, id: &ObjectId) -> bool {
        self.denied_objects.contains(id)
    }

    fn is_package_denied(&self, id: &ObjectId) -> bool {
        self.denied_packages.contains(id)
    }

    fn has_denied_addresses(&self) -> bool {
        !self.denied_addresses.is_empty()
    }

    fn has_denied_objects(&self) -> bool {
        !self.denied_objects.is_empty()
    }

    fn has_denied_packages(&self) -> bool {
        !self.denied_packages.is_empty()
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

/// A validator's full-state proposal for the network deny rules, announced
/// through consensus.
///
/// Each proposal carries the authority's complete proposed rule set; the latest
/// generation per authority supersedes earlier ones. The active rule set the
/// network enforces is the stake-weighted aggregate of all current proposals.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DenyRuleProposal {
    /// The authority announcing this proposal.
    pub authority: AuthorityName,
    /// Per-authority counter used to deduplicate proposals; a higher generation
    /// supersedes earlier proposals from the same authority.
    pub generation: u64,
    /// The complete set of rules this authority proposes.
    pub proposed_rules: DenyRuleSet,
}

#[cfg(test)]
mod tests {
    use iota_sdk_types::{Address, ObjectId};

    use crate::{
        crypto::AuthorityPublicKeyBytes,
        deny_rule_governance::{DenyRuleConfig, DenyRuleProposal, DenyRuleSet},
    };

    fn sample_rule_set() -> DenyRuleSet {
        DenyRuleSet {
            denied_addresses: [Address::new([1u8; 32]), Address::new([2u8; 32])]
                .into_iter()
                .collect(),
            denied_objects: [ObjectId::new([3u8; 32])].into_iter().collect(),
            denied_packages: [ObjectId::new([4u8; 32])].into_iter().collect(),
            package_publish_disabled: true,
            package_upgrade_disabled: false,
            shared_object_disabled: true,
            user_transaction_disabled: false,
            receiving_objects_disabled: true,
            move_authenticator_disabled: false,
        }
    }

    #[test]
    fn deny_rule_set_bcs_round_trip() {
        let rules = sample_rule_set();
        let bytes = bcs::to_bytes(&rules).unwrap();
        assert_eq!(rules, bcs::from_bytes(&bytes).unwrap());
    }

    #[test]
    fn deny_rule_proposal_bcs_round_trip() {
        let proposal = DenyRuleProposal {
            authority: AuthorityPublicKeyBytes::ZERO,
            generation: 42,
            proposed_rules: sample_rule_set(),
        };
        let bytes = bcs::to_bytes(&proposal).unwrap();
        assert_eq!(proposal, bcs::from_bytes(&bytes).unwrap());
    }

    #[test]
    fn deny_rule_config_reflects_set_contents() {
        let rules = sample_rule_set();

        assert!(rules.is_address_denied(&Address::new([1u8; 32])));
        assert!(!rules.is_address_denied(&Address::new([9u8; 32])));
        assert!(rules.is_object_denied(&ObjectId::new([3u8; 32])));
        assert!(!rules.is_object_denied(&ObjectId::new([9u8; 32])));
        assert!(rules.is_package_denied(&ObjectId::new([4u8; 32])));
        assert!(!rules.is_package_denied(&ObjectId::new([9u8; 32])));

        assert!(rules.has_denied_addresses());
        assert!(rules.has_denied_objects());
        assert!(rules.has_denied_packages());

        assert!(rules.package_publish_disabled());
        assert!(!rules.package_upgrade_disabled());
        assert!(rules.shared_object_disabled());
        assert!(!rules.user_transaction_disabled());
        assert!(rules.receiving_objects_disabled());
        assert!(!rules.move_authenticator_disabled());
    }

    /// An empty set denies nothing.
    #[test]
    fn empty_deny_rule_set_denies_nothing() {
        let rules = DenyRuleSet::default();
        assert!(!rules.is_address_denied(&Address::new([1u8; 32])));
        assert!(!rules.is_object_denied(&ObjectId::new([1u8; 32])));
        assert!(!rules.has_denied_addresses());
        assert!(!rules.has_denied_objects());
        assert!(!rules.has_denied_packages());
        assert!(!rules.user_transaction_disabled());
    }
}
