// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use async_graphql::*;
use iota_sdk_types::TransactionDenyRulesUpdate as NativeTransactionDenyRulesUpdate;

use crate::types::{epoch::Epoch, iota_address::IotaAddress, uint53::UInt53};

#[derive(Clone, Eq, PartialEq)]
pub(crate) struct TransactionDenyRulesUpdateTransaction {
    pub native: NativeTransactionDenyRulesUpdate,
    /// The checkpoint sequence number this was viewed at.
    pub checkpoint_viewed_at: u64,
}

/// System transaction that applies an add/remove delta to the on-chain
/// transaction deny rules; the switch states are absolute.
#[Object]
impl TransactionDenyRulesUpdateTransaction {
    /// Epoch of the deny rules update transaction.
    async fn epoch(&self, ctx: &Context<'_>) -> Result<Option<Epoch>> {
        Epoch::query(ctx, Some(self.native.epoch), self.checkpoint_viewed_at)
            .await
            .extend()
    }

    /// Consensus round of the update.
    async fn round(&self) -> Result<UInt53> {
        UInt53::try_from(self.native.round).extend()
    }

    /// Addresses added to the sender-or-sponsor deny list.
    async fn added_addresses(&self) -> Vec<IotaAddress> {
        self.native
            .added_addresses
            .iter()
            .copied()
            .map(IotaAddress::from)
            .collect()
    }

    /// Addresses removed from the sender-or-sponsor deny list.
    async fn removed_addresses(&self) -> Vec<IotaAddress> {
        self.native
            .removed_addresses
            .iter()
            .copied()
            .map(IotaAddress::from)
            .collect()
    }

    /// Objects added to the input-or-receiving deny list.
    async fn added_objects(&self) -> Vec<IotaAddress> {
        self.native
            .added_objects
            .iter()
            .copied()
            .map(IotaAddress::from)
            .collect()
    }

    /// Objects removed from the input-or-receiving deny list.
    async fn removed_objects(&self) -> Vec<IotaAddress> {
        self.native
            .removed_objects
            .iter()
            .copied()
            .map(IotaAddress::from)
            .collect()
    }

    /// Packages added to the dependency deny list.
    async fn added_packages(&self) -> Vec<IotaAddress> {
        self.native
            .added_packages
            .iter()
            .copied()
            .map(IotaAddress::from)
            .collect()
    }

    /// Packages removed from the dependency deny list.
    async fn removed_packages(&self) -> Vec<IotaAddress> {
        self.native
            .removed_packages
            .iter()
            .copied()
            .map(IotaAddress::from)
            .collect()
    }

    /// Whether all package publishing is denied.
    async fn package_publish_disabled(&self) -> bool {
        self.native.package_publish_disabled
    }

    /// Whether all package upgrades are denied.
    async fn package_upgrade_disabled(&self) -> bool {
        self.native.package_upgrade_disabled
    }

    /// Whether transactions that use shared objects as inputs are denied.
    async fn shared_object_disabled(&self) -> bool {
        self.native.shared_object_disabled
    }

    /// Whether all user transactions are denied.
    async fn user_transaction_disabled(&self) -> bool {
        self.native.user_transaction_disabled
    }

    /// Whether transactions that contain receiving objects are denied.
    async fn receiving_objects_disabled(&self) -> bool {
        self.native.receiving_objects_disabled
    }

    /// Whether transactions signed with a Move authenticator are denied.
    async fn move_authenticator_disabled(&self) -> bool {
        self.native.move_authenticator_disabled
    }

    /// The initial version the deny rules object was shared at.
    async fn deny_rules_obj_initial_shared_version(&self) -> Result<UInt53> {
        UInt53::try_from(self.native.deny_rules_obj_initial_shared_version.as_u64()).extend()
    }
}
