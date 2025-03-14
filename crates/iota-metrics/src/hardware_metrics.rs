// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex},
};

use prometheus::{
    core::{Collector, Desc},
    proto::{LabelPair, Metric, MetricFamily, MetricType},
    IntGauge, Opts,
};
use sysinfo::{CpuRefreshKind, Disk, Disks, MemoryRefreshKind, RefreshKind, System};

use crate::RegistryService;

pub fn register_hardware_metrics(
    registry_service: &mut RegistryService,
    db_path: &Path,
) -> Result<(), HardwareMetricsErr> {
    registry_service
        .default_registry
        .register(Box::new(HardwareMetrics::new(db_path)?))
        .map_err(HardwareMetricsErr::ErrRegisterHardwareMetrics)
}

// TODO in 4666 non blocking tokio
// TODO in 4666 re-introduce usage metrics
// TODO in 4666 static metric families
// TODO in 4666 find disk where database mounted

pub struct HardwareMetrics {
    system: Arc<Mutex<System>>,
    /// the disk that holds the database, if found
    // pub db_disk: Option<Arc<RwLock<Disk>>>,
    disks: Arc<Mutex<Disks>>,
    db_disk_index: Option<usize>,

    pub static_metric_families: Vec<MetricFamily>,
    pub static_descriptions: Vec<Desc>,
    // pub cpu_model: String,
    // pub cpu_vendor_id: String,
    // pub cpu_core_count: Option<usize>,
    // pub cpu_specs_metric_family: MetricFamily,
    // pub cpu_usage_collector: Vec<Gauge>,
    // pub memory_specs_collector: IntGauge,
    pub memory_available_collector: IntGauge,
    // pub disk_specs_collector: IntGauge,
    pub disk_available_collector: IntGauge,
    // pub is_docker: bool,
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
        let (db_disk_index, db_disk) = Self::find_db_disk(&disks, &db_path);

        // let cpu_vendor_id: &str = {
        //     let vendor_id = system
        //         .cpus()
        //         .first()
        //         .map_or("cpu_vendor_id_unavailable", |cpu| cpu.vendor_id());
        //     match vendor_id {
        //         "" => "cpu_vendor_id_unavailable",
        //         _ => vendor_id,
        //     }
        // };

        // let cpu_model: &str = {
        //     let brand = system
        //         .cpus()
        //         .first()
        //         .map_or("cpu_model_unavailable", |cpu| cpu.brand());
        //     match brand {
        //         "" => "cpu_model_unavailable",
        //         _ => brand,
        //     }
        // };
        // let cpu_core_count = system.physical_core_count();

        // let cpu_usage_collector: IntGaugeVec = IntGaugeVec::new(
        //     Opts::new("cpu_usage", "CPU usage (array of percentages, 1 per core)"),
        //     &[],
        // )
        // .map_err(HardwareMetricsErr::ErrCreateMetric)?;

        // let mem_total_bytes = system.total_memory();
        // let memory_collector = IntGauge::with_opts(
        //     Opts::new(
        //         "memory_specs",
        //         "Memory specs (constants: total amount, ...)",
        //     )
        //     .const_label("memory_total_bytes", mem_total_bytes.to_string())
        //     .const_label("memory_total_human", human_fmt_bytes(mem_total_bytes)),
        // )
        // .map_err(HardwareMetricsErr::ErrCreateMetric)?;
        // memory_collector.set(mem_total_bytes as i64);

        // We're only interested in the largest disk
        // let disk_total_bytes: u64 = disks
        //     .iter()
        //     .max_by_key(|disk| disk.total_space())
        //     .map(|d| d.total_space())
        //     .unwrap_or(0);
        // let disk_total_collector = IntGauge::with_opts(
        //     Opts::new("disk_specs", "Disk specifications (total disk space, ...)")
        //         .const_label("disk_total_bytes", disk_total_bytes.to_string())
        //         .const_label("disk_total_space_human", human_fmt_bytes(disk_total_bytes)),
        // )
        // .map_err(HardwareMetricsErr::ErrCreateMetric)?;
        // disk_total_collector.set(disk_total_bytes as i64);

        Ok(Self {
            // cpu_model: cpu_model.to_string(),
            // cpu_vendor_id: cpu_vendor_id.to_string(),
            // cpu_core_count,
            static_metric_families: Self::static_metric_families(&system, db_disk)?,
            static_descriptions: Self::static_descriptions(&system, db_disk)?,
            // cpu_usage_collector:Self::,
            // memory_specs_collector: Self::memory_specs_collector(&system)?,
            memory_available_collector: Self::memory_available_collector()?,
            // disk_specs_collector: Self::disk_specs_collector(db_disk.as_ref())?,
            disk_available_collector: Self::disk_available_collector()?,

            system: Arc::new(Mutex::new(system)),
            // db_disk: match db_disk {
            //     Some(disk) => Some(Arc::new(RwLock::new(disk))),
            //     None => None,
            // },
            disks: Arc::new(Mutex::new(disks)),
            db_disk_index,
            // is_docker: is_running_in_docker(),
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
    fn gauge(desc: &Desc, value: f64) -> MetricFamily {
        let mut g = prometheus::proto::Gauge::default();
        let mut m = Metric::default();
        let mut mf = MetricFamily::new();

        g.set_value(value);
        m.set_gauge(g);

        mf.mut_metric().push(m);
        mf.set_name(desc.fq_name.clone());
        mf.set_help(desc.help.clone());
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
    // fn cpu_specs_metric(system: &System) -> Metric {
    //     let mut metric = Metric::new();
    //     metric.set_label({
    //         vec![
    //             Self::label("cpu_model", &Self::cpu_model(system)),
    //             Self::label("cpu_vendor_id", &Self::cpu_vendor_id(system)),
    //             Self::label(
    //                 "cpu_core_count",
    //                 Self::cpu_core_count(system).map_or_else(
    //                     || "cpu_core_count_unavailable".to_string(),
    //                     |c| c.to_string(),
    //                 ),
    //             ),
    //             Self::label("cpu_arch", System::cpu_arch()),
    //         ]
    //         .into()
    //     });
    //     metric
    // }
    fn collect_cpu_specs(system: &System) -> MetricFamily {
        let mut metric = Metric::new();
        metric.set_label({
            vec![
                Self::label("cpu_model", &Self::cpu_model(system)),
                Self::label("cpu_vendor_id", &Self::cpu_vendor_id(system)),
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

    // fn cpu_usage_collector() -> Result<Vec<Gauge>, HardwareMetricsErr> {
    //     Ok(IntGaugeVec::new(
    //         Opts::new("cpu_usage", "CPU usage (array of percentages, 1 per core)"),
    //         &[],
    //     )
    //     .map_err(HardwareMetricsErr::ErrCreateMetric)?)
    // }
    // fn set_cpu_usage(&self, system: &System) -> Result<(), HardwareMetricsErr> {
    //     // let cpu_usage_per_core: Vec<MetricFamily> = system
    //     //     .cpus()
    //     //     .iter()
    //     //     .enumerate()
    //     //     .map(|(idx, cpu)| {
    //     //             cpu.cpu_usage() as f64,
    //     //             Some(&[format!("cpu{}_idx{}", cpu.name(), idx + 1)]),
    //     //         )
    //     //         .unwrap()
    //     //         .collect()
    //     //     })
    //     //     .collect();
    //     todo!()
    // }

    // fn collect_cpu_core_usage(cpu:&Cpu) -> MetricFamily {
    //     let mut m = MetricFamily::new();
    //     mf.set_name("cpu_specs".to_owned());
    //     mf.set_help("CPU specs (brand,vendor,cores)".to_owned());
    //     mf.set_field_type(prometheus::proto::MetricType::GAUGE);
    //     mf.set_metric(vec![{
    //         let mut metric = Metric::new();
    //         metric.set
    //         metric
    //     }].into());
    //     mf
    // }
    // TODO in 4666 call it in collect()
    fn collect_cpu_usage(system: &System) -> Result<Vec<MetricFamily>, HardwareMetricsErr> {
        let cpu_usage_per_core: Vec<MetricFamily> = system
            .cpus()
            .iter()
            .enumerate()
            .map(|(idx, cpu)| {
                Ok(Self::gauge(
                    &Desc::new(
                        format!("cpu{}_idx{}_usage", cpu.name(), idx + 1),
                        format!("CPU{} core {} usage in percent", cpu.name(), idx + 1),
                        vec!["core".to_string()],
                        HashMap::new(),
                    )
                    .map_err(HardwareMetricsErr::ErrCreateMetric)?,
                    cpu.cpu_usage() as f64,
                ))
            })
            .collect::<Result<_, _>>()?;
        Ok(cpu_usage_per_core)
    }

    fn memory_specs_collector(system: &System) -> Result<IntGauge, HardwareMetricsErr> {
        let mem_total_bytes = system.total_memory();
        let memory_specs_collector = IntGauge::with_opts(
            Opts::new(
                "memory_specs",
                "Memory specs (constants: total amount, ...)",
            )
            .const_label("memory_total_bytes", mem_total_bytes.to_string())
            .const_label("memory_total_human", human_fmt_bytes(mem_total_bytes)),
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
        return (None, None);
    }
    // // TODO in 4666 what do we want to do with disk
    // fn db_disk_mut(&self) -> Result<Option<&mut Disk>, HardwareMetricsErr> {
    //     let idx = match self.db_disk_index {
    //         Some(idx) => idx,
    //         None => return Ok(None),
    //     };
    //     let disks = self
    //         .disks
    //         .lock()
    //         .map_err(|e| HardwareMetricsErr::GetLock(e.to_string()))?;
    //     let db_disk

    //     todo!()
    // }
    fn disk_specs_collector(db_disk: Option<&Disk>) -> Result<IntGauge, HardwareMetricsErr> {
        let disk_total_bytes: Option<u64> = db_disk.map(|d| d.total_space());
        let disk_specs_collector = IntGauge::with_opts(
            Opts::new("disk_specs", "Disk specifications (total disk space, ...)")
                .const_label(
                    "disk_total_bytes",
                    match disk_total_bytes {
                        Some(bytes) => bytes.to_string(),
                        None => "disk_total_bytes_unavailable".to_string(),
                    },
                )
                .const_label(
                    "disk_total_space_human",
                    match disk_total_bytes {
                        Some(bytes) => human_fmt_bytes(bytes).to_string(),
                        None => "disk_total_space_human_unavailable".to_string(),
                    },
                ),
        )
        .map_err(HardwareMetricsErr::ErrCreateMetric)?;
        disk_specs_collector.set(disk_total_bytes.unwrap_or(0) as i64);
        Ok(disk_specs_collector)
    }

    fn disk_available_collector() -> Result<IntGauge, HardwareMetricsErr> {
        IntGauge::with_opts(Opts::new("disk_available_bytes", "Disk available (bytes)"))
            .map_err(HardwareMetricsErr::ErrCreateMetric)
    }
    fn collect_disk_available(&self) -> Result<Vec<MetricFamily>, HardwareMetricsErr> {
        let db_disk_idx = match self.db_disk_index {
            Some(idx) => idx,
            None => return Err(HardwareMetricsErr::DbDiskNotFound),
        };
        let mut disks = self
            .disks
            .lock()
            .map_err(|e| HardwareMetricsErr::GetLock(e.to_string()))?;
        let db_disk = disks
            .get_mut(db_disk_idx)
            .ok_or(HardwareMetricsErr::DbDiskNotFound)?;
        db_disk.refresh();

        let disk_available_bytes = match i64::try_from(db_disk.available_space()) {
            Ok(bytes) => bytes,
            Err(e) => return Err(HardwareMetricsErr::TryFromInt(e)),
        };

        self.disk_available_collector.set(disk_available_bytes);
        Ok(self.disk_available_collector.collect())
    }

    // fn system_metric() -> Metric {
    //     let mut metric = Metric::new();
    //     metric.set_label({
    //         vec![
    //             Self::label("is_docker", Self::is_running_in_docker().to_string()),
    //             Self::label(
    //                 "os_version",
    //                 System::long_os_version()
    //                     .unwrap_or_else(|| "os_version_unavailable".to_string()),
    //             ),
    //         ]
    //         .into()
    //     });
    //     metric
    // }
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
        // Check for .dockerenv file instead. This file exists in the debian:__-slim image we use at runtime.
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

    const DB_PATH: LazyLock<PathBuf> = LazyLock::new(|| PathBuf::from("/opt/iota/db"));

    #[test]
    fn test_hardware_specs() {
        let hardware_specs = HardwareMetrics::new(&DB_PATH).unwrap();
        let metric_families = hardware_specs.collect();
        assert_eq!(&metric_families.len(), &6);
    }

    #[tokio::test]
    async fn test_collect_hardware_specs() -> Result<(), String> {
        let prom_server_addr: SocketAddrV4 = "0.0.0.0:9194".parse().unwrap();

        let mut registry_svc = crate::start_prometheus_server(prom_server_addr.into());

        register_hardware_metrics(&mut registry_svc, &DB_PATH)
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

        assert_eq!(&metric_families.len(), &6);

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
