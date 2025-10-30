// In‑node model updater and shared types for congestion tracker interactions.
// Architecture: best‑effort, non‑blocking update + train workers on bounded
// channels; inference reads current histories, then locks the learner briefly
// only for the forward pass. Staleness decay nudges results toward RGP when
// objects go quiet.

use serde::Serialize;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    fs::{File, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
};
use std::sync::{mpsc, mpsc::{SyncSender, Receiver}};
use std::sync::atomic::{AtomicU64, Ordering};

use iota_types::base_types::ObjectID;
use iota_types::transaction::{TransactionData, TransactionDataAPI};

use crate::model;
use tch::{self, IndexOp, Tensor};
use tch::nn; // VarStore for inference snapshot
use arc_swap::ArcSwap;

// -------------------------------
// Types shared with congestion tracker
// -------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct CpUpdateRow {
    pub checkpoint_ms: u64,
    pub reference_gas_price: u64,
    pub object_id: String,

    pub latest_congestion_time: Option<u64>,
    pub highest_congestion_gas_price: u64,
    pub latest_clearing_time: Option<u64>,
    pub lowest_clearing_gas_price: u64,
    pub hotness: f64,

    pub was_touched_in_cp: bool,
    pub was_congested_in_cp: bool,
    pub was_cleared_in_cp: bool,
    pub congested_tx_count_in_cp: u32,
    pub clearing_tx_count_in_cp: u32,

    pub hotness_over_ref: f64,
    pub highest_congestion_over_ref: f64,
    pub lowest_clearing_over_ref: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CpUpdateBatch {
    pub checkpoint_ms: u64,
    pub rows: Vec<CpUpdateRow>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TrainTxItem {
    pub checkpoint_ms: u64,
    pub reference_gas_price: u64,
    pub required_price_in_cp: u64,
    pub object_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TrainTxBatch {
    pub items: Vec<TrainTxItem>,
}

#[derive(Debug, Clone)]
pub struct ObjectSnapshot {
    pub latest_congestion_time: Option<u64>,
    pub highest_congestion_gas_price: u64,
    pub latest_clearing_time: Option<u64>,
    pub lowest_clearing_gas_price: u64,
    pub hotness: f64,
}

#[derive(Debug, Clone, Default)]
pub struct ObjectCheckpointStats {
    pub was_congested: bool,
    pub was_cleared: bool,
    pub congested_tx_count: u32,
    pub clearing_tx_count: u32,
}

pub fn build_cp_update_batch(
    checkpoint_ms: u64,
    reference_gas_price: u64,
    touched: impl IntoIterator<Item = ObjectID>,
    snapshots: &HashMap<ObjectID, ObjectSnapshot>,
    stats: &HashMap<ObjectID, ObjectCheckpointStats>,
) -> CpUpdateBatch {
    let mut rows = Vec::new();
    for oid in touched {
        if let Some(s) = snapshots.get(&oid) {
            let st = stats.get(&oid).cloned().unwrap_or_default();
            let hot = s.hotness;
            let highest = s.highest_congestion_gas_price;
            let lowest = s.lowest_clearing_gas_price;
            rows.push(CpUpdateRow {
                checkpoint_ms,
                reference_gas_price,
                object_id: oid.to_string(),
                latest_congestion_time: s.latest_congestion_time,
                highest_congestion_gas_price: highest,
                latest_clearing_time: s.latest_clearing_time,
                lowest_clearing_gas_price: lowest,
                hotness: hot,
                was_touched_in_cp: true,
                was_congested_in_cp: st.was_congested,
                was_cleared_in_cp: st.was_cleared,
                congested_tx_count_in_cp: st.congested_tx_count,
                clearing_tx_count_in_cp: st.clearing_tx_count,
                hotness_over_ref: hot / reference_gas_price as f64,
                highest_congestion_over_ref: if highest > 0 {
                    highest as f64 / reference_gas_price as f64
                } else { 0.0 },
                lowest_clearing_over_ref: if lowest > 0 {
                    lowest as f64 / reference_gas_price as f64
                } else { 0.0 },
            });
        } else {
            let st = stats.get(&oid).cloned().unwrap_or_default();
            rows.push(CpUpdateRow {
                checkpoint_ms,
                reference_gas_price,
                object_id: oid.to_string(),
                latest_congestion_time: None,
                highest_congestion_gas_price: 0,
                latest_clearing_time: None,
                lowest_clearing_gas_price: 0,
                hotness: 0.0,
                was_touched_in_cp: true,
                was_congested_in_cp: st.was_congested,
                was_cleared_in_cp: st.was_cleared,
                congested_tx_count_in_cp: st.congested_tx_count,
                clearing_tx_count_in_cp: st.clearing_tx_count,
                hotness_over_ref: 0.0,
                highest_congestion_over_ref: 0.0,
                lowest_clearing_over_ref: 0.0,
            });
        }
    }
    CpUpdateBatch { checkpoint_ms, rows }
}

pub fn build_train_tx_batch(
    checkpoint_ms: u64,
    reference_gas_price: u64,
    raw_txs: &[RawTxItem],
    per_object_min_clearing_in_cp: &HashMap<ObjectID, u64>,
) -> Option<TrainTxBatch> {
    let mut items = Vec::new();
    for tx in raw_txs {
        let oids: Vec<String> = tx.touched_objects.iter().map(|o| o.to_string()).collect();
        if oids.is_empty() { continue; }
        const LABEL_DELTA: u64 = 100; // nudge congested labels above RGP
        let required = if tx.is_congested {
            let base = tx.gas_price_feedback.unwrap_or(tx.gas_price).max(reference_gas_price);
            base.saturating_add(LABEL_DELTA)
        } else {
            let mut req = 1000u64;
            for oid in &tx.touched_objects {
                if let Some(min_clear) = per_object_min_clearing_in_cp.get(oid) {
                    if *min_clear > 1000 { req = req.max(*min_clear); }
                }
            }
            req
        };
        items.push(TrainTxItem {
            checkpoint_ms,
            reference_gas_price,
            required_price_in_cp: required,
            object_ids: oids,
        });
    }
    if items.is_empty() { None } else { Some(TrainTxBatch { items }) }
}

#[derive(Debug, Clone)]
pub struct RawTxItem {
    pub tx_digest: String,
    pub is_congested: bool,
    pub gas_price: u64,
    pub gas_price_feedback: Option<u64>,
    pub touched_objects: Vec<ObjectID>,
}

// -------------------------------
// Audit logger (optional; used by tracker elsewhere)
// -------------------------------
pub const AUDIT_LOG_PATH: &str = "congestion_audit.jsonl";

#[derive(Debug, Clone, Serialize)]
pub struct TxAuditRow {
    pub checkpoint_id: u64,
    pub reference_gas_price: u64,
    pub tx_digest: String,
    pub is_congested: bool,
    pub gas_price: u64,
    pub gas_price_feedback: Option<u64>,
    pub touched_objects: Vec<String>,
    pub required_price_in_cp: u64,
    pub overpay: u64,
}

pub struct AuditLogger { writer: Option<Arc<Mutex<File>>> }
impl AuditLogger {
    pub fn new_default() -> Self { Self::new_with_path(PathBuf::from(AUDIT_LOG_PATH)) }
    pub fn new_with_path(path: PathBuf) -> Self {
        let writer = match OpenOptions::new().create(true).append(true).open(&path) {
            Ok(f) => Some(Arc::new(Mutex::new(f))),
            Err(e) => { eprintln!("[congestion/audit] failed to open {:?}: {e}", path); None }
        };
        Self { writer }
    }
    pub fn write_rows(&self, rows: &[TxAuditRow]) {
        if let Some(w) = &self.writer {
            let mut g = w.lock().unwrap();
            for row in rows {
                if let Ok(line) = serde_json::to_string(row) {
                    if let Err(e) = writeln!(g, "{}", line) { eprintln!("[congestion/audit] write error: {e}"); break; }
                }
            }
            let _ = g.flush();
        }
    }
}

// -------------------------------
// In‑node model workers and API bridge
// -------------------------------

pub trait ModelUpdater: Send + Sync {
    fn post_update(&self, batch: CpUpdateBatch);
    fn post_train_tx(&self, batch: TrainTxBatch);
}

const FEAT: usize = model::F as usize;
const WIN: usize = model::T as usize;

// Read-only inference snapshot built from trainer weights.
struct GasInfer {
    vs: nn::VarStore,
    model: model::PriceModel,
}

impl GasInfer {
    fn predict(&self, seqs: &[Tensor], h_anchor: f32) -> (Tensor, Tensor) {
        self.model.forward_tx(seqs, h_anchor, false)
    }
}

fn build_infer_from_trainer(trainer: &model::GasLearner) -> Option<GasInfer> {
    let tmp = std::env::temp_dir().join("iota_nn_snapshot.pt");
    if trainer.vs.save(&tmp).is_err() {
        return None;
    }
    let mut vs = nn::VarStore::new(tch::Device::Cpu);
    let root = &vs.root();
    let model = model::PriceModel::new(root, model::F, model::EMBED_DIM, &model::TAUS);
    if vs.load(&tmp).is_err() {
        return None;
    }
    Some(GasInfer { vs, model })
}

struct HistState {
    histories: HashMap<ObjectID, VecDeque<[f32; FEAT]>>, // oldest .. newest
    ema_low: HashMap<ObjectID, f32>,
    ema_high: HashMap<ObjectID, f32>,
    last_ms: HashMap<ObjectID, u64>,
    last_global_ms: u64,
    pending_train: Vec<TrainTxItem>,
}

impl HistState {
    fn new() -> Self {
        Self {
            histories: HashMap::new(),
            ema_low: HashMap::new(),
            ema_high: HashMap::new(),
            last_ms: HashMap::new(),
            last_global_ms: 0,
            pending_train: Vec::new(),
        }
    }
    fn push_feature_row(&mut self, oid: ObjectID, feat: [f32; FEAT]) {
        let q = self.histories.entry(oid).or_insert_with(|| VecDeque::with_capacity(WIN));
        if q.len() == WIN { q.pop_front(); }
        q.push_back(feat);
    }
    fn build_object_seq(&self, oid: &ObjectID) -> Option<Tensor> {
        let q = self.histories.get(oid)?;
        let mut slice: Vec<[f32; FEAT]> = q
            .iter()
            .rev()
            .take(WIN)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        if slice.len() < WIN {
            let pad_count = WIN - slice.len();
            let mut pad = [0f32; FEAT];
            pad[FEAT - 1] = 1.0; // padded flag
            let mut pads = vec![pad; pad_count];
            pads.append(&mut slice);
            slice = pads;
        }
        Some(model::tensor_from_rows_txf(&slice))
    }
    fn latest_hotness_over_ref(&self, oid: &ObjectID) -> Option<f32> { self.histories.get(oid)?.back().map(|r| r[0]) }
}

const BASE_FEAT: usize = 9;
fn row_to_base_features(row: &CpUpdateRow) -> [f32; BASE_FEAT] {
    let spread = (row.highest_congestion_over_ref - row.lowest_clearing_over_ref).max(0.0);
    [
        row.hotness_over_ref as f32,
        row.highest_congestion_over_ref as f32,
        row.lowest_clearing_over_ref as f32,
        if row.was_touched_in_cp { 1.0 } else { 0.0 },
        if row.was_congested_in_cp { 1.0 } else { 0.0 },
        if row.was_cleared_in_cp { 1.0 } else { 0.0 },
        (row.congested_tx_count_in_cp as f32).ln_1p(),
        (row.clearing_tx_count_in_cp as f32).ln_1p(),
        spread as f32,
    ]
}

impl HistState {
    fn feat_from_row(&mut self, oid: ObjectID, row: &CpUpdateRow) -> [f32; FEAT] {
        let base = row_to_base_features(row);
        let alpha = 0.3f32;
        let low = row.lowest_clearing_over_ref as f32;
        let high = row.highest_congestion_over_ref as f32;
        let ema_low = match self.ema_low.get(&oid) {
            Some(v) => (1.0 - alpha) * *v + alpha * low,
            None => low,
        };
        let ema_high = match self.ema_high.get(&oid) {
            Some(v) => (1.0 - alpha) * *v + alpha * high,
            None => high,
        };
        self.ema_low.insert(oid, ema_low);
        self.ema_high.insert(oid, ema_high);
        let prev = self.last_ms.insert(oid, row.checkpoint_ms);
        let dt_sec = prev
            .map(|p| ((row.checkpoint_ms.saturating_sub(p)) as f32) / 1000.0)
            .unwrap_or(0.0);
        self.last_global_ms = self.last_global_ms.max(row.checkpoint_ms);
        let mut out = [0f32; FEAT];
        out[..BASE_FEAT].copy_from_slice(&base);
        out[9] = ema_low;
        out[10] = ema_high;
        out[11] = dt_sec.ln_1p();
        out[FEAT - 1] = 0.0; // not padded
        out
    }

    fn synth_not_touched(&mut self, oid: ObjectID, checkpoint_ms: u64) -> Option<[f32; FEAT]> {
        let last = self.histories.get(&oid)?.back()?.clone();
        let prev_ms = *self.last_ms.get(&oid)?;
        let dt_sec = ((checkpoint_ms.saturating_sub(prev_ms)) as f32) / 1000.0;
        let mut f = last;
        f[3] = 0.0; // touched flag off
        f[4] = 0.0;
        f[5] = 0.0;
        f[6] = 0.0;
        f[7] = 0.0;
        f[11] = dt_sec.ln_1p();
        self.last_ms.insert(oid, checkpoint_ms);
        Some(f)
    }
}

// Old ModelMsg removed: we now use separate bounded channels per task (update/train)

pub struct InNodeModelUpdater {
    hist: Arc<Mutex<HistState>>, // histories and timing
    learner: Arc<Mutex<model::GasLearner>>, // trainer/infer model
    tx_update: SyncSender<CpUpdateBatch>,
    tx_train: SyncSender<TrainTxBatch>,
    // Snapshot for inference (read-only GasInfer)
    inference: Arc<Mutex<Option<Box<GasInfer>>>>,
    // Lock-free per-object tip snapshot for ultra-fast predictions
    store: Arc<ModelStore>,
}

impl InNodeModelUpdater {
    pub fn new() -> Self {
        // Pin Torch to 1 thread to limit CPU under sustained load.
        tch::set_num_threads(1);
        tch::set_num_interop_threads(1);

        // State and learner
        let hist = Arc::new(Mutex::new(HistState::new()));
        let learner = Arc::new(Mutex::new({
            let l = model::GasLearner::new(tch::Device::Cpu).expect("Init GasLearner");
            l.warmup();
            l
        }));
        // Initialize inference snapshot from trainer (best-effort)
        let inference = {
            let l = learner.lock().unwrap();
            Arc::new(Mutex::new(build_infer_from_trainer(&l).map(Box::new)))
        };
        // Bounded channels
        let (tx_update, rx_update): (SyncSender<CpUpdateBatch>, Receiver<CpUpdateBatch>) = mpsc::sync_channel(1024);
        let (tx_train, rx_train): (SyncSender<TrainTxBatch>, Receiver<TrainTxBatch>) = mpsc::sync_channel(1024);
        // Lock-free store for per-object tips
        let store = Arc::new(ModelStore::new());
        // Update worker
        {
            let hist_up = Arc::clone(&hist);
            let store_up = Arc::clone(&store);
            thread::spawn(move || {
                while let Ok(batch) = rx_update.recv() {
                    let (known, touched, last_ms, deltas) = {
                        let mut st = hist_up.lock().unwrap();
                        st.last_global_ms = st.last_global_ms.max(batch.checkpoint_ms);
                        let mut touched: HashSet<ObjectID> = HashSet::new();
                        let mut deltas: Vec<(ObjectID, u64)> = Vec::new();
                        for row in batch.rows {
                            let oid: ObjectID = row.object_id.parse().unwrap_or_else(|_| ObjectID::from_single_byte(0));
                            let feats = st.feat_from_row(oid, &row);
                            st.push_feature_row(oid, feats);
                            touched.insert(oid);
                            // interpret hotness as tip over reference RGP
                            let tip = row.hotness.max(0.0).round() as u64;
                            deltas.push((oid, tip));
                        }
                        (st.histories.keys().cloned().collect::<Vec<_>>(), touched, st.last_global_ms, deltas)
                    };
                    // second pass without holding lock
                    {
                        let mut st = hist_up.lock().unwrap();
                        for oid in known.into_iter().filter(|o| !touched.contains(o)) {
                            if let Some(f) = st.synth_not_touched(oid, last_ms) {
                                st.push_feature_row(oid, f);
                            }
                        }
                    }
                    if !deltas.is_empty() {
                        store_up.publish_deltas(&deltas);
                    }
                }
            });
        }
        // Train worker
        {
            let hist_tr = Arc::clone(&hist);
            let learner_tr = Arc::clone(&learner);
            let inference_tr = Arc::clone(&inference);
            thread::spawn(move || {
                const TRAIN_BATCH_CAP: usize = 16;
                const SNAPSHOT_EVERY_STEPS: usize = 10; // cadence: refresh snapshot every N steps
                let mut steps_since_refresh: usize = 0;
                while let Ok(batch) = rx_train.recv() {
                    // enqueue items
                    {
                        let mut st = hist_tr.lock().unwrap();
                        st.pending_train.extend(batch.items);
                    }
                    loop {
                        // drain up to TRAIN_BATCH_CAP (also flush partials)
                        let items: Vec<TrainTxItem> = {
                            let mut st = hist_tr.lock().unwrap();
                            let n = st.pending_train.len();
                            if n == 0 { Vec::new() } else {
                                let take = n.min(TRAIN_BATCH_CAP);
                                st.pending_train.drain(..take).collect()
                            }
                        };
                        if items.is_empty() { break; }
                        // build tensors
                        let (xs, hs, ys) = {
                            let st = hist_tr.lock().unwrap();
                            let mut xs: Vec<Vec<Tensor>> = Vec::new();
                            let mut hs: Vec<f32> = Vec::new();
                            let mut ys: Vec<f32> = Vec::new();
                            for item in items.iter() {
                                let mut seqs: Vec<Tensor> = Vec::new();
                                let mut h_anchor: f32 = 0.0;
                                let mut ok = true;
                                for oid_s in item.object_ids.iter() {
                                    let oid: ObjectID = match oid_s.parse() { Ok(x) => x, Err(_) => { ok = false; break; } };
                                    if let Some(seq) = st.build_object_seq(&oid) {
                                        if let Some(h) = st.latest_hotness_over_ref(&oid) { h_anchor = h_anchor.max(h); }
                                        seqs.push(seq);
                                    } else { ok = false; break; }
                                }
                                if ok && !seqs.is_empty() {
                                    let y_log = (item.required_price_in_cp as f32 / item.reference_gas_price as f32).ln();
                                    xs.push(seqs); hs.push(h_anchor); ys.push(y_log);
                                }
                            }
                            (xs, hs, ys)
                        };
                        if !xs.is_empty() {
                            if let Ok(mut l) = learner_tr.lock() {
                                let _ = l.train_step(xs, hs, ys);
                                steps_since_refresh += 1;
                                if steps_since_refresh >= SNAPSHOT_EVERY_STEPS {
                                    if let Some(snap) = build_infer_from_trainer(&l) {
                                        if let Ok(mut guard) = inference_tr.lock() { *guard = Some(Box::new(snap)); }
                                    }
                                    steps_since_refresh = 0;
                                }
                            }
                        }
                    }
                }
            });
        }
        Self { hist, learner, tx_update, tx_train, inference, store }
    }

    pub fn predict_for_objects(&self, object_ids: &[ObjectID], reference_gas_price: u64) -> Option<u64> {
        // Build sequences and timing without holding learner lock
        let (seqs, h_anchor, stale_sec_opt) = {
            let st = self.hist.lock().ok()?;
            let mut seqs: Vec<Tensor> = Vec::new();
            let mut h_anchor: f32 = 0.0;
            let mut min_obj_ms: Option<u64> = None;
            for oid in object_ids {
                let seq = match st.build_object_seq(oid) { Some(s) => s, None => return None };
                if let Some(h) = st.latest_hotness_over_ref(oid) { h_anchor = h_anchor.max(h); }
                if let Some(ms) = st.last_ms.get(oid).copied() { min_obj_ms = Some(min_obj_ms.map(|m| m.min(ms)).unwrap_or(ms)); }
                seqs.push(seq);
            }
            let stale_sec_opt = min_obj_ms.map(|ms| ((st.last_global_ms.saturating_sub(ms)) as f32) / 1000.0);
            (seqs, h_anchor, stale_sec_opt)
        };
        // Predict using read-only snapshot (no trainer lock contention). Try non-blocking lock first.
        if let Ok(guard) = self.inference.try_lock() {
            let y_log = if let Some(ref snap) = *guard {
                let (pred, _attn) = snap.predict(&seqs, h_anchor);
                let idx = model::tau_index(model::DEFAULT_TAU) as i64;
                pred.i(idx).double_value(&[]) as f32
            } else {
                0.0
            };
            // Disable inference-time decay for stability under heavy churn.
            let y_log_decayed = y_log;
            let price = (reference_gas_price as f32 * y_log_decayed.exp()).round() as u64;
            Some(price)
        } else {
            // Snapshot is busy; fall back to OGD baseline: RGP + hotness_raw (hotness_over_ref * RGP)
            let hotness_raw = h_anchor * reference_gas_price as f32;
            Some(reference_gas_price.saturating_add(hotness_raw.round() as u64))
        }
    }
}

impl ModelUpdater for InNodeModelUpdater {
    fn post_update(&self, batch: CpUpdateBatch) {
        static DROP_UPDATES: AtomicU64 = AtomicU64::new(0);
        if let Err(_e) = self.tx_update.try_send(batch) {
            let c = DROP_UPDATES.fetch_add(1, Ordering::Relaxed) + 1;
            if c % 1000 == 0 { eprintln!("[in-model] dropped {} update batches (channel full)", c); }
        }
    }
    fn post_train_tx(&self, batch: TrainTxBatch) {
        static DROP_TRAINS: AtomicU64 = AtomicU64::new(0);
        if let Err(_e) = self.tx_train.try_send(batch) {
            let c = DROP_TRAINS.fetch_add(1, Ordering::Relaxed) + 1;
            if c % 1000 == 0 { eprintln!("[in-model] dropped {} train batches (channel full)", c); }
        }
    }
}

// Export aggregator weights from the learner's VarStore.
// (removed) export_agg_params: reverted aggregator export path

impl InNodeModelUpdater {
    /// Returns a non-blocking reader that uses the lock-free snapshot.
    pub fn reader(&self) -> ModelReader { ModelReader(self.store.clone()) }
}

// ================================
// Lock-free per-object weight snapshot (RCU via ArcSwap)
// ================================

#[derive(Clone, Default)]
struct WeightsSnap {
    version: u64,
    per_obj_tip: HashMap<ObjectID, u64>,
}

struct ModelStore {
    shared: ArcSwap<WeightsSnap>,
}

impl ModelStore {
    fn new() -> Self { Self { shared: ArcSwap::from_pointee(WeightsSnap::default()) } }
    fn publish_deltas(&self, deltas: &[(ObjectID, u64)]) {
        let cur = self.shared.load();
        let mut next_map = cur.per_obj_tip.clone();
        for (oid, tip) in deltas.iter().copied() { next_map.insert(oid, tip); }
        let next = WeightsSnap { version: cur.version + 1, per_obj_tip: next_map };
        self.shared.store(Arc::new(next));
    }
    // (removed) publish_embeddings / publish_agg: reverted
}

#[derive(Clone)]
pub struct ModelReader(Arc<ModelStore>);

impl ModelReader {
    /// Ultra-fast, non-blocking prediction: `RGP + max_tip`.
    pub fn predict_for_tx(&self, tx: &TransactionData, reference_gas_price: u64) -> u64 {
        let snap = self.0.shared.load();
        // Compute max tip
        let mut max_tip = 0u64;
        for obj in tx.shared_input_objects().into_iter().filter(|o| o.mutable).map(|o| o.id) {
            if let Some(t) = snap.per_obj_tip.get(&obj) { max_tip = max_tip.max(*t); }
        }
        reference_gas_price.saturating_add(max_tip)
    }
}

// ---------- Pure-Rust aggregator inference ----------
// (removed) gelu/infer_agg: reverted aggregator inference
