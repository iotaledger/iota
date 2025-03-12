// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

use prometheus::{
    core::{Collector, Desc},
    proto::{LabelPair, Metric, MetricFamily},
    IntGauge, Opts,
};
use sysinfo::{CpuRefreshKind, Disks, MemoryRefreshKind, RefreshKind, System};

use crate::RegistryService;

pub fn register_hardware_metrics(
    registry_service: &mut RegistryService,
) -> Result<(), HardwareMetricsErr> {
    registry_service
        .default_registry
        .register(Box::new(HardwareSpecs::new()?))
        .map_err(HardwareMetricsErr::ErrRegisterHardwareMetrics)
}

pub struct HardwareSpecs {
    pub cpu_model: String,
    pub cpu_vendor_id: String,
    pub cpu_core_count: Option<usize>,
    pub memory_collector: IntGauge,
    pub disk_collector: IntGauge,
    pub is_docker: bool,
}
impl HardwareSpecs {
    pub fn new() -> Result<Self, HardwareMetricsErr> {
        let mut system = System::new_with_specifics(
            RefreshKind::nothing()
                .with_cpu(CpuRefreshKind::nothing())
                .with_memory(MemoryRefreshKind::nothing().with_ram()),
        );
        system.refresh_all();

        let cpu_vendor_id: &str = {
            let vendor_id = system
                .cpus()
                .first()
                .map_or("cpu_vendor_id_unavailable", |cpu| cpu.vendor_id());
            match vendor_id {
                "" => "cpu_vendor_id_unavailable",
                _ => vendor_id,
            }
        };

        let cpu_model: &str = {
            let brand = system
                .cpus()
                .first()
                .map_or("cpu_model_unavailable", |cpu| cpu.brand());
            match brand {
                "" => "cpu_model_unavailable",
                _ => brand,
            }
        };
        let cpu_core_count = system.physical_core_count();

        let mem_total_bytes = system.total_memory();
        let memory_collector = IntGauge::with_opts(
            Opts::new(
                "memory_specs",
                "Memory specs (constants: total amount, ...)",
            )
            .const_label("memory_total_bytes", mem_total_bytes.to_string())
            .const_label("memory_total_human", human_fmt_bytes(mem_total_bytes)),
        )
        .map_err(HardwareMetricsErr::ErrCreateMetric)?;
        memory_collector.set(mem_total_bytes as i64);

        // We're only interested in the largest disk
        let disks = Disks::new_with_refreshed_list();
        let disk_total_bytes: u64 = disks
            .iter()
            .max_by_key(|disk| disk.total_space())
            .map(|d| d.total_space())
            .unwrap_or(0);
        let disk_collector = IntGauge::with_opts(
            Opts::new("disk_specs", "Disk specifications (total disk space, ...)")
                .const_label("disk_total_bytes", disk_total_bytes.to_string())
                .const_label("disk_total_space_human", human_fmt_bytes(disk_total_bytes)),
        )
        .map_err(HardwareMetricsErr::ErrCreateMetric)?;
        disk_collector.set(disk_total_bytes as i64);

        Ok(Self {
            cpu_model: cpu_model.to_string(),
            cpu_vendor_id: cpu_vendor_id.to_string(),
            cpu_core_count,
            memory_collector,
            disk_collector,
            is_docker: is_running_in_docker(),
        })
    }

    fn label(name: &str, value: impl ToString) -> LabelPair {
        let mut label = LabelPair::new();
        label.set_name(name.to_string());
        label.set_value(value.to_string());
        label
    }

    fn cpu_metric(&self) -> Metric {
        let mut metric = Metric::new();
        metric.set_label({
            vec![
                Self::label("cpu_model", &self.cpu_model),
                Self::label("cpu_vendor_id", &self.cpu_vendor_id),
                Self::label(
                    "cpu_core_count",
                    self.cpu_core_count.map_or_else(
                        || "cpu_core_count_unavailable".to_string(),
                        |c| c.to_string(),
                    ),
                ),
                Self::label("cpu_arch", System::cpu_arch()),
            ]
            .into()
        });
        metric
    }
    fn cpu_metric_family(&self) -> MetricFamily {
        let mut mf = MetricFamily::new();
        mf.set_name("cpu_specs".to_owned());
        mf.set_help("CPU specs (brand,vendor,cores)".to_owned());
        mf.set_field_type(prometheus::proto::MetricType::COUNTER);
        mf.set_metric(vec![self.cpu_metric()].into());
        mf
    }

    fn system_metric(&self) -> Metric {
        let mut metric = Metric::new();
        metric.set_label({
            vec![
                Self::label("is_docker", self.is_docker.to_string()),
                Self::label(
                    "os_version",
                    System::long_os_version()
                        .unwrap_or_else(|| "os_version_unavailable".to_string()),
                ),
            ]
            .into()
        });
        metric
    }
    fn system_metric_family(&self) -> MetricFamily {
        let mut mf = MetricFamily::new();
        mf.set_name("system_info".to_owned());
        mf.set_help("System info (OS, version, is_docker, ...)".to_owned());
        mf.set_field_type(prometheus::proto::MetricType::COUNTER);
        mf.set_metric(vec![self.system_metric()].into());
        mf
    }
}

impl Collector for HardwareSpecs {
    fn desc(&self) -> Vec<&Desc> {
        let mut descs = Vec::new();
        descs.extend(self.memory_collector.desc());
        descs.extend(self.disk_collector.desc());
        descs
    }

    fn collect(&self) -> Vec<prometheus::proto::MetricFamily> {
        let mut mfs = Vec::new();
        mfs.push(self.cpu_metric_family());
        mfs.push(self.system_metric_family());
        mfs.extend(self.memory_collector.collect());
        mfs.extend(self.disk_collector.collect());
        mfs
    }
}

pub fn is_running_in_docker() -> bool {
    // Check for .dockerenv file instead. This file exists in the debian:__-slim
    // image we use at runtime.
    Path::new("/.dockerenv").exists()
}

fn human_fmt_bytes(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];

    let mut value = bytes;
    let mut unit_idx = 0;
    // Shift right until we're below 1024^2 or reach end of units
    while value >= (1024 * 1024) && unit_idx < UNITS.len() - 2 {
        value >>= 10;
        unit_idx += 1;
    }
    let value: f64 = value as f64 / 1024.0;
    unit_idx += 1;

    format!("{:.2} {}", value, UNITS[unit_idx])
}

#[derive(thiserror::Error, Debug)]
pub enum HardwareMetricsErr {
    #[error("Failed creating metric")]
    ErrCreateMetric(prometheus::Error),
    #[error("Failed registering hardware metrics onto RegistryService")]
    ErrRegisterHardwareMetrics(prometheus::Error),
}

#[cfg(test)]
mod tests {
    use std::{
        net::SocketAddrV4,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    #[test]
    fn test_hardware_specs() {
        let hardware_specs = HardwareSpecs::new().unwrap();
        let metric_families = hardware_specs.collect();
        assert_eq!(&metric_families.len(), &4);
    }

    #[tokio::test]
    async fn test_collect_hardware_specs() -> Result<(), String> {
        let prom_server_addr: SocketAddrV4 = "0.0.0.0:9194".parse().unwrap();

        let mut registry_svc = crate::start_prometheus_server(prom_server_addr.into());

        register_hardware_metrics(&mut registry_svc).expect("Failed registering hardware metrics");

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

        let find_metric_label = |family_name: &str, label_name: &str| -> Result<String, String> {
            let metric = metric_families
                .iter()
                .find(|mf| mf.get_name() == family_name)
                .ok_or_else(|| format!("Metric family not found: {family_name}"))?
                .get_metric()
                .first()
                .ok_or_else(|| format!("No metrics in family {family_name}"))?;
            Ok(metric
                .get_label()
                .iter()
                .find(|l| l.get_name() == label_name)
                .ok_or_else(|| format!("Label not found: {label_name}"))?
                .get_value()
                .to_string())
        };

        assert_eq!(&metric_families.len(), &4);

        let core_count = find_metric_label("cpu_specs", "cpu_core_count")?
            .parse::<usize>()
            .map_err(|e| format!("Failed parsing cpu_core_count: {e}"))?;
        assert!(core_count > 0 && core_count < 513);

        let mem_total_bytes = find_metric_label("memory_specs", "memory_total_bytes")?;
        assert!(mem_total_bytes.parse::<u64>().is_ok_and(|v| v > 0));

        let disk_total_bytes = find_metric_label("disk_specs", "disk_total_bytes")?;
        assert!(disk_total_bytes.parse::<u64>().is_ok_and(|v| v > 0));
        // we only check these values exist
        let _mem_total_human = find_metric_label("memory_specs", "memory_total_human")?;
        let _disk_total_human = find_metric_label("disk_specs", "disk_total_space_human")?;

        Ok(())
    }
}
