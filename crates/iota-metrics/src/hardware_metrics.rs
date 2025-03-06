use std::mem;

use prometheus::{
    core::{Collector, Desc},
    proto::{LabelPair, Metric, MetricFamily},
    IntCounter, IntGauge, Opts,
};
use sysinfo::{CpuRefreshKind, Disks, MemoryRefreshKind, RefreshKind, System};

use crate::RegistryService;

// const NAMESPACE: &str = "hardware_metrics";
// const HARDWARE_SPECS: LazyLock<HardwareSpecs> = LazyLock::new(|| HardwareSpecs::new());

pub fn register_hardware_metrics(
    registry_service: &mut RegistryService,
) -> Result<(), HardwareMetricsErr> {
    registry_service
        .default_registry
        .register(Box::new(HardwareSpecs::new()?))
        .map_err(HardwareMetricsErr::ErrRegisterHardwareMetrics)
}

// pub struct HardwareMetrics {
//     system: Arc<Mutex<System>>,
//     pub cpu: CpuMetrics,
//     pub memory: MemoryMetrics,
//     pub disk: DiskMetrics,
// }
// impl HardwareMetrics {
//     fn new() -> Result<Self, HardwareMetricsErr> {
//         let mut system = System::new_with_specifics(
//             RefreshKind::nothing()
//                 .with_cpu(CpuRefreshKind::nothing())
//                 .with_memory(MemoryRefreshKind::nothing().with_ram()),
//         );

//         Ok(Self {
//             cpu: CpuMetrics::new(&mut system)?,
//             memory: MemoryMetrics::new(&mut system)?,
//             disk: DiskMetrics::new()?,
//             system: Arc::new(Mutex::new(system)),
//         })
//     }
//     fn update(&self) {
//         let mut system = self.system.lock().unwrap();
//         system.refresh_all();
//         self.cpu.update(&system);
//     }
// }
// impl Collector for HardwareMetrics {
//     fn desc(&self) -> Vec<&Desc> {
//         let mut descs = Vec::new();
//         descs.extend(self.cpu.desc());
//         descs.extend(self.memory.desc());
//         descs.extend(self.disk.desc());
//         descs
//     }
//     fn collect(&self) -> Vec<MetricFamily> {
//         self.update();
//         let mut mfs = Vec::new();
//         mfs.extend(self.cpu.collect());
//         mfs.extend(self.memory.collect());
//         mfs.extend(self.disk.collect());
//         mfs
//     }
// }

// pub struct CpuMetrics {
//     cpu_specs: Counter,
// }
// impl CpuMetrics {
//     pub fn new(system: &System) -> Result<Self, HardwareMetricsErr> {
//         let cpu_vendor_id: &str = system
//             .cpus()
//             .first()
//             .map_or("unknown_cpu_vendor_id", |cpu| cpu.vendor_id());
//         // let cpu_brand: &str = system
//         //     .cpus()
//         //     .first()
//         //     .map_or("unknown_cpu_model", |cpu| cpu.brand());

//         let cpu_specs = Counter::with_opts(
//             Opts::new("cpu_specs", "CPU specs (brand, vendor, cores)")
//                 .const_label("cpu_vendor_id", cpu_vendor_id)
//                 .variable_label("cpu_brand")
//                 .const_label("cpu_arch", System::cpu_arch())
//                 .const_label(
//                     "num_cpu_cores",
//                     system
//                         .physical_core_count()
//                         .map(|c| c.to_string())
//                         .unwrap_or_else(|| "unknown_num_cpu_cores".to_owned()),
//                 )
//                 .namespace(NAMESPACE),
//         )
//         .map_err(HardwareMetricsErr::ErrCreateMetric)?;

//         Ok(Self { cpu_specs })
//     }
//     fn update(&self, system: &System) {}
// }
// const CPU_METRICS_COUNT: usize = 12;
// impl Collector for CpuMetrics {
//     fn desc(&self) -> Vec<&Desc> {
//         let mut desc = Vec::new();

//         desc.extend(&self.cpu_specs.desc());
//         desc
//     }

//     fn collect(&self) -> Vec<MetricFamily> {
//         let mut mfs = Vec::with_capacity(CPU_METRICS_COUNT);

//         // let mut metric = Metric::new();

//         // metric.set_label({
//         //     let mut label = LabelPair::new();
//         //     label.set_name("cpu_brand".to_string());
//         //     label.set_value(HARDWARE_SPECS.cpu_brand.clone());
//         //     vec![label].into()
//         // });
//         // mfs.set_metric(vec![metric].into());

//         mfs.extend(self.cpu_specs.collect());
//         mfs
//     }
// }

// pub struct MemoryMetrics {
//     pub specs: Counter,
// }
// impl MemoryMetrics {
//     pub fn new(system: &System) -> Result<Self, HardwareMetricsErr> {
//         let mem_total = system.total_memory();

//         let mem_specs = Counter::with_opts(
//             Opts::new(
//                 "memory_specs",
//                 "Memory specs (constants: total amount, ...)",
//             )
//             .const_label("mem_total_ram_bytes", mem_total.to_string())
//             .const_label(
//                 "mem_total_ram_human",
//                 format!("{}", human_fmt_bytes(mem_total)),
//             )
//             .namespace(NAMESPACE),
//         )
//         .map_err(HardwareMetricsErr::ErrCreateMetric)?;

//         Ok(Self { specs: mem_specs })
//     }
// }
// const MEMORY_METRICS_COUNT: usize = 3;
// impl Collector for MemoryMetrics {
//     fn desc(&self) -> Vec<&Desc> {
//         let mut desc = Vec::new();
//         desc.extend(&self.specs.desc());
//         desc
//     }
//     fn collect(&self) -> Vec<MetricFamily> {
//         let mut mfs = Vec::with_capacity(MEMORY_METRICS_COUNT);
//         mfs.extend(self.specs.collect());
//         mfs
//     }
// }

// pub struct DiskMetrics {
//     pub specs: Counter,
// }
// impl DiskMetrics {
//     pub fn new() -> Result<Self, HardwareMetricsErr> {
//         let disks = Disks::new_with_refreshed_list();
//         // for disk in disks.iter() {
//         //     println!("disk name: {}", disk.name().to_string_lossy());
//         //     println!("disk_kind: {}", disk.kind().to_string());
//         //     println!("space: {}", disk.total_space());
//         // }
//         let disk_total_space: u64 = disks
//             .iter()
//             .max_by_key(|disk| disk.total_space())
//             .map(|d| d.total_space())
//             .unwrap_or(0);

//         let disk_specs = Counter::with_opts(
//             Opts::new(
//                 "disk_specs",
//                 "Constant disk specifications (total disk space, ...)",
//             )
//             .const_label("disk_total_space_bytes", disk_total_space.to_string())
//             .const_label("disk_total_space_human", human_fmt_bytes(disk_total_space))
//             .namespace(NAMESPACE),
//         )
//         .map_err(HardwareMetricsErr::ErrCreateMetric)?;

//         Ok(Self { specs: disk_specs })
//     }
// }
// const DISK_METRICS_COUNT: usize = 1;
// impl Collector for DiskMetrics {
//     fn desc(&self) -> Vec<&Desc> {
//         let mut desc = Vec::new();
//         desc.extend(&self.specs.desc());
//         desc
//     }
//     fn collect(&self) -> Vec<MetricFamily> {
//         let mut mfs = Vec::with_capacity(DISK_METRICS_COUNT);
//         mfs.extend(self.specs.collect());
//         mfs
//     }
// }

// fn human_fmt_bytes(bytes: u64) -> String {
//     const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];

//     let mut value = bytes;
//     let mut unit_idx = 0;
//     // Shift right until we're below 1024^2 or reach end of units
//     while value >= (1024 * 1024) && unit_idx < UNITS.len() - 2 {
//         // dbg!("val_before", value);
//         value >>= 10;
//         // dbg!("val_after", value);
//         unit_idx += 1;
//     }
//     let value: f64 = value as f64 / 1024.0;
//     unit_idx += 1;

//     format!("{:.2} {}", value, UNITS[unit_idx])
// }

#[derive(thiserror::Error, Debug)]
pub enum HardwareMetricsErr {
    #[error("Failed creating metric")]
    ErrCreateMetric(prometheus::Error),
    #[error("Failed registering hardware metrics onto RegistryService")]
    ErrRegisterHardwareMetrics(prometheus::Error),
}

pub struct HardwareSpecs {
    pub cpu_brand: String,
    pub cpu_vendor_id: String,
    pub cpu_core_count: Option<usize>,
    pub memory_collector: IntGauge,
    // pub memory_total_bytes: u64,
    pub disk_collector: IntGauge,
    // pub disk_total_bytes: u64,
    // pub collector: IntCounterVec,
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
            .const_label("memory_total_bytes", mem_total_bytes.to_string()),
            // .const_label(
            //     "mem_total_ram_human",
            //     format!("{}", human_fmt_bytes(mem_total)),
            // )
            // .namespace(NAMESPACE),
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
                .const_label("disk_total_space_bytes", disk_total_bytes.to_string()), // .const_label("disk_total_space_human", human_fmt_bytes(disk_total_space))
                                                                                      // .namespace(NAMESPACE),
        )
        .map_err(HardwareMetricsErr::ErrCreateMetric)?;
        disk_collector.set(disk_total_bytes as i64);

        // let collector = IntCounterVec::new(
        //     Opts::new(
        //         "hardware_specs",
        //         "Hardware specs (CPU brand/vendor/cores, Memory total, Disk total)",
        //     )
        //     .variable_labels(vec![
        //         "cpu_brand".to_string(),
        //         "cpu_vendor_id".to_string(),
        //         "cpu_core_count".to_string(),
        //     ]),
        //     &["cpu_brand", "cpu_vendor_id", "cpu_core_count"], // TODO IN 4666 all fields (mem disk)
        // )
        // .unwrap(); // TODO in 4666 error

        Ok(Self {
            cpu_brand: cpu_model.to_string(),
            cpu_vendor_id: cpu_vendor_id.to_string(),
            cpu_core_count,
            memory_collector,
            // memory_total_bytes: system.total_memory(),
            // disk_total_bytes,
            disk_collector,
            // collector,
        })
    }

    // fn cpu_label_names() -> [&'static str; 3] {
    //     ["cpu_brand", "cpu_vendor", "cpu_cores"]
    // }
    // fn cpu_metric_desc() -> Desc {
    //     Desc::new(
    //         "cpu_specs".to_owned(),
    //         "CPU specs (brand, vendor, cores)".to_owned(),
    //         Self::cpu_label_names()
    //             .iter()
    //             .map(|l| l.to_string())
    //             .collect(),
    //         Default::default(),
    //     )
    //     .unwrap()
    // }
    fn label(name: &str, value: impl ToString) -> LabelPair {
        // let mut metric = Metric::new();
        // metric.set_label({});
        let mut label = LabelPair::new();
        label.set_name(name.to_string());
        label.set_value(value.to_string());
        label
    }

    fn cpu_metric(&self) -> Metric {
        let mut metric = Metric::new();
        metric.set_label({
            vec![
                Self::label("cpu_brand", &self.cpu_brand.to_slug()),
                Self::label("cpu_vendor_id", &self.cpu_vendor_id),
                Self::label(
                    "cpu_core_count",
                    &self.cpu_core_count.map_or_else(
                        || "cpu_core_count_unavailable".to_string(),
                        |c| c.to_string(),
                    ),
                ),
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

    // fn mem_metric(&self) -> Metric {
    //     let mut metric = Metric::new();
    //     metric.set_label({
    //         vec![Self::label(
    //             "memory_total_bytes",
    //             match self.memory_total_bytes {
    //                 0 => "memory_total_unavailable".to_string(),
    //                 _ => format!("{}", self.memory_total_bytes),
    //             },
    //         )]
    //         .into()
    //     });
    //     metric
    // }
    // fn mem_metric_family(&self) -> MetricFamily {
    //     let mut mf = MetricFamily::new();
    //     mf.set_name("memory_specs".to_owned());
    //     mf.set_help("Memory specs (total)".to_owned());
    //     mf.set_field_type(prometheus::proto::MetricType::COUNTER);
    //     mf.set_metric(vec![self.mem_metric()].into());
    //     mf
    // }

    // fn disk_metric(&self) -> Metric {
    //     let mut metric = Metric::new();
    //     metric.set_label({
    //         vec![Self::label(
    //             "disk_total_bytes",
    //             match self.disk_total_bytes {
    //                 0 => "disk_total_unavailable".to_owned(),
    //                 _ => format!("{}", self.disk_total_bytes),
    //             },
    //         )]
    //         .into()
    //     });
    //     metric
    // }
    // fn disk_metric_family(&self) -> MetricFamily {
    //     let mut mf = MetricFamily::new();
    //     mf.set_name("disk_specs".to_owned());
    //     mf.set_help("Disk specs (total)".to_owned());
    //     mf.set_field_type(prometheus::proto::MetricType::COUNTER);
    //     mf.set_metric(vec![self.disk_metric()].into());
    //     mf
    // }
}

impl Collector for HardwareSpecs {
    fn desc(&self) -> Vec<&Desc> {
        Vec::new()
    }

    fn collect(&self) -> Vec<prometheus::proto::MetricFamily> {
        let mut mfs = Vec::new();
        mfs.push(self.cpu_metric_family());
        mfs.extend(self.memory_collector.collect());
        // mfs.push(self.disk_metric_family());
        mfs.extend(self.disk_collector.collect());
        mfs
    }
}

pub trait Slug {
    fn to_slug(&self) -> String;
    fn slugify(text: &str) -> String {
        let mut result = String::new();

        // Convert to lowercase and process each character
        for c in text.to_lowercase().chars() {
            match c {
                // Keep alphanumeric characters
                'a'..='z' | '0'..='9' => result.push(c),

                // Replace spaces and special characters with hyphens
                ' ' | '_' | '-' => {
                    // Only add hyphen if last char wasn't a hyphen
                    if !result.is_empty() && result.chars().last() != Some('-') {
                        result.push('-');
                    }
                }

                // Convert accented characters to base letters
                'á' | 'à' | 'ã' | 'â' | 'ä' => result.push('a'),
                'é' | 'è' | 'ê' | 'ë' => result.push('e'),
                'í' | 'ì' | 'î' | 'ï' => result.push('i'),
                'ó' | 'ò' | 'õ' | 'ô' | 'ö' => result.push('o'),
                'ú' | 'ù' | 'û' | 'ü' => result.push('u'),
                'ñ' => result.push('n'),

                _ => {}
            }
        }

        // Remove trailing hyphens
        while result.ends_with('-') {
            result.pop();
        }

        result
    }
}
impl Slug for &'_ str {
    fn to_slug(&self) -> String {
        Self::slugify(self)
    }
}
impl Slug for String {
    fn to_slug(&self) -> String {
        Self::slugify(self)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        net::SocketAddrV4,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;
    use prometheus::Encoder;

    #[test]
    fn test_hardware_specs() {
        let hardware_specs = HardwareSpecs::new().unwrap();
        let collected = hardware_specs.collect();
        dbg!(&collected);
    }

    #[tokio::test]
    async fn test_collect_hardware_specs() -> Result<(), String> {
        let prom_server_addr: SocketAddrV4 = "0.0.0.0:9194".parse().unwrap();
        // let prom_server_port = prom_server_addr.port();

        let mut registry_svc = crate::start_prometheus_server(prom_server_addr.into());
        let prometheus_registry = registry_svc.default_registry();

        register_hardware_metrics(&mut registry_svc).expect("Failed registering hardware metrics");

        // // Scrape /metrics endpoint
        // for _ in 1..5 {
        //     let url = format!(
        //         "http://0.0.0.0:{}{}",
        //         prom_server_addr.port(),
        //         METRICS_ROUTE
        //     );
        //     let response = reqwest::get(url).await.unwrap();
        //     dbg!(&response);
        //     tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        // }

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

        let mut buf: Vec<u8> = vec![];
        let encoder = prometheus::ProtobufEncoder::new();
        encoder
            .encode(&metric_families, &mut buf)
            .map_err(|e| format!("failed encoding: {e}"))?;

        let mut s = snap::raw::Encoder::new();
        let compressed = s
            .compress_vec(&buf)
            .map_err(|err| format!("unable to snappy encode; {err}"))?;

        dbg!(compressed);

        Ok(())
    }

    // #[test]
    // fn test_hardware_metrics() -> Result<(), Box<dyn std::error::Error>> {
    //     let metrics_collector = HardwareMetrics::new()?;
    //     let metrics = metrics_collector.collect();
    //     dbg!(metrics);
    //     Ok(())
    // }

    // #[test]
    // fn test_cpu_metrics() -> Result<(), Box<dyn std::error::Error>> {
    //     let mut system = System::new_with_specifics(
    //         RefreshKind::nothing().with_cpu(CpuRefreshKind::everything()),
    //     );
    //     let mut cpu_metrics = CpuMetrics::new(&mut system)?;

    //     // let r = Registry::new();
    //     // r.register(Box::new(cpu_metrics.clone())).unwrap();

    //     system.refresh_all();
    //     let metrics = cpu_metrics.collect();
    //     dbg!(&metrics);

    //     // assert_eq!(cpu_metrics.desc().len(), 1);
    //     Ok(())
    // }
}
