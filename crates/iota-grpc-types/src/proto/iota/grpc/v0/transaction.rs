// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

include!("../../../generated/iota.grpc.v0.transaction.rs");
include!("../../../generated/iota.grpc.v0.transaction.field_info.rs");
include!("../../../generated/iota.grpc.v0.transaction.accessors.rs");

use crate::{field::FieldMaskTree, merge::Merge, v0::bcs::BcsData};

impl Merge<iota_types::effects::TransactionEffects> for TransactionEffects {
    fn merge(
        &mut self,
        source: iota_types::effects::TransactionEffects,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if !mask.contains(Self::DIGEST_FIELD.name) && !mask.contains(Self::BCS_FIELD.name) {
            // No need to convert if no field is requested
            return Ok(());
        }

        // Convert iota_types to iota_sdk_types types for external compatibility
        let sdk_effects: iota_sdk_types::TransactionEffects = source
            .try_into()
            .map_err(|e| format!("failed to convert effects to SDK type: {e}"))?;

        Merge::merge(self, &sdk_effects, mask)
    }
}

impl Merge<&iota_sdk_types::TransactionEffects> for TransactionEffects {
    fn merge(
        &mut self,
        source: &iota_sdk_types::TransactionEffects,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Set digest if requested
        if mask.contains(Self::DIGEST_FIELD.name) {
            self.digest = Some(source.digest().into());
        }

        // Set BCS if requested
        if mask.contains(Self::BCS_FIELD.name) {
            self.bcs = BcsData::serialize(source).ok();
        }

        Ok(())
    }
}

impl Merge<iota_types::effects::TransactionEvents> for TransactionEvents {
    fn merge(
        &mut self,
        source: iota_types::effects::TransactionEvents,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if !mask.contains(Self::DIGEST_FIELD.name) && !mask.contains(Self::EVENTS_FIELD.name) {
            // No need to convert if no field is requested
            return Ok(());
        }

        let sdk_events: iota_sdk_types::TransactionEvents = source
            .try_into()
            .map_err(|e| format!("failed to convert events to SDK type: {e}"))?;

        Merge::merge(self, &sdk_events, mask)
    }
}

// TODO: Wrap TransactionEvents into a type with a version
impl Merge<&iota_sdk_types::TransactionEvents> for TransactionEvents {
    fn merge(
        &mut self,
        source: &iota_sdk_types::TransactionEvents,
        mask: &FieldMaskTree,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Set digest if requested
        if mask.contains(Self::DIGEST_FIELD.name) {
            self.digest = Some(source.digest().into());
        }

        if let Some(events_mask) = mask.subtree(Self::EVENTS_FIELD.name) {
            self.events = Some(super::event::Events::merge_from(source, &events_mask)?);
        }

        Ok(())
    }
}
