// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use prometheus::{IntGauge, Registry, register_int_gauge_with_registry};
use tap::Pipe;

#[derive(Clone)]
pub(super) struct Metrics(Option<Arc<Inner>>);

impl std::fmt::Debug for Metrics {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        fmt.debug_struct("Metrics").finish()
    }
}

impl Metrics {
    pub fn enabled(registry: &Registry) -> Self {
        Metrics(Some(Inner::new(registry)))
    }

    pub fn disabled() -> Self {
        Metrics(None)
    }

    pub fn inc_num_peers_with_external_address(&self) {
        if let Some(inner) = &self.0 {
            inner.num_peers_with_external_address.inc();
        }
    }

    pub fn dec_num_peers_with_external_address(&self) {
        if let Some(inner) = &self.0 {
            inner.num_peers_with_external_address.dec();
        }
    }
}

struct Inner {
    num_peers_with_external_address: IntGauge,
}

impl Inner {
    pub fn new(registry: &Registry) -> Arc<Self> {
        Self {
            num_peers_with_external_address: register_int_gauge_with_registry!(
                "num_peers_with_external_address",
                "Number of peers with an external address configured for discovery",
                registry
            )
            .unwrap(),
        }
        .pipe(Arc::new)
    }
}
