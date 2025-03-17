// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex},
};

use prometheus::{
    IntGauge, Opts,
    core::{Collector, Desc, Number},
    proto::{LabelPair, Metric, MetricFamily, MetricType},
};
use sysinfo::{CpuRefreshKind, Disk, Disks, MemoryRefreshKind, RefreshKind, System};

use crate::RegistryService;

pub fn register_hardware_metrics(
    registry_service: &RegistryService,
    db_path: &Path,
) -> Result<(), HardwareMetricsErr> {
    registry_service
        .default_registry
        .register(Box::new(HardwareMetrics::new(db_path)?))
        .map_err(HardwareMetricsErr::ErrRegisterHardwareMetrics)
}

pub struct HardwareMetrics {
    system: Arc<Mutex<System>>,
    disks: Arc<Mutex<Disks>>,
    pub static_metric_families: Vec<MetricFamily>,
    pub static_descriptions: Vec<Desc>,
    pub memory_available_collector: IntGauge,
}
impl HardwareMetrics {
    pub fn new(db_path: &Path) -> Result<Self, HardwareMetricsErr> {
        let mut system = System::new_with_specifics(
            RefreshKind::nothing()
                .with_cpu(CpuRefreshKind::nothing().with_cpu_usage())
                .with_memory(MemoryRefreshKind::nothing().with_ram()),
        );
        system.refresh_all();

        let disks = Disks::new_with_refreshed_list();
        let (_, db_disk) = Self::find_db_disk(&disks, db_path);

        Ok(Self {
            static_metric_families: Self::static_metric_families(&system, db_disk)?,
            static_descriptions: Self::static_descriptions(&system, db_disk)?,
            memory_available_collector: Self::memory_available_collector()?,
            system: Arc::new(Mutex::new(system)),
            disks: Arc::new(Mutex::new(disks)),
        })
    }

    pub fn static_descriptions(
        system: &System,
        db_disk: Option<&Disk>,
    ) -> Result<Vec<Desc>, HardwareMetricsErr> {
        let mut descs: Vec<Desc> = Vec::new();
        descs.push(Self::metric_family_desc(&Self::collect_cpu_specs(system))?);
        descs.extend(
            Self::memory_specs_collector(system)?
                .desc()
                .into_iter()
                .cloned(),
        );
        descs.extend(
            Self::disk_specs_collector(db_disk)?
                .desc()
                .into_iter()
                .cloned(),
        );
        descs.push(Self::metric_family_desc(&Self::collect_system_info())?);
        Ok(descs)
    }
    pub fn static_metric_families(
        system: &System,
        db_disk: Option<&Disk>,
    ) -> Result<Vec<MetricFamily>, HardwareMetricsErr> {
        let mut mfs = Vec::new();
        mfs.push(Self::collect_cpu_specs(system));
        mfs.extend(Self::memory_specs_collector(system)?.collect());
        mfs.extend(Self::disk_specs_collector(db_disk)?.collect());
        mfs.push(Self::collect_system_info());
        Ok(mfs)
    }

    fn label(name: &str, value: impl ToString) -> LabelPair {
        let mut label = LabelPair::new();
        label.set_name(name.to_string());
        label.set_value(value.to_string());
        label
    }
    fn f64gauge(name: String, help: String, value: f64) -> MetricFamily {
        let mut g = prometheus::proto::Gauge::default();
        let mut m = Metric::default();
        let mut mf = MetricFamily::new();

        g.set_value(value);
        m.set_gauge(g);

        mf.mut_metric().push(m);
        mf.set_name(name);
        mf.set_help(help);
        mf.set_field_type(MetricType::GAUGE);
        mf
    }
    fn uint_gauge(name: String, help: String, value: u64, labels: &[(&str, &str)]) -> MetricFamily {
        let mut g = prometheus::proto::Gauge::default();
        let mut m = Metric::default();
        let mut mf = MetricFamily::new();

        g.set_value(value.into_f64());
        m.set_gauge(g);
        m.set_label(
            labels
                .iter()
                .map(|(k, v)| Self::label(k, v))
                .collect::<Vec<_>>()
                .into(),
        );

        mf.mut_metric().push(m);
        mf.set_name(name);
        mf.set_help(help);
        mf.set_field_type(MetricType::GAUGE);
        mf
    }
    fn metric_family_desc(fam: &MetricFamily) -> Result<Desc, HardwareMetricsErr> {
        Desc::new(
            fam.get_name().to_string(),
            fam.get_help().to_string(),
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
    fn cpu_core_count(system: &System) -> Option<usize> {
        system.physical_core_count()
    }
    fn collect_cpu_specs(system: &System) -> MetricFamily {
        let mut metric = Metric::new();
        metric.set_label({
            vec![
                Self::label("cpu_model", Self::cpu_model(system)),
                Self::label("cpu_vendor_id", Self::cpu_vendor_id(system)),
                Self::label(
                    "cpu_core_count",
                    Self::cpu_core_count(system).map_or_else(
                        || "cpu_core_count_unavailable".to_string(),
                        |c| c.to_string(),
                    ),
                ),
                Self::label("cpu_arch", System::cpu_arch()),
            ]
            .into()
        });

        let mut mf: MetricFamily = MetricFamily::new();
        mf.set_name("cpu_specs".to_owned());
        mf.set_help("CPU specs (brand,vendor,cores)".to_owned());
        mf.set_field_type(prometheus::proto::MetricType::COUNTER);
        mf.set_metric(vec![metric].into());
        mf
    }

    fn collect_cpu_usage(system: &System) -> Result<Vec<MetricFamily>, HardwareMetricsErr> {
        let cpu_usage_per_core: Vec<MetricFamily> = system
            .cpus()
            .iter()
            .map(|core| {
                let thread_name = core.name();
                Self::f64gauge(
                    format!("cpu_{thread_name}_usage"),
                    format!("CPU thread {thread_name} usage in percent"),
                    core.cpu_usage() as f64,
                )
            })
            .collect();
        Ok(cpu_usage_per_core)
    }

    fn memory_specs_collector(system: &System) -> Result<IntGauge, HardwareMetricsErr> {
        let mem_total_bytes = system.total_memory();
        let memory_specs_collector = IntGauge::with_opts(
            Opts::new(
                "memory_specs",
                "Memory specs (constants: total amount, ...)",
            )
            .const_label("memory_total_bytes", mem_total_bytes.to_string()),
        )
        .map_err(HardwareMetricsErr::ErrCreateMetric)?;
        memory_specs_collector.set(mem_total_bytes as i64);
        Ok(memory_specs_collector)
    }

    fn memory_available_collector() -> Result<IntGauge, HardwareMetricsErr> {
        IntGauge::with_opts(Opts::new("memory_available", "Memory available (bytes)"))
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

    fn find_db_disk<'a>(disks: &'a Disks, db_path: &Path) -> (Option<usize>, Option<&'a Disk>) {
        for (idx, disk) in disks.iter().enumerate() {
            if db_path.starts_with(disk.mount_point()) {
                return (Some(idx), Some(disk));
            }
        }
        (None, None)
    }
    fn disk_specs_collector(db_disk: Option<&Disk>) -> Result<IntGauge, HardwareMetricsErr> {
        let disk_total_bytes: Option<u64> = db_disk.map(|d| d.total_space());
        let disk_specs_collector = IntGauge::with_opts(
            Opts::new(
                "db_disk_specs",
                "Disk specifications (total disk space, ...)",
            )
            .const_label(
                "db_disk_total_bytes",
                match disk_total_bytes {
                    Some(bytes) => bytes.to_string(),
                    None => "disk_total_bytes_unavailable".to_string(),
                },
            )
            .const_label(
                "db_disk_name",
                match db_disk {
                    Some(db_disk) => db_disk.name().to_string_lossy().to_string(),
                    None => "db_disk_unknown".to_string(),
                },
            ),
        )
        .map_err(HardwareMetricsErr::ErrCreateMetric)?;
        disk_specs_collector.set(disk_total_bytes.unwrap_or(0) as i64);
        Ok(disk_specs_collector)
    }

    fn collect_disk_available(&self) -> Result<Vec<MetricFamily>, HardwareMetricsErr> {
        let mut disks = self
            .disks
            .lock()
            .map_err(|e| HardwareMetricsErr::GetLock(e.to_string()))?;

        disks.refresh(true);

        let space_available_per_disk: Vec<MetricFamily> = disks
            .iter()
            .enumerate()
            .map(|(idx, disk)| {
                let disk_name = disk.name().to_string_lossy();
                let disk_num = idx + 1;
                Self::uint_gauge(
                    format!("disk_{disk_num}_available_bytes",),
                    format!("Disk available space in bytes, for disk {disk_num}",),
                    disk.available_space(),
                    &[("disk_name", &disk_name)],
                )
            })
            .collect();

        Ok(space_available_per_disk)
    }

    fn collect_system_info() -> MetricFamily {
        let mut metric = Metric::new();
        metric.set_label({
            vec![
                Self::label("is_docker", Self::is_running_in_docker().to_string()),
                Self::label(
                    "os_version",
                    System::long_os_version()
                        .unwrap_or_else(|| "os_version_unavailable".to_string()),
                ),
            ]
            .into()
        });

        let mut mf = MetricFamily::new();
        mf.set_name("system_info".to_owned());
        mf.set_help("System info (OS, version, is_docker, ...)".to_owned());
        mf.set_field_type(prometheus::proto::MetricType::COUNTER);
        mf.set_metric(vec![metric].into());
        mf
    }

    pub fn is_running_in_docker() -> bool {
        // Check for .dockerenv file instead. This file exists in the debian:__-slim
        // image we use at runtime.
        Path::new("/.dockerenv").exists()
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
                return vec![];
            }
        };
        system.refresh_all();

        let mut mfs = self.static_metric_families.clone();
        match Self::collect_cpu_usage(&system) {
            Ok(families) => {
                mfs.extend(families);
            }
            Err(e) => {
                tracing::error!("Failed collecting CPU usage: {e}")
            }
        };
        if let Some(families) = self.collect_memory_available(&system) {
            mfs.extend(families);
        };
        match self.collect_disk_available() {
            Ok(families) => {
                mfs.extend(families);
            }
            Err(e) => {
                tracing::error!("Failed collecting disk metrics: {e}");
            }
        };
        mfs
    }
}

#[derive(thiserror::Error, Debug)]
pub enum HardwareMetricsErr {
    #[error("Failed creating metric: {0}")]
    ErrCreateMetric(prometheus::Error),
    #[error("Failed registering hardware metrics onto RegistryService: {0}")]
    ErrRegisterHardwareMetrics(prometheus::Error),
    #[error("Failed TryFromInt: {0}")]
    TryFromInt(std::num::TryFromIntError),
    #[error("Failed acquiring lock: Poisoned: {0}")]
    GetLock(String),
    #[error("Db disk not found")]
    DbDiskNotFound,
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
            let metric = metric_families
                .iter()
                .find(|mf| mf.get_name() == family_name)
                .ok_or_else(|| format!("Metric family not found: {family_name}"))?
                .get_metric()
                .first()
                .ok_or_else(|| format!("No metrics in family {family_name}"))?;
            Ok(metric)
        };
        let find_metric_label = |family_name: &str, label_name: &str| -> Result<String, String> {
            let metric = find_metric(family_name)?;
            Ok(metric
                .get_label()
                .iter()
                .find(|l| l.get_name() == label_name)
                .ok_or_else(|| format!("Label not found: {label_name}"))?
                .get_value()
                .to_string())
        };

        assert!(metric_families.len() > 6);

        let core_count = find_metric_label("cpu_specs", "cpu_core_count")?
            .parse::<usize>()
            .map_err(|e| format!("Failed parsing cpu_core_count: {e}"))?;
        assert!(core_count > 0 && core_count < 513);

        let mem_total_bytes = find_metric_label("memory_specs", "memory_total_bytes")?;
        assert!(mem_total_bytes.parse::<u64>().is_ok_and(|v| v > 0));

        let disk_total_bytes = find_metric_label("db_disk_specs", "db_disk_total_bytes")?;
        assert!(disk_total_bytes.parse::<u64>().is_ok_and(|v| v > 0));

        let mut system = System::new_all();
        system.refresh_all();
        let cpu1_name = system.cpus().first().unwrap().name();
        // we can only check that the value exists and was collected
        let _cpu_1_usage = find_metric(&format!("cpu_{cpu1_name}_usage"))?;

        let disk_available = find_metric("disk_1_available_bytes")?;
        assert!(disk_available.get_gauge().get_value() > 0.0);

        Ok(())
    }
}
