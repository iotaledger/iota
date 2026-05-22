// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{HashMap, VecDeque},
    path::PathBuf,
    sync::Arc,
};

use anyhow::{Error, Result};
use iota_core::test_utils::{make_pay_iota_transaction, make_transfer_iota_transaction};
use iota_types::{base_types::IotaAddress, crypto::AccountKeyPair};
use tracing::{info, warn};

use crate::{
    ValidatorProxy, bank_cache,
    util::UpdatedAndNewlyMintedGasCoins,
    workloads::{
        Gas, GasCoinConfig,
        payload::Payload,
        workload::{MAX_BUDGET, Workload, WorkloadBuilder},
    },
};

/// Bank is used for generating gas for running the benchmark.
#[derive(Clone)]
pub struct BenchmarkBank {
    /// First fullnode proxy — used by all non-parallel paths (init coin
    /// creation, cache verification, system state, etc.). Kept as the "primary"
    /// for backward-compatibility with callers reading `bank.proxy`.
    pub proxy: Arc<dyn ValidatorProxy + Send + Sync>,
    /// All available fullnode proxies. Parallel coin-generation branches fan
    /// out across these round-robin so a single fullnode doesn't get crushed
    /// by all the simultaneous `pay_iota` PTBs.
    pub proxies: Vec<Arc<dyn ValidatorProxy + Send + Sync>>,
    /// Optional separate proxy used ONLY for cache-verification reads.
    /// `LocalValidatorAggregatorProxy::get_owned_objects` returns Err
    /// (validators don't expose "list by owner") which makes every TD-mode
    /// run cache-miss. Plumbing a FullNodeProxy here lets cache verify
    /// succeed while everything else still goes through TD. None falls back
    /// to `self.proxy`.
    pub read_proxy: Option<Arc<dyn ValidatorProxy + Send + Sync>>,
    // Coin used for paying for gas & splitting into smaller gas coins
    pub primary_coin: Gas,
}

impl BenchmarkBank {
    pub fn new(proxy: Arc<dyn ValidatorProxy + Send + Sync>, primary_coin: Gas) -> Self {
        BenchmarkBank {
            proxy: proxy.clone(),
            proxies: vec![proxy],
            read_proxy: None,
            primary_coin,
        }
    }

    /// Construct a bank with multiple fullnode proxies. The first proxy is
    /// the canonical "primary" used for init-coin creation and cache reads.
    pub fn new_multi(
        proxies: Vec<Arc<dyn ValidatorProxy + Send + Sync>>,
        primary_coin: Gas,
    ) -> Self {
        assert!(
            !proxies.is_empty(),
            "BenchmarkBank requires at least one proxy"
        );
        BenchmarkBank {
            proxy: proxies[0].clone(),
            proxies,
            read_proxy: None,
            primary_coin,
        }
    }

    /// Set a fullnode proxy used only for cache verification (object-by-owner
    /// reads). See `read_proxy` field doc for why.
    pub fn with_read_proxy(mut self, read_proxy: Arc<dyn ValidatorProxy + Send + Sync>) -> Self {
        self.read_proxy = Some(read_proxy);
        self
    }

    /// Proxy used for all setup-time writes (auto-merge, create_init_coin,
    /// pay_iota chunks). Routes through the fullnode (`read_proxy`) when
    /// available so warmup admissions go through the fullnode's buffered
    /// path instead of slamming validators' load-shedding gate directly via
    /// TD. The spam phase still uses `self.proxy`/`self.proxies` (TD direct)
    /// — that's the whole point of the gate-race test.
    fn setup_proxy(&self) -> &Arc<dyn ValidatorProxy + Send + Sync> {
        self.read_proxy.as_ref().unwrap_or(&self.proxy)
    }
    /// Merge all of `primary_coin.1`'s owned gas coins into the largest one
    /// before any pay_iota work, so subsequent `create_init_coin` requests
    /// have access to the full balance — not just the largest single
    /// leftover from a prior partial run. Skips silently if `read_proxy` is
    /// None (we can't enumerate coins) or only one coin is owned.
    async fn merge_primary_owner_coins(&mut self, gas_price: u64) -> Result<()> {
        let Some(read_proxy) = self.read_proxy.clone() else {
            return Ok(());
        };
        let owner = self.primary_coin.1;
        let owned = match read_proxy.get_owned_objects(owner).await {
            Ok(v) => v,
            Err(e) => {
                warn!("auto-merge: failed to list owned objects for {owner}: {e}");
                return Ok(());
            }
        };
        // owned is Vec<(balance, Object)>. We only merge if there are >1
        // distinct gas coins.
        if owned.len() < 2 {
            return Ok(());
        }
        // Pick the largest by balance as the "target" (this becomes the new
        // primary_coin); the rest go in as `coins` to be smashed into gas.
        let mut sorted: Vec<_> = owned;
        sorted.sort_by_key(|(bal, _)| std::cmp::Reverse(*bal));
        let target_obj = sorted[0].1.compute_object_reference();
        let extras: Vec<iota_types::base_types::ObjectRef> = sorted
            .iter()
            .skip(1)
            .map(|(_, o)| o.compute_object_reference())
            .collect();
        let total: u128 = sorted.iter().map(|(b, _)| *b as u128).sum();
        info!(
            "auto-merge: consolidating {} owned coins ({:.3e} nIOTA total) for {owner}",
            sorted.len(),
            total as f64,
        );

        // Build a `pay_all_iota` PTB: all inputs (target + extras) get smashed
        // into the gas object via IOTA's gas-smashing, then TransferObjects
        // transfers the gas coin back to `owner`. Net: one merged coin.
        let tx_data = iota_types::transaction::TransactionData::new_pay_all_iota(
            owner, extras, owner, target_obj, MAX_BUDGET, gas_price,
        );
        let signed =
            iota_types::utils::to_sender_signed_transaction(tx_data, self.primary_coin.2.as_ref());
        let effects = self.setup_proxy().execute_transaction_block(signed).await?;
        if !effects.is_ok() {
            warn!(
                "auto-merge: tx failed with status {}; proceeding without merge",
                effects.status()
            );
            return Ok(());
        }
        // Update primary_coin to the post-merge ref.
        let mutated = effects.mutated();
        let new_ref = mutated
            .into_iter()
            .find(|(r, _)| r.object_id == target_obj.object_id)
            .ok_or_else(|| Error::msg("merged gas object not found in effects.mutated()"))?;
        self.primary_coin.0 = new_ref.0;
        info!(
            "auto-merge: primary_coin updated to {:?} v{}",
            self.primary_coin.0.object_id,
            self.primary_coin.0.version.as_u64()
        );
        Ok(())
    }

    pub async fn generate(
        &mut self,
        builders: Vec<Box<dyn WorkloadBuilder<dyn Payload>>>,
        gas_price: u64,
        chunk_size: u64,
        cache_path: Option<PathBuf>,
        config_hash: Option<String>,
    ) -> Result<Vec<Box<dyn Workload<dyn Payload>>>> {
        // Auto-merge any leftover sub-coins back into the primary coin so
        // repeated stress runs without a network reset don't fail with
        // "InsufficientCoinBalance" (each run peels ~half the primary's
        // largest coin into a sub-init; after a few runs the largest is too
        // small for create_init_coin even though total balance is plenty).
        self.merge_primary_owner_coins(gas_price).await?;

        // --- Cache fast-path ---
        // If a cache file exists and its config_hash matches AND every
        // cached coin is still owned on-chain, skip the (slow) coin
        // generation entirely.
        if let (Some(path), Some(want_hash)) = (cache_path.as_ref(), config_hash.as_ref()) {
            if path.exists() {
                match bank_cache::load(path) {
                    Ok(pool) if pool.config_hash == *want_hash => {
                        info!(
                            "Gas pool cache hit at {}: verifying {} workloads still own their coins...",
                            path.display(),
                            pool.workloads.len()
                        );
                        let verify_proxy = self.read_proxy.as_ref().unwrap_or(&self.proxy);
                        if bank_cache::verify_all_exist(verify_proxy, &pool).await {
                            let per_workload = bank_cache::restore(&pool)?;
                            if per_workload.len() != builders.len() {
                                warn!(
                                    "cache has {} workloads but {} builders; ignoring",
                                    per_workload.len(),
                                    builders.len()
                                );
                            } else {
                                info!("Gas pool cache verified — skipping pay_iota.");
                                let mut workloads = vec![];
                                for (builder, (init, payload)) in
                                    builders.iter().zip(per_workload.into_iter())
                                {
                                    workloads.push(builder.build(init, payload).await);
                                }
                                return Ok(workloads);
                            }
                        } else {
                            warn!("cache verification failed (some coins missing) — regenerating");
                        }
                    }
                    Ok(_) => warn!("cache config_hash mismatch — regenerating"),
                    Err(e) => warn!("failed to load cache from {}: {e}", path.display()),
                }
            }
        }

        // --- Normal generation path ---
        let mut coin_configs = VecDeque::new();
        for builder in builders.iter() {
            let init_gas_config = builder.generate_coin_config_for_init().await;
            let payload_gas_config = builder.generate_coin_config_for_payloads().await;
            coin_configs.push_back(init_gas_config);
            coin_configs.push_back(payload_gas_config);
        }
        let mut all_coin_configs = vec![];
        coin_configs
            .iter()
            .for_each(|v| all_coin_configs.extend(v.clone()));

        let mut new_gas_coins: Vec<Gas> = vec![];
        // Protocol limit: max_arguments per programmable-tx command is 512,
        // enforced as `args.len() < 512` (strict less-than) at
        // crates/iota-types/src/transaction.rs:571. pay_iota emits one
        // SplitCoins command whose `args` Vec has length = chunk_size, so the
        // largest chunk_size that passes the check is 511. Clamp here so
        // callers (CLI --gas-request-chunk-size, env, scripts) can't blow the
        // limit.
        const MAX_PAY_IOTA_OUTPUTS: u64 = 511;
        let effective_chunk_size = chunk_size.min(MAX_PAY_IOTA_OUTPUTS);
        if effective_chunk_size != chunk_size {
            warn!(
                "Clamping gas-request-chunk-size from {} to {} (protocol limit: args.len() < 512 per PTB command)",
                chunk_size, effective_chunk_size,
            );
        }
        let chunked_coin_configs = all_coin_configs.chunks(effective_chunk_size as usize);

        // Split off the initlization coin for this workload, to reduce contention
        // of main gas coin used by other instances of this tool.
        let total_gas_needed: u64 = all_coin_configs.iter().map(|c| c.amount).sum();
        let chunks: Vec<Vec<GasCoinConfig>> = chunked_coin_configs.map(|c| c.to_vec()).collect();
        let num_chunks = chunks.len();
        info!("Number of gas requests = {}", num_chunks);

        // Parallel gas creation: split init_coin into K sub-init coins (each
        // owned by primary_gas_owner), then run K parallel branches, each
        // doing its share of pay_iota chunks. ~K× speedup over strictly
        // sequential.
        //
        // Default K=2 to avoid tripping the validator load-shedding gate
        // during warmup. With M concurrent stress.rs subprocesses each firing
        // K branches, total in-flight pay_iota PTBs is M×K — but the gate
        // (sem_cap = max_pending_transactions / 50) is typically ~20 per
        // validator. With M=8 stress procs, K=2 → 16 in-flight (under cap);
        // K=8 → 64 in-flight (3× over cap, causes rejections + retry storms
        // that contaminate the actual-run metrics). Override with
        // GAS_PARALLEL_BRANCHES env var when running fewer procs or with a
        // higher max_pending_transactions.
        let parallel_branches = std::env::var("GAS_PARALLEL_BRANCHES")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .filter(|n| *n > 0)
            .unwrap_or(2)
            .min(num_chunks.max(1))
            .max(1);

        // Compute per-branch sub-coin funding up front so we can size
        // init_coin correctly. We MUST size all K sub-coins uniformly: the
        // address-map lookup in pay_iota matches `effects.created()` to
        // configs by address only, and all K sub-coins share the primary
        // owner's address, so the returned ordering is unspecified. If we
        // sized per branch and the validator returned them out of order, a
        // branch needing N chunks could end up with a sub-coin sized for N-1
        // and run out of money mid-stream.
        let chunks_per_branch = if parallel_branches > 1 {
            1 + (num_chunks - 1) / parallel_branches
        } else {
            num_chunks
        };
        let max_chunk_outputs: u64 = chunks
            .iter()
            .map(|chunk| chunk.iter().map(|c| c.amount).sum::<u64>())
            .max()
            .unwrap_or(0);
        let per_branch_funding = max_chunk_outputs
            .saturating_mul(chunks_per_branch as u64)
            .saturating_add(MAX_BUDGET * (chunks_per_branch as u64 + 1));

        // init_coin must fund K sub-coins (in parallel mode) or all chunks
        // directly (in sequential mode), plus the splitting tx's own gas
        // (≤ MAX_BUDGET) with 1 MAX_BUDGET safety.
        let init_coin_amount = if parallel_branches > 1 {
            per_branch_funding
                .saturating_mul(parallel_branches as u64)
                .saturating_add(MAX_BUDGET * 2)
        } else {
            total_gas_needed.saturating_add(MAX_BUDGET * (num_chunks as u64 + 1))
        };
        let mut init_coin = self.create_init_coin(init_coin_amount, gas_price).await?;

        if parallel_branches > 1 {
            // Step 1: split init_coin into K sub-init coins via one pay_iota.
            // All K sub-coins use the same uniform `per_branch_funding`
            // (computed above) so they're interchangeable — see the long
            // comment near `per_branch_funding`'s definition for why.
            let primary_addr = self.primary_coin.1;
            let primary_kp = self.primary_coin.2.clone();
            let sub_init_configs: Vec<GasCoinConfig> = (0..parallel_branches)
                .map(|_| GasCoinConfig {
                    amount: per_branch_funding,
                    address: primary_addr,
                    keypair: primary_kp.clone(),
                })
                .collect();
            info!(
                "Parallel gas setup: splitting init coin into {} sub-init branches \
                 (~{} chunks each)",
                parallel_branches, chunks_per_branch,
            );
            // Step 1 split runs on the primary proxy (single tx).
            let primary_proxy = self.setup_proxy().clone();
            let sub_inits = self
                .pay_iota(&sub_init_configs, &mut init_coin, gas_price, &primary_proxy)
                .await?;

            // Step 2: each branch processes its own pre-partitioned chunks.
            // Warmup routes through the FullNodeProxy (setup_proxy) to avoid
            // hammering the validator load-shedding gate directly via TD —
            // the fullnode buffers and serializes admissions internally, so
            // sem_cap is honored without warmup-induced gate trips.
            let warmup_proxy = self.setup_proxy().clone();
            info!(
                "Warmup via {} branches (all routed through setup_proxy / fullnode)",
                parallel_branches,
            );
            let mut handles = vec![];
            for (sub_init, branch_chunks) in sub_inits
                .into_iter()
                .zip(chunks.chunks(chunks_per_branch).map(|c| c.to_vec()))
            {
                let bank_clone = self.clone();
                let branch_proxy = warmup_proxy.clone();
                handles.push(tokio::spawn(async move {
                    let mut local_init = sub_init;
                    let mut branch_new = Vec::new();
                    for chunk in branch_chunks {
                        let coins = bank_clone
                            .pay_iota(&chunk, &mut local_init, gas_price, &branch_proxy)
                            .await?;
                        branch_new.extend(coins);
                    }
                    Ok::<Vec<Gas>, anyhow::Error>(branch_new)
                }));
            }
            for handle in handles {
                let branch_coins = handle.await??;
                new_gas_coins.extend(branch_coins);
            }
        } else {
            // Single-branch (or only one chunk) — sequential, primary proxy.
            let primary_proxy = self.setup_proxy().clone();
            for chunk in chunks {
                let gas_coins = self
                    .pay_iota(&chunk, &mut init_coin, gas_price, &primary_proxy)
                    .await?;
                new_gas_coins.extend(gas_coins);
            }
        }
        let mut workloads = vec![];
        // Capture the per-workload init/payload split so we can save it
        // to disk after the build loop.
        let mut per_workload_gas: Vec<(Vec<Gas>, Vec<Gas>)> = vec![];
        // Bucket the freshly-minted coins by owner address so the per-builder
        // assignment is O(N) total instead of O(N²). The previous code did
        // `find_position` + `Vec::remove(index)` for every config — with
        // IFR=50 and ~750K configs that's ~5.6e11 shifts, several minutes of
        // pure CPU after consensus is already done. With a HashMap of
        // per-address VecDeques, each pop is O(1).
        let mut by_owner: HashMap<IotaAddress, std::collections::VecDeque<Gas>> = HashMap::new();
        for gas in new_gas_coins.drain(..) {
            by_owner.entry(gas.1).or_default().push_back(gas);
        }
        let take_for = |configs: &[GasCoinConfig],
                        by_owner: &mut HashMap<IotaAddress, std::collections::VecDeque<Gas>>|
         -> Vec<Gas> {
            configs
                .iter()
                .map(|c| {
                    by_owner
                        .get_mut(&c.address)
                        .and_then(|q| q.pop_front())
                        .expect("Owner address missing in the gas pool")
                })
                .collect()
        };
        for builder in builders.iter() {
            let init_gas_config = coin_configs.pop_front().unwrap();
            let payload_gas_config = coin_configs.pop_front().unwrap();
            let init_gas = take_for(&init_gas_config, &mut by_owner);
            let payload_gas = take_for(&payload_gas_config, &mut by_owner);
            per_workload_gas.push((init_gas.clone(), payload_gas.clone()));
            workloads.push(builder.build(init_gas, payload_gas).await);
        }

        // --- Save cache for next time ---
        if let (Some(path), Some(hash)) = (cache_path.as_ref(), config_hash.as_ref()) {
            let pool = bank_cache::build_from_workloads(
                hash.clone(),
                self.primary_coin.1,
                &per_workload_gas,
            );
            if let Err(e) = bank_cache::save(path, &pool) {
                warn!("failed to save gas pool cache to {}: {e}", path.display());
            }
        }

        Ok(workloads)
    }

    async fn pay_iota(
        &self,
        coin_configs: &[GasCoinConfig],
        init_coin: &mut Gas,
        gas_price: u64,
        proxy: &Arc<dyn ValidatorProxy + Send + Sync>,
    ) -> Result<UpdatedAndNewlyMintedGasCoins> {
        let recipient_addresses: Vec<IotaAddress> =
            coin_configs.iter().map(|g| g.address).collect();
        let amounts: Vec<u64> = coin_configs.iter().map(|c| c.amount).collect();

        info!(
            "Creating {} coin(s) of balance {}...",
            amounts.len(),
            amounts[0],
        );

        let tx = make_pay_iota_transaction(
            init_coin.0,
            vec![],
            recipient_addresses,
            amounts,
            init_coin.1,
            &init_coin.2,
            gas_price,
            MAX_BUDGET,
        );

        let effects = proxy.execute_transaction_block(tx).await?;

        if !effects.is_ok() {
            effects.print_gas_summary();
            panic!("Could not generate coins for workload...");
        }

        let updated_gas = effects
            .mutated()
            .into_iter()
            .find(|(k, _)| k.object_id == init_coin.0.object_id)
            .ok_or("Input gas missing in the effects")
            .map_err(Error::msg)?;

        init_coin.0 = updated_gas.0;
        init_coin.1 = *updated_gas
            .1
            .address_or_object()
            .ok_or_else(|| Error::msg("not an address or object owner"))?;
        init_coin.2 = self.primary_coin.2.clone();

        let address_map: HashMap<IotaAddress, Arc<AccountKeyPair>> = coin_configs
            .iter()
            .map(|c| (c.address, c.keypair.clone()))
            .collect();

        let transferred_coins: Result<Vec<Gas>> = effects
            .created()
            .into_iter()
            .map(|c| {
                let address =
                    *c.1.address_or_object()
                        .ok_or_else(|| Error::msg("not an address or object owner"))?;
                let keypair = address_map
                    .get(&address)
                    .ok_or("Owner address missing in the address map")
                    .map_err(Error::msg)?;
                Ok((c.0, address, keypair.clone()))
            })
            .collect();

        transferred_coins
    }

    async fn create_init_coin(&mut self, amount: u64, gas_price: u64) -> Result<Gas> {
        info!("Creating initialization coin of value {amount}...");

        let tx = make_transfer_iota_transaction(
            self.primary_coin.0,
            self.primary_coin.1,
            Some(amount),
            self.primary_coin.1,
            &self.primary_coin.2,
            gas_price,
        );

        let effects = self.setup_proxy().execute_transaction_block(tx).await?;

        if !effects.is_ok() {
            effects.print_gas_summary();
            panic!("Failed to create initialization coin for workload.");
        }

        let updated_gas = effects
            .mutated()
            .into_iter()
            .find(|(k, _)| k.object_id == self.primary_coin.0.object_id)
            .ok_or("Input gas missing in the effects")
            .map_err(Error::msg)?;

        self.primary_coin = (
            updated_gas.0,
            *updated_gas
                .1
                .address_or_object()
                .ok_or_else(|| Error::msg("not an address or object owner"))?,
            self.primary_coin.2.clone(),
        );

        match effects.created().first() {
            Some(created_coin) => Ok((
                created_coin.0,
                *created_coin
                    .1
                    .address_or_object()
                    .ok_or_else(|| Error::msg("not an address or object owner"))?,
                self.primary_coin.2.clone(),
            )),
            None => panic!("Failed to create initialization coin for workload."),
        }
    }
}
