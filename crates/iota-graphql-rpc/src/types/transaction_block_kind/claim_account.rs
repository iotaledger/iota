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

/// A transaction that claims the sender's address as an account object.
#[Object]
impl ClaimAccountTransaction {
    /// The type of account created by the claim.
    async fn kind(&self) -> ClaimAccountKind {
        match &self.native.kind {
            NativeAccountClaimKind::SmartAccount(smart) => {
                ClaimAccountKind::SmartAccount(SmartAccountClaimTransaction {
                    public_key_scheme: smart.public_key.scheme().to_u8(),
                    public_key_bytes: Base64::from(smart.public_key.as_ref()),
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

/// The type of account created by a claim.
#[derive(Union, Clone, Eq, PartialEq)]
pub(crate) enum ClaimAccountKind {
    SmartAccount(SmartAccountClaimTransaction),
}

/// Parameters the claimed SmartAccount was created with.
#[derive(SimpleObject, Clone, Eq, PartialEq)]
pub(crate) struct SmartAccountClaimTransaction {
    /// The signature scheme flag byte of the public key (0=Ed25519,
    /// 1=Secp256k1, 2=Secp256r1, 6=Passkey).
    pub public_key_scheme: u8,
    /// The raw public key bytes of the claimed address, base64-encoded.
    pub public_key_bytes: Base64,
    /// Whether the created account object is mutable or immutable.
    pub build_kind: SmartAccountBuildKind,
}

/// Whether a claimed account object can be changed after the claim.
#[derive(Enum, Clone, Copy, Eq, PartialEq)]
pub(crate) enum SmartAccountBuildKind {
    /// The account is a mutable shared object.
    Mutable,
    /// The account is an immutable frozen object.
    Immutable,
}
