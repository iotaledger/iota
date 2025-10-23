// model posting / training logic for congestion tracking.

use serde::Serialize;
use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use iota_types::base_types::ObjectID;

/// Default base URL for the predictor service.
pub const DEFAULT_PREDICTOR_BASE_URL: &str = "http://predictor:9666";
/// Default JSONL audit log path (created/appended in current working dir).
pub const AUDIT_LOG_PATH: &str = "congestion_audit.jsonl";
/// Minimum reference gas price used in required-price lower bound.
pub const MIN_REFERENCE_GAS_PRICE: u64 = 1000;

/// Rows describing per-object state used by the predictor's /update endpoint.
/// Note: This mirrors the shape currently produced inside CongestionTracker.
#[derive(Debug, Clone, Serialize)]
pub struct CpUpdateRow {
    pub checkpoint_ms: u64,
    pub reference_gas_price: u64,
    pub object_id: String,

    // CongestionInfo snapshot (post-update if present)
    pub latest_congestion_time: Option<u64>,
    pub highest_congestion_gas_price: u64,
    pub latest_clearing_time: Option<u64>,
    pub lowest_clearing_gas_price: u64,
    pub hotness: f64,

    // Per-checkpoint flags & counts for the object
    pub was_touched_in_cp: bool,
    pub was_congested_in_cp: bool,
    pub was_cleared_in_cp: bool,
    pub congested_tx_count_in_cp: u32,
    pub clearing_tx_count_in_cp: u32,

    // Convenience normalized helpers
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

/// Snapshot of per-object state needed to compose update rows.
#[derive(Debug, Clone)]
pub struct ObjectSnapshot {
    pub latest_congestion_time: Option<u64>,
    pub highest_congestion_gas_price: u64,
    pub latest_clearing_time: Option<u64>,
    pub lowest_clearing_gas_price: u64,
    pub hotness: f64,
}

/// Per-checkpoint object stats derived from txs in this checkpoint.
#[derive(Debug, Clone, Default)]
pub struct ObjectCheckpointStats {
    pub was_congested: bool,
    pub was_cleared: bool,
    pub congested_tx_count: u32,
    pub clearing_tx_count: u32,
}

/// Build a CpUpdateBatch from touched object ids, snapshots and per-checkpoint stats.
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
            let highest_cong = s.highest_congestion_gas_price;
            let lowest_clear = s.lowest_clearing_gas_price;
            rows.push(CpUpdateRow {
                checkpoint_ms,
                reference_gas_price,
                object_id: oid.to_string(),

                latest_congestion_time: s.latest_congestion_time,
                highest_congestion_gas_price: highest_cong,
                latest_clearing_time: s.latest_clearing_time,
                lowest_clearing_gas_price: lowest_clear,
                hotness: hot,

                was_touched_in_cp: true,
                was_congested_in_cp: st.was_congested,
                was_cleared_in_cp: st.was_cleared,
                congested_tx_count_in_cp: st.congested_tx_count,
                clearing_tx_count_in_cp: st.clearing_tx_count,

                hotness_over_ref: hot / reference_gas_price as f64,
                highest_congestion_over_ref: if highest_cong > 0 {
                    highest_cong as f64 / reference_gas_price as f64
                } else {
                    0.0
                },
                lowest_clearing_over_ref: if lowest_clear > 0 {
                    lowest_clear as f64 / reference_gas_price as f64
                } else {
                    0.0
                },
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

/// Build a TrainTxBatch from raw txs and per-object min clearing prices for this checkpoint.
pub fn build_train_tx_batch(
    checkpoint_ms: u64,
    reference_gas_price: u64,
    raw_txs: &[RawTxItem],
    per_object_min_clearing_in_cp: &HashMap<ObjectID, u64>,
) -> Option<TrainTxBatch> {
    let mut items = Vec::new();
    for tx in raw_txs {
        let oids: Vec<String> = tx.touched_objects.iter().map(|o| o.to_string()).collect();
        if oids.is_empty() {
            continue;
        }

        let required = if tx.is_congested {
            tx.gas_price_feedback
                .unwrap_or(tx.gas_price)
                .max(MIN_REFERENCE_GAS_PRICE)
        } else {
            let mut req = MIN_REFERENCE_GAS_PRICE;
            for oid in &tx.touched_objects {
                if let Some(min_clear) = per_object_min_clearing_in_cp.get(oid) {
                    if *min_clear > MIN_REFERENCE_GAS_PRICE {
                        req = req.max(*min_clear);
                    }
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

/// Trait abstraction for sending model-related updates.
///
/// Implementations should be resilient: never panic on send errors and prefer
/// logging/metrics over propagating failures.
pub trait ModelUpdater: Send + Sync {
    /// Post a batch of per-object updates to the predictor. Should send even if empty.
    fn post_update(&self, batch: CpUpdateBatch);

    /// Post a batch of tx-level training items to the predictor.
    fn post_train_tx(&self, batch: TrainTxBatch);
}

/// No-op implementation useful for tests or when disabled by config.
pub struct NoopModelUpdater;

impl ModelUpdater for NoopModelUpdater {
    fn post_update(&self, batch: CpUpdateBatch) {
        println!(
            "[congestion/noop] skip POST /update (rows={})",
            batch.rows.len()
        );
    }

    fn post_train_tx(&self, batch: TrainTxBatch) {
        println!(
            "[congestion/noop] skip POST /train_tx (items={})",
            batch.items.len()
        );
    }
}

/// HTTP implementation using reqwest, with async-or-blocking fallback depending
/// on the presence of a Tokio runtime.
pub struct HttpModelUpdater {
    base_url: String,
}

impl HttpModelUpdater {
    pub fn new<S: Into<String>>(base_url: S) -> Self {
        Self {
            base_url: base_url.into(),
        }
    }

    fn update_url(&self) -> String {
        format!("{}/update", self.base_url.trim_end_matches('/'))
    }

    fn train_tx_url(&self) -> String {
        format!("{}/train_tx", self.base_url.trim_end_matches('/'))
    }
}

impl Default for HttpModelUpdater {
    fn default() -> Self {
        Self::new(DEFAULT_PREDICTOR_BASE_URL)
    }
}

impl ModelUpdater for HttpModelUpdater {
    fn post_update(&self, batch: CpUpdateBatch) {
        let url = self.update_url();
        let rows = batch.rows.len();

        if let Ok(_h) = tokio::runtime::Handle::try_current() {
            // Prefer async if a runtime exists.
            tokio::spawn(async move {
                let client = reqwest::Client::new();
                match client.post(&url).json(&batch).send().await {
                    Ok(resp) if resp.status().is_success() => {
                        println!("[congestion] POST /update ok (rows={rows})");
                    }
                    Ok(resp) => {
                        println!(
                            "[congestion] POST /update failed: status={} (rows={rows})",
                            resp.status()
                        );
                    }
                    Err(e) => {
                        println!("[congestion] POST /update error: {e} (rows={rows})");
                    }
                }
            });
        } else {
            // Fallback to a blocking client on a background thread.
            std::thread::spawn(move || {
                let client = reqwest::blocking::Client::new();
                match client.post(&url).json(&batch).send() {
                    Ok(resp) if resp.status().is_success() => {
                        println!("[congestion/blocking] POST /update ok (rows={rows})");
                    }
                    Ok(resp) => {
                        println!(
                            "[congestion/blocking] POST /update failed: status={} (rows={rows})",
                            resp.status()
                        );
                    }
                    Err(e) => {
                        println!("[congestion/blocking] POST /update error: {e} (rows={rows})");
                    }
                }
            });
        }
    }

    fn post_train_tx(&self, batch: TrainTxBatch) {
        let url = self.train_tx_url();
        let items = batch.items.len();

        if let Ok(_h) = tokio::runtime::Handle::try_current() {
            tokio::spawn(async move {
                let client = reqwest::Client::new();
                match client.post(&url).json(&batch).send().await {
                    Ok(resp) if resp.status().is_success() => {
                        println!("[congestion] POST /train_tx ok (items={items})");
                    }
                    Ok(resp) => {
                        println!(
                            "[congestion] POST /train_tx failed: status={} (items={items})",
                            resp.status()
                        );
                    }
                    Err(e) => {
                        println!("[congestion] POST /train_tx error: {e} (items={items})");
                    }
                }
            });
        } else {
            std::thread::spawn(move || {
                let client = reqwest::blocking::Client::new();
                match client.post(&url).json(&batch).send() {
                    Ok(resp) if resp.status().is_success() => {
                        println!(
                            "[congestion/blocking] POST /train_tx ok (items={items})"
                        );
                    }
                    Ok(resp) => {
                        println!(
                            "[congestion/blocking] POST /train_tx failed: status={} (items={items})",
                            resp.status()
                        );
                    }
                    Err(e) => {
                        println!(
                            "[congestion/blocking] POST /train_tx error: {e} (items={items})"
                        );
                    }
                }
            });
        }
    }
}

// -------------------------------
// Audit JSONL support
// -------------------------------

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

/// Raw tx representation used to derive audit rows.
#[derive(Debug, Clone)]
pub struct RawTxItem {
    pub tx_digest: String,
    pub is_congested: bool,
    pub gas_price: u64,
    pub gas_price_feedback: Option<u64>,
    pub touched_objects: Vec<ObjectID>,
}

/// Helper for stable default of the object-id to skip in audit.
pub fn default_skip_audit_object_id() -> ObjectID {
    ObjectID::from_single_byte(0x6)
}

/// Build audit rows based on raw tx records and per-object min clearing price in the checkpoint.
pub fn build_tx_audit_rows(
    raw_txs: &[RawTxItem],
    per_object_min_clearing_in_cp: &HashMap<ObjectID, u64>,
    reference_gas_price: u64,
    checkpoint_id: u64,
    skip_object: Option<ObjectID>,
) -> Vec<TxAuditRow> {
    let mut out = Vec::with_capacity(raw_txs.len());
    for tx in raw_txs {
        if let Some(skip) = skip_object {
            if tx.touched_objects.len() == 1 && tx.touched_objects[0] == skip {
                continue;
            }
        }

        // required price: congested -> feedback (or gas price) ;
        // non-congested -> max per-object min clearing price observed in CP.
        let required = if tx.is_congested {
            tx.gas_price_feedback
                .unwrap_or(tx.gas_price)
                .max(MIN_REFERENCE_GAS_PRICE)
        } else {
            let mut req = MIN_REFERENCE_GAS_PRICE;
            for oid in &tx.touched_objects {
                if let Some(min_clear) = per_object_min_clearing_in_cp.get(oid) {
                    if *min_clear > MIN_REFERENCE_GAS_PRICE {
                        req = req.max(*min_clear);
                    }
                }
            }
            req
        };

        let overpay = tx.gas_price.saturating_sub(required);
        out.push(TxAuditRow {
            checkpoint_id,
            reference_gas_price,
            tx_digest: tx.tx_digest.clone(),
            is_congested: tx.is_congested,
            gas_price: tx.gas_price,
            gas_price_feedback: tx.gas_price_feedback,
            touched_objects: tx
                .touched_objects
                .iter()
                .map(|o| o.to_string())
                .collect(),
            required_price_in_cp: required,
            overpay,
        });
    }
    out
}

/// Lightweight JSONL writer with an internal mutex; safe to share.
pub struct AuditLogger {
    writer: Option<Arc<Mutex<File>>>,
}

impl AuditLogger {
    /// Opens (or creates) a JSONL file at `path` for append.
    pub fn new_with_path(path: PathBuf) -> Self {
        let writer = match OpenOptions::new().create(true).append(true).open(&path) {
            Ok(f) => Some(Arc::new(Mutex::new(f))),
            Err(e) => {
                eprintln!("[congestion/audit] failed to open {:?}: {e}", path);
                None
            }
        };
        Self { writer }
    }

    /// Opens (or creates) the default audit file `AUDIT_LOG_PATH` for append.
    pub fn new_default() -> Self {
        Self::new_with_path(PathBuf::from(AUDIT_LOG_PATH))
    }

    /// Writes each row as a serialized JSON line; logs errors and continues.
    pub fn write_rows(&self, rows: &[TxAuditRow]) {
        if let Some(w) = &self.writer {
            let mut guard = w.lock().unwrap();
            for row in rows {
                match serde_json::to_string(row) {
                    Ok(line) => {
                        if let Err(e) = writeln!(guard, "{}", line) {
                            eprintln!("[congestion/audit] write error: {e}");
                            break;
                        }
                    }
                    Err(e) => {
                        eprintln!("[congestion/audit] serialize error: {e}");
                    }
                }
            }
            let _ = guard.flush();
        }
    }
}
