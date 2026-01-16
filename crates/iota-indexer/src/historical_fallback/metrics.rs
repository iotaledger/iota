// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_storage::http_key_value_store::ItemType;
use prometheus::{IntCounterVec, Registry, register_int_counter_vec_with_registry};

#[derive(Clone)]
pub struct HistoricalFallbackClientMetrics {
    pub(crate) cache_hits: IntCounterVec,
    pub(crate) cache_misses: IntCounterVec,
}

impl HistoricalFallbackClientMetrics {
    pub fn new(registry: &Registry) -> Self {
        Self {
            cache_hits: register_int_counter_vec_with_registry!(
                "historical_fallback_cache_hits",
                "Historical fallback cache hits",
                &["resource"],
                registry,
            )
            .unwrap(),
            cache_misses: register_int_counter_vec_with_registry!(
                "historical_fallback_cache_misses",
                "Historical fallback cache misses",
                &["resource"],
                registry,
            )
            .unwrap(),
        }
    }

    pub(crate) fn record_cache_hit(&self, item_type: ItemType) {
        self.cache_hits
            .with_label_values(&[item_type.to_string()])
            .inc();
    }

    pub(crate) fn record_cache_miss(&self, item_type: ItemType) {
        self.cache_misses
            .with_label_values(&[item_type.to_string()])
            .inc();
    }
}
