// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    ops::{Bound::Included, RangeInclusive},
    sync::Arc,
};

use parking_lot::RwLock;
use rand::{Rng, SeedableRng, rngs::StdRng, seq::SliceRandom};
use starfish_config::{AuthorityIndex, ProtocolKeyPair};

use crate::{
    CommitRef, CommittedSubDag,
    block_header::{
        BlockHeaderAPI, BlockHeaderDigest, BlockRef, BlockTimestampMs, Round, Slot,
        TestBlockHeader, Transaction, TransactionsCommitment, VerifiedBlock, VerifiedBlockHeader,
        VerifiedTransactions, genesis_block_headers,
    },
    commit::{CertifiedCommit, CommitAPI, CommitDigest, TrustedCommit, WAVE_LENGTH},
    context::Context,
    dag_state::{DagState, DataSource},
    encoder::{ShardEncoder, create_encoder},
    leader_schedule::{LeaderSchedule, LeaderSwapTable},
    linearizer::{BlockStoreAPI, Linearizer},
    transaction_ref::TransactionRef,
};

/// DagBuilder API
///
/// Usage:
///
/// DAG Building
/// ```ignore
/// use std::sync::Arc;
/// use super::context::Context;
/// use super::test_dag_builder::DagBuilder;
/// let context = Arc::new(Context::new_for_test(4).0);
/// let mut dag_builder = DagBuilder::new(context);
/// dag_builder.layer(1).build(); // Round 1 is fully connected with parents by default.
/// dag_builder.layers(2..=10).build(); // Rounds 2 ~ 10 are fully connected with parents by default.
/// dag_builder.layers(11).skip_acknowledgements(vec![1,2]).build(); // Round 11 skips acknowledgments for blocks from authorities 1 and 2.
/// dag_builder.layers(12).min_parent_links().build(); // Round 11 is minimally and randomly connected with parents, without weak links.
/// dag_builder.layers(13).no_leader_block(0).build(); // Round 12 misses leader block. Other blocks are fully connected with parents.
/// dag_builder.layers(14).no_leader_link(12, 0).build(); // Round 13 misses votes for leader block. Other blocks are fully connected with parents.
/// dag_builder.layers(15).authorities(vec![3,5]).skip_block().build(); // Round 14 authorities 3 and 5 will not propose any block.
/// dag_builder.layers(16).authorities(vec![3,5]).skip_ancestor_links(vec![1,2]).build(); // Round 15 authorities 3 and 5 will not link to ancestors 1 and 2
/// dag_builder.layers(17).authorities(vec![3,5]).equivocate(3).build(); // Round 16 authorities 3 and 5 will produce 3 equivocating blocks.
/// ```
///
/// Persisting to DagState by Layer
/// ```ignore
/// use std::sync::{Arc, RwLock};
///
/// use super::{
///     context::Context, dag_state::DagState, storage::MemStore, test_dag_builder::DagBuilder,
/// };
/// let dag_state = Arc::new(RwLock::new(DagState::new(
///     dag_builder.context.clone(),
///     Arc::new(MemStore::new(context.clone())),
/// )));
/// let context = Arc::new(Context::new_for_test(4).0);
/// let dag_builder = DagBuilder::new(context);
/// dag_builder
///     .layer(1)
///     .build()
///     .persist_layers(dag_state.clone()); // persist the layer
/// ```
///
/// Persisting entire DAG to DagState
/// ```ignore
/// use std::sync::{Arc, RwLock};
///
/// use super::{
///     context::Context, dag_state::DagState, storage::MemStore, test_dag_builder::DagBuilder,
/// };
/// let context = Arc::new(Context::new_for_test(4).0);
/// let dag_builder = DagBuilder::new(context);
/// let dag_state = Arc::new(RwLock::new(DagState::new(
///     dag_builder.context.clone(),
///     Arc::new(MemStore::new(context.clone())),
/// )));
///
/// dag_builder.layer(1).build();
/// dag_builder.layers(2..10).build();
/// dag_builder.persist_all_blocks(dag_state.clone()); // persist entire DAG
/// ```
///
/// Printing DAG
/// ```ignore
/// use std::sync::Arc;
///
/// use super::{context::Context, test_dag_builder::DagBuilder};
/// let context = Arc::new(Context::new_for_test(4).0);
/// let dag_builder = DagBuilder::new(context);
/// dag_builder.layer(1).build();
/// dag_builder.print(); // pretty print the entire DAG
/// ```
pub(crate) struct DagBuilder {
    pub(crate) context: Arc<Context>,
    pub(crate) leader_schedule: LeaderSchedule,
    // The genesis blocks
    pub(crate) genesis: BTreeMap<BlockRef, VerifiedBlockHeader>,
    // The current set of ancestors that any new layer will attempt to connect to.
    pub(crate) last_ancestors: Vec<BlockRef>,
    // All blocks created by dag builder. Will be used to pretty print or to be
    // retrieved for testing/persiting to dag state.
    pub(crate) block_headers: BTreeMap<BlockRef, VerifiedBlockHeader>,
    pub(crate) transactions: BTreeMap<BlockRef, VerifiedTransactions>,
    // All the committed sub dags created by the dag builder.
    pub(crate) committed_sub_dags: Vec<(CommittedSubDag, TrustedCommit)>,
    pub(crate) last_committed_rounds: Vec<Round>,

    wave_length: Round,
    number_of_leaders: u32,
    pipeline: bool,
    // Protocol keypairs are used to compute signature for headers. If it is None, then the Default
    // signature is used
    protocol_keypair: Option<Vec<ProtocolKeyPair>>,

    encoder: Box<dyn ShardEncoder + Send + Sync>,
}
/// The `AncestorSelection` enum is an interim data structure used to specify
/// how ancestors should be selected for a block in the `DagBuilder`. `UseLast`
/// equates to the "*" in the parser, while `IncludeFrom(Slot)` and
/// `ExcludeFrom(Slot)` equate to "A3" and "-A3" respectively.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Hash)]
pub(crate) enum AncestorSelection {
    UseLast,
    IncludeFrom(Slot),
    ExcludeFrom(Slot),
}
/// The `AncestorConnectionSpec` enum is an interim data structure used to
/// represent round definitions of the `test_dag_parser`, where `FullyConnected`
/// equates to the { * } in the parser, while `AuthoritySpecific` equates to the
/// { A -> [], B -> [] } in the parser.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum AncestorConnectionSpec {
    FullyConnected,
    AuthoritySpecific(
        Vec<(AuthorityIndex, Vec<AncestorSelection>)>,
        HashMap<AuthorityIndex, Vec<AncestorSelection>>,
    ),
}

impl DagBuilder {
    pub(crate) fn new(context: Arc<Context>) -> Self {
        let leader_schedule = LeaderSchedule::new(context.clone(), LeaderSwapTable::default());
        let genesis_blocks = genesis_block_headers(&context);
        let genesis: BTreeMap<BlockRef, VerifiedBlockHeader> = genesis_blocks
            .into_iter()
            .map(|block| (block.reference(), block))
            .collect();
        let last_ancestors = genesis.keys().cloned().collect();

        let encoder = create_encoder(&context);
        Self {
            last_committed_rounds: vec![0; context.committee.size()],
            context,
            leader_schedule,
            wave_length: WAVE_LENGTH,
            number_of_leaders: 1,
            pipeline: false,
            genesis,
            last_ancestors,
            block_headers: BTreeMap::new(),
            transactions: BTreeMap::new(),
            committed_sub_dags: vec![],
            protocol_keypair: None,
            encoder,
        }
    }

    pub(crate) fn set_protocol_keypair(mut self, protocol_keypairs: Vec<ProtocolKeyPair>) -> Self {
        self.protocol_keypair = Some(protocol_keypairs);
        self
    }

    pub(crate) fn blocks(&self, rounds: RangeInclusive<Round>) -> Vec<VerifiedBlock> {
        assert!(
            !self.block_headers.is_empty(),
            "No blocks have been created, please make sure that you have called build method"
        );
        self.block_headers
            .iter()
            .filter_map(|(block_ref, block_header)| {
                rounds.contains(&block_ref.round).then_some(VerifiedBlock {
                    verified_block_header: block_header.clone(),
                    verified_transactions: self.transactions.get(block_ref).cloned()?,
                })
            })
            .collect::<Vec<VerifiedBlock>>()
    }

    pub(crate) fn block_headers(&self, rounds: RangeInclusive<Round>) -> Vec<VerifiedBlockHeader> {
        assert!(
            !self.block_headers.is_empty(),
            "No blocks have been created, please make sure that you have called build method"
        );
        self.block_headers
            .iter()
            .filter_map(|(block_ref, block_header)| {
                rounds.contains(&block_ref.round).then_some(block_header)
            })
            .cloned()
            .collect::<Vec<VerifiedBlockHeader>>()
    }

    pub(crate) fn transactions(&self, rounds: RangeInclusive<Round>) -> Vec<VerifiedTransactions> {
        assert!(
            !self.transactions.is_empty(),
            "No transactions have been created, please make sure that you have called build method"
        );
        self.transactions
            .iter()
            .filter_map(|(block_ref, verified_transactions)| {
                rounds
                    .contains(&block_ref.round)
                    .then_some(verified_transactions)
            })
            .cloned()
            .collect::<Vec<VerifiedTransactions>>()
    }

    pub(crate) fn all_block_headers(&self) -> Vec<VerifiedBlockHeader> {
        assert!(
            !self.block_headers.is_empty(),
            "No block headers have been created, please make sure that you have called build method"
        );
        self.block_headers.values().cloned().collect()
    }

    pub(crate) fn get_sub_dag_and_commits(
        &mut self,
        leader_rounds: RangeInclusive<Round>,
    ) -> Vec<(CommittedSubDag, TrustedCommit)> {
        let (last_leader_round, mut last_commit_ref, mut last_timestamp_ms) =
            if let Some((sub_dag, _)) = self.committed_sub_dags.last() {
                (
                    sub_dag.leader.round,
                    sub_dag.commit_ref,
                    sub_dag.timestamp_ms,
                )
            } else {
                (0, CommitRef::new(0, CommitDigest::MIN), 0)
            };

        struct BlockStorage {
            // the tuple represents the block and whether it is committed
            // blocks: BTreeMap<BlockRef, (VerifiedBlock, bool)>,
            block_headers: BTreeMap<BlockRef, (VerifiedBlockHeader, bool)>,
            genesis: BTreeMap<BlockRef, VerifiedBlockHeader>,
        }
        impl BlockStoreAPI for BlockStorage {
            fn get_block_headers(&self, refs: &[BlockRef]) -> Vec<Option<VerifiedBlockHeader>> {
                refs.iter()
                    .map(|block_ref| {
                        if block_ref.round == 0 {
                            return self.genesis.get(block_ref).cloned();
                        }
                        self.block_headers
                            .get(block_ref)
                            .map(|(header, _committed)| header.clone())
                    })
                    .collect()
            }
        }
        let storage = BlockStorage {
            block_headers: self
                .block_headers
                .clone()
                .into_iter()
                .map(|(k, v)| (k, (v, false)))
                .collect(),
            genesis: self.genesis.clone(),
        };

        // Create any remaining committed sub dags
        for leader_block in self
            .leader_blocks(last_leader_round + 1..=*leader_rounds.end())
            .into_iter()
            .flatten()
        {
            let leader_block_ref = leader_block.reference();
            last_timestamp_ms = leader_block.timestamp_ms().max(last_timestamp_ms);

            let to_commit = Linearizer::linearize_sub_dag(
                leader_block.clone(),
                self.last_committed_rounds.clone(),
                &storage,
                self.context.protocol_config.gc_depth(),
            );

            last_timestamp_ms = Linearizer::calculate_commit_timestamp(
                &self.context.clone(),
                &storage,
                &leader_block,
                last_timestamp_ms,
            );

            // Update the last committed rounds
            for block_header in &to_commit {
                self.last_committed_rounds[block_header.author()] =
                    self.last_committed_rounds[block_header.author()].max(block_header.round());
            }

            let commit = TrustedCommit::new_for_test(
                &self.context,
                last_commit_ref.index + 1,
                last_commit_ref.digest,
                last_timestamp_ms,
                leader_block_ref,
                to_commit
                    .iter()
                    .map(|block| block.reference())
                    .collect::<Vec<_>>(),
                vec![],
            );

            last_commit_ref = commit.reference();

            let sub_dag = CommittedSubDag::new(
                leader_block_ref,
                to_commit,
                commit.block_headers().to_vec(),
                vec![],
                last_timestamp_ms,
                commit.reference(),
                vec![],
            );

            self.committed_sub_dags.push((sub_dag, commit));
        }

        self.committed_sub_dags
            .clone()
            .into_iter()
            .filter(|(sub_dag, _)| leader_rounds.contains(&sub_dag.leader.round))
            .collect()
    }
    pub(crate) fn leader_blocks(
        &self,
        rounds: RangeInclusive<Round>,
    ) -> Vec<Option<VerifiedBlockHeader>> {
        assert!(
            !self.block_headers.is_empty(),
            "No blocks have been created, please make sure that you have called build method"
        );
        rounds
            .into_iter()
            .map(|round| self.leader_block(round))
            .collect()
    }

    pub(crate) fn get_sub_dag_and_certified_commits(
        &mut self,
        leader_rounds: RangeInclusive<Round>,
    ) -> Vec<(CommittedSubDag, CertifiedCommit)> {
        let commits = self.get_sub_dag_and_commits(leader_rounds);
        commits
            .into_iter()
            .map(|(sub_dag, commit)| {
                // TODO: we need to request real blocks from sub_dag after we add the
                // corresponding field and logic in sub_dag
                let block_headers = sub_dag.headers.clone();
                let transactions = sub_dag.transactions.clone();

                let certified_commit =
                    CertifiedCommit::new_certified(commit, block_headers, transactions);
                (sub_dag, certified_commit)
            })
            .collect()
    }

    pub(crate) fn leader_block(&self, round: Round) -> Option<VerifiedBlockHeader> {
        assert!(
            !self.block_headers.is_empty(),
            "No blocks have been created, please make sure that you have called build method"
        );
        self.block_headers.iter().find_map(|(block_ref, block)| {
            (block_ref.round == round
                && block_ref.author == self.leader_schedule.elect_leader(round, 0))
            .then_some(block.clone())
        })
    }

    #[expect(unused)]
    pub(crate) fn with_wave_length(mut self, wave_length: Round) -> Self {
        self.wave_length = wave_length;
        self
    }

    #[expect(unused)]
    pub(crate) fn with_number_of_leaders(mut self, number_of_leaders: u32) -> Self {
        self.number_of_leaders = number_of_leaders;
        self
    }

    #[expect(unused)]
    pub(crate) fn with_pipeline(mut self, pipeline: bool) -> Self {
        self.pipeline = pipeline;
        self
    }

    pub(crate) fn layer(&mut self, round: Round) -> LayerBuilder<'_> {
        LayerBuilder::new(self, round)
    }

    pub(crate) fn layers(&mut self, rounds: RangeInclusive<Round>) -> LayerBuilder<'_> {
        let mut builder = LayerBuilder::new(self, *rounds.start());
        builder.end_round = Some(*rounds.end());
        builder
    }

    pub(crate) fn persist_all_blocks(&self, dag_state: Arc<RwLock<DagState>>) {
        dag_state.write().accept_block_headers(
            self.block_headers.values().cloned().collect(),
            DataSource::Test,
        );
        for block_transactions in self.transactions.values() {
            dag_state
                .write()
                .add_transactions(block_transactions.clone(), DataSource::Test);
        }
    }

    pub(crate) fn print(&self) {
        let mut dag_str = "DAG {\n".to_string();

        let mut round = 0;
        for block in self.block_headers.values() {
            if block.round() > round {
                round = block.round();
                dag_str.push_str(&format!("Round {round} : \n"));
            }
            dag_str.push_str(&format!("    Block {block:#?}\n"));
        }
        dag_str.push_str("}\n");

        tracing::info!("{dag_str}");
    }

    // Gets all uncommitted blocks in a slot.
    pub(crate) fn get_uncommitted_blocks_at_slot(&self, slot: Slot) -> Vec<VerifiedBlockHeader> {
        let mut blocks = vec![];
        for (_block_ref, block) in self.block_headers.range((
            Included(BlockRef::new(
                slot.round,
                slot.authority,
                BlockHeaderDigest::MIN,
            )),
            Included(BlockRef::new(
                slot.round,
                slot.authority,
                BlockHeaderDigest::MAX,
            )),
        )) {
            blocks.push(block.clone())
        }
        blocks
    }

    pub(crate) fn genesis_block_refs(&self) -> Vec<BlockRef> {
        self.genesis.keys().cloned().collect()
    }

    fn get_blocks(&self, slot: Slot) -> Vec<BlockRef> {
        // note: special case for genesis blocks as they are cached separately
        let block_refs = if slot.round == 0 {
            self.genesis_block_refs()
                .into_iter()
                .filter(|block| Slot::from(*block) == slot)
                .collect::<Vec<_>>()
        } else {
            self.get_uncommitted_blocks_at_slot(slot)
                .iter()
                .map(|block| block.reference())
                .collect::<Vec<_>>()
        };
        block_refs
    }

    // Converts the ancestor selections into block references from the DagBuilder's
    // last ancestors or from the blocks in the specified slots.
    fn get_references_from_ancestor_selections(
        &self,
        ancestor_selections: Vec<AncestorSelection>,
    ) -> Vec<BlockRef> {
        let mut block_refs = vec![];
        for ancestor_selection in ancestor_selections {
            match ancestor_selection {
                AncestorSelection::UseLast => {
                    block_refs.extend(self.last_ancestors.clone());
                }
                AncestorSelection::ExcludeFrom(slot) => {
                    let stored_block_refs = self.get_blocks(slot);
                    block_refs.extend(self.last_ancestors.clone());

                    block_refs.retain(|ancestor| !stored_block_refs.contains(ancestor));
                }
                AncestorSelection::IncludeFrom(slot) => {
                    let stored_block_refs = self.get_blocks(slot);
                    block_refs.extend(stored_block_refs);
                }
            }
        }
        block_refs
    }

    fn get_transaction_acks_from_ancestor_selections(
        &self,
        ancestor_selections: Vec<AncestorSelection>,
    ) -> Vec<BlockRef> {
        let mut block_refs = vec![];
        for ancestor_selection in ancestor_selections {
            match ancestor_selection {
                AncestorSelection::UseLast => {
                    block_refs.extend(self.last_ancestors.clone());
                }
                AncestorSelection::ExcludeFrom(slot) => {
                    let stored_block_refs = self.get_blocks(slot);
                    block_refs.extend(self.last_ancestors.clone());

                    block_refs.retain(|ancestor| !stored_block_refs.contains(ancestor));
                }
                AncestorSelection::IncludeFrom(slot) => {
                    let stored_block_refs = self.get_blocks(slot);
                    block_refs.extend(stored_block_refs);
                }
            }
        }
        block_refs
    }

    // TODO: merge into layer builder?
    // This method allows the user to specify specific links to ancestors. The
    // layer is written to dag state and the blocks are cached in [`DagBuilder`]
    // state.
    pub(crate) fn layer_with_connections(
        &mut self,
        ancestor_connection_spec: AncestorConnectionSpec,
        round: Round,
    ) {
        let (transaction_acks, connections) = match ancestor_connection_spec {
            AncestorConnectionSpec::FullyConnected => {
                let ancestors = self.last_ancestors.clone();
                let connections = self
                    .context
                    .committee
                    .authorities()
                    .map(|authority| (authority.0, ancestors.clone()))
                    .collect::<Vec<_>>();
                let transaction_acks = if round == 1 {
                    HashMap::new()
                } else {
                    connections.clone().into_iter().collect()
                };
                (transaction_acks, connections)
            }
            AncestorConnectionSpec::AuthoritySpecific(ancestor_connections, transaction_acks) => {
                let mut connections = vec![];
                for (authority, ancestor_specs) in ancestor_connections {
                    let block_refs = self.get_references_from_ancestor_selections(ancestor_specs);
                    connections.push((authority, block_refs));
                }
                let mut transaction_acks_map: HashMap<AuthorityIndex, Vec<BlockRef>> =
                    HashMap::new();
                if round > 1 {
                    for (authority, transactions_ancestor_specs) in transaction_acks {
                        let transaction_acks = self.get_transaction_acks_from_ancestor_selections(
                            transactions_ancestor_specs,
                        );
                        transaction_acks_map.insert(authority, transaction_acks);
                    }
                }
                (transaction_acks_map, connections)
            }
        };

        let mut references = Vec::new();

        for (authority, ancestors) in connections {
            let author = authority.value() as u8;
            let base_ts = round as BlockTimestampMs * 1000;
            let block = VerifiedBlockHeader::new_for_test(
                TestBlockHeader::new(round, author)
                    .set_ancestors(ancestors)
                    .set_acknowledgments(
                        transaction_acks
                            .get(&authority)
                            .cloned()
                            .unwrap_or_default(),
                    )
                    .set_timestamp_ms(base_ts + author as u64)
                    .build(),
            );
            references.push(block.reference());
            self.block_headers.insert(block.reference(), block.clone());
        }
        let mut rng = StdRng::from_entropy();
        let unique_transaction_acks: Vec<BlockRef> = transaction_acks
            .values()
            .flatten()
            .cloned()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        for block_ref in unique_transaction_acks {
            // Create random transactions and their commitment.
            let mut tx_bytes = [0u8; 32];
            rng.fill(&mut tx_bytes[..]);
            let transactions = vec![Transaction::new(tx_bytes.to_vec())];
            let serialized_transactions = Transaction::serialize(&transactions).unwrap();
            let commitment = TransactionsCommitment::compute_transactions_commitment(
                &serialized_transactions,
                &self.context,
                &mut self.encoder,
            )
            .unwrap();

            let verified_transactions = VerifiedTransactions::new(
                transactions,
                TransactionRef::new(block_ref, commitment),
                Some(block_ref.digest),
                serialized_transactions,
            );
            self.transactions.insert(block_ref, verified_transactions);
        }
        self.last_ancestors = references;
    }
}
/// Refer to doc comments for [`DagBuilder`] for usage information.
pub struct LayerBuilder<'a> {
    dag_builder: &'a mut DagBuilder,

    start_round: Round,
    end_round: Option<Round>,

    // Configuration options applied to specified authorities
    // TODO: convert configuration options into an enum
    specified_authorities: Option<Vec<AuthorityIndex>>,
    // Number of equivocating blocks per specified authority
    equivocations: usize,
    // Skip block proposal for specified authorities
    skip_block: bool,
    // Skip specified ancestor links for specified authorities
    skip_ancestor_links: Option<Vec<AuthorityIndex>>,
    // Skip specified acknowledgements for blocks from specified authorities
    skip_acknowledgements: Option<Vec<AuthorityIndex>>,
    // Only acknowledge blocks from specified authorities
    only_acknowledge: Option<Vec<AuthorityIndex>>,
    // Skip leader link for specified authorities
    no_leader_link: bool,

    // Skip leader block proposal
    no_leader_block: bool,
    // Used for leader based configurations
    specified_leader_link_offsets: Option<Vec<u32>>,
    specified_leader_block_offsets: Option<Vec<u32>>,
    leader_round: Option<Round>,

    // All ancestors will be linked to the current layer
    fully_linked_ancestors: bool,
    // Only 2f+1 random ancestors will be linked to the current layer using a
    // seed, if provided
    min_ancestor_links: bool,
    min_ancestor_links_random_seed: Option<u64>,
    // Add random weak links to the current layer using a seed, if provided
    random_weak_links: bool,
    random_weak_links_random_seed: Option<u64>,
    // All transactions from the previous round will be linked in the current round.
    fully_linked_acknowledgments: bool,
    // Ancestors to link to the current layer
    ancestors: Vec<BlockRef>,

    // The block timestamps for the layer for each specified authority. This will work as base
    // timestamp and the round will be added to make sure that timestamps do offset.
    timestamps: Vec<BlockTimestampMs>,

    // add timestamp delay
    timestamp_delay_ms: Option<u64>,

    // Accumulated blocks to write to dag state
    block_headers: Vec<VerifiedBlockHeader>,
    pub(crate) transactions: Vec<VerifiedTransactions>,
}

#[expect(unused)]
impl<'a> LayerBuilder<'a> {
    fn new(dag_builder: &'a mut DagBuilder, start_round: Round) -> Self {
        assert!(start_round > 0, "genesis round is created by default");
        let ancestors = dag_builder.last_ancestors.clone();
        Self {
            dag_builder,
            start_round,
            end_round: None,
            specified_authorities: None,
            equivocations: 0,
            skip_block: false,
            skip_ancestor_links: None,
            no_leader_link: false,
            no_leader_block: false,
            specified_leader_link_offsets: None,
            specified_leader_block_offsets: None,
            leader_round: None,
            fully_linked_ancestors: true,
            min_ancestor_links: false,
            min_ancestor_links_random_seed: None,
            random_weak_links: false,
            random_weak_links_random_seed: None,
            fully_linked_acknowledgments: true,
            skip_acknowledgements: None,
            only_acknowledge: None,
            timestamp_delay_ms: None,
            ancestors,
            timestamps: vec![],
            block_headers: vec![],
            transactions: vec![],
        }
    }

    // Configuration methods

    // Only link 2f+1 random ancestors to the current layer round using a seed,
    // if provided. Also provide a flag to guarantee the leader is included.
    // note: configuration is terminal and layer will be built after this call.
    pub fn min_ancestor_links(mut self, include_leader: bool, seed: Option<u64>) -> Self {
        self.min_ancestor_links = true;
        self.min_ancestor_links_random_seed = seed;
        if include_leader {
            self.leader_round = Some(self.ancestors.iter().max_by_key(|b| b.round).unwrap().round);
        }
        self.fully_linked_ancestors = false;
        self.build()
    }

    // No links will be created between the specified ancestors and the specified
    // authorities at the layer round.
    // note: configuration is terminal and layer will be built after this call.
    pub fn skip_ancestor_links(mut self, ancestors_to_skip: Vec<AuthorityIndex>) -> Self {
        // authorities must be specified for this to apply
        assert!(self.specified_authorities.is_some());
        self.skip_ancestor_links = Some(ancestors_to_skip);
        self.fully_linked_ancestors = false;
        self
    }

    // Add random weak links to the current layer round using a seed, if provided
    pub fn random_weak_links(mut self, seed: Option<u64>) -> Self {
        self.random_weak_links = true;
        self.random_weak_links_random_seed = seed;
        self
    }

    // Should be called when building a leader round. Will ensure leader block is
    // missing. A list of specified leader offsets can be provided to skip those
    // leaders. If none are provided all potential leaders for the round will be
    // skipped.
    pub fn no_leader_block(mut self, specified_leader_offsets: Vec<u32>) -> Self {
        self.no_leader_block = true;
        self.specified_leader_block_offsets = Some(specified_leader_offsets);
        self
    }

    // Should be called when building a voting round. Will ensure vote is missing.
    // A list of specified leader offsets can be provided to skip those leader
    // links. If none are provided all potential leaders for the round will be
    // skipped. note: configuration is terminal and layer will be built after
    // this call.
    pub fn no_leader_link(
        mut self,
        leader_round: Round,
        specified_leader_offsets: Vec<u32>,
    ) -> Self {
        self.no_leader_link = true;
        self.specified_leader_link_offsets = Some(specified_leader_offsets);
        self.leader_round = Some(leader_round);
        self.fully_linked_ancestors = false;
        self.build()
    }

    pub fn authorities(mut self, authorities: Vec<AuthorityIndex>) -> Self {
        assert!(
            self.specified_authorities.is_none(),
            "Specified authorities already set"
        );
        self.specified_authorities = Some(authorities);
        self
    }

    // Multiple blocks will be created for the specified authorities at the layer
    // round.
    pub fn equivocate(mut self, equivocations: usize) -> Self {
        // authorities must be specified for this to apply
        assert!(self.specified_authorities.is_some());
        self.equivocations = equivocations;
        self
    }

    // No blocks will be created for the specified authorities at the layer round.
    pub fn skip_block(mut self) -> Self {
        // authorities must be specified for this to apply
        assert!(self.specified_authorities.is_some());
        self.skip_block = true;
        self
    }

    // Skip specified acknowledgments for blocks from specified authorities
    pub fn skip_acknowledgements(mut self, acks_to_skip: Vec<AuthorityIndex>) -> Self {
        self.skip_acknowledgements = Some(acks_to_skip);
        self.fully_linked_acknowledgments = false;
        self
    }

    // Only acknowledge blocks from specified authorities
    pub fn only_acknowledge(mut self, only_acknowledge: Vec<AuthorityIndex>) -> Self {
        self.only_acknowledge = Some(only_acknowledge);
        self.fully_linked_acknowledgments = false;
        self
    }

    pub fn with_timestamps(mut self, timestamps: Vec<BlockTimestampMs>) -> Self {
        // authorities must be specified for this to apply
        assert!(self.specified_authorities.is_some());
        assert_eq!(
            self.specified_authorities.as_ref().unwrap().len(),
            timestamps.len(),
            "Timestamps should be provided for each specified authority"
        );
        self.timestamps = timestamps;
        self
    }

    // Apply the configurations & build the dag layer(s).
    pub fn build(mut self) -> Self {
        for round in self.start_round..=self.end_round.unwrap_or(self.start_round) {
            tracing::debug!("BUILDING LAYER ROUND {round}...");

            let authorities =
                if let Some(specified_authorities) = self.specified_authorities.clone() {
                    specified_authorities
                } else {
                    self.dag_builder
                        .context
                        .committee
                        .authorities()
                        .map(|x| x.0)
                        .collect()
                };

            // TODO: investigate if these configurations can be called in combination
            // for the same layer
            let mut connections = if self.fully_linked_ancestors {
                self.configure_fully_linked_ancestors()
            } else if self.min_ancestor_links {
                self.configure_min_parent_links()
            } else if self.no_leader_link {
                self.configure_no_leader_links(&authorities, round)
            } else if let Some(ancestors_to_skip) = self.skip_ancestor_links.clone() {
                self.configure_skipped_ancestor_links(&authorities, ancestors_to_skip)
            } else {
                vec![]
            };

            // Do not acknowledge transactions in round 0 (genesis).
            let acknowledgments = if round <= 1 {
                HashMap::new()
            } else if self.fully_linked_acknowledgments {
                self.configure_fully_linked_acknowledgments()
            } else if let Some(acks_to_skip) = self.skip_acknowledgements.clone() {
                self.configure_skipped_acknowledgements(authorities, acks_to_skip)
            } else if let Some(only_acknowledge) = self.only_acknowledge.clone() {
                self.configure_only_acknowledge(authorities, only_acknowledge)
            } else {
                HashMap::new()
            };

            if self.random_weak_links {
                connections.append(&mut self.configure_random_weak_links());
            }
            // reorder ancestors such that the own previous block is referenced first
            self.reorder_ancestors(&mut connections);

            self.create_blocks(round, connections, acknowledgments);
        }

        self.dag_builder.last_ancestors = self.ancestors.clone();
        self
    }

    pub fn persist_layers(&self, dag_state: Arc<RwLock<DagState>>) {
        assert!(
            !self.block_headers.is_empty(),
            "Called to persist layers although no blocks have been created. Make sure you have called build before."
        );
        let mut dag_state = dag_state.write();
        dag_state.accept_block_headers(self.block_headers.clone(), DataSource::Test);
        for transactions in self.transactions.clone() {
            dag_state.add_transactions(transactions, DataSource::Test);
        }
    }

    pub fn configure_timestamp_delay_ms(mut self, timestamp_delay_ms: u64) -> Self {
        self.timestamp_delay_ms = Some(timestamp_delay_ms);
        self
    }

    // Layer round is minimally and randomly connected with ancestors.
    pub fn configure_min_parent_links(&mut self) -> Vec<(AuthorityIndex, Vec<BlockRef>)> {
        let quorum_threshold = self.dag_builder.context.committee.quorum_threshold() as usize;
        let mut authorities: Vec<AuthorityIndex> = self
            .dag_builder
            .context
            .committee
            .authorities()
            .map(|authority| authority.0)
            .collect();

        let mut rng = match self.min_ancestor_links_random_seed {
            Some(s) => StdRng::seed_from_u64(s),
            None => StdRng::from_entropy(),
        };

        let mut authorities_to_shuffle = authorities.clone();

        let mut leaders = vec![];
        if let Some(leader_round) = self.leader_round {
            let leader_offsets = (0..self.dag_builder.number_of_leaders).collect::<Vec<_>>();

            for leader_offset in leader_offsets {
                leaders.push(
                    self.dag_builder
                        .leader_schedule
                        .elect_leader(leader_round, leader_offset),
                );
            }
        }

        authorities
            .iter()
            .map(|authority| {
                authorities_to_shuffle.shuffle(&mut rng);

                // TODO: handle quorum threshold properly with stake
                let min_ancestors: HashSet<AuthorityIndex> = authorities_to_shuffle
                    .iter()
                    .take(quorum_threshold)
                    .cloned()
                    .collect();

                (
                    *authority,
                    self.ancestors
                        .iter()
                        .filter(|a| {
                            leaders.contains(&a.author) || min_ancestors.contains(&a.author)
                        })
                        .cloned()
                        .collect::<Vec<BlockRef>>(),
                )
            })
            .collect()
    }

    // TODO: configure layer round randomly connected with weak links.
    fn configure_random_weak_links(&mut self) -> Vec<(AuthorityIndex, Vec<BlockRef>)> {
        unimplemented!("configure_random_weak_links");
    }

    // Layer round misses link to leader, but other blocks are fully connected with
    // ancestors.
    fn configure_no_leader_links(
        &mut self,
        authorities: &[AuthorityIndex],
        round: Round,
    ) -> Vec<(AuthorityIndex, Vec<BlockRef>)> {
        let mut missing_leaders = Vec::new();
        let mut specified_leader_offsets = self
            .specified_leader_link_offsets
            .clone()
            .expect("specified_leader_offsets should be set");
        let leader_round = self.leader_round.expect("leader round should be set");

        // When no specified leader offsets are available, all leaders are
        // expected to be missing.
        if specified_leader_offsets.is_empty() {
            specified_leader_offsets.extend(0..self.dag_builder.number_of_leaders);
        }

        for leader_offset in specified_leader_offsets {
            missing_leaders.push(
                self.dag_builder
                    .leader_schedule
                    .elect_leader(leader_round, leader_offset),
            );
        }

        self.configure_skipped_ancestor_links(authorities, missing_leaders)
    }

    fn configure_fully_linked_acknowledgments(&mut self) -> HashMap<AuthorityIndex, Vec<BlockRef>> {
        self.dag_builder
            .context
            .committee
            .authorities()
            .map(|authority| (authority.0, self.ancestors.clone()))
            .collect()
    }

    fn configure_skipped_acknowledgements(
        &mut self,
        authorities: Vec<AuthorityIndex>,
        acks_to_skip: Vec<AuthorityIndex>,
    ) -> HashMap<AuthorityIndex, Vec<BlockRef>> {
        let filtered_acks = self
            .ancestors
            .clone()
            .into_iter()
            .filter(|ancestor| !acks_to_skip.contains(&ancestor.author))
            .collect::<Vec<_>>();
        authorities
            .into_iter()
            .map(|authority| (authority, filtered_acks.clone()))
            .collect()
    }

    fn configure_only_acknowledge(
        &mut self,
        authorities: Vec<AuthorityIndex>,
        only_acknowledge: Vec<AuthorityIndex>,
    ) -> HashMap<AuthorityIndex, Vec<BlockRef>> {
        let filtered_acks = self
            .ancestors
            .clone()
            .into_iter()
            .filter(|ancestor| only_acknowledge.contains(&ancestor.author))
            .collect::<Vec<_>>();
        authorities
            .into_iter()
            .map(|authority| (authority, filtered_acks.clone()))
            .collect()
    }

    fn configure_fully_linked_ancestors(&mut self) -> Vec<(AuthorityIndex, Vec<BlockRef>)> {
        self.dag_builder
            .context
            .committee
            .authorities()
            .map(|authority| (authority.0, self.ancestors.clone()))
            .collect::<Vec<_>>()
    }

    fn configure_skipped_ancestor_links(
        &mut self,
        authorities: &[AuthorityIndex],
        ancestors_to_skip: Vec<AuthorityIndex>,
    ) -> Vec<(AuthorityIndex, Vec<BlockRef>)> {
        let filtered_ancestors = self
            .ancestors
            .clone()
            .into_iter()
            .filter(|ancestor| !ancestors_to_skip.contains(&ancestor.author))
            .collect::<Vec<_>>();
        authorities
            .iter()
            .map(|authority| (*authority, filtered_ancestors.clone()))
            .collect::<Vec<_>>()
    }

    // Creates the blocks for the new layer based on configured connections, also
    // sets the ancestors and transaction acks for future layers to be linked to
    fn create_blocks(
        &mut self,
        round: Round,
        connections: Vec<(AuthorityIndex, Vec<BlockRef>)>,
        transaction_acknowledgments: HashMap<AuthorityIndex, Vec<BlockRef>>,
    ) {
        let mut references = Vec::new();
        let mut rng = StdRng::from_entropy();

        for (authority, ancestors) in connections {
            if self.should_skip_block(round, authority) {
                continue;
            };
            let num_blocks = self.num_blocks_to_create(authority);

            for num_block in 0..num_blocks {
                let timestamp = self.block_timestamp(authority, round, num_block);

                // Create random transactions and their commitment.
                let mut tx_bytes = [0u8; 32];
                rng.fill(&mut tx_bytes[..]);
                let transactions = vec![Transaction::new(tx_bytes.to_vec())];
                let serialized_transactions = Transaction::serialize(&transactions).unwrap();
                let commitment = TransactionsCommitment::compute_transactions_commitment(
                    &serialized_transactions,
                    &self.dag_builder.context,
                    &mut self.dag_builder.encoder,
                )
                .unwrap();

                let test_block_header = TestBlockHeader::new(round, authority.value() as u8)
                    .set_ancestors(ancestors.clone())
                    .set_acknowledgments(
                        transaction_acknowledgments
                            .get(&authority)
                            .cloned()
                            .unwrap_or_default(),
                    )
                    .set_timestamp_ms(timestamp)
                    .set_commitment(commitment)
                    .build();
                let block_header =
                    if let Some(protocol_keypair) = self.dag_builder.protocol_keypair.as_ref() {
                        VerifiedBlockHeader::new_from_header_with_signature(
                            test_block_header,
                            &protocol_keypair[authority.value()],
                        )
                    } else {
                        VerifiedBlockHeader::new_for_test(test_block_header)
                    };

                let verified_transactions = VerifiedTransactions::new(
                    transactions,
                    block_header.transaction_ref(),
                    Some(block_header.digest()),
                    serialized_transactions,
                );

                references.push(block_header.reference());
                self.dag_builder
                    .block_headers
                    .insert(block_header.reference(), block_header.clone());
                self.dag_builder
                    .transactions
                    .insert(block_header.reference(), verified_transactions.clone());
                self.block_headers.push(block_header.clone());
                self.transactions.push(verified_transactions);
            }
        }
        self.ancestors = references;
    }

    fn num_blocks_to_create(&self, authority: AuthorityIndex) -> u32 {
        if self.specified_authorities.is_some()
            && self
                .specified_authorities
                .clone()
                .unwrap()
                .contains(&authority)
        {
            // Always create 1 block and then the equivocating blocks on top of that.
            1 + self.equivocations as u32
        } else {
            1
        }
    }

    fn block_timestamp(
        &self,
        authority: AuthorityIndex,
        round: Round,
        num_block: u32,
    ) -> BlockTimestampMs {
        if self.specified_authorities.is_some() && !self.timestamps.is_empty() {
            let specified_authorities = self.specified_authorities.as_ref().unwrap();
            if let Some(position) = specified_authorities.iter().position(|&x| x == authority) {
                return self.timestamps[position]
                    + (round + num_block) as u64
                    + self.timestamp_delay_ms.unwrap_or_default();
            }
        }
        let author = authority.value() as u32;
        let base_ts = round as BlockTimestampMs * 1000;
        base_ts + (author + round + num_block) as u64 + self.timestamp_delay_ms.unwrap_or_default()
    }

    fn should_skip_block(&self, round: Round, authority: AuthorityIndex) -> bool {
        // Safe to unwrap as specified authorities has to be set before skip
        // is specified.
        if self.skip_block
            && self
                .specified_authorities
                .clone()
                .unwrap()
                .contains(&authority)
        {
            return true;
        }
        if self.no_leader_block {
            let mut specified_leader_offsets = self
                .specified_leader_block_offsets
                .clone()
                .expect("specified_leader_block_offsets should be set");

            // When no specified leader offsets are available, all leaders are
            // expected to be skipped.
            if specified_leader_offsets.is_empty() {
                specified_leader_offsets.extend(0..self.dag_builder.number_of_leaders);
            }

            for leader_offset in specified_leader_offsets {
                let leader = self
                    .dag_builder
                    .leader_schedule
                    .elect_leader(round, leader_offset);

                if leader == authority {
                    return true;
                }
            }
        }
        false
    }

    // reorder ancestors in connections such that the reference to own block is
    // first
    fn reorder_ancestors(&self, connections: &mut [(AuthorityIndex, Vec<BlockRef>)]) {
        for (author, ancestors) in connections.iter_mut() {
            if let Some(pos) = ancestors.iter().position(|b| b.author == *author) {
                let own_block_ref = ancestors.remove(pos);
                ancestors.insert(0, own_block_ref);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    #[tokio::test]
    async fn test_fully_linked_acknowledgments() {
        let context = Arc::new(Context::new_for_test(4).0);
        let mut dag_builder = DagBuilder::new(context);
        dag_builder.layer(1).build(); // Round 1 is fully connected with parents by default.
        dag_builder.layers(2..=10).build();
        for (block_ref, block_header) in dag_builder.block_headers {
            if block_ref.round <= 1 {
                assert!(block_header.acknowledgments().is_empty())
            } else {
                assert!(block_header.acknowledgments().len() == 4)
            }
        }
    }

    #[tokio::test]
    async fn test_skip_acknowledgments() {
        let context = Arc::new(Context::new_for_test(4).0);
        let mut dag_builder = DagBuilder::new(context);
        let authorities_to_skip = vec![1.into(), 2.into()];
        dag_builder.layers(1..=5).build();
        // Round 6 and above should skip acknowledgments from authorities to skip
        dag_builder
            .layers(6..=10)
            .skip_acknowledgements(authorities_to_skip.clone())
            .build();
        for (block_ref, block_header) in dag_builder.block_headers {
            if block_ref.round <= 1 {
                assert!(block_header.acknowledgments().is_empty());
            } else if block_ref.round <= 5 {
                assert_eq!(block_header.acknowledgments().len(), 4);
            } else {
                // Round 6 and above should not have acknowledgments from authorities to skip
                assert_eq!(block_header.acknowledgments().len(), 2);
                // Check that acknowledgments from authorities to skip are not present
                for ack in block_header.acknowledgments() {
                    assert!(!authorities_to_skip.contains(&ack.author));
                }
            }
        }
    }

    #[tokio::test]
    async fn test_only_acknowledge() {
        let context = Arc::new(Context::new_for_test(4).0);
        let mut dag_builder = DagBuilder::new(context);
        let only_acknowledge = vec![1.into(), 2.into()];
        dag_builder.layers(1..=5).build();
        // Round 6 and above should only acknowledge blocks from authorities to
        // acknowledge
        dag_builder
            .layers(6..=10)
            .only_acknowledge(only_acknowledge.clone())
            .build();
        for (block_ref, block_header) in dag_builder.block_headers {
            if block_ref.round <= 1 {
                assert!(block_header.acknowledgments().is_empty());
            } else if block_ref.round <= 5 {
                assert_eq!(block_header.acknowledgments().len(), 4);
            } else {
                // Round 6 and above should only have acknowledgments from authorities to
                // acknowledge
                assert_eq!(block_header.acknowledgments().len(), 2);
                // Check that acknowledgments are only from authorities to acknowledge
                for ack in block_header.acknowledgments() {
                    assert!(only_acknowledge.contains(&ack.author));
                }
            }
        }
    }
}
