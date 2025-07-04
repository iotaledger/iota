// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use anyhow::{Ok, Result, anyhow};
use iota_sdk::{
    IotaClient,
    rpc_types::{IotaData, IotaObjectDataOptions, IotaTransactionBlockResponse, ObjectChange},
    types::{Identifier, id::UID},
};
use move_core_types::language_storage::StructTag;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

/// A Move struct representing a fully signed transaction on-chain.
///
/// This is typically created after all required participants (e.g., Alice &
/// Bob) have registered their verified signatures for a proposed transaction.
/// The object is published and stored on-chain as a Move object.
///
/// Fields:
/// - `id`: The UID of the SignedTx object.
/// - `tx_digest`: Digest of the original transaction.
/// - `tx_bytes`: Raw transaction bytes (used for re-submission).
/// - `verified_signatures`: Vector of base-encoded pure signatures (64-byte
///   each).
#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct SignedTx {
    pub id: UID,
    pub tx_digest: Vec<u8>,
    pub tx_bytes: Vec<u8>,
    pub verified_signatures: Vec<String>,
}

impl SignedTx {
    pub async fn from_tx_response(
        iota_client: &IotaClient,
        transaction_response: IotaTransactionBlockResponse,
    ) -> Result<Self> {
        let signed_tx_name = Identifier::new("SignedTx")
            .map_err(|e| anyhow!("Invalid identifier for SignedTx: {}", e))?;
        let signed_tx_object_id = transaction_response
            .object_changes
            .as_ref()
            .and_then(|changes| {
                changes.iter().find(|change| {
                    matches!(change, ObjectChange::Created {
                    object_type: StructTag {
                        name,
                        ..
                    },
                    ..
                } if name == &signed_tx_name)
                })
            })
            .map(|change| change.object_ref())
            .ok_or_else(|| anyhow!("No SignedTx object created in the transaction"))?
            .0;

        let signed_tx_object = iota_client
            .read_api()
            .get_object_with_options(
                signed_tx_object_id,
                IotaObjectDataOptions::default().with_bcs(),
            )
            .await?
            .data
            .ok_or_else(|| anyhow!("SignedTx object data is missing"))?;

        let signed_tx = bcs::from_bytes::<SignedTx>(
            &signed_tx_object
                .bcs
                .expect("should contain bcs")
                .try_as_move()
                .expect("should convert it to a move object")
                .bcs_bytes,
        )?;
        Ok(signed_tx)
    }
}
