// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeMap, sync::Arc};

use async_trait::async_trait;
use prometheus::Registry;

use crate::{
    drivers::Interval,
    system_state_observer::SystemStateObserver,
    workloads::{GroupID, WorkloadInfo},
    ValidatorProxy,
};

#[async_trait]
pub trait Driver<T> {
    async fn run(
        &self,
        proxies: Vec<Arc<dyn ValidatorProxy + Send + Sync>>,
        workloads_by_group_id: BTreeMap<GroupID, Vec<WorkloadInfo>>,
        system_state_observer: Arc<SystemStateObserver>,
        registry: &Registry,
        show_progress: bool,
        run_duration: Interval,
    ) -> Result<T, anyhow::Error>;
}
