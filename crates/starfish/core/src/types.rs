// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub type AuthorityIndex = u64;

#[derive(Clone, Eq, PartialEq, Serialize, Deserialize, Default)]
pub struct Transaction {
    data: Vec<u8>,
}

pub type RoundNumber = u64;

pub type ShardIndex = usize;
pub type BlockDigest = starfish_config::BlockDigest;
pub type Stake = u64;
pub type KeyPair = u64;
pub type PublicKey = starfish_config::PublicKey;
pub type Shard = Vec<u8>;
pub type Encoder = ReedSolomonEncoder;
pub type Decoder = ReedSolomonDecoder;

use eyre::{bail, ensure};
//use minibytes::Bytes;
use reed_solomon_simd::{ReedSolomonDecoder, ReedSolomonEncoder};
use serde::{Deserialize, Serialize};
use std::sync::atomic::Ordering;
use std::{
    fmt,
    hash::{Hash, Hasher},
    time::Duration,
};
#[cfg(test)]
pub use test::Dag;

use crate::crypto::TransactionsCommitment;
//use crate::data::{IN_MEMORY_BLOCKS, IN_MEMORY_BLOCKS_BYTES};
use crate::encoder::ShardEncoder;
use crate::threshold_clock::threshold_clock_valid_verified_block;
use crate::{
    committee::Committee,
    crypto,
    crypto::{AsBytes, CryptoHash, SignatureBytes, Signer},
    data::Data,
};

pub type EpochStatus = bool;

#[derive(PartialEq, Default, Clone, Copy, Serialize, Deserialize)]
pub enum InternalEpochStatus {
    #[default]
    Open,
    /// Change is triggered by an external deterministic mechanism
    BeginChange,
    /// Epoch is safe to close -- committed blocks from >= 2f+1 stake indicate epoch change
    SafeToClose,
}

#[derive(Clone, Copy, Eq, PartialEq, Serialize, Deserialize, Default)]
pub struct BlockReference {
    pub authority: AuthorityIndex,
    pub round: RoundNumber,
    pub digest: BlockDigest,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub enum BaseStatement {
    /// Authority Shares a transactions, without accepting it or not.
    Share(Transaction),
}

impl Hash for BlockReference {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.authority.hash(state);
        self.round.hash(state);
        state.write(&self.digest.as_ref()[..8]);
    }
}

#[derive(Clone, Serialize, Deserialize)]
// Important. Adding fields here requires updating BlockDigest::new, and StatementBlock::verify
pub struct VerifiedStatementBlock {
    reference: BlockReference,

    //  A list of block references to other blocks that this block includes
    //  Note that the order matters: if a reference to two blocks from the same round and same authority
    //  are included, then the first reference is the one that this block conceptually votes for.
    includes: Vec<BlockReference>,

    // Transaction data acknowledgment
    acknowledgement_statements: Vec<BlockReference>,

    // Creation time of the block as reported by creator, currently not enforced
    meta_creation_time_ns: TimestampNs,

    epoch_marker: EpochStatus,

    // Signature by the block author
    signature: SignatureBytes,
    // It could be either a vector of BaseStatement or None
    statements: Option<Vec<BaseStatement>>,
    // It could be a pair of encoded shard and position or none
    encoded_shard: Option<(Shard, ShardIndex)>,
    // This is Some only when the above has one some
    merkle_proof_encoded_shard: Option<Vec<u8>>,
    // merkle root is computed for encoded_statements
    transactions_commitment: TransactionsCommitment,
}

#[derive(Clone, Serialize, Deserialize)]
// Important. Adding fields here requires updating BlockDigest::new, and StatementBlock::verify
pub struct CachedStatementBlock {
    reference: BlockReference,

    //  A list of block references to other blocks that this block includes
    //  Note that the order matters: if a reference to two blocks from the same round and same authority
    //  are included, then the first reference is the one that this block conceptually votes for.
    includes: Vec<BlockReference>,

    // Transaction data acknowledgment
    acknowledgement_statements: Vec<BlockReference>,

    // Creation time of the block as reported by creator, currently not enforced
    meta_creation_time_ns: TimestampNs,

    epoch_marker: EpochStatus,

    // Signature by the block author
    signature: SignatureBytes,
    // It could be either a vector of BaseStatement or None
    statements: Option<Vec<BaseStatement>>,
    // It could be a pair of encoded shard and position or none
    encoded_statements: Vec<Option<Shard>>,
    // This is Some only when the above has one some
    merkle_proof_encoded_shard: Option<Vec<u8>>,
    // merkle root is computed for encoded_statements
    merkle_root_encoded_statements: TransactionsCommitment,
}

impl CachedStatementBlock {
    pub(crate) fn to_verified_block(
        &self,
        own_id: usize,
        merkle_proof: Vec<u8>,
        info_length: usize,
    ) -> VerifiedStatementBlock {
        let encoded_shard: Option<(Shard, usize)> = Some((
            self.encoded_statements[own_id]
                .clone()
                .expect("Should be shard"),
            own_id,
        ));
        if self.statements.is_some() {
            VerifiedStatementBlock {
                reference: self.reference,
                includes: self.includes.clone(),
                acknowledgement_statements: self.acknowledgement_statements.clone(),
                meta_creation_time_ns: self.meta_creation_time_ns,
                epoch_marker: self.epoch_marker,
                signature: self.signature,
                statements: self.statements.clone(),
                encoded_shard,
                merkle_proof_encoded_shard: Some(merkle_proof),
                transactions_commitment: self.merkle_root_encoded_statements,
            }
        } else {
            let info_shards: Vec<Vec<u8>> = self
                .encoded_statements()
                .iter()
                .enumerate()
                .filter(|(i, _s)| *i < info_length)
                .map(|(_, s)| s.clone().expect("Should be Some for all info length")) // Safe to unwrap because we filtered for `is_some()`
                .collect();
            // Combine all the shards into a single Vec<u8> (assuming they are in order)
            let mut reconstructed_data = Vec::new();
            for shard in info_shards {
                reconstructed_data.extend(shard);
            }

            // Read the first 4 bytes for `bytes_length` to get the size of the original serialized block
            if reconstructed_data.len() < 4 {
                panic!("Reconstructed data is too short to contain a valid length");
            }

            let bytes_length = u32::from_le_bytes(
                reconstructed_data[0..4]
                    .try_into()
                    .expect("Failed to read bytes_length"),
            ) as usize;

            // Ensure the data length matches the declared length
            if reconstructed_data.len() < 4 + bytes_length {
                panic!(
                    "Reconstructed data length does not match the declared bytes_length; {} {}",
                    reconstructed_data.len(),
                    bytes_length
                );
            } else {
                tracing::debug!(
                    "Reconstructed data length {}, bytes_length {}",
                    reconstructed_data.len(),
                    bytes_length
                );
            }

            // Deserialize the rest of the data into `Vec<BaseStatement>`
            let serialized_block = &reconstructed_data[4..4 + bytes_length];
            let reconstructed_statements: Vec<BaseStatement> =
                bincode::deserialize(serialized_block)
                    .expect("Deserialization of reconstructed data failed");
            VerifiedStatementBlock {
                reference: self.reference,
                includes: self.includes.clone(),
                acknowledgement_statements: self.acknowledgement_statements.clone(),
                meta_creation_time_ns: self.meta_creation_time_ns,
                epoch_marker: self.epoch_marker,
                signature: self.signature,
                statements: Some(reconstructed_statements),
                encoded_shard,
                merkle_proof_encoded_shard: Some(merkle_proof),
                transactions_commitment: self.merkle_root_encoded_statements,
            }
        }
    }
}

impl CachedStatementBlock {
    pub fn encoded_statements(&self) -> &Vec<Option<Shard>> {
        &self.encoded_statements
    }

    pub fn add_encoded_shard(&mut self, position: usize, shard: Shard) {
        self.encoded_statements[position] = Some(shard);
    }

    pub fn add_encoded_statements(&mut self, encoded_statements: Vec<Option<Shard>>) {
        self.encoded_statements = encoded_statements;
    }

    pub fn copy_shard(&mut self, block: &VerifiedStatementBlock) {
        if block.encoded_shard.is_some() {
            let (shard, shard_index) = block
                .encoded_shard()
                .as_ref()
                .expect("It should be some because of the above check");
            self.encoded_statements[*shard_index] = Some(shard.clone());
        }
    }

    pub fn merkle_root(&self) -> TransactionsCommitment {
        self.merkle_root_encoded_statements
    }
}

impl VerifiedStatementBlock {
    pub fn new(
        authority: AuthorityIndex,
        round: RoundNumber,
        includes: Vec<BlockReference>,
        acknowledgement_statements: Vec<BlockReference>,
        meta_creation_time_ns: TimestampNs,
        epoch_marker: EpochStatus,
        signature: SignatureBytes,
        statements: Vec<BaseStatement>,
        encoded_shard: Option<(Shard, usize)>,
        merkle_proof: Option<Vec<u8>>,
        merkle_root: TransactionsCommitment,
    ) -> Self {
        Self {
            reference: BlockReference {
                authority,
                round,
                digest: BlockDigest::new_without_statements(
                    authority,
                    round,
                    &includes,
                    &acknowledgement_statements,
                    meta_creation_time_ns,
                    epoch_marker,
                    &signature,
                    merkle_root,
                ),
            },
            includes,
            acknowledgement_statements,
            meta_creation_time_ns,
            epoch_marker,
            signature,
            statements: Some(statements),
            encoded_shard,
            merkle_proof_encoded_shard: merkle_proof,
            transactions_commitment: merkle_root,
        }
    }

    pub fn to_cached_block(&self, committee_size: usize) -> CachedStatementBlock {
        let mut encoded_statements: Vec<Option<Shard>> = Vec::new();
        for i in 0..committee_size {
            if let Some((shard, position)) = self.encoded_shard.as_ref() {
                if i == *position {
                    let new_shard: Shard = shard.clone();
                    encoded_statements.push(Some(new_shard));
                }
            }
            encoded_statements.push(None);
        }
        CachedStatementBlock {
            reference: self.reference,
            includes: self.includes.clone(),
            acknowledgement_statements: self.acknowledgement_statements.clone(),
            meta_creation_time_ns: self.meta_creation_time_ns,
            epoch_marker: self.epoch_marker,
            signature: self.signature,
            statements: self.statements.clone(),
            encoded_statements, // Replace `0` with the actual position logic
            merkle_proof_encoded_shard: self.merkle_proof_encoded_shard.clone(),
            merkle_root_encoded_statements: self.transactions_commitment,
        }
    }

    pub fn from_storage_to_transmission(&self, own_id: AuthorityIndex) -> Self {
        if own_id != self.reference.authority {
            if let Some((_, position)) = self.encoded_shard().as_ref() {
                if *position != own_id as usize {
                    return Self {
                        reference: self.reference,
                        includes: self.includes.clone(),
                        acknowledgement_statements: self.acknowledgement_statements.clone(),
                        meta_creation_time_ns: self.meta_creation_time_ns,
                        epoch_marker: self.epoch_marker,
                        signature: self.signature,
                        statements: None,
                        encoded_shard: None,
                        merkle_proof_encoded_shard: None,
                        transactions_commitment: self.transactions_commitment,
                    };
                }
            }
            Self {
                reference: self.reference,
                includes: self.includes.clone(),
                acknowledgement_statements: self.acknowledgement_statements.clone(),
                meta_creation_time_ns: self.meta_creation_time_ns,
                epoch_marker: self.epoch_marker,
                signature: self.signature,
                statements: None,
                encoded_shard: self.encoded_shard.clone(),
                merkle_proof_encoded_shard: self.merkle_proof_encoded_shard.clone(),
                transactions_commitment: self.transactions_commitment,
            }
        } else {
            Self {
                reference: self.reference,
                includes: self.includes.clone(),
                acknowledgement_statements: self.acknowledgement_statements.clone(),
                meta_creation_time_ns: self.meta_creation_time_ns,
                epoch_marker: self.epoch_marker,
                signature: self.signature,
                statements: self.statements.clone(),
                encoded_shard: None,
                merkle_proof_encoded_shard: None,
                transactions_commitment: self.transactions_commitment,
            }
        }
    }

    pub fn new_genesis(authority: AuthorityIndex) -> Data<Self> {
        Data::new(Self::new(
            authority,
            GENESIS_ROUND,
            vec![],
            vec![],
            0,
            false,
            SignatureBytes::default(),
            vec![],
            None,
            None,
            TransactionsCommitment::default(),
        ))
    }

    pub fn merkle_root(&self) -> TransactionsCommitment {
        self.transactions_commitment
    }

    pub fn acknowledgement_statements(&self) -> &Vec<BlockReference> {
        &self.acknowledgement_statements
    }

    pub fn reference(&self) -> &BlockReference {
        &self.reference
    }

    pub fn includes(&self) -> &Vec<BlockReference> {
        &self.includes
    }

    pub fn encoded_shard(&self) -> &Option<(Shard, usize)> {
        &self.encoded_shard
    }

    pub fn statements(&self) -> &Option<Vec<BaseStatement>> {
        &self.statements
    }

    pub fn number_transactions(&self) -> usize {
        if self.statements.is_some() {
            return self.statements().as_ref().unwrap().len();
        }
        0
    }

    pub fn author(&self) -> AuthorityIndex {
        self.reference.authority
    }

    pub fn round(&self) -> RoundNumber {
        self.reference.round
    }

    pub fn digest(&self) -> BlockDigest {
        self.reference.digest
    }

    pub fn author_round(&self) -> (AuthorityIndex, RoundNumber) {
        self.reference.author_round()
    }

    pub fn signature(&self) -> &SignatureBytes {
        &self.signature
    }

    pub fn meta_creation_time_ns(&self) -> TimestampNs {
        self.meta_creation_time_ns
    }

    pub fn meta_creation_time(&self) -> Duration {
        // Some context: https://github.com/rust-lang/rust/issues/51107
        let secs = self.meta_creation_time_ns / NANOS_IN_SEC;
        let nanos = self.meta_creation_time_ns % NANOS_IN_SEC;
        Duration::new(secs as u64, nanos as u32)
    }

    pub fn epoch_changed(&self) -> EpochStatus {
        self.epoch_marker
    }

    // pub fn from_bytes(bytes: Bytes) -> bincode::Result<Self> {
    //     IN_MEMORY_BLOCKS.fetch_add(1, Ordering::Relaxed);
    //     IN_MEMORY_BLOCKS_BYTES.fetch_add(bytes.len(), Ordering::Relaxed);
    //     let t = bincode::deserialize(&bytes)?;
    //     Ok(t)
    // }

    pub fn set_merkle_proof(&mut self, merkle_proof: Vec<u8>) {
        self.merkle_proof_encoded_shard = Some(merkle_proof);
    }

    pub fn new_with_signer(
        authority: AuthorityIndex,
        round: RoundNumber,
        includes: Vec<BlockReference>,
        acknowledgement_statements: Vec<BlockReference>,
        meta_creation_time_ns: TimestampNs,
        epoch_marker: EpochStatus,
        signer: &Signer,
        statements: Vec<BaseStatement>,
        encoded_statements: Option<Vec<Shard>>,
    ) -> Self {
        let transactions_commitment =
                TransactionsCommitment::new_from_encoded_statements(
                    encoded_statements.as_ref().unwrap(),
                    authority as usize,
                ).0;
        let signature = signer.sign_block(
            authority,
            round,
            &includes,
            &acknowledgement_statements,
            meta_creation_time_ns,
            epoch_marker,
            transactions_commitment,
        );

        Self::new(
            authority,
            round,
            includes,
            acknowledgement_statements,
            meta_creation_time_ns,
            epoch_marker,
            signature,
            statements,
            None,
            None,
            transactions_commitment,
        )
    }

    pub fn verify(
        &mut self,
        committee: &Committee,
        own_id: usize,
        peer_id: usize,
        encoder: &mut Encoder,
    ) -> eyre::Result<()> {
        let round = self.round();


                let committee_size = committee.len();
                let info_length = committee.info_length();
                let parity_length = committee_size - info_length;
                if self.statements.is_some() {
                    let encoded_statements = encoder.encode_statements(
                        self.statements.clone().unwrap(),
                        info_length,
                        parity_length,
                    );
                    let (transactions_commitment, merkle_proof_bytes) =
                        TransactionsCommitment::new_from_encoded_statements(
                            &encoded_statements,
                            own_id,
                        );
                    ensure!(
                        transactions_commitment == self.transactions_commitment,
                        "Incorrect Merkle root"
                    );
                    self.merkle_proof_encoded_shard = Some(merkle_proof_bytes);
                    self.encoded_shard = Some((encoded_statements[own_id].clone(), own_id));
                } else if self.encoded_shard.is_some() {
                    let (encoded_shard, position) = self
                        .encoded_shard
                        .clone()
                        .expect("We expect an encoded shard");
                    if position != peer_id {
                        bail!("The peer delivers a wrong encoded chunk");
                    }
                    if self.merkle_proof_encoded_shard.is_none() {
                        bail!("The peer didn't include the proof for the encoded shard");
                    }
                    ensure!(
                        TransactionsCommitment::check_correctness_merkle_leaf(
                            encoded_shard,
                            self.transactions_commitment,
                            self.merkle_proof_encoded_shard.as_ref().cloned().unwrap(),
                            committee_size,
                            position
                        ),
                        "Merkle proof check failed"
                    );
                }



        let digest = BlockDigest::new(
            self.author(),
            round,
            &self.includes,
            &self.acknowledgement_statements,
            self.meta_creation_time_ns,
            self.epoch_marker,
            &self.signature,
            self.transactions_commitment,
        );
        ensure!(
            digest == self.digest(),
            "Digest does not match, calculated {:?}, provided {:?}",
            digest,
            self.digest()
        );
        let pub_key = committee.get_public_key(self.author());
        let Some(pub_key) = pub_key else {
            bail!("Unknown block author {}", self.author())
        };
        if round == GENESIS_ROUND {
            bail!("Genesis block should not go through verification");
        }
        if let Err(e) = pub_key.verify_signature_in_block(self) {
            bail!("Block signature verification has failed: {:?}", e);
        }
        for include in &self.includes {
            // Also check duplicate includes?
            ensure!(
                committee.known_authority(include.authority),
                "Include {:?} references unknown authority",
                include
            );
            ensure!(
                include.round < round,
                "Include {:?} round is greater or equal to own round {}",
                include,
                round
            );
        }
        ensure!(
            threshold_clock_valid_verified_block(self, committee),
            "Threshold clock is not valid"
        );
        Ok(())
    }
}
#[derive(
    Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize, Default, Debug,
)]
pub struct AuthoritySet(u128); // todo - support more then 128 authorities

pub type TimestampNs = u128;
const NANOS_IN_SEC: u128 = Duration::from_secs(1).as_nanos();

const GENESIS_ROUND: RoundNumber = 0;

impl PartialOrd for BlockReference {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for BlockReference {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.round, self.authority, self.digest).cmp(&(other.round, other.authority, other.digest))
    }
}

#[derive(Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize, Default)]
pub struct TransactionLocator {
    block: BlockReference,
    offset: u64,
}

impl BlockReference {
    #[cfg(test)]
    pub fn new_test(authority: AuthorityIndex, round: RoundNumber) -> Self {
        if round == 0 {
            VerifiedStatementBlock::new_genesis(authority).reference
        } else {
            Self {
                authority,
                round,
                digest: Default::default(),
            }
        }
    }

    pub fn round(&self) -> RoundNumber {
        self.round
    }

    pub fn author_round(&self) -> (AuthorityIndex, RoundNumber) {
        (self.authority, self.round)
    }

    pub fn author_digest(&self) -> (AuthorityIndex, BlockDigest) {
        (self.authority, self.digest)
    }
}

impl fmt::Debug for BlockReference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self)
    }
}

impl fmt::Display for BlockReference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.authority < 26 {
            write!(
                f,
                "{}{}{}",
                format_authority_index(self.authority),
                self.round,
                self.digest
            )
        } else {
            write!(f, "[{:02}]{}", self.authority, self.round)
        }
    }
}

impl AuthoritySet {
    #[inline]
    pub fn insert(&mut self, v: AuthorityIndex) -> bool {
        let bit = 1u128 << v;
        if self.0 & bit == bit {
            return false;
        }
        self.0 |= bit;
        true
    }

    pub fn present(&self) -> impl Iterator<Item = AuthorityIndex> + '_ {
        (0..128).filter(move |&bit| (self.0 & (1u128 << bit)) != 0)
    }

    #[inline]
    pub fn clear(&mut self) {
        self.0 = 0;
    }
}

pub fn format_authority_index(i: AuthorityIndex) -> char {
    ('A' as u64 + i) as u8 as char
}

pub fn format_authority_round(i: AuthorityIndex, r: RoundNumber) -> String {
    format!("{}{}", format_authority_index(i), r)
}

impl fmt::Debug for VerifiedStatementBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self)
    }
}

impl fmt::Display for VerifiedStatementBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:[", self.reference)?;
        for include in self.includes() {
            write!(f, "{},", include)?;
        }
        write!(f, "](")?;
        if self.statements.is_some() {
            write!(f, "ledger")?;
        } else if self.encoded_shard.is_some() {
            write!(f, "shard")?;
        } else {
            write!(f, "header")?;
        }
        write!(f, ")")
    }
}
impl PartialEq for VerifiedStatementBlock {
    fn eq(&self, other: &Self) -> bool {
        self.reference == other.reference
    }
}
impl Eq for VerifiedStatementBlock {}

impl std::hash::Hash for VerifiedStatementBlock {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.reference.hash(state);
    }
}

impl fmt::Debug for BaseStatement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self)
    }
}

impl CryptoHash for BlockReference {
    fn crypto_hash(&self, state: &mut crypto::Blake3Hasher) {
        self.authority.crypto_hash(state);
        self.round.crypto_hash(state);
        self.digest.crypto_hash(state);
    }
}

impl CryptoHash for Shard {
    fn crypto_hash(&self, state: &mut crypto::Blake3Hasher) {
        state.update(self);
    }
}
impl CryptoHash for EpochStatus {
    fn crypto_hash(&self, state: &mut crypto::Blake3Hasher) {
        match self {
            false => [0].crypto_hash(state),
            true => [1].crypto_hash(state),
        }
    }
}

impl CryptoHash for BaseStatement {
    fn crypto_hash(&self, state: &mut crypto::Blake3Hasher) {
        match self {
            BaseStatement::Share(tx) => {
                [0].crypto_hash(state);
                tx.crypto_hash(state);
            }
        }
    }
}

impl fmt::Display for BaseStatement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BaseStatement::Share(_tx) => write!(f, "tx"),
        }
    }
}

impl Transaction {
    pub fn new(data: Vec<u8>) -> Self {
        Self { data }
    }

    #[allow(dead_code)]
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    #[allow(dead_code)]
    pub fn into_data(self) -> Vec<u8> {
        self.data
    }
}

impl AsBytes for Transaction {
    fn as_bytes(&self) -> &[u8] {
        &self.data
    }
}

#[cfg(test)]
mod test {
    use std::{
        collections::{HashMap, HashSet},
        sync::Arc,
    };

    use rand::{prelude::SliceRandom, Rng};

    use super::*;

    pub struct Dag(HashMap<BlockReference, Data<VerifiedStatementBlock>>);

    #[cfg(test)]
    impl Dag {
        /// Takes a string in form "Block:[Dependencies, ...]; ..."
        /// Where Block is one letter denoting a node and a number denoting a round
        /// For example B3 is a block for round 3 made by validator index 2
        /// Note that blocks are separated with semicolon(;) and dependencies within block are separated with coma(,)
        pub fn draw(s: &str) -> Self {
            let mut blocks = HashMap::new();
            for block in s.split(";") {
                let block = Self::draw_block(block);
                blocks.insert(*block.reference(), Data::new(block));
            }
            Self(blocks)
        }

        pub fn draw_block(block: &str) -> VerifiedStatementBlock {
            let block = block.trim();
            assert!(block.ends_with(']'), "Invalid block definition: {}", block);
            let block = &block[..block.len() - 1];
            let Some((name, includes)) = block.split_once(":[") else {
                panic!("Invalid block definition: {}", block);
            };
            let reference = Self::parse_name(name);
            let includes = includes.trim();
            let includes = if includes.len() == 0 {
                vec![]
            } else {
                let includes = includes.split(',');
                includes.map(Self::parse_name).collect()
            };
            let acknowledgement_statements = includes.clone();
            VerifiedStatementBlock {
                reference,
                includes,
                acknowledgement_statements,
                meta_creation_time_ns: 0,
                epoch_marker: false,
                signature: Default::default(),
                statements: None,
                encoded_shard: None,
                merkle_proof_encoded_shard: None,
                transactions_commitment: TransactionsCommitment::default(),
            }
        }

        fn parse_name(s: &str) -> BlockReference {
            let s = s.trim();
            assert!(s.len() >= 2, "Invalid block: {}", s);
            let authority = s.as_bytes()[0];
            let authority = authority.wrapping_sub('A' as u8);
            assert!(authority < 26, "Invalid block: {}", s);
            let Ok(round): Result<u64, _> = s[1..].parse() else {
                panic!("Invalid block: {}", s);
            };
            BlockReference::new_test(authority as u64, round)
        }

        /// For each authority add a 0 round block if not present
        pub fn add_genesis_blocks(mut self) -> Self {
            for authority in self.authorities() {
                let block = VerifiedStatementBlock::new_genesis(authority);
                let entry = self.0.entry(*block.reference());
                entry.or_insert_with(move || block);
            }
            self
        }

        pub fn random_iter(&self, rng: &mut impl Rng) -> RandomDagIter {
            let mut v: Vec<_> = self.0.keys().cloned().collect();
            v.shuffle(rng);
            RandomDagIter(self, v.into_iter())
        }

        pub fn len(&self) -> usize {
            self.0.len()
        }

        fn authorities(&self) -> HashSet<AuthorityIndex> {
            let mut authorities = HashSet::new();
            for (k, v) in &self.0 {
                authorities.insert(k.authority);
                for include in v.includes() {
                    authorities.insert(include.authority);
                }
            }
            authorities
        }

        pub fn committee(&self) -> Arc<Committee> {
            Committee::new_test(vec![1; self.authorities().len()])
        }
    }

    pub struct RandomDagIter<'a>(&'a Dag, std::vec::IntoIter<BlockReference>);

    impl<'a> Iterator for RandomDagIter<'a> {
        type Item = &'a Data<VerifiedStatementBlock>;

        fn next(&mut self) -> Option<Self::Item> {
            let next = self.1.next()?;
            Some(self.0 .0.get(&next).unwrap())
        }
    }

    #[test]
    fn test_draw_dag() {
        let d = Dag::draw("A1:[A0, B1]; B2:[B1]").0;
        assert_eq!(d.len(), 2);
        let a0: BlockReference = BlockReference::new_test(0, 1);
        let b2: BlockReference = BlockReference::new_test(1, 2);
        assert_eq!(&d.get(&a0).unwrap().reference, &a0);
        assert_eq!(
            &d.get(&a0).unwrap().includes,
            &vec![
                BlockReference::new_test(0, 0),
                BlockReference::new_test(1, 1)
            ]
        );
        assert_eq!(&d.get(&b2).unwrap().reference, &b2);
        assert_eq!(
            &d.get(&b2).unwrap().includes,
            &vec![BlockReference::new_test(1, 1)]
        );
    }

    #[test]
    fn authority_set_test() {
        let mut a = AuthoritySet::default();
        assert!(a.insert(0));
        assert!(!a.insert(0));
        assert!(a.insert(1));
        assert!(a.insert(2));
        assert!(!a.insert(1));
        assert!(a.insert(127));
        assert!(!a.insert(127));
        assert!(a.insert(3));
        assert!(!a.insert(3));
        assert!(!a.insert(2));
    }

    #[test]
    fn authority_present_test() {
        let mut a = AuthoritySet::default();
        let present = vec![1, 2, 3, 4, 5, 64, 127];
        for x in &present {
            a.insert(*x);
        }
        assert_eq!(present, a.present().collect::<Vec<_>>());
    }
}
