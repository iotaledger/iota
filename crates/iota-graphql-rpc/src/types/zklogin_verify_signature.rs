// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use async_graphql::*;
use im::hashmap::HashMap as ImHashMap;
use iota_types::{
    IOTA_AUTHENTICATOR_STATE_ADDRESS, TypeTag,
    authenticator_state::{ActiveJwk, AuthenticatorStateInner},
    crypto::ToFromBytes,
    dynamic_field::{DynamicFieldType, Field},
    signature::{GenericSignature, VerifyParams},
    signature_verification::VerifiedDigestCache,
    transaction::TransactionData,
};
use shared_crypto::intent::{
    AppId, Intent, IntentMessage, IntentScope, IntentVersion, PersonalMessage,
};
use tracing::warn;

use crate::{
    config::ZkLoginConfig,
    error::Error,
    server::watermark_task::Watermark,
    types::{
        base64::Base64,
        dynamic_field::{DynamicField, DynamicFieldName},
        epoch::Epoch,
        iota_address::IotaAddress,
        type_filter::ExactTypeFilter,
    },
};

/// An enum that specifies the intent scope to be used to parse the bytes for
/// signature verification.
#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub(crate) enum ZkLoginIntentScope {
    /// Indicates that the bytes are to be parsed as transaction data bytes.
    TransactionData,
    /// Indicates that the bytes are to be parsed as a personal message.
    PersonalMessage,
}

/// The result of the zkLogin signature verification.
#[derive(SimpleObject, Clone, Debug)]
pub(crate) struct ZkLoginVerifyResult {
    /// The boolean result of the verification. If true, errors should be empty.
    pub success: bool,
    /// The errors field captures any verification error
    pub errors: Vec<String>,
}

/// Verifies a zkLogin signature based on the bytes (parsed as either
/// TransactionData or PersonalMessage based on the intent scope) and its
/// author.
pub(crate) async fn verify_zklogin_signature(
    ctx: &Context<'_>,
    bytes: Base64,
    signature: Base64,
    intent_scope: ZkLoginIntentScope,
    author: IotaAddress,
) -> Result<ZkLoginVerifyResult, Error> {
    let Watermark { checkpoint, .. } = *ctx.data_unchecked();

    // get current epoch from db.
    let Some(curr_epoch) = Epoch::query(ctx, None, checkpoint).await? else {
        return Err(Error::Internal(
            "Cannot get current epoch from db".to_string(),
        ));
    };
    let curr_epoch = curr_epoch.stored.epoch as u64;

    // get the zklogin_env that will be needed for verify the signature.
    let cfg: &ZkLoginConfig = ctx.data_unchecked();
    let zklogin_env_native = cfg.env;

    // validates and parse the signature as a zklogin signature.
    let GenericSignature::ZkLoginAuthenticator(zklogin_sig) =
        GenericSignature::from_bytes(&signature.0)
            .map_err(|_| Error::Client("Cannot parse generic signature".to_string()))?
    else {
        return Err(Error::Client(
            "Endpoint only supports zkLogin signature".to_string(),
        ));
    };

    // fetch on-chain JWKs from dynamic field of system object.
    let df = DynamicField::query(
        ctx,
        IOTA_AUTHENTICATOR_STATE_ADDRESS.into(),
        None,
        DynamicFieldName {
            type_: ExactTypeFilter(TypeTag::U64),
            bcs: Base64(bcs::to_bytes(&1u64).unwrap()),
        },
        DynamicFieldType::DynamicField,
        checkpoint,
    )
    .await
    .map_err(|e| as_jwks_read_error(e.to_string()))?;

    let binding = df.ok_or(as_jwks_read_error("Cannot find df".to_string()))?;
    let move_object = &binding.super_.native;

    let inner = bcs::from_bytes::<Field<u64, AuthenticatorStateInner>>(move_object.contents())
        .map_err(|e| as_jwks_read_error(e.to_string()))?
        .value;

    // construct verify params with active jwks and zklogin_env.
    let mut oidc_provider_jwks = ImHashMap::new();
    for active_jwk in &inner.active_jwks {
        let ActiveJwk { jwk_id, jwk, .. } = active_jwk;
        match oidc_provider_jwks.entry(jwk_id.clone()) {
            im::hashmap::Entry::Occupied(_) => {
                warn!("JWK with kid {:?} already exists", jwk_id);
            }
            im::hashmap::Entry::Vacant(entry) => {
                entry.insert(jwk.clone());
            }
        }
    }
    let verify_params = VerifyParams::new(
        oidc_provider_jwks,
        vec![],
        zklogin_env_native,
        true,
        Some(30),
    );

    let bytes = bytes.0;
    match intent_scope {
        ZkLoginIntentScope::TransactionData => {
            let tx_data: TransactionData = bcs::from_bytes(&bytes)
                .map_err(|_| Error::Client("Invalid tx data bytes".to_string()))?;
            let intent_msg = IntentMessage::new(Intent::iota_transaction(), tx_data.clone());
            let sig = GenericSignature::ZkLoginAuthenticator(zklogin_sig);
            match sig.verify_authenticator(
                &intent_msg,
                author.into(),
                curr_epoch,
                &verify_params,
                Arc::new(VerifiedDigestCache::new_empty()),
            ) {
                Ok(_) => Ok(ZkLoginVerifyResult {
                    success: true,
                    errors: vec![],
                }),
                Err(e) => Ok(ZkLoginVerifyResult {
                    success: false,
                    errors: vec![e.to_string()],
                }),
            }
        }
        ZkLoginIntentScope::PersonalMessage => {
            let data = PersonalMessage { message: bytes };
            let intent_msg = IntentMessage::new(
                Intent {
                    scope: IntentScope::PersonalMessage,
                    version: IntentVersion::V0,
                    app_id: AppId::Iota,
                },
                data,
            );

            let sig = GenericSignature::ZkLoginAuthenticator(zklogin_sig);
            match sig.verify_authenticator(
                &intent_msg,
                author.into(),
                curr_epoch,
                &verify_params,
                Arc::new(VerifiedDigestCache::new_empty()),
            ) {
                Ok(_) => Ok(ZkLoginVerifyResult {
                    success: true,
                    errors: vec![],
                }),
                Err(e) => Ok(ZkLoginVerifyResult {
                    success: false,
                    errors: vec![e.to_string()],
                }),
            }
        }
    }
}

/// Format the error message for failed JWK read.
fn as_jwks_read_error(e: String) -> Error {
    Error::Internal(format!("Failed to read JWK from system object 0x7: {}", e))
}