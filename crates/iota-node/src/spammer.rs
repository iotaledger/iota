// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use iota_core::{
    authority::{AuthorityState, authority_per_epoch_store::AuthorityPerEpochStore},
    consensus_adapter::ConsensusAdapter,
};
use iota_types::messages_consensus::ConsensusTransaction;
use serde::Serialize;
use tokio::{
    sync::{Notify, RwLock},
    task::JoinHandle,
    time::{self, MissedTickBehavior},
};
use tracing::{info, warn};

#[derive(Clone, Debug)]
pub struct SpammerConfig {
    pub target_tps: u64,
    pub mean_size_bytes: usize,
    pub std_dev_size_bytes: usize,
}

impl SpammerConfig {
    pub fn new(target_tps: u64, mean_size_bytes: usize, std_dev_size_bytes: Option<usize>) -> Self {
        let std_dev = std_dev_size_bytes.unwrap_or(mean_size_bytes / 10);
        Self {
            target_tps,
            mean_size_bytes,
            std_dev_size_bytes: std_dev,
        }
    }
}

#[derive(Clone, Serialize)]
pub struct SpammerStatus {
    pub enabled: bool,
    pub tps: u64,
    pub mean_size: usize,
    pub std_dev_size: usize,
    pub submitted: u64,
    pub errors: u64,
}

pub struct SpammerService {
    state: Arc<AuthorityState>,
    consensus_adapter: Arc<ConsensusAdapter>,
    config: Arc<RwLock<Option<SpammerConfig>>>,
    config_changed: Arc<Notify>,
    num_submitted: Arc<AtomicU64>,
    num_errors: Arc<AtomicU64>,
}

impl SpammerService {
    pub fn new(state: Arc<AuthorityState>, consensus_adapter: Arc<ConsensusAdapter>) -> Self {
        Self {
            state,
            consensus_adapter,
            config: Arc::new(RwLock::new(None)),
            config_changed: Arc::new(Notify::new()),
            num_submitted: Arc::new(AtomicU64::new(0)),
            num_errors: Arc::new(AtomicU64::new(0)),
        }
    }

    pub async fn start(&self, config: SpammerConfig) {
        info!(
            "Starting spammer: tps={}, mean_size={}, std_dev={}",
            config.target_tps, config.mean_size_bytes, config.std_dev_size_bytes
        );

        // Update config
        *self.config.write().await = Some(config);

        // Notify the restart loop that config changed
        self.config_changed.notify_one();
    }

    pub async fn stop(&self) {
        info!("Stopping spammer");

        // Clear config (signals disabled)
        *self.config.write().await = None;

        // Notify the task to check config
        self.config_changed.notify_one();
    }

    pub async fn get_status(&self) -> SpammerStatus {
        let config = self.config.read().await;

        match config.as_ref() {
            Some(cfg) => SpammerStatus {
                enabled: true,
                tps: cfg.target_tps,
                mean_size: cfg.mean_size_bytes,
                std_dev_size: cfg.std_dev_size_bytes,
                submitted: self.num_submitted.load(Ordering::Relaxed),
                errors: self.num_errors.load(Ordering::Relaxed),
            },
            None => SpammerStatus {
                enabled: false,
                tps: 0,
                mean_size: 0,
                std_dev_size: 0,
                submitted: self.num_submitted.load(Ordering::Relaxed),
                errors: self.num_errors.load(Ordering::Relaxed),
            },
        }
    }

    pub fn spawn_spammer_loop(self: Arc<Self>) -> JoinHandle<()> {
        let self_clone = self.clone();
        tokio::spawn(async move {
            self_clone.run_with_epoch_restart().await;
        })
    }

    async fn run_with_epoch_restart(&self) {
        loop {
            // Wait for config to be set (non-None)
            loop {
                let config = self.config.read().await;
                if config.is_some() {
                    break;
                }
                drop(config);

                // Wait for config change notification
                self.config_changed.notified().await;
            }

            // Get the current epoch store
            let epoch_store = self.state.load_epoch_store_one_call_per_task();
            let epoch = epoch_store.epoch();

            info!("Starting spammer task for epoch {}", epoch);

            // Run epoch-specific task
            let result = epoch_store
                .within_alive_epoch(self.run_spammer_for_epoch(epoch_store.clone()))
                .await;

            match result {
                Err(()) => {
                    // Epoch ended, check if still enabled
                    let config = self.config.read().await;
                    if config.is_some() {
                        info!("Epoch {} ended, restarting spammer for new epoch", epoch);
                        // Loop continues to restart with new epoch
                    } else {
                        info!("Epoch {} ended and spammer is disabled, exiting", epoch);
                        break;
                    }
                }
                Ok(()) => {
                    // Task exited normally (config was set to None)
                    info!("Spammer task exited normally");
                    break;
                }
            }
        }
    }

    async fn run_spammer_for_epoch(&self, epoch_store: Arc<AuthorityPerEpochStore>) {
        loop {
            // Get current config
            let config = {
                let cfg = self.config.read().await;
                match cfg.as_ref() {
                    Some(c) => c.clone(),
                    None => {
                        // Config cleared, exit task
                        return;
                    }
                }
            };

            // Calculate interval from TPS
            let delay_micros = if config.target_tps > 0 {
                1_000_000 / config.target_tps
            } else {
                1_000_000 // Default to 1 second if TPS is 0
            };

            let mut interval = time::interval(Duration::from_micros(delay_micros));
            interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

            // Run submission loop
            loop {
                tokio::select! {
                    _ = self.config_changed.notified() => {
                        // Config changed, check if disabled or need to update interval
                        let new_config = self.config.read().await;
                        if new_config.is_none() {
                            // Disabled, exit task
                            return;
                        }
                        // Config updated, break inner loop to recreate interval
                        break;
                    }
                    _ = interval.tick() => {
                        // Generate random bytes with normal distribution
                        let size = Self::sample_normal_distribution(
                            config.mean_size_bytes as f64,
                            config.std_dev_size_bytes as f64,
                        );
                         let size = size.max(8.0) as usize; // Ensure at least 8 bytes

                        // 1. Generate random bytes of given size
                        let mut bytes = Self::generate_random_bytes(size);

                        // 2. Generate timestamp
                        let timestamp = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_micros() as u64;

                        // 3. Convert timestamp to 8 bytes
                        let ts_bytes = timestamp.to_be_bytes();

                        // 4. Overwrite first 8 bytes
                        bytes[..8].copy_from_slice(&ts_bytes);

                        // Create and submit transaction
                        let tx = ConsensusTransaction::new_random_bytes(bytes);

                        match self.consensus_adapter.submit(tx, None, &epoch_store) {
                            Ok(_) => {
                                self.num_submitted.fetch_add(1, Ordering::Relaxed);
                            }
                            Err(e) => {
                                self.num_errors.fetch_add(1, Ordering::Relaxed);
                                warn!("Failed to submit RandomBytes transaction: {}", e);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Generate random bytes of specified size using thread_rng
    fn generate_random_bytes(size: usize) -> Vec<u8> {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        (0..size).map(|_| rng.gen::<u8>()).collect()
    }

    /// Sample from normal distribution using Box-Muller transform
    /// Returns a value from N(mean, std_dev^2)
    fn sample_normal_distribution(mean: f64, std_dev: f64) -> f64 {
        use std::f64::consts::PI;

        use rand::Rng;

        let mut rng = rand::thread_rng();

        // Box-Muller transform to generate normally distributed random variable
        let u1: f64 = rng.gen::<f64>();
        let u2: f64 = rng.gen::<f64>();

        // Avoid log(0)
        let u1 = if u1 < f64::EPSILON { f64::EPSILON } else { u1 };

        let z0: f64 = (-2.0 * u1.ln()).sqrt() * (2.0 * PI * u2).cos();

        mean + std_dev * z0
    }
}
