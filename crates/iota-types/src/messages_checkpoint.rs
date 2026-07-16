// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    slice::Iter,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::Result;
use fastcrypto::hash::MultisetHash;
use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::{
    CheckpointContentsDigest, CheckpointContentsV1, CheckpointDigest, Digest, RandomnessRound,
    checkpoint::CheckpointTransactionInfo,
    crypto::{Intent, IntentScope, UserSignature},
    gas::GasCostSummary,
};
#[cfg(not(target_arch = "wasm32"))]
use prometheus_filtered::Histogram;
use serde::{Deserialize, Serialize};
#[cfg(not(target_arch = "wasm32"))]
use tap::TapFallible;
use tracing::instrument;
#[cfg(not(target_arch = "wasm32"))]
use tracing::warn;

use crate::{
    base_types::{ExecutionData, ExecutionDigests, VerifiedExecutionData, random_object_ref},
    committee::{Committee, EpochId},
    crypto::{
        AccountKeyPair, AggregateAuthoritySignature, AuthoritySignInfo, AuthoritySignInfoTrait,
        AuthorityStrongQuorumSignInfo, default_hash, get_key_pair,
    },
    effects::{TestEffectsBuilder, TransactionEffectsAPI},
    error::{IotaError, IotaResult},
    global_state_hash::GlobalStateHash,
    message_envelope::{Envelope, Message, TrustedEnvelope, VerifiedEnvelope},
    storage::ReadStore,
    transaction::{Transaction, TransactionData, TransactionDataAPI},
};

pub type CheckpointSequenceNumber = u64;
pub type CheckpointTimestamp = u64;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckpointRequest {
    /// if a sequence number is specified, return the checkpoint with that
    /// sequence number; otherwise if None returns the latest checkpoint
    /// stored (authenticated or pending, depending on the value of
    /// `certified` flag)
    pub sequence_number: Option<CheckpointSequenceNumber>,
    // A flag, if true also return the contents of the
    // checkpoint besides the meta-data.
    pub request_content: bool,
    // If true, returns certified checkpoint, otherwise returns pending checkpoint
    pub certified: bool,
}

#[expect(clippy::large_enum_variant)]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CheckpointSummaryResponse {
    Certified(CertifiedCheckpointSummary),
    Pending(CheckpointSummary),
}

impl CheckpointSummaryResponse {
    pub fn content_digest(&self) -> CheckpointContentsDigest {
        match self {
            Self::Certified(s) => s.content_digest,
            Self::Pending(s) => s.content_digest,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckpointResponse {
    pub checkpoint: Option<CheckpointSummaryResponse>,
    pub contents: Option<CheckpointContents>,
}

// The constituent parts of checkpoints, signed and certified

pub use iota_sdk_types::checkpoint::CheckpointCommitment;

/// The Sha256 digest of an EllipticCurveMultisetHash committing to the live
/// object set.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ECMHLiveObjectSetDigest {
    pub digest: Digest,
}

impl From<fastcrypto::hash::Digest<32>> for ECMHLiveObjectSetDigest {
    fn from(digest: fastcrypto::hash::Digest<32>) -> Self {
        Self {
            digest: Digest::new(digest.digest),
        }
    }
}

impl Default for ECMHLiveObjectSetDigest {
    fn default() -> Self {
        GlobalStateHash::default().digest().into()
    }
}

pub use iota_sdk_types::checkpoint::{CheckpointSummary, EndOfEpochData};

impl Message for CheckpointSummary {
    type DigestType = CheckpointDigest;
    const SCOPE: IntentScope = IntentScope::CheckpointSummary;

    fn digest(&self) -> Self::DigestType {
        CheckpointDigest::new(default_hash(self))
    }
}

mod checkpoint_summary_ext {
    pub trait Sealed {}
    impl Sealed for super::CheckpointSummary {}
}

/// Node-only helpers for [`CheckpointSummary`], which is defined in
/// `iota_sdk_types`. These live on an extension trait because inherent methods
/// cannot be added to a type that is foreign to this crate.
pub trait CheckpointSummaryExt: Sized + checkpoint_summary_ext::Sealed {
    fn new_with_protocol_config(
        protocol_config: &ProtocolConfig,
        epoch: EpochId,
        sequence_number: CheckpointSequenceNumber,
        network_total_transactions: u64,
        transactions: &CheckpointContents,
        previous_digest: Option<CheckpointDigest>,
        epoch_rolling_gas_cost_summary: GasCostSummary,
        end_of_epoch_data: Option<EndOfEpochData>,
        timestamp_ms: CheckpointTimestamp,
        randomness_rounds: Vec<RandomnessRound>,
    ) -> Self;

    fn verify_epoch(&self, epoch: EpochId) -> IotaResult;

    fn timestamp(&self) -> SystemTime;

    #[cfg(not(target_arch = "wasm32"))]
    fn report_checkpoint_age(&self, metrics: &Histogram);

    fn parse_version_specific_data(
        &self,
        config: &ProtocolConfig,
    ) -> Result<Option<CheckpointVersionSpecificData>>;
}

impl CheckpointSummaryExt for CheckpointSummary {
    fn new_with_protocol_config(
        protocol_config: &ProtocolConfig,
        epoch: EpochId,
        sequence_number: CheckpointSequenceNumber,
        network_total_transactions: u64,
        transactions: &CheckpointContents,
        previous_digest: Option<CheckpointDigest>,
        epoch_rolling_gas_cost_summary: GasCostSummary,
        end_of_epoch_data: Option<EndOfEpochData>,
        timestamp_ms: CheckpointTimestamp,
        randomness_rounds: Vec<RandomnessRound>,
    ) -> Self {
        let content_digest = transactions.digest();

        let version_specific_data =
            match protocol_config.checkpoint_summary_version_specific_data_as_option() {
                None | Some(0) => Vec::new(),
                Some(1) => bcs::to_bytes(&CheckpointVersionSpecificData::V1(
                    CheckpointVersionSpecificDataV1 { randomness_rounds },
                ))
                .expect("version specific data should serialize"),
                _ => unimplemented!(
                    "unrecognized version_specific_data version for
    CheckpointSummary"
                ),
            };

        Self {
            epoch,
            sequence_number,
            network_total_transactions,
            content_digest,
            previous_digest,
            epoch_rolling_gas_cost_summary,
            end_of_epoch_data,
            timestamp_ms,
            version_specific_data,
            checkpoint_commitments: Default::default(),
        }
    }

    fn verify_epoch(&self, epoch: EpochId) -> IotaResult {
        fp_ensure!(
            self.epoch == epoch,
            IotaError::WrongEpoch {
                expected_epoch: epoch,
                actual_epoch: self.epoch,
            }
        );
        Ok(())
    }

    fn timestamp(&self) -> SystemTime {
        UNIX_EPOCH + Duration::from_millis(self.timestamp_ms)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn report_checkpoint_age(&self, metrics: &Histogram) {
        SystemTime::now()
            .duration_since(self.timestamp())
            .map(|latency| {
                metrics.observe(latency.as_secs_f64());
            })
            .tap_err(|err| {
                warn!(
                    checkpoint_seq = self.sequence_number,
                    "unable to compute checkpoint age: {}", err
                )
            })
            .ok();
    }

    fn parse_version_specific_data(
        &self,
        config: &ProtocolConfig,
    ) -> Result<Option<CheckpointVersionSpecificData>> {
        match config.checkpoint_summary_version_specific_data_as_option() {
            None | Some(0) => Ok(None),
            Some(1) => Ok(Some(bcs::from_bytes(&self.version_specific_data)?)),
            _ => unimplemented!("unrecognized version_specific_data version in CheckpointSummary"),
        }
    }
}

// Checkpoints are signed by an authority and 2f+1 form a
// certificate that others can use to catch up. The actual
// content of the digest must at the very least commit to
// the set of transactions contained in the certificate but
// we might extend this to contain roots of merkle trees,
// or other authenticated data structures to support light
// clients and more efficient sync protocols.

pub type CheckpointSummaryEnvelope<S> = Envelope<CheckpointSummary, S>;
pub type CertifiedCheckpointSummary = CheckpointSummaryEnvelope<AuthorityStrongQuorumSignInfo>;
pub type SignedCheckpointSummary = CheckpointSummaryEnvelope<AuthoritySignInfo>;

pub type VerifiedCheckpoint = VerifiedEnvelope<CheckpointSummary, AuthorityStrongQuorumSignInfo>;
pub type TrustedCheckpoint = TrustedEnvelope<CheckpointSummary, AuthorityStrongQuorumSignInfo>;

impl CertifiedCheckpointSummary {
    #[instrument(level = "trace", skip_all)]
    pub fn verify_authority_signatures(&self, committee: &Committee) -> IotaResult {
        self.data().verify_epoch(self.auth_sig().epoch)?;
        self.auth_sig().verify_secure(
            self.data(),
            Intent::iota_app(IntentScope::CheckpointSummary),
            committee,
        )
    }

    pub fn try_into_verified(self, committee: &Committee) -> IotaResult<VerifiedCheckpoint> {
        self.verify_authority_signatures(committee)?;
        Ok(VerifiedCheckpoint::new_from_verified(self))
    }

    pub fn verify_with_contents(
        &self,
        committee: &Committee,
        contents: Option<&CheckpointContents>,
    ) -> IotaResult {
        self.verify_authority_signatures(committee)?;

        if let Some(contents) = contents {
            let content_digest = contents.digest();
            fp_ensure!(
                content_digest == self.data().content_digest,
                IotaError::GenericAuthority {
                    error: format!(
                        "Checkpoint contents digest mismatch: summary={:?}, received content digest {:?}, received {} transactions",
                        self.data(),
                        content_digest,
                        contents.len()
                    )
                }
            );
        }

        Ok(())
    }

    pub fn into_summary_and_sequence(self) -> (CheckpointSequenceNumber, CheckpointSummary) {
        let summary = self.into_data();
        (summary.sequence_number, summary)
    }

    pub fn get_validator_signature(self) -> AggregateAuthoritySignature {
        self.auth_sig().signature.clone()
    }
}

impl SignedCheckpointSummary {
    #[instrument(level = "trace", skip_all)]
    pub fn verify_authority_signatures(&self, committee: &Committee) -> IotaResult {
        self.data().verify_epoch(self.auth_sig().epoch)?;
        self.auth_sig().verify_secure(
            self.data(),
            Intent::iota_app(IntentScope::CheckpointSummary),
            committee,
        )
    }

    pub fn try_into_verified(
        self,
        committee: &Committee,
    ) -> IotaResult<VerifiedEnvelope<CheckpointSummary, AuthoritySignInfo>> {
        self.verify_authority_signatures(committee)?;
        Ok(VerifiedEnvelope::<CheckpointSummary, AuthoritySignInfo>::new_from_verified(self))
    }
}

impl VerifiedCheckpoint {
    pub fn into_summary_and_sequence(self) -> (CheckpointSequenceNumber, CheckpointSummary) {
        self.into_inner().into_summary_and_sequence()
    }
}

/// This is a message validators publish to consensus in order to sign
/// checkpoint
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckpointSignatureMessage {
    pub summary: SignedCheckpointSummary,
}

impl CheckpointSignatureMessage {
    pub fn verify(&self, committee: &Committee) -> IotaResult {
        self.summary.verify_authority_signatures(committee)
    }
}

pub use iota_sdk_types::checkpoint::CheckpointContents;

fn execution_digests(info: &CheckpointTransactionInfo) -> ExecutionDigests {
    ExecutionDigests {
        transaction: info.transaction,
        effects: info.effects,
    }
}

mod checkpoint_contents_ext {
    pub trait Sealed {}
    impl Sealed for super::CheckpointContents {}
}

/// Node-only helpers for [`CheckpointContents`], which is defined in
/// `iota_sdk_types`. They bridge the node's `ExecutionDigests` representation
/// to the SDK type's parallel [`CheckpointTransactionInfo`] form.
pub trait CheckpointContentsExt: Sized + checkpoint_contents_ext::Sealed {
    fn new_with_digests_and_signatures(
        contents: impl IntoIterator<Item = ExecutionDigests>,
        user_signatures: Vec<Vec<UserSignature>>,
    ) -> Self;

    fn new_with_causally_ordered_execution_data<'a>(
        contents: impl IntoIterator<Item = &'a VerifiedExecutionData>,
    ) -> Self;

    fn new_with_digests_only_for_tests(
        contents: impl IntoIterator<Item = ExecutionDigests>,
    ) -> Self;

    fn iter(&self) -> impl DoubleEndedIterator<Item = ExecutionDigests> + ExactSizeIterator + '_;

    fn into_iter_with_signatures(
        self,
    ) -> impl Iterator<Item = (ExecutionDigests, Vec<UserSignature>)>;

    /// Enumerate the transactions in the contents, pairing each with its index
    /// in the global ordering of executed transactions since genesis.
    fn enumerate_transactions(
        &self,
        ckpt: &CheckpointSummary,
    ) -> impl Iterator<Item = (u64, ExecutionDigests)> + '_;
}

impl CheckpointContentsExt for CheckpointContents {
    fn new_with_digests_and_signatures(
        contents: impl IntoIterator<Item = ExecutionDigests>,
        user_signatures: Vec<Vec<UserSignature>>,
    ) -> Self {
        let transactions: Vec<_> = contents.into_iter().collect();
        assert_eq!(transactions.len(), user_signatures.len());
        Self::new_v1(CheckpointContentsV1::new(
            transactions
                .into_iter()
                .zip(user_signatures)
                .map(|(digests, signatures)| CheckpointTransactionInfo {
                    transaction: digests.transaction,
                    effects: digests.effects,
                    signatures,
                })
                .collect(),
        ))
    }

    fn new_with_causally_ordered_execution_data<'a>(
        contents: impl IntoIterator<Item = &'a VerifiedExecutionData>,
    ) -> Self {
        Self::new_v1(CheckpointContentsV1::new(
            contents
                .into_iter()
                .map(|data| {
                    let digests = data.digests();
                    CheckpointTransactionInfo {
                        transaction: digests.transaction,
                        effects: digests.effects,
                        signatures: data.transaction.inner().data().tx_signatures().to_owned(),
                    }
                })
                .collect(),
        ))
    }

    fn new_with_digests_only_for_tests(
        contents: impl IntoIterator<Item = ExecutionDigests>,
    ) -> Self {
        Self::new_v1(CheckpointContentsV1::new(
            contents
                .into_iter()
                .map(|digests| CheckpointTransactionInfo {
                    transaction: digests.transaction,
                    effects: digests.effects,
                    signatures: Vec::new(),
                })
                .collect(),
        ))
    }

    fn iter(&self) -> impl DoubleEndedIterator<Item = ExecutionDigests> + ExactSizeIterator + '_ {
        self.transactions().iter().map(execution_digests)
    }

    fn into_iter_with_signatures(
        self,
    ) -> impl Iterator<Item = (ExecutionDigests, Vec<UserSignature>)> {
        match self {
            CheckpointContents::V1(v1) => v1.into_transactions().into_iter().map(|info| {
                let digests = execution_digests(&info);
                (digests, info.signatures)
            }),
            _ => unimplemented!("a new CheckpointContents variant was added and must be handled"),
        }
    }

    fn enumerate_transactions(
        &self,
        ckpt: &CheckpointSummary,
    ) -> impl Iterator<Item = (u64, ExecutionDigests)> + '_ {
        let start = ckpt.network_total_transactions - self.len() as u64;

        (0u64..)
            .zip(self.iter())
            .map(move |(i, digests)| (i + start, digests))
    }
}

/// Same as CheckpointContents, but contains full contents of all Transactions
/// and TransactionEffects associated with the checkpoint.
// NOTE: This data structure is used for state sync of checkpoints. Therefore we attempt
// to estimate its size in CheckpointBuilder in order to limit the maximum serialized
// size of a checkpoint sent over the network. If this struct is modified,
// CheckpointBuilder::split_checkpoint_chunks should also be updated accordingly.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FullCheckpointContents {
    transactions: Vec<ExecutionData>,
    /// This field 'pins' user signatures for the checkpoint
    /// The length of this vector is same as length of transactions vector
    /// System transactions has empty signatures
    user_signatures: Vec<Vec<UserSignature>>,
}

impl FullCheckpointContents {
    pub fn new_with_causally_ordered_transactions<T>(contents: T) -> Self
    where
        T: IntoIterator<Item = ExecutionData>,
    {
        let (transactions, user_signatures): (Vec<_>, Vec<_>) = contents
            .into_iter()
            .map(|data| {
                let sig = data.transaction.data().tx_signatures().to_owned();
                (data, sig)
            })
            .unzip();
        assert_eq!(transactions.len(), user_signatures.len());
        Self {
            transactions,
            user_signatures,
        }
    }

    pub fn from_contents_and_execution_data(
        contents: CheckpointContents,
        execution_data: impl Iterator<Item = ExecutionData>,
    ) -> Self {
        let transactions: Vec<_> = execution_data.collect();
        let user_signatures = contents
            .into_iter_with_signatures()
            .map(|(_, signatures)| signatures)
            .collect();
        Self {
            transactions,
            user_signatures,
        }
    }

    pub fn try_from_checkpoint_contents<S>(
        store: S,
        contents: CheckpointContents,
    ) -> Result<Option<Self>, crate::storage::error::Error>
    where
        S: ReadStore,
    {
        let (digests, user_signatures): (Vec<_>, Vec<_>) =
            contents.into_iter_with_signatures().unzip();
        let mut transactions = Vec::with_capacity(digests.len());
        for tx in &digests {
            if let (Some(t), Some(e)) = (
                store.try_get_transaction(&tx.transaction)?,
                store.try_get_transaction_effects(&tx.transaction)?,
            ) {
                transactions.push(ExecutionData::new((*t).clone().into_inner(), e))
            } else {
                return Ok(None);
            }
        }
        Ok(Some(Self {
            transactions,
            user_signatures,
        }))
    }

    pub fn iter(&self) -> Iter<'_, ExecutionData> {
        self.transactions.iter()
    }

    /// Verifies that this checkpoint's digest matches the given digest, and
    /// that all internal Transaction and TransactionEffects digests are
    /// consistent.
    pub fn verify_digests(&self, digest: CheckpointContentsDigest) -> Result<()> {
        let self_digest = self.checkpoint_contents().digest();
        fp_ensure!(
            digest == self_digest,
            anyhow::anyhow!(
                "checkpoint contents digest {self_digest} does not match expected digest {digest}"
            )
        );
        for tx in self.iter() {
            let transaction_digest = tx.transaction.digest();
            fp_ensure!(
                tx.effects.transaction_digest() == transaction_digest,
                anyhow::anyhow!(
                    "transaction digest {transaction_digest} does not match expected digest {}",
                    tx.effects.transaction_digest()
                )
            );
        }
        Ok(())
    }

    pub fn checkpoint_contents(&self) -> CheckpointContents {
        CheckpointContents::new_with_digests_and_signatures(
            self.transactions.iter().map(|tx| tx.digests()),
            self.user_signatures.clone(),
        )
    }

    pub fn into_checkpoint_contents(self) -> CheckpointContents {
        let digests: Vec<_> = self.transactions.iter().map(|tx| tx.digests()).collect();
        CheckpointContents::new_with_digests_and_signatures(digests, self.user_signatures)
    }

    pub fn size(&self) -> usize {
        self.transactions.len()
    }

    pub fn random_for_testing() -> Self {
        let (a, key): (_, AccountKeyPair) = get_key_pair();
        let transaction = Transaction::from_data_and_signer(
            TransactionData::new_transfer(
                a,
                random_object_ref(),
                a,
                random_object_ref(),
                100000000000,
                100,
            ),
            vec![&key],
        );
        let effects = TestEffectsBuilder::new(transaction.data()).build();
        let exe_data = ExecutionData {
            transaction,
            effects,
        };
        FullCheckpointContents::new_with_causally_ordered_transactions(vec![exe_data])
    }
}

impl IntoIterator for FullCheckpointContents {
    type Item = ExecutionData;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.transactions.into_iter()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifiedCheckpointContents {
    transactions: Vec<VerifiedExecutionData>,
    /// This field 'pins' user signatures for the checkpoint
    /// The length of this vector is same as length of transactions vector
    /// System transactions has empty signatures
    user_signatures: Vec<Vec<UserSignature>>,
}

impl VerifiedCheckpointContents {
    pub fn new_unchecked(contents: FullCheckpointContents) -> Self {
        Self {
            transactions: contents
                .transactions
                .into_iter()
                .map(VerifiedExecutionData::new_unchecked)
                .collect(),
            user_signatures: contents.user_signatures,
        }
    }

    pub fn iter(&self) -> Iter<'_, VerifiedExecutionData> {
        self.transactions.iter()
    }

    pub fn transactions(&self) -> &[VerifiedExecutionData] {
        &self.transactions
    }

    pub fn into_inner(self) -> FullCheckpointContents {
        FullCheckpointContents {
            transactions: self
                .transactions
                .into_iter()
                .map(|tx| tx.into_inner())
                .collect(),
            user_signatures: self.user_signatures,
        }
    }

    pub fn into_checkpoint_contents(self) -> CheckpointContents {
        self.into_inner().into_checkpoint_contents()
    }

    pub fn into_checkpoint_contents_digest(self) -> CheckpointContentsDigest {
        self.into_inner().into_checkpoint_contents().digest()
    }

    pub fn num_of_transactions(&self) -> usize {
        self.transactions.len()
    }
}

/// Holds data in CheckpointSummary that is serialized into the
/// `version_specific_data` field.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckpointVersionSpecificData {
    V1(CheckpointVersionSpecificDataV1),
}

impl CheckpointVersionSpecificData {
    pub fn as_v1(&self) -> &CheckpointVersionSpecificDataV1 {
        match self {
            Self::V1(v) => v,
        }
    }

    pub fn into_v1(self) -> CheckpointVersionSpecificDataV1 {
        match self {
            Self::V1(v) => v,
        }
    }

    pub fn empty_for_tests() -> CheckpointVersionSpecificData {
        CheckpointVersionSpecificData::V1(CheckpointVersionSpecificDataV1 {
            randomness_rounds: Vec::new(),
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointVersionSpecificDataV1 {
    /// Lists the rounds for which RandomnessStateUpdate transactions are
    /// present in the checkpoint.
    pub randomness_rounds: Vec<RandomnessRound>,
}

#[cfg(test)]
mod tests {
    use fastcrypto::traits::KeyPair;
    use iota_sdk_types::{ConsensusCommitDigest, TransactionDigest, TransactionEffectsDigest};
    use rand::{SeedableRng, prelude::StdRng};

    use super::*;
    use crate::{transaction::VerifiedTransaction, utils::make_committee_key};

    // TODO use the file name as a seed
    const RNG_SEED: [u8; 32] = [
        21, 23, 199, 200, 234, 250, 252, 178, 94, 15, 202, 178, 62, 186, 88, 137, 233, 192, 130,
        157, 179, 179, 65, 9, 31, 249, 221, 123, 225, 112, 199, 247,
    ];

    #[test]
    fn test_signed_checkpoint() {
        let mut rng = StdRng::from_seed(RNG_SEED);
        let (keys, committee) = make_committee_key(&mut rng);
        let (_, committee2) = make_committee_key(&mut rng);

        let set = CheckpointContents::new_with_digests_only_for_tests([ExecutionDigests::random()]);

        // TODO: duplicated in a test below.

        let signed_checkpoints: Vec<_> = keys
            .iter()
            .map(|k| {
                let name = k.public().into();

                SignedCheckpointSummary::new(
                    committee.epoch,
                    CheckpointSummary::new_with_protocol_config(
                        &ProtocolConfig::get_for_max_version_UNSAFE(),
                        committee.epoch,
                        1,
                        0,
                        &set,
                        None,
                        GasCostSummary::default(),
                        None,
                        0,
                        Vec::new(),
                    ),
                    k,
                    name,
                )
            })
            .collect();

        signed_checkpoints.iter().for_each(|c| {
            c.verify_authority_signatures(&committee)
                .expect("signature ok")
        });

        // fails when not signed by member of committee
        signed_checkpoints
            .iter()
            .for_each(|c| assert!(c.verify_authority_signatures(&committee2).is_err()));
    }

    #[test]
    fn test_certified_checkpoint() {
        let mut rng = StdRng::from_seed(RNG_SEED);
        let (keys, committee) = make_committee_key(&mut rng);

        let set = CheckpointContents::new_with_digests_only_for_tests([ExecutionDigests::random()]);

        let summary = CheckpointSummary::new_with_protocol_config(
            &ProtocolConfig::get_for_max_version_UNSAFE(),
            committee.epoch,
            1,
            0,
            &set,
            None,
            GasCostSummary::default(),
            None,
            0,
            Vec::new(),
        );

        let sign_infos: Vec<_> = keys
            .iter()
            .map(|k| {
                let name = k.public().into();

                SignedCheckpointSummary::sign(committee.epoch, &summary, k, name)
            })
            .collect();

        let checkpoint_cert =
            CertifiedCheckpointSummary::new(summary, sign_infos, &committee).expect("Cert is OK");

        // Signature is correct on proposal, and with same transactions
        assert!(
            checkpoint_cert
                .verify_with_contents(&committee, Some(&set))
                .is_ok()
        );

        // Make a bad proposal
        let signed_checkpoints: Vec<_> = keys
            .iter()
            .map(|k| {
                let name = k.public().into();
                let set = CheckpointContents::new_with_digests_only_for_tests([
                    ExecutionDigests::random(),
                ]);

                SignedCheckpointSummary::new(
                    committee.epoch,
                    CheckpointSummary::new_with_protocol_config(
                        &ProtocolConfig::get_for_max_version_UNSAFE(),
                        committee.epoch,
                        1,
                        0,
                        &set,
                        None,
                        GasCostSummary::default(),
                        None,
                        0,
                        Vec::new(),
                    ),
                    k,
                    name,
                )
            })
            .collect();

        let summary = signed_checkpoints[0].data().clone();
        let sign_infos = signed_checkpoints
            .into_iter()
            .map(|v| v.into_sig())
            .collect();
        assert!(
            CertifiedCheckpointSummary::new(summary, sign_infos, &committee)
                .unwrap()
                .verify_authority_signatures(&committee)
                .is_err()
        )
    }

    // Generate a CheckpointSummary from the input transaction digest. All the other
    // fields in the generated CheckpointSummary will be the same. The generated
    // CheckpointSummary can be used to test how input transaction digest
    // affects CheckpointSummary.
    fn generate_test_checkpoint_summary_from_digest(
        digest: TransactionDigest,
    ) -> CheckpointSummary {
        CheckpointSummary::new_with_protocol_config(
            &ProtocolConfig::get_for_max_version_UNSAFE(),
            1,
            2,
            10,
            &CheckpointContents::new_with_digests_only_for_tests([ExecutionDigests::new(
                digest,
                TransactionEffectsDigest::ZERO,
            )]),
            None,
            GasCostSummary::default(),
            None,
            100,
            Vec::new(),
        )
    }

    // Tests that ConsensusCommitPrologue with different consensus commit digest
    // will result in different checkpoint content.
    #[test]
    fn test_checkpoint_summary_with_different_consensus_digest() {
        // First, tests that same consensus commit digest will produce the same
        // checkpoint content.
        {
            let t1 = VerifiedTransaction::new_consensus_commit_prologue_v1(
                1,
                2,
                100,
                ConsensusCommitDigest::default(),
                Vec::new(),
            );
            let t2 = VerifiedTransaction::new_consensus_commit_prologue_v1(
                1,
                2,
                100,
                ConsensusCommitDigest::default(),
                Vec::new(),
            );
            let c1 = generate_test_checkpoint_summary_from_digest(*t1.digest());
            let c2 = generate_test_checkpoint_summary_from_digest(*t2.digest());
            assert_eq!(c1.digest(), c2.digest());
        }

        // Next, tests that different consensus commit digests will produce the
        // different checkpoint contents.
        {
            let t1 = VerifiedTransaction::new_consensus_commit_prologue_v1(
                1,
                2,
                100,
                ConsensusCommitDigest::default(),
                Vec::new(),
            );
            let t2 = VerifiedTransaction::new_consensus_commit_prologue_v1(
                1,
                2,
                100,
                ConsensusCommitDigest::random(),
                Vec::new(),
            );
            let c1 = generate_test_checkpoint_summary_from_digest(*t1.digest());
            let c2 = generate_test_checkpoint_summary_from_digest(*t2.digest());
            assert_ne!(c1.digest(), c2.digest());
        }
    }
}
