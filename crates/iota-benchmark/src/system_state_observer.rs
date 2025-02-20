// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{sync::Arc, time::Duration};

use iota_protocol_config::{Chain, ProtocolConfig, ProtocolVersion};
use iota_types::iota_system_state::iota_system_state_summary::IotaSystemStateSummary;
use tokio::{
    sync::{oneshot::Sender, watch, watch::Receiver},
    time,
    time::Instant,
};
use tracing::{error, info};

use crate::ValidatorProxy;

#[derive(Debug, Clone)]
pub struct SystemState {
    pub reference_gas_price: u64,
    pub protocol_config: Option<ProtocolConfig>,
}

#[derive(Debug)]
pub struct SystemStateObserver {
    pub state: Receiver<SystemState>,
    pub _sender: Sender<()>,
}

impl SystemStateObserver {
    pub fn new(proxy: Arc<dyn ValidatorProxy + Send + Sync>) -> Self {
        let (sender, mut recv) = tokio::sync::oneshot::channel();
        let mut interval = tokio::time::interval_at(Instant::now(), Duration::from_secs(60));
        interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
        let (tx, rx) = watch::channel(SystemState {
            reference_gas_price: 1u64,
            protocol_config: None,
        });
        tokio::task::spawn(async move {
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        match proxy.get_latest_system_state_object().await {
                            Ok(iota_system_state) => {
                                let (protocol_version, reference_gas_price) = match iota_system_state {
                                    IotaSystemStateSummary::V1(v1) => (v1.protocol_version, v1.reference_gas_price),
                                    IotaSystemStateSummary::V2(v2) => (v2.protocol_version, v2.reference_gas_price),
                                    _ => panic!("unsupported IotaSystemStateSummary"),
                                };
                                let p = ProtocolConfig::get_for_version(ProtocolVersion::new(protocol_version), Chain::Unknown);
                                if tx.send(SystemState { reference_gas_price, protocol_config: Some(p)}).is_ok() {
                                    info!("Reference gas price = {:?}", reference_gas_price);
                                }
                            }
                            Err(err) => {
                                error!("Failed to get system state object: {:?}", err);
                            }
                        }
                    }
                    _ = &mut recv => break,
                }
            }
        });
        Self {
            state: rx,
            _sender: sender,
        }
    }
}
