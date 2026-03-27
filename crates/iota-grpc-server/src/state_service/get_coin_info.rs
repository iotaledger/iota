// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use iota_grpc_types::{
    google::rpc::bad_request::FieldViolation,
    v1::{
        coin::{
            CoinMetadata, CoinTreasury, RegulatedCoinMetadata, coin_treasury::SupplyState,
            regulated_coin_metadata::CoinRegulatedState,
        },
        error_reason::ErrorReason,
        state_service::{GetCoinInfoRequest, GetCoinInfoResponse},
    },
};

use crate::{error::RpcError, types::GrpcReader, validation::object_id_proto};

/// Get coin info for a given coin type.
///
/// This endpoint does not use the Merge trait / read_mask pattern because the
/// proto `GetCoinInfoRequest` has no `read_mask` field — all response fields
/// are always returned. The response is small and fixed-shape, so field
/// filtering would add complexity without meaningful savings.
#[tracing::instrument(skip(reader))]
pub(crate) fn get_coin_info(
    reader: Arc<GrpcReader>,
    GetCoinInfoRequest { coin_type, .. }: GetCoinInfoRequest,
) -> Result<GetCoinInfoResponse, RpcError> {
    // Validate coin_type
    let coin_type_str = coin_type.as_deref().ok_or_else(|| {
        FieldViolation::new("coin_type")
            .with_description("coin_type is required")
            .with_reason(ErrorReason::FieldMissing)
    })?;

    let core_coin_type = iota_types::parse_iota_struct_tag(coin_type_str).map_err(|e| {
        FieldViolation::new("coin_type")
            .with_description(format!("invalid coin_type: {e}"))
            .with_reason(ErrorReason::FieldInvalid)
    })?;

    let (coin_info, regulated_available) = reader.get_coin_v2_info(&core_coin_type)?;
    let coin_info = coin_info.ok_or_else(|| {
        RpcError::new(
            tonic::Code::NotFound,
            format!(
                "No coin info found for type {coin_type_str} \
                 — CoinMetadata or TreasuryCap may not be published"
            ),
        )
    })?;

    let mut response = GetCoinInfoResponse::default().with_coin_type(coin_type_str.to_string());

    // Populate metadata if available
    if let Some(coin_metadata_object_id) = coin_info.coin_metadata_object_id {
        if let Some(object) = reader.get_object(&coin_metadata_object_id)? {
            match iota_types::coin::CoinMetadata::try_from(object) {
                Ok(metadata) => {
                    let mut grpc_metadata = CoinMetadata::default()
                        .with_id(object_id_proto(&metadata.id.id.bytes))
                        .with_decimals(metadata.decimals as u32)
                        .with_name(metadata.name)
                        .with_symbol(metadata.symbol)
                        .with_description(metadata.description);
                    if let Some(icon_url) = metadata.icon_url {
                        grpc_metadata = grpc_metadata.with_icon_url(icon_url);
                    }
                    response.metadata = Some(grpc_metadata);
                }
                Err(e) => {
                    tracing::error!(
                        "Unable to read object {coin_metadata_object_id} as CoinMetadata \
                         for coin type {coin_type_str}: {e}"
                    );
                }
            }
        }
    }

    // Populate treasury if available
    if let Some(treasury_object_id) = coin_info.treasury_object_id {
        if let Some(object) = reader.get_object(&treasury_object_id)? {
            // Determine supply_state from ownership before consuming the object.
            // Immutable or owned by 0x0 means the TreasuryCap can never be used
            // to mint, so supply is fixed.
            let supply_state = match &object.owner {
                iota_types::object::Owner::Immutable => SupplyState::Fixed,
                iota_types::object::Owner::AddressOwner(addr)
                    if *addr == iota_types::base_types::IotaAddress::ZERO =>
                {
                    SupplyState::Fixed
                }
                _ => SupplyState::Unknown,
            };

            match iota_types::coin::TreasuryCap::try_from(object) {
                Ok(treasury) => {
                    response.treasury = Some(
                        CoinTreasury::default()
                            .with_id(object_id_proto(&treasury.id.id.bytes))
                            .with_total_supply(treasury.total_supply.value)
                            .with_supply_state(supply_state),
                    );
                }
                Err(e) => {
                    tracing::error!(
                        "Unable to read object {treasury_object_id} as TreasuryCap \
                         for coin type {coin_type_str}: {e}"
                    );
                }
            }
        }
    } else if iota_types::gas_coin::GAS::is_gas(&core_coin_type) {
        // Special case for native GAS coin: get treasury from system state.
        // The native gas coin's total supply is fixed (no TreasuryCap can mint more).
        let summary = reader.get_system_state_summary()?;
        response.treasury = Some(
            CoinTreasury::default()
                .with_id(object_id_proto(&summary.iota_treasury_cap_id()))
                .with_total_supply(summary.iota_total_supply())
                .with_supply_state(SupplyState::Fixed),
        );
    }

    // Populate regulated metadata.
    //
    // NOTE: Unlike `CoinMetadata` and `TreasuryCap` above which use the
    // type-safe `TryFrom<Object>` conversion, `RegulatedCoinMetadata` is
    // deserialized via raw `bcs::from_bytes`. This is because
    // `iota_types::deny_list_v1::RegulatedCoinMetadata` does not implement
    // `TryFrom<Object>`. If that impl is added upstream, this should be
    // updated to use it for consistency and better error handling.
    //
    // When `regulated_available` is false the `coin_v2` backfill has not
    // completed, so we cannot tell whether the coin is regulated. In that
    // case we omit the field entirely so clients see "field not populated"
    // rather than an incorrect "unregulated".
    if !regulated_available {
        // coin_v2 backfill in progress — regulated status unknown; omit field.
    } else if let Some(regulated_object_id) = coin_info.regulated_coin_metadata_object_id {
        if let Some(object) = reader.get_object(&regulated_object_id)? {
            if let Some(move_obj) = object.data.try_as_move() {
                match bcs::from_bytes::<iota_types::deny_list_v1::RegulatedCoinMetadata>(
                    move_obj.contents(),
                ) {
                    Ok(regulated) => {
                        // Proto fields `allow_global_pause` and `variant` are not populated
                        // because the on-chain `RegulatedCoinMetadata` struct does not store
                        // these values — it only contains `id`, `coin_metadata_object`, and
                        // `deny_cap_object`.
                        response.regulated_metadata = Some(
                            RegulatedCoinMetadata::default()
                                .with_id(object_id_proto(&regulated.id.id.bytes))
                                .with_coin_metadata_object(object_id_proto(
                                    &regulated.coin_metadata_object.bytes,
                                ))
                                .with_deny_cap_object(object_id_proto(
                                    &regulated.deny_cap_object.bytes,
                                ))
                                .with_coin_regulated_state(CoinRegulatedState::Regulated),
                        );
                    }
                    Err(e) => {
                        tracing::error!(
                            "Unable to read object {regulated_object_id} as \
                             RegulatedCoinMetadata for coin type {coin_type_str}: {e}"
                        );
                    }
                }
            } else {
                tracing::error!("Object {regulated_object_id} is not a Move object");
            }
        }
    } else {
        // No RegulatedCoinMetadata object exists for this coin type — explicitly
        // mark as unregulated so clients can distinguish "not regulated" from
        // "field not populated".
        response.regulated_metadata = Some(
            RegulatedCoinMetadata::default()
                .with_coin_regulated_state(CoinRegulatedState::Unregulated),
        );
    }

    Ok(response)
}
