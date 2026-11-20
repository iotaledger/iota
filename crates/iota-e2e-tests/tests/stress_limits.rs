// TEMP benchmark: runs one kind of work (a kernel of the `stress` package, or a
// call into an upgraded package with a large first module) in one transaction,
// to find which work costs a validator the most time per unit of gas. Protocol
// limits are set through IOTA_PROTOCOL_CONFIG_OVERRIDE_*. Not for commit.

use std::{path::PathBuf, sync::Mutex, time::Instant};

use iota_grpc_client::read_mask_fields::TransactionField;
use iota_move_build::BuildConfig;
use iota_sdk_types::{
    Address, Argument, Identifier, ObjectId, ObjectReference, Owner, SharedObjectReference, TransactionEffects, Version,
};
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    crypto::{AccountPrivateKey, get_account_private_key},
    effects::TransactionEffectsAPI,
    messages_checkpoint::CheckpointContentsExt,
    move_package::UpgradePolicy,
    object::Object,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::{CallArg, TransactionEnvelope},
};
use test_cluster::{TestCluster, TestClusterBuilder};

// Kept byte for byte the same as the setup 2 copy, so not formatted here.
#[rustfmt::skip]
#[path = "ptb_limits/ptb_limits_tx.rs"]
mod ptb_limits_tx;

#[global_allocator]
static ALLOC: iota_core::load_probe::CountingAlloc = iota_core::load_probe::CountingAlloc;

const SETUP_BUDGET: u64 = 50_000_000_000;

struct Sender<'a> {
    cluster: &'a TestCluster,
    address: Address,
    key: AccountPrivateKey,
    gas: ObjectReference,
    rgp: u64,
}

impl Sender<'_> {
    async fn send(&mut self, tx: TransactionEnvelope) -> (Result<TransactionEffects, String>, String, f64) {
        let digest = tx.digest().to_string();
        let start = Instant::now();
        let response = self
            .cluster
            .grpc_client()
            .execute_transaction(tx.into())
            .checkpoint_inclusion_timeout_ms(120_000)
            .read_mask(TransactionField::EFFECTS_BCS)
            .await;
        let latency_ms = start.elapsed().as_secs_f64() * 1000.0;
        let outcome = match response {
            Ok(response) => {
                let effects = response.into_inner().effects().unwrap().effects().unwrap();
                self.gas = *effects.gas_object().reference();
                Ok(effects)
            }
            Err(error) => Err(error.to_string().chars().take(400).collect()),
        };
        (outcome, digest, latency_ms)
    }

    async fn run(&mut self, builder: ProgrammableTransactionBuilder) -> TransactionEffects {
        let tx = TestTransactionBuilder::new(self.address, self.gas, self.rgp)
            .with_gas_budget(SETUP_BUDGET)
            .programmable(builder.finish())
            .build_and_sign(&self.key);
        match self.send(tx).await.0 {
            Ok(effects) if effects.status().is_success() => effects,
            Ok(effects) => panic!("setup failed: {:?}", effects.status()),
            Err(error) => panic!("setup rejected: {error}"),
        }
    }

    /// Publishes the package at `path`; returns its id and, if `keep_cap`, the
    /// upgrade cap.
    async fn publish(&mut self, path: PathBuf) -> (ObjectId, Option<ObjectReference>) {
        let package = BuildConfig::new_for_testing().build(&path).unwrap();
        let mut b = ProgrammableTransactionBuilder::new();
        let cap = b.publish_upgradeable(package.get_package_bytes(false), package.get_published_dependencies_ids());
        b.transfer_arg(self.address, cap);
        let effects = self.run(b).await;
        let (mut package_id, mut cap) = (None, None);
        for created in effects.created() {
            match created.owner() {
                Owner::Immutable => package_id = Some(created.reference().object_id),
                Owner::Address(_) => cap = Some(*created.reference()),
                _ => {}
            }
        }
        (package_id.unwrap(), cap)
    }

    /// Upgrades `package` with the same modules; returns the new package id.
    async fn upgrade(&mut self, path: PathBuf, package: ObjectId, cap: ObjectReference) -> ObjectId {
        let compiled = BuildConfig::new_for_testing().build(&path).unwrap();
        let mut b = ProgrammableTransactionBuilder::new();
        let cap = b.obj(CallArg::ImmutableOrOwned(cap)).unwrap();
        let policy = b.pure(UpgradePolicy::COMPATIBLE).unwrap();
        let digest = b.pure(compiled.get_package_digest(false).to_vec()).unwrap();
        let ticket = b.programmable_move_call(
            ObjectId::FRAMEWORK,
            Identifier::from_static("package"),
            Identifier::from_static("authorize_upgrade"),
            vec![],
            vec![cap, policy, digest],
        );
        let receipt = b.upgrade(
            package,
            ticket,
            compiled.get_published_dependencies_ids(),
            compiled.get_package_bytes(false),
        );
        b.programmable_move_call(
            ObjectId::FRAMEWORK,
            Identifier::from_static("package"),
            Identifier::from_static("commit_upgrade"),
            vec![],
            vec![cap, receipt],
        );
        let effects = self.run(b).await;
        effects
            .created()
            .into_iter()
            .find(|c| matches!(c.owner(), Owner::Immutable))
            .unwrap()
            .reference()
            .object_id
    }
}

/// The fullnode's highest executed checkpoint.
fn last_checkpoint(cluster: &TestCluster) -> u64 {
    let state = cluster.fullnode_handle.iota_node.with(|node| node.state());
    state.get_checkpoint_store().get_highest_executed_checkpoint_seq_number().unwrap().unwrap_or(0)
}

/// Prints one `STRESS_CHECKPOINT` line per checkpoint the fullnode executed
/// after `after`: its time, the bytes of its transactions, effects and events,
/// and the digests of its transactions.
fn report_checkpoints(cluster: &TestCluster, after: u64) {
    let state = cluster.fullnode_handle.iota_node.with(|node| node.state());
    let store = state.get_checkpoint_store();
    let cache = state.get_transaction_cache_reader();
    for seq in after + 1..=last_checkpoint(cluster) {
        let Some(checkpoint) = store.get_checkpoint_by_sequence_number(seq).unwrap() else { continue };
        let summary = checkpoint.data();
        let contents = store.get_checkpoint_contents(&summary.contents_digest).unwrap().unwrap();
        let (mut tx_bytes, mut effects_bytes, mut event_bytes, mut digests) = (0, 0, 0, Vec::new());
        for d in contents.iter() {
            if let Some(tx) = cache.get_transaction_block(&d.transaction) {
                tx_bytes += bcs::serialized_size(tx.data()).unwrap_or(0);
            }
            if let Some(effects) = cache.get_executed_effects(&d.transaction) {
                effects_bytes += bcs::serialized_size(&effects).unwrap_or(0);
            }
            if let Some(events) = cache.get_events(&d.transaction) {
                event_bytes += bcs::serialized_size(&events).unwrap_or(0);
            }
            digests.push(d.transaction.to_string());
        }
        eprintln!(
            "STRESS_CHECKPOINT seq={seq} ts_ms={} txs={} tx_bytes={tx_bytes} effects_bytes={effects_bytes} event_bytes={event_bytes} digests={}",
            summary.timestamp_ms,
            digests.len(),
            digests.join(",")
        );
    }
}

fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

#[tokio::test(flavor = "multi_thread")]
async fn stress_limits() {
    let spec = ptb_limits_tx::Spec::from_env();
    let (kernel, commands, n, bytes, budget) = (spec.kernel.clone(), spec.commands, spec.n, spec.bytes, spec.budget);
    let repeats = env_usize("STRESS_REPEATS", 3);
    let stress_path: PathBuf = std::env::var("STRESS_PACKAGE").unwrap().into();

    // With more than one sender, each sends `STRESS_COUNT` transactions, the next
    // one as soon as the previous one is final. With `STRESS_SECONDS` set, a
    // sender starts no new transaction after that many seconds.
    let senders = env_usize("STRESS_SENDERS", 1);
    let count = env_usize("STRESS_COUNT", 10);
    let seconds = std::env::var("STRESS_SECONDS").ok().and_then(|v| v.parse::<f64>().ok());

    // Attack mode: worst transactions at a fixed rate from a pool of senders, and
    // small transactions at a fixed rate from another pool, for `STRESS_DURATION`
    // seconds. Each send waits for its slot, not for the previous transaction.
    let env_f64 = |name: &str, default: f64| std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default);
    let attack_rate = env_f64("STRESS_ATTACK_RATE", 0.0);
    let victim_rate = env_f64("STRESS_VICTIM_RATE", 0.0);
    let duration = env_f64("STRESS_DURATION", 30.0);
    let attack_pool = env_usize("STRESS_ATTACK_POOL", 64);
    let victim_pool = env_usize("STRESS_VICTIM_POOL", 20);
    let attack_mode = attack_rate > 0.0 || victim_rate > 0.0;
    let keys_needed = if attack_mode { 1 + attack_pool + victim_pool } else { senders };
    let keys: Vec<(Address, AccountPrivateKey)> = (0..keys_needed).map(|_| get_account_private_key()).collect();
    let gas_objects: Vec<Object> = keys.iter().map(|(address, _)| Object::with_owner_for_testing(*address)).collect();
    let gas_ids: Vec<ObjectId> = gas_objects.iter().map(|o| o.id()).collect();
    let cluster = TestClusterBuilder::new()
        .with_num_validators(env_usize("STRESS_VALIDATORS", 1))
        .with_objects(gas_objects)
        .build()
        .await;
    let rgp = cluster.get_reference_gas_price().await;
    let mut all_senders = Vec::new();
    for ((address, key), gas_id) in keys.into_iter().zip(gas_ids) {
        let gas = cluster.get_object_from_fullnode_store(&gas_id).await.unwrap().object_ref();
        all_senders.push(Sender { cluster: &cluster, address, key, gas, rgp });
    }
    let mut sender = all_senders.remove(0);

    let setup_start = Instant::now();
    let (stress, _) = sender.publish(stress_path).await;
    let mut objects = ptb_limits_tx::Objects { stress, big: None, parent: None, inputs: Vec::new() };
    if let Some(upgrade) = spec.big_package() {
        let big_path: PathBuf = std::env::var("STRESS_BIG").unwrap().into();
        let (big, cap) = sender.publish(big_path.clone()).await;
        objects.big = Some(if upgrade { sender.upgrade(big_path, big, cap.unwrap()).await } else { big });
    }
    if let Some(total) = spec.parent_fields() {
        let effects = sender.run(ptb_limits_tx::create_parent(stress)).await;
        let created = effects.created().into_iter().find(|c| matches!(c.owner(), Owner::Shared(_))).unwrap();
        let parent = match created.owner() {
            Owner::Shared(v) => (created.reference().object_id, *v),
            _ => unreachable!(),
        };
        for b in ptb_limits_tx::fill_parent(stress, parent, total) {
            sender.run(b).await;
        }
        objects.parent = Some(parent);
    }
    if let Some(total) = spec.owned_objects() {
        for b in ptb_limits_tx::create_objects(stress, total) {
            let effects = sender.run(b).await;
            objects.inputs.extend(
                effects
                    .created()
                    .into_iter()
                    .filter(|c| matches!(c.owner(), Owner::Address(_)))
                    .map(|c| *c.reference()),
            );
        }
    }
    eprintln!("STRESS_SETUP kernel={kernel} secs={:.0}", setup_start.elapsed().as_secs_f64());
    let first_checkpoint = last_checkpoint(&cluster);
    let objects = Mutex::new(objects);

    let data = spec.data();
    let (spec, data) = (&spec, &data);
    let kernel = kernel.as_str();
    let send_one = async |sender: &mut Sender<'_>, tag: String| {
        let b = ptb_limits_tx::test_transaction(spec, &objects.lock().unwrap(), data);
        let tx = TestTransactionBuilder::new(sender.address, sender.gas, sender.rgp)
            .with_gas_budget(budget)
            .programmable(b.finish())
            .build_and_sign(&sender.key);
        let tx_bytes = bcs::serialized_size(&tx).unwrap_or(0);
        let (outcome, digest, latency_ms) = sender.send(tx).await;
        if let (Ok(effects), "inputs") = (&outcome, kernel) {
            // Executed inputs move to new versions, which the next transaction must name.
            let moved: std::collections::HashMap<ObjectId, ObjectReference> =
                effects.mutated().into_iter().map(|m| (m.reference().object_id, *m.reference())).collect();
            for input in objects.lock().unwrap().inputs.iter_mut() {
                if let Some(new) = moved.get(&input.object_id) {
                    *input = *new;
                }
            }
        }
        let detail = match &outcome {
            Ok(effects) => format!(
                "units={} effects_bytes={} outcome={}",
                effects.gas_cost_summary().computation_cost / sender.rgp,
                bcs::serialized_size(effects).unwrap_or(0),
                if effects.status().is_success() { "ok".to_string() } else { format!("failed: {:?}", effects.status()) },
            ),
            Err(error) => format!("units=0 outcome=rejected: {error}"),
        };
        eprintln!(
            "STRESS_PROBE {tag} kernel={kernel} commands={commands} n={n} bytes={bytes} tx_bytes={tx_bytes} digest={digest} latency_ms={latency_ms:.0} {detail}"
        );
    };

    if attack_mode {
        let mut pool = all_senders.into_iter();
        let mut attackers: Vec<_> = pool.by_ref().take(attack_pool).collect();
        let mut victims: Vec<_> = pool.collect();
        let send_small = async |sender: &mut Sender<'_>, tag: String| {
            let mut b = ProgrammableTransactionBuilder::new();
            b.programmable_move_call(stress, Identifier::from_static("kernels"), Identifier::from_static("noop"), vec![], vec![]);
            let tx = TestTransactionBuilder::new(sender.address, sender.gas, sender.rgp)
                .with_gas_budget(50_000_000)
                .programmable(b.finish())
                .build_and_sign(&sender.key);
            let (outcome, digest, latency_ms) = sender.send(tx).await;
            let outcome = match outcome {
                Ok(effects) if effects.status().is_success() => "ok".to_string(),
                Ok(effects) => format!("failed: {:?}", effects.status()),
                Err(error) => format!("rejected: {error}"),
            };
            eprintln!("STRESS_PROBE {tag} kernel=small digest={digest} latency_ms={latency_ms:.0} units=0 outcome={outcome}");
        };
        eprintln!("STRESS_START");
        let start = Instant::now();
        // Slot `j + m * pool` of a stream at `rate` per second starts at that index / rate.
        let wait_for = async |slot: usize, rate: f64| {
            let at = std::time::Duration::from_secs_f64(slot as f64 / rate);
            let now = start.elapsed();
            if at > now {
                tokio::time::sleep(at - now).await;
            }
            (start.elapsed().as_secs_f64() - at.as_secs_f64()) * 1000.0
        };
        let attacks = attackers.iter_mut().enumerate().map(|(j, sender)| {
            let (send_one, wait_for) = (&send_one, &wait_for);
            async move {
                let mut m = 0;
                while attack_rate > 0.0 && ((j + m * attack_pool) as f64) < duration * attack_rate {
                    let late_ms = wait_for(j + m * attack_pool, attack_rate).await;
                    send_one(sender, format!("role=attack slot={} late_ms={late_ms:.0}", j + m * attack_pool)).await;
                    m += 1;
                }
            }
        });
        let smalls = victims.iter_mut().enumerate().map(|(j, sender)| {
            let (send_small, wait_for) = (&send_small, &wait_for);
            async move {
                let mut m = 0;
                while victim_rate > 0.0 && ((j + m * victim_pool) as f64) < duration * victim_rate {
                    let late_ms = wait_for(j + m * victim_pool, victim_rate).await;
                    send_small(sender, format!("role=victim slot={} late_ms={late_ms:.0}", j + m * victim_pool)).await;
                    m += 1;
                }
            }
        });
        futures::future::join(futures::future::join_all(attacks), futures::future::join_all(smalls)).await;
        eprintln!("STRESS_RESULT secs={:.1}", start.elapsed().as_secs_f64());
        report_checkpoints(&cluster, first_checkpoint);
        return;
    }
    if senders == 1 {
        for repeat in 0..repeats {
            send_one(&mut sender, format!("repeat={repeat}")).await;
        }
        report_checkpoints(&cluster, first_checkpoint);
        return;
    }
    all_senders.insert(0, sender);
    eprintln!("STRESS_START");
    let start = Instant::now();
    futures::future::join_all(all_senders.iter_mut().enumerate().map(|(s, sender)| {
        let send_one = &send_one;
        async move {
            for k in 0..count {
                if seconds.is_some_and(|seconds| start.elapsed().as_secs_f64() >= seconds) {
                    break;
                }
                send_one(sender, format!("sender={s} k={k}")).await;
            }
        }
    }))
    .await;
    eprintln!(
        "STRESS_RESULT senders={senders} count={count} secs={:.1}",
        start.elapsed().as_secs_f64()
    );
    report_checkpoints(&cluster, first_checkpoint);
}
