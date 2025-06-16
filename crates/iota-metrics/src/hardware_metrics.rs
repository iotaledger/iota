// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use prometheus::{
    IntGauge, Opts, Registry,
    core::{Collector, Desc, Number},
    proto::{LabelPair, Metric, MetricFamily, MetricType},
};
use sysinfo::{CpuRefreshKind, Disk, Disks, MemoryRefreshKind, RefreshKind, System};

use crate::RegistryService;

#[derive(thiserror::Error, Debug)]
pub enum HardwareMetricsErr {
    #[error("Failed creating metric: {0}")]
    ErrCreateMetric(prometheus::Error),
    #[error("Failed registering hardware metrics onto RegistryService: {0}")]
    ErrRegisterHardwareMetrics(prometheus::Error),
}

/// Register all hardware matrics: CPU specs, Memory specs/usage, Disk
/// specs/usage
/// These metrics are all named with a prefix "hw_"
/// They are both pushed to iota-proxy and exposed on the /metrics endpoint.
pub fn register_hardware_metrics(
    registry_service: &RegistryService,
    db_path: &Path,
) -> Result<(), HardwareMetricsErr> {
    let registry = Registry::new_custom(Some("hw".to_string()), None)
        .map_err(HardwareMetricsErr::ErrRegisterHardwareMetrics)?;
    registry
        .register(Box::new(HardwareMetrics::new(db_path)?))
        .map_err(HardwareMetricsErr::ErrRegisterHardwareMetrics)?;
    registry_service.add(registry);
    Ok(())
}

pub struct HardwareMetrics {
    system: Arc<Mutex<System>>,
    disks: Arc<Mutex<Disks>>,
    // Descriptions for the static metrics
    pub static_descriptions: Vec<Desc>,
    // Static metrics contain metrics that are not expected to change during runtime
    // e.g. CPU model, memory total, disk total, etc.
    pub static_metric_families: Vec<MetricFamily>,
    pub memory_available_collector: IntGauge,
    // Path where the database is mounted (to identify which disk contains the DB)
    pub db_path: PathBuf,
}

impl HardwareMetrics {
    pub fn new(db_path: &Path) -> Result<Self, HardwareMetricsErr> {
        let mut system = System::new_with_specifics(
            RefreshKind::nothing()
                .with_cpu(CpuRefreshKind::nothing())
                .with_memory(MemoryRefreshKind::nothing().with_ram()),
        );
        system.refresh_all();

        let disks = Disks::new_with_refreshed_list();

        Ok(Self {
            static_descriptions: Self::static_descriptions(&system, &disks, db_path)?,
            static_metric_families: Self::static_metric_families(&system, &disks, db_path)?,
            memory_available_collector: Self::memory_available_collector()?,
            system: Arc::new(Mutex::new(system)),
            disks: Arc::new(Mutex::new(disks)),
            db_path: PathBuf::from(db_path),
        })
    }

    pub fn static_descriptions(
        system: &System,
        disks: &Disks,
        db_path: &Path,
    ) -> Result<Vec<Desc>, HardwareMetricsErr> {
        let mut descs: Vec<Desc> = Vec::new();
        for mf in Self::static_metric_families(system, disks, db_path)? {
            descs.push(Self::metric_family_desc(&mf)?);
        }
        Ok(descs)
    }

    pub fn static_metric_families(
        system: &System,
        disks: &Disks,
        db_path: &Path,
    ) -> Result<Vec<MetricFamily>, HardwareMetricsErr> {
        let mut mfs = Vec::new();
        mfs.push(Self::collect_cpu_specs(system));
        mfs.extend(Self::memory_total_collector(system)?.collect());
        for mf in Self::collect_disks_total_bytes(disks, db_path) {
            mfs.push(mf);
        }
        Ok(mfs)
    }

    fn label(name: &str, value: impl ToString) -> LabelPair {
        let mut label = LabelPair::new();
        label.set_name(name.to_string());
        label.set_value(value.to_string());
        label
    }

    fn uint_gauge(
        name: &str,
        help: &str,
        value: u64,
        labels: &[Option<LabelPair>],
    ) -> MetricFamily {
        let mut g = prometheus::proto::Gauge::default();
        let mut m = Metric::default();
        let mut mf = MetricFamily::new();

        g.set_value(value.into_f64());
        m.set_gauge(g);
        m.set_label(
            labels
                .iter()
                .filter_map(|opt| opt.as_ref())
                .cloned()
                .collect::<Vec<_>>(),
        );

        mf.mut_metric().push(m);
        mf.set_name(name.to_string());
        mf.set_help(help.to_string());
        mf.set_field_type(MetricType::GAUGE);
        mf
    }

    fn metric_family_desc(fam: &MetricFamily) -> Result<Desc, HardwareMetricsErr> {
        Desc::new(
            fam.name().to_string(),
            fam.help().to_string(),
            vec![],
            HashMap::new(),
        )
        .map_err(HardwareMetricsErr::ErrCreateMetric)
    }

    fn cpu_vendor_id(system: &System) -> String {
        let vendor_id = system
            .cpus()
            .first()
            .map_or("cpu_vendor_id_unavailable", |cpu| cpu.vendor_id());
        match vendor_id {
            "" => "cpu_vendor_id_unavailable",
            _ => vendor_id,
        }
        .to_string()
    }

    fn cpu_model(system: &System) -> String {
        let brand = system
            .cpus()
            .first()
            .map_or("cpu_model_unavailable", |cpu| cpu.brand());
        match brand {
            "" => "cpu_model_unavailable",
            _ => brand,
        }
        .to_string()
    }

    fn collect_cpu_specs(system: &System) -> MetricFamily {
        Self::uint_gauge(
            "cpu_core_count",
            "CPU core count (and labels: model,vendor_id,arch)",
            system.physical_core_count().unwrap_or_default() as u64,
            &[
                Some(Self::label("model", Self::cpu_model(system))),
                Some(Self::label("vendor_id", Self::cpu_vendor_id(system))),
                Some(Self::label("arch", System::cpu_arch())),
            ],
        )
    }

    // we deactivated collecting CPU usage per core to avoid performance impact
    // fn collect_cpu_usage(system: &System) -> Result<Vec<MetricFamily>,
    // HardwareMetricsErr> { let cpu_usage_per_core: Vec<MetricFamily> =
    // system.cpus()         .iter()
    //         .map(|core| {
    //             let core_name = core.name();
    //             Self::f64gauge(
    //                 format!("cpu_{core_name}_usage"),
    //                 format!("CPU core {core_name} usage in percent"),
    //                 core.cpu_usage() as f64,
    //             )
    //         })
    //         .collect();
    //     Ok(cpu_usage_per_core)
    // }

    fn memory_total_collector(system: &System) -> Result<IntGauge, HardwareMetricsErr> {
        let mem_total_bytes = system.total_memory();
        let memory_total_collector =
            IntGauge::with_opts(Opts::new("memory_total_bytes", "Memory total (bytes)"))
                .map_err(HardwareMetricsErr::ErrCreateMetric)?;
        memory_total_collector.set(mem_total_bytes as i64);
        Ok(memory_total_collector)
    }

    fn memory_available_collector() -> Result<IntGauge, HardwareMetricsErr> {
        IntGauge::with_opts(Opts::new(
            "memory_available_bytes",
            "Memory available (bytes)",
        ))
        .map_err(HardwareMetricsErr::ErrCreateMetric)
    }

    fn collect_memory_available(&self, system: &System) -> Option<Vec<MetricFamily>> {
        let memory_available_bytes = match i64::try_from(system.available_memory()) {
            Ok(bytes) => bytes,
            Err(e) => {
                tracing::error!("Failed converting memory_available_bytes to i64: {e}");
                return None;
            }
        };
        self.memory_available_collector.set(memory_available_bytes);
        Some(self.memory_available_collector.collect())
    }

    fn disk_has_db(disk: &Disk, db_path: &Path) -> bool {
        db_path.starts_with(disk.mount_point())
    }

    fn collect_disk_available(&self, disks: &Disks) -> Vec<MetricFamily> {
        let space_available_per_disk: Vec<MetricFamily> = disks
            .iter()
            .enumerate()
            .map(|(idx, disk)| {
                let disk_name = disk.name().to_string_lossy();
                let disk_num = idx + 1;
                Self::uint_gauge(
                    &format!("disk_{disk_num}_available_bytes",),
                    &format!("Disk space available (bytes), for disk {disk_num}",),
                    disk.available_space(),
                    &[
                        Some(Self::label("disk_name", disk_name.to_string())),
                        if Self::disk_has_db(disk, &self.db_path) {
                            Some(Self::label("is_database_disk", true))
                        } else {
                            None
                        },
                    ],
                )
            })
            .collect();

        space_available_per_disk
    }

    fn collect_disks_total_bytes(disks: &Disks, db_path: &Path) -> Vec<MetricFamily> {
        let total_bytes_per_disk: Vec<MetricFamily> = disks
            .iter()
            .enumerate()
            .map(|(idx, disk)| {
                let disk_name = disk.name().to_string_lossy();
                let disk_num = idx + 1;
                Self::uint_gauge(
                    &format!("disk_{disk_num}_total_bytes",),
                    &format!("Disk space total (bytes), for disk {disk_num}",),
                    disk.total_space(),
                    &[
                        Some(Self::label("disk_name", disk_name.to_string())),
                        if Self::disk_has_db(disk, db_path) {
                            Some(Self::label("is_database_disk", true))
                        } else {
                            None
                        },
                    ],
                )
            })
            .collect();

        total_bytes_per_disk
    }
}

impl Collector for HardwareMetrics {
    fn desc(&self) -> Vec<&Desc> {
        self.static_descriptions.iter().collect()
    }

    fn collect(&self) -> Vec<MetricFamily> {
        let mut system = match self.system.lock() {
            Ok(lock) => lock,
            Err(e) => {
                tracing::error!("Failed acquiring lock on System: Lock is poisoned: {e}");
                return Vec::new();
            }
        };
        system.refresh_memory();

        let mut disks = match self.disks.lock() {
            Ok(lock) => lock,
            Err(e) => {
                tracing::error!("Failed acquiring lock on Disks: Lock is poisoned: {e}");
                return Vec::new();
            }
        };
        disks.refresh(true);

        let mut mfs = self.static_metric_families.clone();
        if let Some(families) = self.collect_memory_available(&system) {
            mfs.extend(families);
        };

        mfs.extend(self.collect_disk_available(&disks));

        mfs
    }
}

#[cfg(test)]
mod tests {
    use std::{
        net::SocketAddrV4,
        path::PathBuf,
        sync::LazyLock,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    static DB_PATH: LazyLock<PathBuf> = LazyLock::new(|| PathBuf::from("/opt/iota/db"));

    #[tokio::test]
    async fn test_collect_hardware_specs() -> Result<(), String> {
        let prom_server_addr: SocketAddrV4 = "0.0.0.0:9194".parse().unwrap();

        let registry_svc = crate::start_prometheus_server(prom_server_addr.into());

        register_hardware_metrics(&registry_svc, &DB_PATH)
            .expect("Failed registering hardware metrics");

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        let mut metric_families = registry_svc.gather_all();
        for mf in metric_families.iter_mut() {
            for m in mf.mut_metric() {
                m.set_timestamp_ms(now);
            }
        }

        let find_metric = |family_name: &str| -> Result<&Metric, String> {
            let fname_namespaced = format!("hw_{}", family_name.trim_start_matches("hw_"));
            let metric = metric_families
                .iter()
                .find(|mf| mf.name() == fname_namespaced)
                .ok_or_else(|| format!("Metric family not found: {fname_namespaced}"))?
                .get_metric()
                .first()
                .ok_or_else(|| format!("No metrics in family {fname_namespaced}"))?;
            Ok(metric)
        };
        let find_metric_label = |family_name: &str, label_name: &str| -> Result<String, String> {
            let metric = find_metric(family_name)?;
            Ok(metric
                .get_label()
                .iter()
                .find(|l| l.name() == label_name)
                .ok_or_else(|| format!("Label not found: {label_name}"))?
                .value()
                .to_string())
        };

        let cpu_core_count = find_metric("cpu_core_count")?;
        let core_count: usize = cpu_core_count.get_gauge().value() as usize;
        assert!(core_count > 0 && core_count < 513);

        // we only check specs are present in labels
        let _ = find_metric_label("cpu_core_count", "model")?;
        let _ = find_metric_label("cpu_core_count", "vendor_id")?;
        let _ = find_metric_label("cpu_core_count", "arch")?;

        let mem_total_bytes = find_metric("memory_total_bytes")?.get_gauge().value();
        assert!(mem_total_bytes > 0.0);
        let mem_available_bytes = find_metric("memory_available_bytes")?.get_gauge().value();
        assert!(mem_available_bytes > 0.0);

        let disk_1_total_bytes = find_metric("disk_1_total_bytes")?;
        assert!(disk_1_total_bytes.get_gauge().value() > 0.0);
        let disk_available = find_metric("disk_1_available_bytes")?;
        assert!(disk_available.get_gauge().value() > 0.0);

        Ok(())
    }
}
