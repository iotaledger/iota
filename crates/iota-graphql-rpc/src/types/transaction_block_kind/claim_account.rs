// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use async_graphql::*;
use iota_sdk_types::{
    AccountClaimKind as NativeAccountClaimKind,
    ClaimAccountTransaction as NativeClaimAccountTransaction,
    SmartAccountBuildKind as NativeSmartAccountBuildKind,
};

use crate::types::base64::Base64;

#[derive(Clone, Eq, PartialEq)]
pub(crate) struct ClaimAccountTransaction {
    pub native: NativeClaimAccountTransaction,
    /// The checkpoint sequence number this was viewed at.
    pub checkpoint_viewed_at: u64,
}

/// A transaction that creates an Account object via the constrained builder
/// pipeline.
#[Object]
impl ClaimAccountTransaction {
    /// The specific kind of account being claimed.
    async fn kind(&self) -> ClaimAccountKind {
        match &self.native.kind {
            NativeAccountClaimKind::SmartAccount(smart) => {
                ClaimAccountKind::SmartAccount(SmartAccountClaimTransaction {
                    public_key_scheme: smart.public_key.scheme() as u8,
                    public_key_bytes: Base64::from(smart.public_key.as_ref()),
                    claim_registry_initial_shared_version: smart
                        .claim_registry_initial_shared_version,
                    field_count: smart.fields.len() as u32,
                    build_kind: match smart.build_kind {
                        NativeSmartAccountBuildKind::Mutable => SmartAccountBuildKind::Mutable,
                        NativeSmartAccountBuildKind::Immutable => SmartAccountBuildKind::Immutable,
                    },
                })
            }
            _ => unimplemented!(
                "a new AccountClaimKind enum variant was added and needs to be handled"
            ),
        }
    }
}

/// The specific account type being claimed.
#[derive(Union, Clone, Eq, PartialEq)]
pub(crate) enum ClaimAccountKind {
    SmartAccount(SmartAccountClaimTransaction),
}

/// Parameters for the SmartAccount constrained builder pipeline.
#[derive(SimpleObject, Clone, Eq, PartialEq)]
pub(crate) struct SmartAccountClaimTransaction {
    /// The signature scheme flag byte (0=Ed25519, 1=Secp256k1, 2=Secp256r1,
    /// 6=Passkey).
    pub public_key_scheme: u8,
    /// The raw public key bytes, base64-encoded.
    pub public_key_bytes: Base64,
    /// Initial shared version of the ClaimRegistry object at 0x10.
    pub claim_registry_initial_shared_version: u64,
    /// Number of `with_field` calls in this claim (0 means no extra fields).
    pub field_count: u32,
    /// Whether the resulting SmartAccount object will be mutable (shared) or
    /// immutable (frozen).
    pub build_kind: SmartAccountBuildKind,
}

/// Selects the terminal builder function.
#[derive(Enum, Clone, Copy, Eq, PartialEq)]
pub(crate) enum SmartAccountBuildKind {
    /// The account will be a mutable shared object (`build_v1`).
    Mutable,
    /// The account will be an immutable frozen object (`build_immutable_v1`).
    Immutable,
}
