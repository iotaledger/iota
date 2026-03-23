// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
use std::{
    collections::{BTreeMap, BTreeSet, btree_map::Entry},
    sync::Arc,
    time::Instant,
};

use itertools::Itertools;
use starfish_config::AuthorityIndex;
use tracing::debug;

use crate::{BlockHeaderAPI, BlockRef, VerifiedBlockHeader, context::Context};

struct SuspendedBlockHeader {
    block_header: VerifiedBlockHeader,
    missing_ancestors: BTreeSet<BlockRef>,
    timestamp: Instant,
}

impl SuspendedBlockHeader {
    fn new(block: VerifiedBlockHeader, missing_ancestors: &BTreeSet<BlockRef>) -> Self {
        let mut ma = BTreeSet::new();
        for ancestor in missing_ancestors {
            ma.insert(*ancestor);
        }
        Self {
            block_header: block,
            missing_ancestors: ma,
            timestamp: Instant::now(),
        }
    }
}

pub(crate) struct BlockSuspender {
    context: Arc<Context>,
    /// Keeps all the suspended block headers. A suspended block header is a
    /// header that is missing part of its causal history and thus can't be
    /// immediately processed. A block header will remain in this map until
    /// all its causal history has been successfully processed.
    suspended_headers: BTreeMap<BlockRef, SuspendedBlockHeader>,
    /// A map that keeps all the blocks that we are missing (keys) and the
    /// corresponding blocks that reference the missing blocks as ancestors
    /// and need them to get unsuspended. It is possible for a missing
    /// dependency (key) to be a suspended block, so the block has been
    /// already fetched but itself is still missing some of its ancestors to be
    /// processed.
    missing_ancestors: BTreeMap<BlockRef, BTreeSet<BlockRef>>,
    /// A map of blocks that need to be fetched to the set of authorities
    /// expected to have them available locally. This set is approximated
    /// based on the block's author and the authors of its direct children.
    /// A block is considered missing if it appears in `missing_ancestors`
    /// and has not yet been fetched. Blocks already stored or present in
    /// `suspended_headers` are excluded.
    headers_to_fetch: BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>,
}

impl BlockSuspender {
    pub(crate) fn new(context: Arc<Context>) -> Self {
        Self {
            context,
            suspended_headers: BTreeMap::new(),
            missing_ancestors: BTreeMap::new(),
            headers_to_fetch: BTreeMap::new(),
        }
    }

    /// Reinitialize BlockSuspender after fast sync completes.
    /// Clears all suspended blocks, missing ancestors, and fetch queues.
    pub(crate) fn reinitialize(&mut self) {
        self.suspended_headers.clear();
        self.missing_ancestors.clear();
        self.headers_to_fetch.clear();
    }

    /// Accept or suspend a batch of received block headers based on their
    /// missing ancestors.
    ///
    /// Each header is either:
    /// - Accepted immediately if all its ancestors are resolved (directly or
    ///   through this batch),
    /// - Or suspended if it still has missing or unresolved ancestors.
    ///
    /// After initially processing the batch, this function also recursively
    /// unsuspends any other blocks that were previously suspended and are
    /// now unblocked due to newly accepted ancestors.
    ///
    /// # Arguments
    /// * `missing_ancestors_map` - A map of verified block headers to the set
    ///   of their unresolved ancestor references as read from the DagState by
    ///   the caller.
    ///
    /// # Returns
    /// * `Vec<VerifiedBlockHeader>` — All headers that were accepted, either
    ///   directly or via recursive unsuspension.
    /// * `BTreeSet<BlockRef>` — Set of ancestor block references that are still
    ///   missing and should be fetched.
    ///
    /// # Side Effects
    /// - Updates the suspension state of blocks.
    /// - May recursively unsuspend dependent blocks.
    /// - Updates internal fetch statistics.
    pub(crate) fn accept_or_suspend_received_headers(
        &mut self,
        missing_ancestors_map: BTreeMap<VerifiedBlockHeader, BTreeSet<BlockRef>>,
    ) -> (Vec<VerifiedBlockHeader>, BTreeSet<BlockRef>) {
        // Decide which headers are resolved now, which need to be suspended, and which
        // ancestors must still be fetched
        let (fully_resolved_headers, ancestors_to_fetch) =
            self.resolve_or_suspend_headers(missing_ancestors_map);
        let mut accepted_headers = vec![];

        for fully_resolved_header in fully_resolved_headers {
            let fully_resolved_header_reference = fully_resolved_header.reference();
            // Accept the resolved header
            accepted_headers.push(fully_resolved_header);
            // Recursively attempt to unsuspend any blocks that were waiting on this one
            let unsuspended =
                self.recursively_unsuspend_dependents(fully_resolved_header_reference);
            // Add them to the list of accepted headers
            accepted_headers.extend(unsuspended);
        }
        self.update_stats(ancestors_to_fetch.len() as u64);
        (accepted_headers, ancestors_to_fetch)
    }
    /// Processes a batch of verified block headers and updates their suspension
    /// state based on missing ancestors.
    ///
    /// This function evaluates each incoming block to determine whether it can
    /// be accepted immediately (i.e., all its ancestors are resolved), or
    /// if it must be suspended due to unresolved or missing ancestors.
    /// Suspended blocks are tracked, and any additional ancestor blocks that
    /// need to be fetched are identified.
    ///
    /// # Arguments
    /// * `missing_ancestors_map` - A map from the incoming verified block
    ///   headers to the set of their missing ancestor references as read from
    ///   the DagState by the calling code in BlockManager.
    ///
    /// # Returns
    /// A tuple containing:
    /// * `Vec<VerifiedBlockHeader>` - List of block headers that are fully
    ///   resolved and ready for further processing to check if their children
    ///   can be unsuspended.
    /// * `BTreeSet<BlockRef>` - Set of ancestor block references that still
    ///   need to be fetched.
    ///
    /// # Side Effects
    /// This function:
    /// - Removes incoming batch block references from `self.missing_blocks`.
    /// - Registers suspended blocks and their missing ancestors in internal
    ///   tracking structures.
    /// - Triggers reporting mechanisms for suspended blocks and missing
    ///   ancestors.
    /// - Identifies which ancestor blocks must still be fetched externally.
    fn resolve_or_suspend_headers(
        &mut self,
        missing_ancestors_map: BTreeMap<VerifiedBlockHeader, BTreeSet<BlockRef>>,
    ) -> (Vec<VerifiedBlockHeader>, BTreeSet<BlockRef>) {
        // Collect references of all incoming blocks in this batch
        let incoming_block_refs: BTreeSet<BlockRef> = missing_ancestors_map
            .keys()
            .map(|b| b.reference())
            .collect();
        debug!(
            "Trying to accept block headers: {}",
            incoming_block_refs.iter().map(|b| b.to_string()).join(",")
        );
        let mut fully_resolved_headers: Vec<VerifiedBlockHeader> = vec![];
        let mut ancestors_to_fetch = BTreeSet::new();
        for (incoming_block_header, missing_ancestors) in missing_ancestors_map {
            let block_ref = incoming_block_header.reference();
            let block_header_author = incoming_block_header.author();

            // We're now processing this block, so remove it from the missing_blocks list
            self.headers_to_fetch.remove(&block_ref);

            // If there are no missing ancestors, we can mark the block header as fully
            // resolved
            if missing_ancestors.is_empty() {
                fully_resolved_headers.push(incoming_block_header);
                continue;
            }

            // Otherwise, suspend the block and track its missing ancestors
            self.register_suspended_block(incoming_block_header, &missing_ancestors);
            self.report_suspended_block(&block_ref);
            for ancestor in missing_ancestors {
                self.register_missing_ancestor(block_ref, ancestor);
                self.report_missing_ancestor(&ancestor);

                // If this ancestor is not already suspended and is not part of the incoming
                // batch, we may need to fetch it
                if !self.suspended_headers.contains_key(&ancestor)
                    && !incoming_block_refs.contains(&ancestor)
                {
                    ancestors_to_fetch.insert(ancestor);
                    // Only report the block as missing if we’re not already fetching it
                    if !self.headers_to_fetch.contains_key(&ancestor) {
                        self.report_block_to_fetch(ancestor.author);
                    }
                    self.register_block_to_fetch(&ancestor, block_header_author);
                }
            }
        }
        (fully_resolved_headers, ancestors_to_fetch)
    }
    /// Recursively unsuspends all blocks that were dependent on a now-accepted
    /// block.
    ///
    /// Starting from `resolved_block`, this function walks the dependency graph
    /// and attempts to unsuspend any suspended blocks that were blocked on
    /// it. Successfully unsuspended blocks are added to the result, and
    /// their own dependents are checked in turn.
    ///
    /// # Arguments
    /// * `resolved_block_ref` - The block that was just accepted and may
    ///   unsuspend other blocks.
    ///
    /// # Returns
    /// * A list of verified block headers that have been unsuspended.
    fn recursively_unsuspend_dependents(
        &mut self,
        resolved_block_ref: BlockRef,
    ) -> Vec<VerifiedBlockHeader> {
        let mut ready_blocks = vec![];
        let mut stack = vec![resolved_block_ref];

        while let Some(popped_resolved_block_ref) = stack.pop() {
            // And try to check if its direct children can be unsuspended
            if let Some(suspended_block_refs_with_missing_deps) =
                self.missing_ancestors.remove(&popped_resolved_block_ref)
            {
                for suspended_block_ref in suspended_block_refs_with_missing_deps {
                    // For each dependency try to unsuspend it. If that's successful then we add it
                    // to the stack so we can recursively try to unsuspend its
                    // children.
                    if let Some(unsuspended_block) =
                        self.unsuspend_if_ready(&suspended_block_ref, &popped_resolved_block_ref)
                    {
                        self.report_unsuspended_block(&unsuspended_block);
                        stack.push(unsuspended_block.block_header.reference());
                        ready_blocks.push(unsuspended_block.block_header);
                    }
                }
            }
        }
        ready_blocks
    }
    /// Attempts to unsuspend a block if one of its missing dependencies has
    /// just been accepted.
    ///
    /// This function is called when a block that was previously suspended (due
    /// to missing ancestors) may now be unblocked because one of its
    /// dependencies (`resolved_dependency`) has been resolved.
    /// The dependency is removed from the suspended block’s set of missing
    /// ancestors. If this was the final missing dependency, the block is
    /// removed from the suspension map and returned.
    ///
    /// # Arguments
    /// * `suspended_block_ref` - Reference to the suspended block being
    ///   re-evaluated.
    /// * `resolved_dependency` - A block reference that was just accepted and
    ///   may resolve a dependency.
    ///
    /// # Returns
    /// * `Some(SuspendedBlockHeader)` if the block no longer has missing
    ///   ancestors and is ready to be reprocessed.
    /// * `None` if the block still has unresolved ancestors.
    ///
    /// # Panics
    /// - If the block is not found in the `suspended_block_headers` map.
    /// - If `resolved_dependency` is not in the block's `missing_ancestors`
    ///   set.
    ///
    /// # Side Effects
    /// - Mutates internal state by removing resolved dependencies and possibly
    ///   removing the suspended block.
    fn unsuspend_if_ready(
        &mut self,
        suspended_block_ref: &BlockRef,
        resolved_dependency: &BlockRef,
    ) -> Option<SuspendedBlockHeader> {
        let block = self
            .suspended_headers
            .get_mut(suspended_block_ref)
            .expect("Block should be in suspended map");

        assert!(
            block.missing_ancestors.remove(resolved_dependency),
            "Block reference {} should be present in missing dependencies of {:?}",
            suspended_block_ref,
            block.block_header
        );
        // If there are no more missing ancestors, unsuspend and return the block
        if block.missing_ancestors.is_empty() {
            return self.suspended_headers.remove(suspended_block_ref);
        }
        // Otherwise, keep it suspended
        None
    }
    fn register_suspended_block(
        &mut self,
        block_header: VerifiedBlockHeader,
        missing_ancestors: &BTreeSet<BlockRef>,
    ) {
        let block_ref = block_header.reference();
        match self.suspended_headers.entry(block_ref) {
            Entry::Vacant(v) => {
                let suspended_block = SuspendedBlockHeader::new(block_header, missing_ancestors);
                v.insert(suspended_block);
            }
            Entry::Occupied(mut o) => {
                o.get_mut().missing_ancestors.extend(missing_ancestors);
            }
        };
    }
    fn register_missing_ancestor(&mut self, block_ref: BlockRef, ancestor: BlockRef) {
        self.missing_ancestors
            .entry(ancestor)
            .or_default()
            .insert(block_ref);
    }
    fn report_suspended_block(&self, block_ref: &BlockRef) {
        self.context
            .metrics
            .node_metrics
            .block_headers_suspensions
            .with_label_values(&[self.context.authority_hostname(block_ref.author)])
            .inc();
    }
    fn report_missing_ancestor(&mut self, ancestor: &BlockRef) {
        self.context
            .metrics
            .node_metrics
            .block_manager_missing_ancestors_by_authority
            .with_label_values(&[self.context.authority_hostname(ancestor.author)])
            .inc();
    }
    fn register_block_to_fetch(
        &mut self,
        missing_ancestor: &BlockRef,
        child_author: AuthorityIndex,
    ) {
        match self.headers_to_fetch.entry(*missing_ancestor) {
            Entry::Vacant(v) => {
                // Both the child block's author and the actual missing ancestor's author
                // are expected to have this block available.
                v.insert(BTreeSet::from([missing_ancestor.author, child_author]));
            }
            Entry::Occupied(mut o) => {
                o.get_mut().insert(child_author);
            }
        }
    }

    fn report_block_to_fetch(&mut self, block_to_fetch_author: AuthorityIndex) {
        self.context
            .metrics
            .node_metrics
            .block_manager_missing_block_headers_by_authority
            .with_label_values(&[self.context.authority_hostname(block_to_fetch_author)])
            .inc();
    }
    fn report_unsuspended_block(&self, unsuspended_block: &SuspendedBlockHeader) {
        let now = Instant::now();
        self.context
            .metrics
            .node_metrics
            .block_header_unsuspensions
            .with_label_values(&[self
                .context
                .authority_hostname(unsuspended_block.block_header.author())])
            .inc();
        self.context
            .metrics
            .node_metrics
            .suspended_block_header_time
            .with_label_values(&[self
                .context
                .authority_hostname(unsuspended_block.block_header.author())])
            .observe(
                now.saturating_duration_since(unsuspended_block.timestamp)
                    .as_secs_f64(),
            );
    }
    pub(crate) fn is_block_ref_suspended(&self, block_ref: &BlockRef) -> bool {
        self.suspended_headers.contains_key(block_ref)
    }
    pub(crate) fn headers_to_fetch(&self) -> BTreeMap<BlockRef, BTreeSet<AuthorityIndex>> {
        self.headers_to_fetch.clone()
    }
    fn update_stats(&mut self, blocks_to_fetch: u64) {
        let metrics = &self.context.metrics.node_metrics;
        metrics.missing_block_headers_total.inc_by(blocks_to_fetch);
        metrics
            .block_manager_suspended_block_headers
            .set(self.suspended_headers.len() as i64);
        metrics
            .block_manager_missing_ancestors
            .set(self.missing_ancestors.len() as i64);
        metrics
            .block_manager_missing_block_headers
            .set(self.headers_to_fetch.len() as i64);
    }
    #[cfg(test)]
    pub(crate) fn blocks_to_fetch_len(&self) -> usize {
        self.headers_to_fetch.len()
    }
    #[cfg(test)]
    pub(crate) fn insert_block_to_fetch(
        &mut self,
        block_ref: BlockRef,
        set: BTreeSet<AuthorityIndex>,
    ) -> Option<BTreeSet<AuthorityIndex>> {
        self.headers_to_fetch.insert(block_ref, set)
    }
    #[cfg(test)]
    pub(crate) fn set_missing_ancestors_with_no_children(&mut self, block_ref: BlockRef) {
        self.missing_ancestors.entry(block_ref).or_default();
    }
    #[cfg(test)]
    pub(crate) fn suspended_blocks_refs(&self) -> BTreeSet<BlockRef> {
        self.suspended_headers.keys().cloned().collect()
    }
    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.suspended_headers.is_empty()
            && self.missing_ancestors.is_empty()
            && self.headers_to_fetch.is_empty()
    }
    #[cfg(test)]
    pub(crate) fn blocks_to_fetch_refs(&self) -> BTreeSet<BlockRef> {
        self.headers_to_fetch.keys().cloned().collect()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    /// Evaluates a set of verified block headers to determine which blocks
    /// should be suspended and which are still missing.
    ///
    /// This function is called in randomized tests to verify the
    /// suspension logic of the `BlockSuspender`.
    /// returns:
    /// tuple of:
    /// - `Vec<BlockRef>`: all block references that should be suspended
    /// - `Vec<BlockRef>`: all block references that are still missing and need
    ///   to be fetched
    pub(crate) fn evaluate_block_headers(
        seen_so_far: &[VerifiedBlockHeader],
    ) -> (BTreeSet<BlockRef>, BTreeSet<BlockRef>) {
        let mut suspended = BTreeSet::new();
        let mut missing = BTreeSet::new();
        let mut seen_so_far_ordered = seen_so_far.iter().collect::<Vec<_>>();
        let seen_so_far_refs: BTreeSet<BlockRef> =
            seen_so_far.iter().map(|b| b.reference()).collect();
        seen_so_far_ordered.sort_by_key(|b| b.round());
        for block in seen_so_far_ordered {
            if block.round() == 1 {
                // Skip the first block as it has no ancestors.
                continue;
            }
            let block_ref = block.reference();
            for ancestor in block.ancestors() {
                let is_missing = !seen_so_far_refs.contains(ancestor);
                let is_suspended = suspended.contains(ancestor);

                if is_missing {
                    missing.insert(*ancestor);
                }

                if is_missing || is_suspended {
                    suspended.insert(block_ref);
                }
            }
        }
        (suspended, missing)
    }
}
