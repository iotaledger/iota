// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::result::Result;

use anyhow::{Ok, anyhow, bail};
use iota_json_rpc_types::IotaObjectDataOptions;
use iota_sdk_types::{Identifier, ObjectId};
use iota_types::{
    base_types::IotaAddress,
    move_package::MovePackage,
    object::Owner,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::{
        Argument, CallArg, SharedObjectRef, TransactionData, TransactionDataAPI, TransactionKind,
    },
};

use crate::TransactionBuilder;

impl TransactionBuilder {
    /// Build a [`TransactionKind::Programmable`] that contains
    /// [`iota_sdk_types::Command::Publish`] for the provided package.
    pub async fn publish_tx_kind(
        &self,
        sender: IotaAddress,
        modules: Vec<Vec<u8>>,
        dep_ids: Vec<ObjectId>,
    ) -> Result<TransactionKind, anyhow::Error> {
        let pt = {
            let mut builder = ProgrammableTransactionBuilder::new();
            let upgrade_cap = builder.publish_upgradeable(modules, dep_ids);
            builder.transfer_arg(sender, upgrade_cap);
            builder.finish()
        };
        Ok(TransactionKind::new_programmable(pt))
    }

    /// Publish a new move package.
    pub async fn publish(
        &self,
        sender: IotaAddress,
        compiled_modules: Vec<Vec<u8>>,
        dep_ids: Vec<ObjectId>,
        gas: impl Into<Option<ObjectId>>,
        gas_budget: u64,
    ) -> anyhow::Result<TransactionData> {
        let gas_price = self.0.get_reference_gas_price().await?;
        let gas = self
            .select_gas(sender, gas, gas_budget, vec![], gas_price)
            .await?;
        Ok(TransactionData::new_module(
            sender,
            gas,
            compiled_modules,
            dep_ids,
            gas_budget,
            gas_price,
        ))
    }

    /// Build a [`TransactionKind::Programmable`] that contains
    /// [`iota_sdk_types::Command::Upgrade`] for the provided package.
    pub async fn upgrade_tx_kind(
        &self,
        package_id: ObjectId,
        modules: Vec<Vec<u8>>,
        dep_ids: Vec<ObjectId>,
        upgrade_capability: ObjectId,
        upgrade_policy: u8,
        digest: Vec<u8>,
    ) -> Result<TransactionKind, anyhow::Error> {
        let upgrade_capability = self
            .0
            .get_object_with_options(
                upgrade_capability,
                IotaObjectDataOptions::new().with_owner(),
            )
            .await?
            .into_object()?;
        let capability_owner = upgrade_capability
            .owner
            .ok_or_else(|| anyhow!("Unable to determine ownership of upgrade capability"))?;
        let pt = {
            let mut builder = ProgrammableTransactionBuilder::new();
            let capability_arg = match capability_owner {
                Owner::Address(_) => CallArg::ImmutableOrOwned(upgrade_capability.object_ref()),
                Owner::Shared(initial_shared_version) => CallArg::Shared(SharedObjectRef::new(
                    upgrade_capability.object_ref().object_id,
                    initial_shared_version,
                    true,
                )),
                Owner::Immutable => {
                    bail!("Upgrade capability is stored immutably and cannot be used for upgrades")
                }
                // If the capability is owned by an object, then the module defining the owning
                // object gets to decide how the upgrade capability should be used.
                Owner::Object(_) => {
                    bail!("Upgrade capability controlled by object");
                }
                _ => unimplemented!("a new Owner enum variant was added and needs to be handled"),
            };
            builder.obj(capability_arg).unwrap();
            let upgrade_arg = builder.pure(upgrade_policy).unwrap();
            let digest_arg = builder.pure(digest).unwrap();
            let upgrade_ticket = builder.programmable_move_call(
                ObjectId::FRAMEWORK,
                Identifier::PACKAGE_MODULE,
                Identifier::from_static("authorize_upgrade"),
                vec![],
                vec![Argument::Input(0), upgrade_arg, digest_arg],
            );
            let upgrade_receipt = builder.upgrade(package_id, upgrade_ticket, dep_ids, modules);

            builder.programmable_move_call(
                ObjectId::FRAMEWORK,
                Identifier::PACKAGE_MODULE,
                Identifier::from_static("commit_upgrade"),
                vec![],
                vec![Argument::Input(0), upgrade_receipt],
            );

            builder.finish()
        };

        Ok(TransactionKind::new_programmable(pt))
    }

    /// Upgrade an existing move package.
    pub async fn upgrade(
        &self,
        sender: IotaAddress,
        package_id: ObjectId,
        compiled_modules: Vec<Vec<u8>>,
        dep_ids: Vec<ObjectId>,
        upgrade_capability: ObjectId,
        upgrade_policy: u8,
        gas: impl Into<Option<ObjectId>>,
        gas_budget: u64,
    ) -> anyhow::Result<TransactionData> {
        let gas_price = self.0.get_reference_gas_price().await?;
        let gas = self
            .select_gas(sender, gas, gas_budget, vec![], gas_price)
            .await?;
        let upgrade_cap = self
            .0
            .get_object_with_options(
                upgrade_capability,
                IotaObjectDataOptions::new().with_owner(),
            )
            .await?
            .into_object()?;
        let cap_owner = upgrade_cap
            .owner
            .ok_or_else(|| anyhow!("Unable to determine ownership of upgrade capability"))?;
        let digest = MovePackage::compute_digest_for_modules_and_deps(&compiled_modules, &dep_ids)
            .into_inner()
            .to_vec();
        TransactionData::new_upgrade(
            sender,
            gas,
            package_id,
            compiled_modules,
            dep_ids,
            (upgrade_cap.object_ref(), cap_owner),
            upgrade_policy,
            digest,
            gas_budget,
            gas_price,
        )
    }
}
