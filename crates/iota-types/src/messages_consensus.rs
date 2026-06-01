// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::hash_map::DefaultHasher,
    fmt::{Debug, Formatter},
    hash::{Hash, Hasher},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use byteorder::{BigEndian, ReadBytesExt};
use fastcrypto::{error::FastCryptoResult, groups::bls12381, hash::HashFunction};
use fastcrypto_tbls::dkg_v1;
use iota_sdk_types::crypto::IntentScope;
pub use iota_sdk_types::{
    CancelledTransaction, CheckpointTimestamp as TimestampMs, ConsensusCommitPrologueV1,
    ConsensusDeterminedVersionAssignments, VersionAssignment,
};
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::{
    base_types::{AuthorityName, ConciseableName, ObjectRef, TransactionDigest},
    crypto::{AuthoritySignature, DefaultHash, default_hash},
    digests::{Digest, MisbehaviorReportDigest},
    message_envelope::{Envelope, Message, VerifiedEnvelope},
    messages_checkpoint::{CheckpointSequenceNumber, CheckpointSignatureMessage},
    supported_protocol_versions::{
        Chain, SupportedProtocolVersions, SupportedProtocolVersionsWithHashes,
    },
    transaction::CertifiedTransaction,
};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ConsensusTransaction {
    /// Encodes an u64 unique tracking id to allow us trace a message between
    /// IOTA and consensus. Use an byte array instead of u64 to ensure stable
    /// serialization.
    pub tracking_id: [u8; 8],
    pub kind: ConsensusTransactionKind,
}

#[derive(Serialize, Deserialize, Clone, Hash, PartialEq, Eq, Ord, PartialOrd)]
pub enum ConsensusTransactionKey {
    Certificate(TransactionDigest),
    CheckpointSignature(AuthorityName, CheckpointSequenceNumber),
    EndOfPublish(AuthorityName),
    CapabilityNotification(AuthorityName, u64 /* generation */),
    #[deprecated(note = "Authenticator state (JWK) is deprecated and was never enabled on IOTA")]
    NewJWKFetchedDeprecated,
    RandomnessDkgMessage(AuthorityName),
    RandomnessDkgConfirmation(AuthorityName),
    MisbehaviorReport(
        AuthorityName,
        MisbehaviorReportDigest,
        CheckpointSequenceNumber,
    ),
    // New entries should be added at the end to preserve serialization compatibility. DO NOT
    // CHANGE THE ORDER OF EXISTING ENTRIES!
}

impl Debug for ConsensusTransactionKey {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Certificate(digest) => write!(f, "Certificate({digest})"),
            Self::CheckpointSignature(name, seq) => {
                write!(f, "CheckpointSignature({:?}, {:?})", name.concise(), seq)
            }
            Self::EndOfPublish(name) => write!(f, "EndOfPublish({:?})", name.concise()),
            Self::CapabilityNotification(name, generation) => write!(
                f,
                "CapabilityNotification({:?}, {:?})",
                name.concise(),
                generation
            ),
            #[allow(deprecated)]
            Self::NewJWKFetchedDeprecated => {
                write!(
                    f,
                    "NewJWKFetched(deprecated: Authenticator state (JWK) is deprecated and was never enabled on IOTA)"
                )
            }
            Self::RandomnessDkgMessage(name) => {
                write!(f, "RandomnessDkgMessage({:?})", name.concise())
            }
            Self::RandomnessDkgConfirmation(name) => {
                write!(f, "RandomnessDkgConfirmation({:?})", name.concise())
            }
            Self::MisbehaviorReport(name, digest, checkpoint_seq) => {
                write!(
                    f,
                    "MisbehaviorReport({:?}, {:?}, {:?})",
                    name.concise(),
                    digest,
                    checkpoint_seq
                )
            }
        }
    }
}

pub type SignedAuthorityCapabilitiesV1 = Envelope<AuthorityCapabilitiesV1, AuthoritySignature>;

pub type VerifiedAuthorityCapabilitiesV1 =
    VerifiedEnvelope<AuthorityCapabilitiesV1, AuthoritySignature>;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AuthorityCapabilitiesDigest(Digest);

impl AuthorityCapabilitiesDigest {
    pub const fn new(digest: [u8; 32]) -> Self {
        Self(Digest::new(digest))
    }
}

impl Debug for AuthorityCapabilitiesDigest {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("AuthorityCapabilitiesDigest")
            .field(&self.0)
            .finish()
    }
}

/// Used to advertise capabilities of each authority via consensus. This allows
/// validators to negotiate the creation of the ChangeEpoch transaction.
#[derive(Serialize, Deserialize, Clone, Hash)]
pub struct AuthorityCapabilitiesV1 {
    /// Originating authority - must match transaction source authority from
    /// consensus or the signature of a non-committee active validator.
    pub authority: AuthorityName,
    /// Generation number set by sending authority. Used to determine which of
    /// multiple AuthorityCapabilities messages from the same authority is
    /// the most recent.
    ///
    /// (Currently, we just set this to the current time in milliseconds since
    /// the epoch, but this should not be interpreted as a timestamp.)
    pub generation: u64,

    /// ProtocolVersions that the authority supports, including the hash of the
    /// serialized ProtocolConfig of that authority per version.
    pub supported_protocol_versions: SupportedProtocolVersionsWithHashes,

    /// The ObjectRefs of all versions of system packages that the validator
    /// possesses. Used to determine whether to do a framework/movestdlib
    /// upgrade.
    pub available_system_packages: Vec<ObjectRef>,
}

impl Message for AuthorityCapabilitiesV1 {
    type DigestType = AuthorityCapabilitiesDigest;
    const SCOPE: IntentScope = IntentScope::AuthorityCapabilities;

    fn digest(&self) -> Self::DigestType {
        // Ensure deterministic serialization for digest
        let mut hasher = DefaultHash::new();
        let serialized = bcs::to_bytes(&self).expect("BCS should not fail");
        hasher.update(&serialized);
        AuthorityCapabilitiesDigest::new(<[u8; 32]>::from(hasher.finalize()))
    }
}

impl Debug for AuthorityCapabilitiesV1 {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthorityCapabilities")
            .field("authority", &self.authority.concise())
            .field("generation", &self.generation)
            .field(
                "supported_protocol_versions",
                &self.supported_protocol_versions,
            )
            .field("available_system_packages", &self.available_system_packages)
            .finish()
    }
}

impl AuthorityCapabilitiesV1 {
    pub fn new(
        authority: AuthorityName,
        chain: Chain,
        supported_protocol_versions: SupportedProtocolVersions,
        available_system_packages: Vec<ObjectRef>,
    ) -> Self {
        let generation = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("IOTA did not exist prior to 1970")
            .as_millis()
            .try_into()
            .expect("This build of iota is not supported in the year 500,000,000");
        Self {
            authority,
            generation,
            supported_protocol_versions:
                SupportedProtocolVersionsWithHashes::from_supported_versions(
                    supported_protocol_versions,
                    chain,
                ),
            available_system_packages,
        }
    }
}

impl SignedAuthorityCapabilitiesV1 {
    pub fn cache_digest(&self, epoch: u64) -> AuthorityCapabilitiesDigest {
        // Create a tuple that includes both the capabilities data and the epoch
        let data_with_epoch = (self.data(), epoch);

        // Ensure deterministic serialization for digest
        let mut hasher = DefaultHash::new();
        let serialized = bcs::to_bytes(&data_with_epoch).expect("BCS should not fail");
        hasher.update(&serialized);
        AuthorityCapabilitiesDigest::new(<[u8; 32]>::from(hasher.finalize()))
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ConsensusTransactionKind {
    CertifiedTransaction(Box<CertifiedTransaction>),
    CheckpointSignature(Box<CheckpointSignatureMessage>),
    EndOfPublish(AuthorityName),

    CapabilityNotificationV1(AuthorityCapabilitiesV1),
    SignedCapabilityNotificationV1(SignedAuthorityCapabilitiesV1),

    #[deprecated(note = "Authenticator state (JWK) is deprecated and was never enabled on IOTA")]
    NewJWKFetchedDeprecated,

    // DKG is used to generate keys for use in the random beacon protocol.
    // `RandomnessDkgMessage` is sent out at start-of-epoch to initiate the process.
    // Contents are a serialized `fastcrypto_tbls::dkg::Message`.
    RandomnessDkgMessage(AuthorityName, Vec<u8>),
    // `RandomnessDkgConfirmation` is the second DKG message, sent as soon as a threshold amount
    // of `RandomnessDkgMessages` have been received locally, to complete the key generation
    // process. Contents are a serialized `fastcrypto_tbls::dkg::Confirmation`.
    RandomnessDkgConfirmation(AuthorityName, Vec<u8>),
    MisbehaviorReport(VersionedMisbehaviorReport),
    // New entries should be added at the end to preserve serialization compatibility. DO NOT
    // CHANGE THE ORDER OF EXISTING ENTRIES!
}

impl ConsensusTransactionKind {
    pub fn is_dkg(&self) -> bool {
        matches!(
            self,
            ConsensusTransactionKind::RandomnessDkgMessage(_, _)
                | ConsensusTransactionKind::RandomnessDkgConfirmation(_, _)
        )
    }
}

/// A misbehavior report carrying a versioned payload plus a memoized digest.
///
/// Wire format is BCS over the `Serialize`-derived fields in declaration order:
/// `authority || payload || generation`. This exactly matches the pre-refactor
/// `ConsensusTransactionKind::MisbehaviorReport(AuthorityName,
/// VersionedMisbehaviorReport { payload }, CheckpointSequenceNumber)` 3-tuple
/// — see `tests::misbehavior_report_wire_format_unchanged` which pins the
/// equivalence. Reordering or inserting any non-`skip` field here would change
/// the consensus wire format and halt a running testnet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionedMisbehaviorReport {
    /// Originating authority — must match the transaction source authority
    /// from consensus. Verified at the consensus boundary.
    pub authority: AuthorityName,
    /// Versioned payload of the misbehavior report.
    pub payload: MisbehaviorObservations,
    /// Generation number set by the sending authority. Used to identify the
    /// most recent report from each authority. Currently set to the
    /// checkpoint sequence number at which the report was generated.
    pub generation: u64,
    #[serde(skip)]
    digest: OnceCell<MisbehaviorReportDigest>,
}

/// Versioned per-authority misbehavior observations. New variants get their
/// own named-field payload type (`MisbehaviorObservationsV2`,
/// `MisbehaviorObservationsV3`, ...) so the wire schema stays compile-time
/// checked. Also serves as the in-memory representation in
/// `MisbehaviorMonitor` / `ReportAggregator`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MisbehaviorObservations {
    V1(MisbehaviorObservationsV1),
}

impl VersionedMisbehaviorReport {
    pub fn new_v1(
        authority: AuthorityName,
        generation: u64,
        observations: MisbehaviorObservationsV1,
    ) -> Self {
        Self {
            authority,
            payload: MisbehaviorObservations::V1(observations),
            generation,
            digest: OnceCell::new(),
        }
    }

    /// Returns the digest of the misbehavior report, caching it if it has not
    /// been computed yet.
    pub fn digest(&self) -> &MisbehaviorReportDigest {
        self.digest
            .get_or_init(|| MisbehaviorReportDigest::new(default_hash(self)))
    }

    /// Returns the summary of the misbehavior report, defined as the sum of all
    /// metrics for all authorities.
    pub fn summary(&self) -> u64 {
        let summary = match &self.payload {
            MisbehaviorObservations::V1(report) => [
                &report.faulty_blocks_provable,
                &report.faulty_blocks_unprovable,
                &report.missing_proposals,
                &report.equivocations,
            ]
            .into_iter()
            .flatten()
            .fold(0u64, |acc, metric| acc.saturating_add(*metric)),
        };
        if summary == u64::MAX {
            warn!("MisbehaviorReport summary reached its maximum value.");
        }
        summary
    }
}

/// V1 misbehavior observations: per-authority counts for each tracked
/// misbehavior category (faulty blocks, equivocations, missing proposals).
/// Field order is part of the wire format — BCS serializes named struct
/// fields in declaration order. This first version does not include any
/// type of proof.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MisbehaviorObservationsV1 {
    pub faulty_blocks_provable: Vec<u64>,
    pub faulty_blocks_unprovable: Vec<u64>,
    pub missing_proposals: Vec<u64>,
    pub equivocations: Vec<u64>,
}

impl MisbehaviorObservationsV1 {
    pub fn verify(&self, committee_size: usize) -> bool {
        // This version of reports are valid as long as they contain the counts for all
        // authorities. Future versions may contain proofs that need verification.
        // However, since the validity of a proof is deeply coupled with the protocol
        // version and the consensus mechanism being used, we cannot verify it here. In
        // the future, reports should be unwrapped (or translated) to a type verifiable
        // by the starfish crate, which means that the verification logic will probably
        // move out of this crate.
        if (self.faulty_blocks_provable.len() != committee_size)
            || (self.faulty_blocks_unprovable.len() != committee_size)
            || (self.equivocations.len() != committee_size)
            || (self.missing_proposals.len() != committee_size)
        {
            return false;
        }
        true
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VersionedDkgMessage {
    V1(dkg_v1::Message<bls12381::G2Element, bls12381::G2Element>),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VersionedDkgConfirmation {
    V1(dkg_v1::Confirmation<bls12381::G2Element>),
}

impl Debug for VersionedDkgMessage {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            VersionedDkgMessage::V1(msg) => write!(
                f,
                "DKG V1 Message with sender={}, vss_pk.degree={}, encrypted_shares.len()={}",
                msg.sender,
                msg.vss_pk.degree(),
                msg.encrypted_shares.len(),
            ),
        }
    }
}

impl VersionedDkgMessage {
    pub fn sender(&self) -> u16 {
        match self {
            VersionedDkgMessage::V1(msg) => msg.sender,
        }
    }

    pub fn create(
        dkg_version: u64,
        party: Arc<dkg_v1::Party<bls12381::G2Element, bls12381::G2Element>>,
    ) -> FastCryptoResult<VersionedDkgMessage> {
        assert_eq!(dkg_version, 1, "BUG: invalid DKG version");
        let msg = party.create_message(&mut rand::thread_rng())?;
        Ok(VersionedDkgMessage::V1(msg))
    }

    pub fn unwrap_v1(self) -> dkg_v1::Message<bls12381::G2Element, bls12381::G2Element> {
        match self {
            VersionedDkgMessage::V1(msg) => msg,
        }
    }

    pub fn is_valid_version(&self, dkg_version: u64) -> bool {
        matches!((self, dkg_version), (VersionedDkgMessage::V1(_), 1))
    }
}

impl VersionedDkgConfirmation {
    pub fn sender(&self) -> u16 {
        match self {
            VersionedDkgConfirmation::V1(msg) => msg.sender,
        }
    }

    pub fn num_of_complaints(&self) -> usize {
        match self {
            VersionedDkgConfirmation::V1(msg) => msg.complaints.len(),
        }
    }

    pub fn unwrap_v1(&self) -> &dkg_v1::Confirmation<bls12381::G2Element> {
        match self {
            VersionedDkgConfirmation::V1(msg) => msg,
        }
    }

    pub fn is_valid_version(&self, dkg_version: u64) -> bool {
        matches!((self, dkg_version), (VersionedDkgConfirmation::V1(_), 1))
    }
}

impl ConsensusTransaction {
    pub fn new_certificate_message(
        authority: &AuthorityName,
        certificate: CertifiedTransaction,
    ) -> Self {
        let mut hasher = DefaultHasher::new();
        let tx_digest = certificate.digest();
        tx_digest.hash(&mut hasher);
        authority.hash(&mut hasher);
        let tracking_id = hasher.finish().to_le_bytes();
        Self {
            tracking_id,
            kind: ConsensusTransactionKind::CertifiedTransaction(Box::new(certificate)),
        }
    }

    pub fn new_checkpoint_signature_message(data: CheckpointSignatureMessage) -> Self {
        let mut hasher = DefaultHasher::new();
        data.summary.auth_sig().signature.hash(&mut hasher);
        let tracking_id = hasher.finish().to_le_bytes();
        Self {
            tracking_id,
            kind: ConsensusTransactionKind::CheckpointSignature(Box::new(data)),
        }
    }

    pub fn new_end_of_publish(authority: AuthorityName) -> Self {
        let mut hasher = DefaultHasher::new();
        authority.hash(&mut hasher);
        let tracking_id = hasher.finish().to_le_bytes();
        Self {
            tracking_id,
            kind: ConsensusTransactionKind::EndOfPublish(authority),
        }
    }

    pub fn new_capability_notification_v1(capabilities: AuthorityCapabilitiesV1) -> Self {
        let mut hasher = DefaultHasher::new();
        capabilities.hash(&mut hasher);
        let tracking_id = hasher.finish().to_le_bytes();
        Self {
            tracking_id,
            kind: ConsensusTransactionKind::CapabilityNotificationV1(capabilities),
        }
    }

    pub fn new_signed_capability_notification_v1(
        signed_capabilities: SignedAuthorityCapabilitiesV1,
    ) -> Self {
        let mut hasher = DefaultHasher::new();
        signed_capabilities.data().hash(&mut hasher);
        signed_capabilities.auth_sig().hash(&mut hasher);
        let tracking_id = hasher.finish().to_le_bytes();
        Self {
            tracking_id,
            kind: ConsensusTransactionKind::SignedCapabilityNotificationV1(signed_capabilities),
        }
    }

    pub fn new_randomness_dkg_message(
        authority: AuthorityName,
        versioned_message: &VersionedDkgMessage,
    ) -> Self {
        let message =
            bcs::to_bytes(versioned_message).expect("message serialization should not fail");
        let mut hasher = DefaultHasher::new();
        message.hash(&mut hasher);
        let tracking_id = hasher.finish().to_le_bytes();
        Self {
            tracking_id,
            kind: ConsensusTransactionKind::RandomnessDkgMessage(authority, message),
        }
    }
    pub fn new_randomness_dkg_confirmation(
        authority: AuthorityName,
        versioned_confirmation: &VersionedDkgConfirmation,
    ) -> Self {
        let confirmation =
            bcs::to_bytes(versioned_confirmation).expect("message serialization should not fail");
        let mut hasher = DefaultHasher::new();
        confirmation.hash(&mut hasher);
        let tracking_id = hasher.finish().to_le_bytes();
        Self {
            tracking_id,
            kind: ConsensusTransactionKind::RandomnessDkgConfirmation(authority, confirmation),
        }
    }

    pub fn new_misbehavior_report(report: VersionedMisbehaviorReport) -> Self {
        let serialized_report =
            bcs::to_bytes(&report).expect("report serialization should not fail");
        let mut hasher = DefaultHasher::new();
        serialized_report.hash(&mut hasher);
        let tracking_id = hasher.finish().to_le_bytes();
        Self {
            tracking_id,
            kind: ConsensusTransactionKind::MisbehaviorReport(report),
        }
    }

    pub fn get_tracking_id(&self) -> u64 {
        (&self.tracking_id[..])
            .read_u64::<BigEndian>()
            .unwrap_or_default()
    }

    pub fn key(&self) -> ConsensusTransactionKey {
        match &self.kind {
            ConsensusTransactionKind::CertifiedTransaction(cert) => {
                ConsensusTransactionKey::Certificate(*cert.digest())
            }
            ConsensusTransactionKind::CheckpointSignature(data) => {
                ConsensusTransactionKey::CheckpointSignature(
                    data.summary.auth_sig().authority,
                    data.summary.sequence_number,
                )
            }
            ConsensusTransactionKind::EndOfPublish(authority) => {
                ConsensusTransactionKey::EndOfPublish(*authority)
            }
            ConsensusTransactionKind::CapabilityNotificationV1(cap) => {
                ConsensusTransactionKey::CapabilityNotification(cap.authority, cap.generation)
            }
            ConsensusTransactionKind::SignedCapabilityNotificationV1(signed_cap) => {
                ConsensusTransactionKey::CapabilityNotification(
                    signed_cap.authority,
                    signed_cap.generation,
                )
            }

            #[allow(deprecated)]
            ConsensusTransactionKind::NewJWKFetchedDeprecated => {
                ConsensusTransactionKey::NewJWKFetchedDeprecated
            }
            ConsensusTransactionKind::RandomnessDkgMessage(authority, _) => {
                ConsensusTransactionKey::RandomnessDkgMessage(*authority)
            }
            ConsensusTransactionKind::RandomnessDkgConfirmation(authority, _) => {
                ConsensusTransactionKey::RandomnessDkgConfirmation(*authority)
            }
            ConsensusTransactionKind::MisbehaviorReport(report) => {
                ConsensusTransactionKey::MisbehaviorReport(
                    report.authority,
                    *report.digest(),
                    report.generation,
                )
            }
        }
    }

    pub fn is_user_certificate(&self) -> bool {
        matches!(self.kind, ConsensusTransactionKind::CertifiedTransaction(_))
    }

    pub fn is_end_of_publish(&self) -> bool {
        matches!(self.kind, ConsensusTransactionKind::EndOfPublish(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pre-refactor wire shape of `VersionedMisbehaviorReport` — only `payload`
    /// crossed the wire (the digest cache was `#[serde(skip)]`). Used to pin
    /// post-refactor bytes against the legacy encoding.
    #[derive(Serialize)]
    struct LegacyVersionedMisbehaviorReport<'a> {
        payload: &'a MisbehaviorObservations,
    }

    fn sample_payload() -> MisbehaviorObservations {
        MisbehaviorObservations::V1(MisbehaviorObservationsV1 {
            faulty_blocks_provable: vec![1, 2, 3],
            faulty_blocks_unprovable: vec![4, 5, 6],
            missing_proposals: vec![7, 8, 9],
            equivocations: vec![10, 11, 12],
        })
    }

    /// Pins the BCS encoding of `VersionedMisbehaviorReport` against the
    /// pre-refactor 3-tuple layout `(AuthorityName, { payload }, u64)`. Testnet
    /// is running the legacy format; if the bytes ever drift, validators on
    /// the new build will reject reports from validators on the old build (or
    /// vice versa) and consensus halts. Reordering struct fields, adding a
    /// non-`skip` field, or renaming a field's serde tag will all trip this
    /// test.
    #[test]
    fn misbehavior_report_wire_format_unchanged() {
        let authority = AuthorityName::default();
        let generation: u64 = 42;
        let payload = sample_payload();

        let legacy_bytes = bcs::to_bytes(&(
            authority,
            LegacyVersionedMisbehaviorReport { payload: &payload },
            generation,
        ))
        .unwrap();

        let new = VersionedMisbehaviorReport {
            authority,
            payload,
            generation,
            digest: OnceCell::new(),
        };
        let new_bytes = bcs::to_bytes(&new).unwrap();

        assert_eq!(
            legacy_bytes, new_bytes,
            "VersionedMisbehaviorReport wire format must not change — testnet is live"
        );
    }

    /// `ConsensusTransactionKind::MisbehaviorReport`'s variant tag is its
    /// position in the enum (BCS encodes enum variants as ULEB128 of the
    /// declaration index). Reordering variants — even if the new wrapping
    /// layout is byte-identical otherwise — would shift the tag and break
    /// every node still on the old build. This test catches that and also
    /// confirms the post-tag bytes equal the legacy 3-tuple encoding.
    #[test]
    fn misbehavior_report_consensus_kind_wire_format_unchanged() {
        let authority = AuthorityName::default();
        let generation: u64 = 7;
        let payload = sample_payload();

        let new_kind = ConsensusTransactionKind::MisbehaviorReport(VersionedMisbehaviorReport {
            authority,
            payload: payload.clone(),
            generation,
            digest: OnceCell::new(),
        });
        let new_bytes = bcs::to_bytes(&new_kind).unwrap();

        // Legacy encoding: variant tag (8 = position of MisbehaviorReport in
        // the enum, ULEB128 single byte) followed by the 3-tuple body.
        let mut legacy_bytes = vec![8u8];
        legacy_bytes.extend(
            bcs::to_bytes(&(
                authority,
                LegacyVersionedMisbehaviorReport { payload: &payload },
                generation,
            ))
            .unwrap(),
        );

        assert_eq!(
            legacy_bytes, new_bytes,
            "ConsensusTransactionKind::MisbehaviorReport wire format must not change — testnet is live"
        );
    }
}
