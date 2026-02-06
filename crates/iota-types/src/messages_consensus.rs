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
use fastcrypto_zkp::bn254::zk_login::{JWK, JwkId};
use iota_sdk_types::crypto::IntentScope;
use once_cell::sync::OnceCell;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    base_types::{
        AuthorityName, ConciseableName, ObjectID, ObjectRef, SequenceNumber, TransactionDigest,
    },
    crypto::{AuthoritySignature, DefaultHash, default_hash},
    digests::{ConsensusCommitDigest, Digest, MisbehaviorReportDigest},
    message_envelope::{Envelope, Message, VerifiedEnvelope},
    messages_checkpoint::{CheckpointSequenceNumber, CheckpointSignatureMessage},
    supported_protocol_versions::{
        Chain, SupportedProtocolVersions, SupportedProtocolVersionsWithHashes,
    },
    transaction::CertifiedTransaction,
};

/// Non-decreasing timestamp produced by consensus in ms.
pub type TimestampMs = u64;

/// Uses an enum to allow for future expansion of the
/// ConsensusDeterminedVersionAssignments.
#[derive(Debug, PartialEq, Eq, Hash, Clone, Serialize, Deserialize, JsonSchema)]
pub enum ConsensusDeterminedVersionAssignments {
    // Cancelled transaction version assignment.
    CancelledTransactions(Vec<(TransactionDigest, Vec<(ObjectID, SequenceNumber)>)>),
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Serialize, Deserialize)]
pub struct ConsensusCommitPrologueV1 {
    /// Epoch of the commit prologue transaction
    pub epoch: u64,
    /// Consensus round of the commit
    pub round: u64,
    /// The sub DAG index of the consensus commit. This field will be populated
    /// if there are multiple consensus commits per round.
    pub sub_dag_index: Option<u64>,
    /// Unix timestamp from consensus
    pub commit_timestamp_ms: TimestampMs,
    /// Digest of consensus output
    pub consensus_commit_digest: ConsensusCommitDigest,
    /// Stores consensus handler determined shared object version assignments.
    pub consensus_determined_version_assignments: ConsensusDeterminedVersionAssignments,
}

// In practice, JWKs are about 500 bytes of json each, plus a bit more for the
// ID. 4096 should give us plenty of space for any imaginable JWK while
// preventing DoSes.
static MAX_TOTAL_JWK_SIZE: usize = 4096;

pub fn check_total_jwk_size(id: &JwkId, jwk: &JWK) -> bool {
    id.iss.len() + id.kid.len() + jwk.kty.len() + jwk.alg.len() + jwk.e.len() + jwk.n.len()
        <= MAX_TOTAL_JWK_SIZE
}

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
    // Key must include both id and jwk, because honest validators could be given multiple jwks
    // for the same id by malfunctioning providers.
    NewJWKFetched(Box<(AuthorityName, JwkId, JWK)>),
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
            Self::Certificate(digest) => write!(f, "Certificate({digest:?})"),
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
            Self::NewJWKFetched(key) => {
                let (authority, id, jwk) = &**key;
                write!(
                    f,
                    "NewJWKFetched({:?}, {:?}, {:?})",
                    authority.concise(),
                    id,
                    jwk
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

    NewJWKFetched(AuthorityName, JwkId, JWK),

    // DKG is used to generate keys for use in the random beacon protocol.
    // `RandomnessDkgMessage` is sent out at start-of-epoch to initiate the process.
    // Contents are a serialized `fastcrypto_tbls::dkg::Message`.
    RandomnessDkgMessage(AuthorityName, Vec<u8>),
    // `RandomnessDkgConfirmation` is the second DKG message, sent as soon as a threshold amount
    // of `RandomnessDkgMessages` have been received locally, to complete the key generation
    // process. Contents are a serialized `fastcrypto_tbls::dkg::Confirmation`.
    RandomnessDkgConfirmation(AuthorityName, Vec<u8>),
    MisbehaviorReport(
        AuthorityName,
        VersionedMisbehaviorReport,
        CheckpointSequenceNumber,
    ),
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VersionedMisbehaviorReport {
    V1(
        MisbehaviorsV1<Vec<u64>>,
        #[serde(skip)] OnceCell<MisbehaviorReportDigest>,
    ),
}

impl VersionedMisbehaviorReport {
    pub fn new_v1(misbehaviors: MisbehaviorsV1<Vec<u64>>) -> Self {
        VersionedMisbehaviorReport::V1(misbehaviors, OnceCell::new())
    }

    pub fn verify(&self, committee_size: usize) -> bool {
        match self {
            VersionedMisbehaviorReport::V1(report, _) => report.verify(committee_size),
        }
    }
    /// Returns an iterator over references to some of the fields in the report.
    pub fn iterate_over_metrics(&self) -> std::vec::IntoIter<&Vec<u64>> {
        match self {
            VersionedMisbehaviorReport::V1(report, _) => report.iter(),
        }
    }
    /// Returns the digest of the misbehavior report, caching it if it has not
    /// been computed yet.
    pub fn digest(&self) -> &MisbehaviorReportDigest {
        match self {
            VersionedMisbehaviorReport::V1(_, digest) => {
                digest.get_or_init(|| MisbehaviorReportDigest::new(default_hash(self)))
            }
        }
    }
}

// MisbehaviorsV1 contains lists of all metrics used in v1 of misbehavior
// reports, with a value for each metric. The metrics (misbeheaviors) include,
// faulty blocks, equivocation and missing proposal counts for each authority.
// This first version does not include any type of proof.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MisbehaviorsV1<T> {
    pub faulty_blocks_provable: T,
    pub faulty_blocks_unprovable: T,
    pub missing_proposals: T,
    pub equivocations: T,
}

impl MisbehaviorsV1<Vec<u64>> {
    pub fn verify(&self, committee_size: usize) -> bool {
        // This version of reports are valid as long as they contain the counts for all
        // authorities. Future versions may contain proofs that need verification.
        // However, since the validity of a proof is deeply coupled with the protocol
        // version and the consensus mechanism being used, we cannot verify it here. In
        // the future, reports should be unwrapped (or translated) to a type verifiable
        // by the consensus crate, which means that the verification logic will probably
        // move out of this crate.
        if (self.faulty_blocks_provable.len() != committee_size)
            | (self.faulty_blocks_unprovable.len() != committee_size)
            | (self.equivocations.len() != committee_size)
            | (self.missing_proposals.len() != committee_size)
        {
            return false;
        }
        true
    }
}
impl<T> MisbehaviorsV1<T> {
    pub fn iter(&self) -> std::vec::IntoIter<&T> {
        vec![
            &self.faulty_blocks_provable,
            &self.faulty_blocks_unprovable,
            &self.missing_proposals,
            &self.equivocations,
        ]
        .into_iter()
    }
    // Returns an iterator over references to major misbehavior fields in the
    // report. Major misbehaviors carry a higher penalty in the scoring system.
    pub fn iter_major_misbehaviors(&self) -> std::vec::IntoIter<&T> {
        vec![&self.equivocations].into_iter()
    }
    // Returns an iterator over references to minor misbehavior fields in the
    // report. Minor misbehaviors carry a lower penalty in the scoring system.
    pub fn iter_minor_misbehaviors(&self) -> std::vec::IntoIter<&T> {
        vec![
            &self.faulty_blocks_provable,
            &self.faulty_blocks_unprovable,
            &self.missing_proposals,
        ]
        .into_iter()
    }
}

impl<T> FromIterator<T> for MisbehaviorsV1<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut iterator = iter.into_iter();
        Self {
            faulty_blocks_provable: iterator.next().expect("Not enough elements in iterator"),
            faulty_blocks_unprovable: iterator.next().expect("Not enough elements in iterator"),
            missing_proposals: iterator.next().expect("Not enough elements in iterator"),
            equivocations: iterator.next().expect("Not enough elements in iterator"),
        }
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

    pub fn new_mysticeti_certificate(
        round: u64,
        offset: u64,
        certificate: CertifiedTransaction,
    ) -> Self {
        let mut hasher = DefaultHasher::new();
        let tx_digest = certificate.digest();
        tx_digest.hash(&mut hasher);
        round.hash(&mut hasher);
        offset.hash(&mut hasher);
        let tracking_id = hasher.finish().to_le_bytes();
        Self {
            tracking_id,
            kind: ConsensusTransactionKind::CertifiedTransaction(Box::new(certificate)),
        }
    }

    pub fn new_jwk_fetched(authority: AuthorityName, id: JwkId, jwk: JWK) -> Self {
        let mut hasher = DefaultHasher::new();
        id.hash(&mut hasher);
        let tracking_id = hasher.finish().to_le_bytes();
        Self {
            tracking_id,
            kind: ConsensusTransactionKind::NewJWKFetched(authority, id, jwk),
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

    pub fn new_misbehavior_report(
        authority: AuthorityName,
        report: &VersionedMisbehaviorReport,
        checkpoint_seq: CheckpointSequenceNumber,
    ) -> Self {
        let serialized_report =
            bcs::to_bytes(report).expect("report serialization should not fail");
        let mut hasher = DefaultHasher::new();
        serialized_report.hash(&mut hasher);
        let tracking_id = hasher.finish().to_le_bytes();
        Self {
            tracking_id,
            kind: ConsensusTransactionKind::MisbehaviorReport(
                authority,
                report.clone(),
                checkpoint_seq,
            ),
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

            ConsensusTransactionKind::NewJWKFetched(authority, id, key) => {
                ConsensusTransactionKey::NewJWKFetched(Box::new((
                    *authority,
                    id.clone(),
                    key.clone(),
                )))
            }
            ConsensusTransactionKind::RandomnessDkgMessage(authority, _) => {
                ConsensusTransactionKey::RandomnessDkgMessage(*authority)
            }
            ConsensusTransactionKind::RandomnessDkgConfirmation(authority, _) => {
                ConsensusTransactionKey::RandomnessDkgConfirmation(*authority)
            }
            ConsensusTransactionKind::MisbehaviorReport(authority, report, checkpoint_seq) => {
                ConsensusTransactionKey::MisbehaviorReport(
                    *authority,
                    *report.digest(),
                    *checkpoint_seq,
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

#[test]
fn test_jwk_compatibility() {
    // Ensure that the JWK and JwkId structs in fastcrypto do not change formats.
    // If this test breaks DO NOT JUST UPDATE THE EXPECTED BYTES. Instead, add a
    // local JWK or JwkId struct that mirrors the fastcrypto struct, use it in
    // AuthenticatorStateUpdate, and add Into/From as necessary.
    let jwk = JWK {
        kty: "a".to_string(),
        e: "b".to_string(),
        n: "c".to_string(),
        alg: "d".to_string(),
    };

    let expected_jwk_bytes = vec![1, 97, 1, 98, 1, 99, 1, 100];
    let jwk_bcs = bcs::to_bytes(&jwk).unwrap();
    assert_eq!(jwk_bcs, expected_jwk_bytes);

    let id = JwkId {
        iss: "abc".to_string(),
        kid: "def".to_string(),
    };

    let expected_id_bytes = vec![3, 97, 98, 99, 3, 100, 101, 102];
    let id_bcs = bcs::to_bytes(&id).unwrap();
    assert_eq!(id_bcs, expected_id_bytes);
}
