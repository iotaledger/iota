// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, HashMap},
    ops::Deref,
    sync::Arc,
};

use futures::{StreamExt, stream::FuturesUnordered};
use iota_config::node::RunWithRange;
use iota_sdk_types::{
    Address, ObjectId, ObjectReference, OwnedObjectReference, TransactionEffects, Version,
};
use iota_test_transaction_builder::PublishData;
use iota_types::{
    effects::TransactionEffectsAPI,
    messages_grpc::HandleTransactionResponse,
    mock_checkpoint_builder::ValidatorKeypairProvider,
    transaction::{
        CertifiedTransaction, SenderSignedTransactionAPI, SignedTransaction, TransactionEnvelope,
        VerifiedTransaction,
    },
};
use tracing::info;

use crate::{
    command::{BenchmarkConfig, Component},
    mock_account::{Account, batch_create_account_and_gas},
    mock_storage::InMemoryObjectStore,
    single_node::SingleValidator,
    tx_generator::{
        OwnedObjectCreateTxGenerator, RootObjectCreateTxGenerator, SharedObjectCreateTxGenerator,
        TxGenerator,
    },
    workload::Workload,
};

pub struct BenchmarkContext {
    validator: SingleValidator,
    user_accounts: BTreeMap<Address, Account>,
    admin_account: Account,
    benchmark_component: Component,
    /// Execute transactions one at a time instead of concurrently, so
    /// per-transaction wall-clock measurements are not contaminated by
    /// contention.
    sequential: bool,
    /// Cap on transactions executing at once (0 = unbounded). Used by the
    /// concurrency contrast runs to measure per-transaction time under a
    /// controlled number of parallel workers.
    concurrency: usize,
}

/// Total size of all files under `path`, for tracking store growth.
fn dir_size_bytes(path: &std::path::Path) -> u64 {
    let mut total = 0;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                if meta.is_dir() {
                    total += dir_size_bytes(&entry.path());
                } else {
                    total += meta.len();
                }
            }
        }
    }
    total
}

impl BenchmarkContext {
    pub(crate) async fn new(
        workload: Workload,
        benchmark_component: Component,
        config: &BenchmarkConfig,
    ) -> Self {
        // Reserve 1 account for package publishing.
        let mut num_accounts = workload.num_accounts() + 1;
        if config.print_sample_tx {
            // Reserver another one to generate a sample transaction.
            num_accounts += 1;
        }
        let gas_object_num_per_account = workload.gas_object_num_per_account();
        let total = num_accounts * gas_object_num_per_account;

        info!(
            "Creating {} accounts and {} gas objects",
            num_accounts, total
        );
        let (mut user_accounts, genesis_gas_objects) =
            batch_create_account_and_gas(num_accounts, gas_object_num_per_account).await;
        assert_eq!(genesis_gas_objects.len() as u64, total);
        let (_, admin_account) = user_accounts.pop_last().unwrap();

        info!("Initializing validator");
        let validator = SingleValidator::new(
            &genesis_gas_objects[..],
            benchmark_component,
            config.db_path.as_deref(),
            config.enable_write_stall,
        )
        .await;

        Self {
            validator,
            user_accounts,
            admin_account,
            benchmark_component,
            sequential: config.sequential,
            concurrency: config.concurrency,
        }
    }

    /// Sustained mode: run rounds of the workload until the deadline,
    /// committing every round's outputs through the real store and reusing
    /// the accounts (gas and mutated objects are refreshed from effects).
    /// Emits one JSON line of round statistics to `stats_output`.
    pub(crate) async fn benchmark_sustained_execution(
        &mut self,
        tx_generator: Arc<dyn TxGenerator>,
        config: &BenchmarkConfig,
    ) {
        assert!(
            matches!(self.benchmark_component, Component::Baseline),
            "sustained mode supports the baseline component only"
        );
        let db_path = config
            .db_path
            .as_ref()
            .expect("sustained mode requires --db-path");
        let mut stats_out = config
            .stats_output
            .as_ref()
            .map(|p| std::fs::File::create(p).expect("failed to create --stats-output"));
        let start = std::time::Instant::now();
        let deadline = start + std::time::Duration::from_secs(config.duration_secs);
        let cache_commit = self.validator.get_validator().get_cache_commit().clone();
        let mut round = 0u64;
        let mut total_txs = 0u64;
        info!(
            "Sustained mode: {} txs per round for {}s",
            self.user_accounts.len(),
            config.duration_secs
        );
        while std::time::Instant::now() < deadline {
            let generate_start = std::time::Instant::now();
            let transactions = self.generate_transactions(tx_generator.clone()).await;
            let transactions = self.certify_transactions(transactions, true).await;
            let generate_ms = generate_start.elapsed().as_millis() as u64;

            let execute_start = std::time::Instant::now();
            let tasks: FuturesUnordered<_> = transactions
                .into_iter()
                .map(|tx| {
                    let validator = self.validator();
                    tokio::spawn(async move {
                        validator.execute_certificate(tx, Component::Baseline).await
                    })
                })
                .collect();
            let results: Vec<_> = tasks.collect().await;
            let effects: Vec<TransactionEffects> =
                results.into_iter().map(|r| r.unwrap()).collect();
            let execute_ms = execute_start.elapsed().as_millis() as u64;
            let Some(first) = effects.first() else { break };
            let epoch = first.epoch();

            // Commit each transaction's outputs to the store, in order — the
            // same serial writer semantics as the checkpoint executor.
            let commit_start = std::time::Instant::now();
            for effect in &effects {
                let digest = *effect.transaction_digest();
                let batch = cache_commit.build_db_batch(epoch, 0, std::slice::from_ref(&digest));
                cache_commit.commit_transaction_outputs(
                    epoch,
                    batch,
                    std::slice::from_ref(&digest),
                );
            }
            let commit_ms = commit_start.elapsed().as_millis() as u64;

            let mut new_refs = HashMap::new();
            let (mut created, mut mutated, mut deleted) = (0u64, 0u64, 0u64);
            for effect in &effects {
                created += effect.created().len() as u64;
                deleted += effect.deleted().len() as u64;
                for OwnedObjectReference { reference: oref, .. } in effect.mutated() {
                    new_refs.insert(oref.object_id, oref);
                    mutated += 1;
                }
            }
            let txs = effects.len() as u64;
            total_txs += txs;
            self.refresh_gas_objects(new_refs);

            let db_bytes = dir_size_bytes(db_path);
            let line = serde_json::json!({
                "round": round,
                "elapsed_secs": start.elapsed().as_secs(),
                "txs": txs,
                "generate_ms": generate_ms,
                "execute_ms": execute_ms,
                "commit_ms": commit_ms,
                "created": created,
                "mutated": mutated,
                "deleted": deleted,
                "db_bytes": db_bytes,
            });
            if let Some(out) = stats_out.as_mut() {
                use std::io::Write;
                writeln!(out, "{line}").expect("failed to write --stats-output");
            }
            info!(
                "round {round}: {txs} txs, execute {execute_ms}ms, commit {commit_ms}ms, db {} MiB",
                db_bytes >> 20
            );
            round += 1;
        }
        info!(
            "Sustained mode finished: {round} rounds, {total_txs} txs in {:.0}s",
            start.elapsed().as_secs_f64()
        );
    }

    pub(crate) fn validator(&self) -> SingleValidator {
        self.validator.clone()
    }

    pub(crate) async fn publish_package(&mut self, publish_data: PublishData) -> ObjectReference {
        let mut gas_objects = self.admin_account.gas_objects.deref().clone();
        let (package, updated_gas) = self
            .validator
            .publish_package(
                publish_data,
                self.admin_account.sender,
                &self.admin_account.private_key,
                gas_objects[0],
            )
            .await;
        gas_objects[0] = updated_gas;
        self.admin_account.gas_objects = Arc::new(gas_objects);
        package
    }

    /// Mint per-account owned-object fixtures for the mutate-in-place and
    /// burn workloads.
    pub(crate) async fn preparing_owned_objects(
        &mut self,
        move_package: ObjectId,
        objects_per_account: u64,
        object_size: u16,
    ) -> HashMap<Address, Vec<ObjectReference>> {
        let mut owned_objects: HashMap<Address, Vec<ObjectReference>> = HashMap::new();
        if objects_per_account == 0 {
            return owned_objects;
        }
        info!("Preparing owned-object fixtures");
        let transactions = self
            .generate_transactions(Arc::new(OwnedObjectCreateTxGenerator::new(
                move_package,
                objects_per_account,
                object_size,
            )))
            .await;
        let results = self.execute_raw_transactions(transactions).await;
        let mut new_gas_objects = HashMap::new();
        let cache_commit = self.validator().get_validator().get_cache_commit().clone();
        for effects in results {
            let batch =
                cache_commit.build_db_batch(effects.epoch(), 0, &[*effects.transaction_digest()]);
            cache_commit.commit_transaction_outputs(
                effects.epoch(),
                batch,
                &[*effects.transaction_digest()],
            );
            for OwnedObjectReference {
                reference: oref,
                owner,
            } in effects.created()
            {
                if let Some(owner) = owner.as_opt_address() {
                    owned_objects.entry(*owner).or_default().push(oref);
                }
            }
            let gas_object = effects.gas_object().reference;
            new_gas_objects.insert(gas_object.object_id, gas_object);
        }
        self.refresh_gas_objects(new_gas_objects);
        info!("Finished preparing owned-object fixtures");
        owned_objects
    }

    /// In order to benchmark transactions that can read dynamic fields, we must
    /// first create a root object with dynamic fields for each account
    /// address.
    pub(crate) async fn preparing_dynamic_fields(
        &mut self,
        move_package: ObjectId,
        num_dynamic_fields: u64,
        payload_size: u64,
    ) -> HashMap<Address, ObjectReference> {
        let mut root_objects = HashMap::new();

        if num_dynamic_fields == 0 {
            return root_objects;
        }

        info!("Preparing root object with dynamic fields");
        let root_object_create_transactions = self
            .generate_transactions(Arc::new(RootObjectCreateTxGenerator::new(
                move_package,
                num_dynamic_fields,
                payload_size,
            )))
            .await;
        let results = self
            .execute_raw_transactions(root_object_create_transactions)
            .await;
        let mut new_gas_objects = HashMap::new();
        let cache_commit = self.validator().get_validator().get_cache_commit().clone();
        for effects in results {
            let batch =
                cache_commit.build_db_batch(effects.epoch(), 0, &[*effects.transaction_digest()]);

            cache_commit.commit_transaction_outputs(
                effects.epoch(),
                batch,
                &[*effects.transaction_digest()],
            );

            let (owner, root_object) = effects
                .created()
                .into_iter()
                .filter_map(
                    |OwnedObjectReference {
                         reference: oref,
                         owner,
                     }| owner.as_opt_address().map(|owner| (*owner, oref)),
                )
                .next()
                .unwrap();
            root_objects.insert(owner, root_object);
            let gas_object = effects.gas_object().reference;
            new_gas_objects.insert(gas_object.object_id, gas_object);
        }
        self.refresh_gas_objects(new_gas_objects);
        info!("Finished preparing root object with dynamic fields");
        root_objects
    }

    pub(crate) async fn prepare_shared_objects(
        &mut self,
        move_package: ObjectId,
        num_shared_objects: usize,
    ) -> Vec<(ObjectId, Version)> {
        let mut shared_objects = Vec::new();

        if num_shared_objects == 0 {
            return shared_objects;
        }
        assert!(num_shared_objects <= self.user_accounts.len());

        info!("Preparing shared objects");
        let generator = SharedObjectCreateTxGenerator::new(move_package);
        let shared_object_create_transactions: Vec<_> = self
            .user_accounts
            .values()
            .take(num_shared_objects)
            .map(|account| generator.generate_tx(account.clone()))
            .collect();
        let results = self
            .execute_raw_transactions(shared_object_create_transactions)
            .await;
        let mut new_gas_objects = HashMap::new();
        let cache_commit = self.validator.get_validator().get_cache_commit();
        for effects in results {
            let shared_object = effects
                .created()
                .into_iter()
                .filter_map(
                    |OwnedObjectReference {
                         reference: oref,
                         owner,
                     }| {
                        if owner.is_shared() {
                            Some((oref.object_id, oref.version))
                        } else {
                            None
                        }
                    },
                )
                .next()
                .unwrap();
            shared_objects.push(shared_object);
            let gas_object = effects.gas_object().reference;
            new_gas_objects.insert(gas_object.object_id, gas_object);
            // Make sure to commit them to DB. This is needed by both the execution-only
            // mode and the checkpoint-executor mode. For execution-only mode,
            // we iterate through all live objects to construct the in memory
            // object store, hence requiring these objects committed to DB.
            // For checkpoint executor, in order to commit a checkpoint it is required
            // previous versions of objects are already committed.
            let batch =
                cache_commit.build_db_batch(effects.epoch(), 0, &[*effects.transaction_digest()]);
            cache_commit.commit_transaction_outputs(
                effects.epoch(),
                batch,
                &[*effects.transaction_digest()],
            );
        }
        self.refresh_gas_objects(new_gas_objects);
        info!("Finished preparing shared objects");
        shared_objects
    }

    pub(crate) async fn generate_transactions(
        &self,
        tx_generator: Arc<dyn TxGenerator>,
    ) -> Vec<TransactionEnvelope> {
        info!(
            "{}: Creating {} transactions",
            tx_generator.name(),
            self.user_accounts.len()
        );
        let tasks: FuturesUnordered<_> = self
            .user_accounts
            .values()
            .map(|account| {
                let account = account.clone();
                let tx_generator = tx_generator.clone();
                tokio::spawn(async move { tx_generator.generate_tx(account) })
            })
            .collect();
        let results: Vec<_> = tasks.collect().await;
        results.into_iter().map(|r| r.unwrap()).collect()
    }

    pub(crate) async fn certify_transactions(
        &self,
        transactions: Vec<TransactionEnvelope>,
        skip_signing: bool,
    ) -> Vec<CertifiedTransaction> {
        info!("Creating transaction certificates");
        let tasks: FuturesUnordered<_> = transactions
            .into_iter()
            .map(|tx| {
                let validator = self.validator();
                tokio::spawn(async move {
                    let committee = validator.get_committee();
                    let validator_state = validator.get_validator();
                    let sig = if skip_signing {
                        SignedTransaction::sign(
                            0,
                            &tx,
                            &*validator_state.secret,
                            validator_state.name,
                        )
                    } else {
                        let verified_tx = VerifiedTransaction::new_unchecked(tx.clone());
                        validator_state
                            .handle_transaction(validator.get_epoch_store(), verified_tx)
                            .await
                            .unwrap()
                            .status
                            .into_signed_for_testing()
                    };
                    CertifiedTransaction::new(tx.into_data(), vec![sig], committee).unwrap()
                })
            })
            .collect();
        let results: Vec<_> = tasks.collect().await;
        results.into_iter().map(|r| r.unwrap()).collect()
    }

    pub(crate) async fn benchmark_transaction_execution(
        &self,
        transactions: Vec<CertifiedTransaction>,
        print_sample_tx: bool,
    ) {
        if print_sample_tx {
            // We must use remove(0) in case there are shared objects and the transactions
            // must be executed in order.
            self.execute_sample_transaction(transactions[0].clone())
                .await;
        }

        let tx_count = transactions.len();
        let start_time = std::time::Instant::now();
        info!(
            "Started executing {} transactions. You can now attach a profiler",
            transactions.len()
        );

        let has_shared_object = transactions.iter().any(|tx| tx.contains_shared_object());
        if has_shared_object || self.sequential || self.concurrency == 1 {
            // With shared objects, we must execute each transaction in order.
            for transaction in transactions {
                self.validator
                    .execute_certificate(transaction, self.benchmark_component)
                    .await;
            }
        } else {
            // A permit is acquired before execution begins, so at most
            // `concurrency` transactions run at once; the per-transaction
            // wall-clock timer starts after the permit is held, so queueing
            // time is excluded from the measurement. `concurrency == 0` leaves
            // the semaphore off (unbounded, all-at-once).
            let limit = self.concurrency_limiter();
            let tasks: FuturesUnordered<_> = transactions
                .into_iter()
                .map(|tx| {
                    let validator = self.validator();
                    let component = self.benchmark_component;
                    let limit = limit.clone();
                    tokio::spawn(async move {
                        let _permit = match &limit {
                            Some(sem) => Some(sem.acquire().await.unwrap()),
                            None => None,
                        };
                        validator.execute_certificate(tx, component).await
                    })
                })
                .collect();
            let results: Vec<_> = tasks.collect().await;
            results.into_iter().for_each(|r| {
                r.unwrap();
            });
        }

        let elapsed = start_time.elapsed().as_millis() as f64 / 1000f64;
        info!(
            "Execution finished in {}s, TPS={}",
            elapsed,
            tx_count as f64 / elapsed
        );
    }

    pub(crate) async fn benchmark_transaction_execution_in_memory(
        &self,
        transactions: Vec<CertifiedTransaction>,
        print_sample_tx: bool,
    ) {
        if print_sample_tx {
            self.execute_sample_transaction(transactions[0].clone())
                .await;
        }

        let tx_count = transactions.len();
        let in_memory_store = self.validator.create_in_memory_store();
        let start_time = std::time::Instant::now();
        info!(
            "Started executing {} transactions. You can now attach a profiler",
            transactions.len()
        );

        self.execute_transactions_in_memory(in_memory_store.clone(), transactions)
            .await;

        let elapsed = start_time.elapsed().as_millis() as f64 / 1000f64;
        info!(
            "Execution finished in {}s, TPS={}, number of DB object reads per transaction: {}",
            elapsed,
            tx_count as f64 / elapsed,
            in_memory_store.get_num_object_reads() as f64 / tx_count as f64
        );
    }

    /// Print out a sample transaction and its effects so that we can get a
    /// rough idea what we are measuring.
    async fn execute_sample_transaction(&self, sample_transaction: CertifiedTransaction) {
        info!(
            "Sample transaction digest={:?}: {:?}",
            sample_transaction.digest(),
            sample_transaction.data()
        );
        let effects = self
            .validator()
            .execute_dry_run(sample_transaction.into_unsigned())
            .await;
        info!("Sample effects: {:?}\n\n", effects);
        assert!(effects.status().is_success());
    }

    /// Benchmark parallel signing a vector of transactions and measure the TPS.
    pub(crate) async fn benchmark_transaction_signing(
        &self,
        transactions: Vec<TransactionEnvelope>,
        print_sample_tx: bool,
    ) {
        if print_sample_tx {
            let sample_transaction = &transactions[0];
            info!("Sample transaction: {:?}", sample_transaction.data());
        }

        let tx_count = transactions.len();
        let start_time = std::time::Instant::now();
        self.validator_sign_transactions(transactions).await;
        let elapsed = start_time.elapsed().as_millis() as f64 / 1000f64;
        info!(
            "Transaction signing finished in {}s, TPS={}.",
            elapsed,
            tx_count as f64 / elapsed,
        );
    }

    pub(crate) async fn benchmark_checkpoint_executor(
        &self,
        transactions: Vec<CertifiedTransaction>,
        checkpoint_size: usize,
    ) {
        self.execute_sample_transaction(transactions[0].clone())
            .await;

        info!("Executing all transactions to generate effects");
        let tx_count = transactions.len();
        let in_memory_store = self.validator.create_in_memory_store();
        let effects: BTreeMap<_, _> = self
            .execute_transactions_in_memory(in_memory_store.clone(), transactions.clone())
            .await
            .into_iter()
            .map(|e| (*e.transaction_digest(), e))
            .collect();

        info!("Building checkpoints");
        let validator = self.validator();
        let checkpoints = validator
            .build_checkpoints(transactions, effects, checkpoint_size)
            .await;
        info!("Built {} checkpoints", checkpoints.len());
        let last_checkpoint_seq = checkpoints.last().unwrap().0.sequence_number();
        let checkpoint_executor = validator.create_checkpoint_executor();
        for (checkpoint, contents) in checkpoints {
            let state = validator.get_validator();
            state
                .get_checkpoint_store()
                .insert_verified_checkpoint(&checkpoint)
                .unwrap();
            state
                .get_state_sync_store()
                .multi_insert_transaction_and_effects(contents.transactions());
            state
                .get_checkpoint_store()
                .insert_verified_checkpoint_contents(&checkpoint, contents)
                .unwrap();
            state
                .get_checkpoint_store()
                .update_highest_synced_checkpoint(&checkpoint)
                .unwrap();
        }
        let start_time = std::time::Instant::now();
        info!("Starting checkpoint execution. You can now attach a profiler");
        checkpoint_executor
            .run_epoch(Some(RunWithRange::Checkpoint(last_checkpoint_seq)))
            .await;
        let elapsed = start_time.elapsed().as_millis() as f64 / 1000f64;
        info!(
            "Checkpoint execution finished in {}s, TPS={}.",
            elapsed,
            tx_count as f64 / elapsed,
        );
    }

    async fn execute_raw_transactions(
        &self,
        transactions: Vec<TransactionEnvelope>,
    ) -> Vec<TransactionEffects> {
        let tasks: FuturesUnordered<_> = transactions
            .into_iter()
            .map(|tx| {
                let validator = self.validator();
                tokio::spawn(async move { validator.execute_raw_transaction(tx).await })
            })
            .collect();
        let results: Vec<_> = tasks.collect().await;
        results.into_iter().map(|r| r.unwrap()).collect()
    }

    async fn execute_transactions_in_memory(
        &self,
        store: InMemoryObjectStore,
        transactions: Vec<CertifiedTransaction>,
    ) -> Vec<TransactionEffects> {
        let has_shared_object = transactions.iter().any(|tx| tx.contains_shared_object());
        if has_shared_object || self.sequential || self.concurrency == 1 {
            // With shared objects, we must execute each transaction in order.
            let mut effects = Vec::new();
            for transaction in transactions {
                effects.push(
                    self.validator
                        .execute_transaction_in_memory(store.clone(), transaction)
                        .await,
                );
            }
            effects
        } else {
            let limit = self.concurrency_limiter();
            let tasks: FuturesUnordered<_> = transactions
                .into_iter()
                .map(|tx| {
                    let store = store.clone();
                    let validator = self.validator();
                    let limit = limit.clone();
                    tokio::spawn(async move {
                        let _permit = match &limit {
                            Some(sem) => Some(sem.acquire().await.unwrap()),
                            None => None,
                        };
                        validator.execute_transaction_in_memory(store, tx).await
                    })
                })
                .collect();
            let results: Vec<_> = tasks.collect().await;
            results.into_iter().map(|r| r.unwrap()).collect()
        }
    }

    /// A semaphore bounding concurrent execution to `self.concurrency`, or
    /// `None` when unbounded (`concurrency == 0`). `concurrency == 1` is
    /// handled by the sequential branch, so this is only consulted for
    /// `concurrency >= 2`.
    fn concurrency_limiter(&self) -> Option<Arc<tokio::sync::Semaphore>> {
        (self.concurrency >= 2).then(|| Arc::new(tokio::sync::Semaphore::new(self.concurrency)))
    }

    fn refresh_gas_objects(&mut self, mut new_gas_objects: HashMap<ObjectId, ObjectReference>) {
        info!("Refreshing gas objects");
        for account in self.user_accounts.values_mut() {
            let refreshed_gas_objects: Vec<_> = account
                .gas_objects
                .iter()
                .map(|oref| {
                    if let Some(new_oref) = new_gas_objects.remove(&oref.object_id) {
                        new_oref
                    } else {
                        *oref
                    }
                })
                .collect();
            account.gas_objects = Arc::new(refreshed_gas_objects);
        }
    }
    pub(crate) async fn validator_sign_transactions(
        &self,
        transactions: Vec<TransactionEnvelope>,
    ) -> Vec<HandleTransactionResponse> {
        info!(
            "Started signing {} transactions. You can now attach a profiler",
            transactions.len(),
        );
        let tasks: FuturesUnordered<_> = transactions
            .into_iter()
            .map(|tx| {
                let validator = self.validator();
                tokio::spawn(async move { validator.sign_transaction(tx).await })
            })
            .collect();
        let results: Vec<_> = tasks.collect().await;
        results.into_iter().map(|r| r.unwrap()).collect()
    }
}
