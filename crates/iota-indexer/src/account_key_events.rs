// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Rust mirrors of the account-discoverability Move events, and the fold that
//! turns them into `account_key_links` and `claimed_accounts` rows.
//!
//! These types mirror BCS layouts frozen in the iota-framework
//! (`builtin_authenticator_functions.move`, `smart_account.move`). Once the
//! feature activates on a public network their evolution is additive-only: new
//! event types may be added, existing fields never change.
//!
//! The fold is total. `key_id` hashes bytes without parsing them, no address is
//! derived anywhere, and an event whose type matches but whose payload will not
//! decode yields nothing — this build predates a framework change and cannot
//! interpret it. There is no other failure path.

use iota_sdk_types::{Address, Event, ObjectId};
use iota_types::account_abstraction::public_key::MovePublicKey;
use serde::Deserialize;

use crate::models::claimed_accounts::StoredClaimedAccount;

const BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE: &str = "builtin_authenticator_functions";
const SMART_ACCOUNT_MODULE: &str = "smart_account";

const PUBLIC_KEY_ATTACHED: &str = "PublicKeyAttached";
const PUBLIC_KEY_DETACHED: &str = "PublicKeyDetached";
const PUBLIC_KEY_ROTATED: &str = "PublicKeyRotated";
const SMART_ACCOUNT_CLAIMED: &str = "SmartAccountClaimed";

/// Mirror of `iota::builtin_authenticator_functions::PublicKeyAttached`.
#[derive(Debug, Clone, Deserialize)]
pub struct PublicKeyAttachedEvent {
    pub account_id: ObjectId,
    pub public_key: MovePublicKey,
}

/// Mirror of `iota::builtin_authenticator_functions::PublicKeyDetached`.
#[derive(Debug, Clone, Deserialize)]
pub struct PublicKeyDetachedEvent {
    pub account_id: ObjectId,
    pub public_key: MovePublicKey,
}

/// Mirror of `iota::builtin_authenticator_functions::PublicKeyRotated`.
#[derive(Debug, Clone, Deserialize)]
pub struct PublicKeyRotatedEvent {
    pub account_id: ObjectId,
    pub from: MovePublicKey,
    pub to: MovePublicKey,
}

/// Mirror of `iota::smart_account::SmartAccountClaimed`.
#[derive(Debug, Clone, Deserialize)]
pub struct SmartAccountClaimedEvent {
    pub account_id: ObjectId,
    pub public_key: MovePublicKey,
    pub immutable: bool,
}

/// Whether a fold step establishes a link or tombstones one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkOpKind {
    Link,
    Unlink,
}

/// Provenance of a link row.
///
/// Discriminants are persisted in the `source` column: never renumber them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkSource {
    Attach = 0,
    Rotate = 1,
    Detach = 2,
    Claim = 3,
}

impl LinkSource {
    /// The variant a persisted `source` value stands for, or `None` if it was
    /// written by a build that knows a provenance this one does not.
    pub fn from_stored(value: i16) -> Option<Self> {
        match value {
            0 => Some(Self::Attach),
            1 => Some(Self::Rotate),
            2 => Some(Self::Detach),
            3 => Some(Self::Claim),
            _ => None,
        }
    }
}

/// One step of the fold: the effect a single event has on one
/// `(key_id, account_id)` pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountKeyLinkOp {
    pub key_id: [u8; 32],
    pub account_id: [u8; 32],
    /// The signature scheme flag of the key, uninterpreted.
    pub scheme: u8,
    pub source: LinkSource,
    pub kind: LinkOpKind,
    pub tx_sequence_number: i64,
    pub epoch: i64,
}

impl AccountKeyLinkOp {
    fn new(
        public_key: &MovePublicKey,
        account_id: &ObjectId,
        source: LinkSource,
        kind: LinkOpKind,
        tx_sequence_number: u64,
        epoch: u64,
    ) -> Self {
        Self {
            key_id: public_key.key_id(),
            account_id: (*account_id).into(),
            scheme: public_key.scheme_flag(),
            source,
            kind,
            tx_sequence_number: tx_sequence_number as i64,
            epoch: epoch as i64,
        }
    }
}

/// Whether `event`'s type is the framework struct `0x2::<module>::<name>`.
fn is_framework_event(event: &Event, module: &str, name: &str) -> bool {
    event.struct_tag.address() == Address::FRAMEWORK
        && event.struct_tag.module().as_str() == module
        && event.struct_tag.name().as_str() == name
}

/// Decodes `event` into the link operations it contributes, in the order they
/// must be applied. Events that are not part of the discoverability stream, and
/// payloads this build cannot decode, yield an empty vector.
pub fn account_key_link_ops(
    event: &Event,
    tx_sequence_number: u64,
    epoch: u64,
) -> Vec<AccountKeyLinkOp> {
    if is_framework_event(
        event,
        BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE,
        PUBLIC_KEY_ATTACHED,
    ) {
        let Ok(attached) = bcs::from_bytes::<PublicKeyAttachedEvent>(&event.contents) else {
            return vec![];
        };
        return vec![AccountKeyLinkOp::new(
            &attached.public_key,
            &attached.account_id,
            LinkSource::Attach,
            LinkOpKind::Link,
            tx_sequence_number,
            epoch,
        )];
    }

    if is_framework_event(
        event,
        BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE,
        PUBLIC_KEY_DETACHED,
    ) {
        let Ok(detached) = bcs::from_bytes::<PublicKeyDetachedEvent>(&event.contents) else {
            return vec![];
        };
        return vec![AccountKeyLinkOp::new(
            &detached.public_key,
            &detached.account_id,
            LinkSource::Detach,
            LinkOpKind::Unlink,
            tx_sequence_number,
            epoch,
        )];
    }

    if is_framework_event(
        event,
        BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE,
        PUBLIC_KEY_ROTATED,
    ) {
        let Ok(rotated) = bcs::from_bytes::<PublicKeyRotatedEvent>(&event.contents) else {
            return vec![];
        };
        // The unlink must come first, so that rotating a key onto itself leaves
        // the link active.
        return vec![
            AccountKeyLinkOp::new(
                &rotated.from,
                &rotated.account_id,
                LinkSource::Rotate,
                LinkOpKind::Unlink,
                tx_sequence_number,
                epoch,
            ),
            AccountKeyLinkOp::new(
                &rotated.to,
                &rotated.account_id,
                LinkSource::Rotate,
                LinkOpKind::Link,
                tx_sequence_number,
                epoch,
            ),
        ];
    }

    if is_framework_event(event, SMART_ACCOUNT_MODULE, SMART_ACCOUNT_CLAIMED) {
        let Ok(claimed) = bcs::from_bytes::<SmartAccountClaimedEvent>(&event.contents) else {
            return vec![];
        };
        return vec![AccountKeyLinkOp::new(
            &claimed.public_key,
            &claimed.account_id,
            LinkSource::Claim,
            LinkOpKind::Link,
            tx_sequence_number,
            epoch,
        )];
    }

    vec![]
}

/// The `claimed_accounts` row `event` contributes, if it is a claim.
pub fn claimed_account_row(
    event: &Event,
    tx_sequence_number: u64,
    epoch: u64,
) -> Option<StoredClaimedAccount> {
    if !is_framework_event(event, SMART_ACCOUNT_MODULE, SMART_ACCOUNT_CLAIMED) {
        return None;
    }
    let claimed = bcs::from_bytes::<SmartAccountClaimedEvent>(&event.contents).ok()?;
    Some(StoredClaimedAccount {
        account_id: claimed.account_id.as_bytes().to_vec(),
        key_id: claimed.public_key.key_id().to_vec(),
        immutable: claimed.immutable,
        claim_tx_sequence_number: tx_sequence_number as i64,
        claim_epoch: epoch as i64,
    })
}

#[cfg(test)]
mod tests {
    use iota_sdk_types::{Identifier, SignatureScheme, StructTag};

    use super::*;

    // Same key material as the Move test vectors in public_key_tests.move, so
    // the key_id pinned below is directly comparable across the two languages.
    const ED25519_RAW_HEX: &str =
        "cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88";
    const SECP256K1_RAW_HEX: &str =
        "02337cca2171fdbfcfd657fa59881f46269f1e590b5ffab6023686c7ad2ecc2c1c";
    /// `blake2b256(0x00 || ED25519_RAW)`, pinned by `key_id_vectors` in
    /// `public_key_tests.move`.
    const ED25519_KEY_ID_HEX: &str =
        "43541042c153e0e498a08a8db868f1614c9366694fa730bd8a07fc5d7c931f0d";

    const ACCOUNT: [u8; 32] = [0x11; 32];

    #[test]
    fn move_public_key_bcs_layout_and_key_id_match_move() {
        // flag || uleb(len) || raw, the Move `PublicKey` layout.
        let bytes = move_public_key_bcs(SignatureScheme::Ed25519.to_u8(), &ed25519_raw());
        let decoded: MovePublicKey = bcs::from_bytes(&bytes).unwrap();

        assert_eq!(decoded.scheme_flag(), SignatureScheme::Ed25519.to_u8());
        assert_eq!(
            decoded.key_id().to_vec(),
            hex::decode(ED25519_KEY_ID_HEX).unwrap()
        );
    }

    #[test]
    fn link_source_discriminants_round_trip() {
        // The discriminants are persisted, so this pins them against a
        // renumbering that would silently reinterpret existing rows.
        for source in [
            LinkSource::Attach,
            LinkSource::Rotate,
            LinkSource::Detach,
            LinkSource::Claim,
        ] {
            assert_eq!(LinkSource::from_stored(source as i16), Some(source));
        }
        assert_eq!(LinkSource::from_stored(4), None);
    }

    #[test]
    fn attached_event_yields_one_active_attach_link() {
        let ops = account_key_link_ops(&attached_event(), 7, 3);

        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].source, LinkSource::Attach);
        assert_eq!(ops[0].kind, LinkOpKind::Link);
        assert_eq!(ops[0].account_id, ACCOUNT);
        assert_eq!(ops[0].scheme, SignatureScheme::Ed25519.to_u8());
        assert_eq!(ops[0].tx_sequence_number, 7);
        assert_eq!(ops[0].epoch, 3);
    }

    #[test]
    fn detached_event_tombstones_the_link() {
        let ops = account_key_link_ops(&detached_event(), 7, 3);

        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].source, LinkSource::Detach);
        assert_eq!(ops[0].kind, LinkOpKind::Unlink);
    }

    #[test]
    fn rotated_event_produces_unlink_then_link() {
        let ops = account_key_link_ops(&rotated_event(&ed25519_raw(), &secp256k1_raw()), 7, 3);

        assert_eq!(ops.len(), 2);
        assert_eq!(ops[0].kind, LinkOpKind::Unlink);
        assert_eq!(ops[1].kind, LinkOpKind::Link);
        assert_ne!(ops[0].key_id, ops[1].key_id);
        assert!(ops.iter().all(|op| op.source == LinkSource::Rotate));
    }

    #[test]
    fn rotation_onto_the_same_key_ends_linked() {
        // Both ops address the same row, so the order decides the final state:
        // the unlink must come first for the row to stay active.
        let ops = account_key_link_ops(&rotated_event(&ed25519_raw(), &ed25519_raw()), 7, 3);

        assert_eq!(ops[0].key_id, ops[1].key_id);
        assert_eq!(ops[0].kind, LinkOpKind::Unlink);
        assert_eq!(ops[1].kind, LinkOpKind::Link);
    }

    #[test]
    fn claimed_event_yields_a_claim_link_and_a_claimed_row() {
        let event = claimed_event(false);
        let ops = account_key_link_ops(&event, 9, 4);

        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].source, LinkSource::Claim);
        assert_eq!(ops[0].kind, LinkOpKind::Link);

        let row = claimed_account_row(&event, 9, 4).unwrap();
        assert_eq!(row.account_id, ACCOUNT.to_vec());
        assert_eq!(row.key_id, ops[0].key_id.to_vec());
        assert!(!row.immutable);
        assert_eq!(row.claim_tx_sequence_number, 9);
        assert_eq!(row.claim_epoch, 4);
    }

    #[test]
    fn claimed_event_carries_the_build_kind() {
        let row = claimed_account_row(&claimed_event(true), 9, 4).unwrap();
        assert!(row.immutable);
    }

    #[test]
    fn only_a_claim_writes_a_claimed_row() {
        assert!(claimed_account_row(&attached_event(), 1, 1).is_none());
        assert!(claimed_account_row(&detached_event(), 1, 1).is_none());
        assert!(
            claimed_account_row(&rotated_event(&ed25519_raw(), &secp256k1_raw()), 1, 1).is_none()
        );
    }

    #[test]
    fn events_from_another_package_are_ignored() {
        let mut event = claimed_event(false);
        event.struct_tag = StructTag::new(
            Address::new([0x99; 32]),
            Identifier::new(SMART_ACCOUNT_MODULE).unwrap(),
            Identifier::new(SMART_ACCOUNT_CLAIMED).unwrap(),
            Vec::new(),
        );

        assert!(account_key_link_ops(&event, 1, 1).is_empty());
        assert!(claimed_account_row(&event, 1, 1).is_none());
    }

    #[test]
    fn other_structs_in_a_matched_module_are_ignored() {
        let mut event = claimed_event(false);
        event.struct_tag = framework_struct_tag(SMART_ACCOUNT_MODULE, "SomethingElse");

        assert!(account_key_link_ops(&event, 1, 1).is_empty());
        assert!(claimed_account_row(&event, 1, 1).is_none());
    }

    #[test]
    fn a_payload_this_build_cannot_decode_yields_nothing() {
        let mut event = claimed_event(false);
        event.contents.truncate(4);

        assert!(account_key_link_ops(&event, 1, 1).is_empty());
        assert!(claimed_account_row(&event, 1, 1).is_none());
    }

    #[test]
    fn an_unknown_scheme_flag_does_not_fail_the_fold() {
        // A scheme added to the framework after this build. key_id hashes the
        // flag without interpreting it, so the link is still indexed.
        let unknown_flag = 0x7f;
        let mut contents = ACCOUNT.to_vec();
        contents.extend(move_public_key_bcs(unknown_flag, &ed25519_raw()));
        let event = framework_event(
            BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE,
            PUBLIC_KEY_ATTACHED,
            contents,
        );

        let ops = account_key_link_ops(&event, 1, 1);
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].scheme, unknown_flag);
    }

    // === Helpers ===

    fn ed25519_raw() -> Vec<u8> {
        hex::decode(ED25519_RAW_HEX).unwrap()
    }

    fn secp256k1_raw() -> Vec<u8> {
        hex::decode(SECP256K1_RAW_HEX).unwrap()
    }

    /// The Move `PublicKey` wire layout: `flag || uleb(len) || raw_bytes`.
    fn move_public_key_bcs(flag: u8, raw_bytes: &[u8]) -> Vec<u8> {
        let mut bytes = vec![flag];
        bytes.extend(bcs::to_bytes(&raw_bytes.to_vec()).unwrap());
        bytes
    }

    fn framework_struct_tag(module: &str, name: &str) -> StructTag {
        StructTag::new(
            Address::FRAMEWORK,
            Identifier::new(module).unwrap(),
            Identifier::new(name).unwrap(),
            Vec::new(),
        )
    }

    fn framework_event(module: &str, name: &str, contents: Vec<u8>) -> Event {
        Event {
            package_id: Address::FRAMEWORK.into(),
            module: Identifier::new(module).unwrap(),
            sender: Address::new([0x22; 32]),
            struct_tag: framework_struct_tag(module, name),
            contents,
        }
    }

    fn attached_event() -> Event {
        let mut contents = ACCOUNT.to_vec();
        contents.extend(move_public_key_bcs(
            SignatureScheme::Ed25519.to_u8(),
            &ed25519_raw(),
        ));
        framework_event(
            BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE,
            PUBLIC_KEY_ATTACHED,
            contents,
        )
    }

    fn detached_event() -> Event {
        let mut event = attached_event();
        event.struct_tag =
            framework_struct_tag(BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE, PUBLIC_KEY_DETACHED);
        event
    }

    fn rotated_event(from_raw: &[u8], to_raw: &[u8]) -> Event {
        let flag_of = |raw: &[u8]| {
            if raw.len() == 32 {
                SignatureScheme::Ed25519.to_u8()
            } else {
                SignatureScheme::Secp256k1.to_u8()
            }
        };
        let mut contents = ACCOUNT.to_vec();
        contents.extend(move_public_key_bcs(flag_of(from_raw), from_raw));
        contents.extend(move_public_key_bcs(flag_of(to_raw), to_raw));
        framework_event(
            BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE,
            PUBLIC_KEY_ROTATED,
            contents,
        )
    }

    fn claimed_event(immutable: bool) -> Event {
        let mut contents = ACCOUNT.to_vec();
        contents.extend(move_public_key_bcs(
            SignatureScheme::Ed25519.to_u8(),
            &ed25519_raw(),
        ));
        contents.push(immutable as u8);
        framework_event(SMART_ACCOUNT_MODULE, SMART_ACCOUNT_CLAIMED, contents)
    }
}
