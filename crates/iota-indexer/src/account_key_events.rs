// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Rust mirrors of the account-discoverability Move events, and the fold that
//! turns them into `account_key_links` operations.
//!
//! The reverse index `key_id -> accounts` is a pure left-fold of five framework
//! events, applied in total order `(checkpoint, transaction, event)`:
//!
//! | Event | Effect |
//! |---|---|
//! | `claim_registry::ClaimedAddress` | link `(key_id, addr)`, source `Claim` |
//! | `builtin_authenticator_functions::PublicKeyAttached` | link `(key_id(pk), account)`, source `Attach` |
//! | `builtin_authenticator_functions::PublicKeyRotated` | unlink `(key_id(from), account)`, then link `(key_id(to), account)`, source `Rotate` |
//! | `builtin_authenticator_functions::PublicKeyDetached` | unlink `(key_id(pk), account)` |
//!
//! Because the whole index is derived from public events, any third party can
//! replay the same stream and reproduce it byte for byte.
//!
//! The structs below mirror BCS layouts frozen in the iota-framework
//! (`claim_registry.move`, `builtin_authenticator_functions.move`). Once the
//! feature activates on a public network those layouts evolve additively only:
//! new event types may be added, existing fields never change.

use iota_sdk_types::{Address, Identifier, ObjectId, events::Event};
use iota_types::claim_registry::key_id;
use serde::{Deserialize, Serialize};

const CLAIM_REGISTRY_MODULE: Identifier = Identifier::from_static("claim_registry");
const BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE: Identifier =
    Identifier::from_static("builtin_authenticator_functions");

const CLAIMED_ADDRESS_EVENT: Identifier = Identifier::from_static("ClaimedAddress");
const PUBLIC_KEY_ATTACHED_EVENT: Identifier = Identifier::from_static("PublicKeyAttached");
const PUBLIC_KEY_DETACHED_EVENT: Identifier = Identifier::from_static("PublicKeyDetached");
const PUBLIC_KEY_ROTATED_EVENT: Identifier = Identifier::from_static("PublicKeyRotated");

/// Mirror of `iota::signature_scheme::SignatureScheme`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MoveSignatureScheme {
    pub flag: u8,
}

/// Mirror of `iota::public_key::PublicKey`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MovePublicKey {
    pub scheme: MoveSignatureScheme,
    pub raw_bytes: Vec<u8>,
}

impl MovePublicKey {
    /// Returns the canonical identity hash of this key, as stored in the
    /// `key_id` column.
    pub fn key_id(&self) -> Vec<u8> {
        key_id(self.scheme.flag, &self.raw_bytes)
            .as_bytes()
            .to_vec()
    }
}

/// Mirror of `iota::claim_registry::ClaimedAddress`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClaimedAddressEvent {
    pub addr: Address,
    pub scheme: u8,
    pub key_id: Address,
    pub epoch: u64,
}

/// Mirror of `iota::builtin_authenticator_functions::PublicKeyAttached`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PublicKeyAttachedEvent {
    pub account_id: ObjectId,
    pub public_key: MovePublicKey,
}

/// Mirror of `iota::builtin_authenticator_functions::PublicKeyDetached`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PublicKeyDetachedEvent {
    pub account_id: ObjectId,
    pub public_key: MovePublicKey,
}

/// Mirror of `iota::builtin_authenticator_functions::PublicKeyRotated`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PublicKeyRotatedEvent {
    pub account_id: ObjectId,
    pub from: MovePublicKey,
    pub to: MovePublicKey,
}

/// Whether a fold step establishes a link or tombstones one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkOpKind {
    Link,
    Unlink,
}

/// Which operation the current state of a link came from.
///
/// The discriminants are persisted in the `source` column, so they are part of
/// the on-disk format and must not be renumbered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkSource {
    Claim = 0,
    Attach = 1,
    Rotate = 2,
    Detach = 3,
}

/// One step of the discoverability fold: the effect a single event has on a
/// single `(key_id, account_id)` pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountKeyLinkOp {
    pub key_id: Vec<u8>,
    pub account_id: Vec<u8>,
    pub scheme: i16,
    pub source: LinkSource,
    pub kind: LinkOpKind,
    pub tx_sequence_number: i64,
    pub epoch: i64,
}

/// Decodes `event` into the link operations it implies — none for events that
/// are not part of the discoverability stream, one for a claim, attach or
/// detach, and two for a rotation (the unlink of the old key first, then the
/// link of the new one).
///
/// An event whose contents fail to decode yields no operations: the type name
/// matched but the payload did not, which means this indexer build predates a
/// framework change and cannot interpret it.
pub fn account_key_link_ops(
    event: &Event,
    tx_sequence_number: i64,
    epoch: i64,
) -> Vec<AccountKeyLinkOp> {
    if is_framework_event(event, &CLAIM_REGISTRY_MODULE, &CLAIMED_ADDRESS_EVENT) {
        let Ok(claimed) = bcs::from_bytes::<ClaimedAddressEvent>(&event.contents) else {
            return vec![];
        };
        return vec![AccountKeyLinkOp {
            key_id: claimed.key_id.as_bytes().to_vec(),
            account_id: claimed.addr.as_bytes().to_vec(),
            scheme: claimed.scheme as i16,
            source: LinkSource::Claim,
            kind: LinkOpKind::Link,
            tx_sequence_number,
            epoch,
        }];
    }

    if is_framework_event(
        event,
        &BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE,
        &PUBLIC_KEY_ATTACHED_EVENT,
    ) {
        let Ok(attached) = bcs::from_bytes::<PublicKeyAttachedEvent>(&event.contents) else {
            return vec![];
        };
        return vec![link_op(
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
        &BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE,
        &PUBLIC_KEY_DETACHED_EVENT,
    ) {
        let Ok(detached) = bcs::from_bytes::<PublicKeyDetachedEvent>(&event.contents) else {
            return vec![];
        };
        return vec![link_op(
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
        &BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE,
        &PUBLIC_KEY_ROTATED_EVENT,
    ) {
        let Ok(rotated) = bcs::from_bytes::<PublicKeyRotatedEvent>(&event.contents) else {
            return vec![];
        };
        return ops_from_rotated(&rotated, tx_sequence_number, epoch);
    }

    vec![]
}

/// Splits a rotation into the unlink of the previous key followed by the link
/// of the new one. Order matters: rotating a key back onto itself must leave
/// the link active.
pub fn ops_from_rotated(
    rotated: &PublicKeyRotatedEvent,
    tx_sequence_number: i64,
    epoch: i64,
) -> Vec<AccountKeyLinkOp> {
    vec![
        link_op(
            &rotated.from,
            &rotated.account_id,
            LinkSource::Rotate,
            LinkOpKind::Unlink,
            tx_sequence_number,
            epoch,
        ),
        link_op(
            &rotated.to,
            &rotated.account_id,
            LinkSource::Rotate,
            LinkOpKind::Link,
            tx_sequence_number,
            epoch,
        ),
    ]
}

fn link_op(
    public_key: &MovePublicKey,
    account_id: &ObjectId,
    source: LinkSource,
    kind: LinkOpKind,
    tx_sequence_number: i64,
    epoch: i64,
) -> AccountKeyLinkOp {
    AccountKeyLinkOp {
        key_id: public_key.key_id(),
        account_id: account_id.as_bytes().to_vec(),
        scheme: public_key.scheme.flag as i16,
        source,
        kind,
        tx_sequence_number,
        epoch,
    }
}

fn is_framework_event(event: &Event, module: &Identifier, name: &Identifier) -> bool {
    event.type_.address() == Address::FRAMEWORK
        && event.type_.module() == module
        && event.type_.name() == name
}

#[cfg(test)]
mod tests {
    use iota_sdk_types::StructTag;

    use super::*;

    const ED25519_RAW: [u8; 32] = [0xAA; 32];
    const SECP256K1_RAW: [u8; 33] = [0xBB; 33];
    const ACCOUNT: [u8; 32] = [0x11; 32];

    fn move_public_key(flag: u8, raw_bytes: &[u8]) -> MovePublicKey {
        MovePublicKey {
            scheme: MoveSignatureScheme { flag },
            raw_bytes: raw_bytes.to_vec(),
        }
    }

    fn framework_event(module: &str, name: &str, contents: Vec<u8>) -> Event {
        Event {
            package_id: ObjectId::new(Address::FRAMEWORK.into_bytes()),
            module: Identifier::new(module).unwrap(),
            sender: Address::ZERO,
            type_: StructTag::new(
                Address::FRAMEWORK,
                Identifier::new(module).unwrap(),
                Identifier::new(name).unwrap(),
                vec![],
            ),
            contents,
        }
    }

    #[test]
    fn move_public_key_bcs_layout() {
        // Move `PublicKey { scheme: SignatureScheme { flag }, raw_bytes }`
        // serializes as the flag byte, then the ULEB-prefixed raw bytes.
        let mut contents = vec![0x00u8, 32];
        contents.extend(ED25519_RAW);

        let public_key: MovePublicKey = bcs::from_bytes(&contents).unwrap();

        assert_eq!(public_key.scheme.flag, 0x00);
        assert_eq!(public_key.raw_bytes, ED25519_RAW);
    }

    #[test]
    fn claimed_address_event_yields_a_claim_link() {
        let claimed = ClaimedAddressEvent {
            addr: Address::new(ACCOUNT),
            scheme: 0x00,
            key_id: Address::new([0x22; 32]),
            epoch: 3,
        };
        let event = framework_event(
            "claim_registry",
            "ClaimedAddress",
            bcs::to_bytes(&claimed).unwrap(),
        );

        let ops = account_key_link_ops(&event, 7, 3);

        assert_eq!(
            ops,
            vec![AccountKeyLinkOp {
                key_id: [0x22; 32].to_vec(),
                account_id: ACCOUNT.to_vec(),
                scheme: 0,
                source: LinkSource::Claim,
                kind: LinkOpKind::Link,
                tx_sequence_number: 7,
                epoch: 3,
            }]
        );
    }

    #[test]
    fn attached_event_yields_a_link_and_detached_an_unlink() {
        let public_key = move_public_key(0x00, &ED25519_RAW);
        let expected_key_id = public_key.key_id();

        let attached = framework_event(
            "builtin_authenticator_functions",
            "PublicKeyAttached",
            bcs::to_bytes(&(ACCOUNT, 0x00u8, ED25519_RAW.to_vec())).unwrap(),
        );
        let detached = framework_event(
            "builtin_authenticator_functions",
            "PublicKeyDetached",
            bcs::to_bytes(&(ACCOUNT, 0x00u8, ED25519_RAW.to_vec())).unwrap(),
        );

        let attach_ops = account_key_link_ops(&attached, 7, 3);
        let detach_ops = account_key_link_ops(&detached, 8, 3);

        assert_eq!(attach_ops.len(), 1);
        assert_eq!(attach_ops[0].kind, LinkOpKind::Link);
        assert_eq!(attach_ops[0].source, LinkSource::Attach);
        assert_eq!(attach_ops[0].key_id, expected_key_id);
        assert_eq!(attach_ops[0].account_id, ACCOUNT.to_vec());

        assert_eq!(detach_ops.len(), 1);
        assert_eq!(detach_ops[0].kind, LinkOpKind::Unlink);
        assert_eq!(detach_ops[0].source, LinkSource::Detach);
        assert_eq!(detach_ops[0].key_id, expected_key_id);
    }

    #[test]
    fn rotated_event_unlinks_the_old_key_before_linking_the_new_one() {
        let from = move_public_key(0x00, &ED25519_RAW);
        let to = move_public_key(0x01, &SECP256K1_RAW);
        let rotated = PublicKeyRotatedEvent {
            account_id: ObjectId::new(ACCOUNT),
            from: from.clone(),
            to: to.clone(),
        };

        let ops = ops_from_rotated(&rotated, 7, 3);

        assert_eq!(ops.len(), 2);
        assert_eq!(ops[0].kind, LinkOpKind::Unlink);
        assert_eq!(ops[0].key_id, from.key_id());
        assert_eq!(ops[0].scheme, 0);
        assert_eq!(ops[1].kind, LinkOpKind::Link);
        assert_eq!(ops[1].key_id, to.key_id());
        assert_eq!(ops[1].scheme, 1);
    }

    #[test]
    fn rotated_event_decodes_from_wire_bytes() {
        let contents = bcs::to_bytes(&(
            ACCOUNT,
            0x00u8,
            ED25519_RAW.to_vec(),
            0x01u8,
            SECP256K1_RAW.to_vec(),
        ))
        .unwrap();
        let event = framework_event(
            "builtin_authenticator_functions",
            "PublicKeyRotated",
            contents,
        );

        let ops = account_key_link_ops(&event, 7, 3);

        assert_eq!(ops.len(), 2);
        assert_eq!(ops[0].key_id, move_public_key(0x00, &ED25519_RAW).key_id());
        assert_eq!(
            ops[1].key_id,
            move_public_key(0x01, &SECP256K1_RAW).key_id()
        );
    }

    #[test]
    fn unrelated_events_yield_no_ops() {
        // Right name, wrong module.
        let wrong_module = framework_event("account", "ClaimedAddress", vec![]);
        // Right module, wrong name.
        let wrong_name = framework_event("claim_registry", "SomethingElse", vec![]);
        // Right type, but a payload this build cannot decode.
        let undecodable = framework_event("claim_registry", "ClaimedAddress", vec![0x00]);

        assert!(account_key_link_ops(&wrong_module, 7, 3).is_empty());
        assert!(account_key_link_ops(&wrong_name, 7, 3).is_empty());
        assert!(account_key_link_ops(&undecodable, 7, 3).is_empty());
    }

    #[test]
    fn events_from_other_packages_are_ignored() {
        let mut event = framework_event(
            "claim_registry",
            "ClaimedAddress",
            bcs::to_bytes(&ClaimedAddressEvent {
                addr: Address::new(ACCOUNT),
                scheme: 0x00,
                key_id: Address::new([0x22; 32]),
                epoch: 3,
            })
            .unwrap(),
        );
        event.type_ = StructTag::new(
            Address::new([0x99; 32]),
            Identifier::new("claim_registry").unwrap(),
            Identifier::new("ClaimedAddress").unwrap(),
            vec![],
        );

        assert!(account_key_link_ops(&event, 7, 3).is_empty());
    }
}
