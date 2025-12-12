// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashMap, fs, io::BufRead, path::Path, time::Duration};

use prettytable::{Table, row};
use prometheus_parse::Scrape;
use serde::{Deserialize, Serialize};

use crate::{
    benchmark::{BenchmarkParameters, BenchmarkType},
    display,
    protocol::ProtocolMetrics,
    settings::Settings,
};

/// The identifier of prometheus latency buckets.
type BucketId = String;

/// A snapshot measurement at a given time.
#[derive(Serialize, Deserialize, Default, Clone)]
pub struct Measurement {
    /// The type of the workload, e.g. "transfer_object", "shared_counter".
    pub workload: String,
    /// Duration since the beginning of the benchmark.
    timestamp: Duration,
    /// Latency buckets.
    buckets: HashMap<BucketId, usize>,
    /// Sum of the latencies of all finalized transactions.
    sum: Duration,
    /// Total number of finalized transactions
    count: usize,
    /// Square of the latencies of all finalized transactions.
    squared_sum: Duration,
}

impl Measurement {
    /// Parse measurements from Prometheus metrics text format.
    pub fn from_prometheus<M: ProtocolMetrics>(text: &str) -> HashMap<String, Self> {
        let br = std::io::BufReader::new(text.as_bytes());
        let parsed = Scrape::parse(br.lines()).expect("Failed to parse Prometheus metrics");

        // Pre-group samples by workload to avoid repeated iteration
        let mut samples_by_workload: HashMap<String, Vec<&prometheus_parse::Sample>> =
            HashMap::new();
        for sample in &parsed.samples {
            if let Some(workload) = sample.labels.get("workload") {
                samples_by_workload
                    .entry(workload.to_string())
                    .or_default()
                    .push(sample);
            }
        }

        if samples_by_workload.is_empty() {
            // No workload labels found; return empty measurements
            return HashMap::new();
        }

        // Also get the global timestamp (without workload label) as fallback
        let global_timestamp = parsed
            .samples
            .iter()
            .find(|x| x.metric == M::BENCHMARK_DURATION && x.labels.get("workload").is_none())
            .and_then(|x| match x.value {
                prometheus_parse::Value::Gauge(value) => Some(Duration::from_secs(value as u64)),
                _ => None,
            })
            .unwrap_or_default();

        // Extract the measurement for each workload.
        samples_by_workload
            .into_iter()
            .map(|(workload, workload_samples)| {
                let buckets: HashMap<_, _> = workload_samples
                    .iter()
                    .find(|x| x.metric == M::LATENCY_BUCKETS)
                    .and_then(|sample| match &sample.value {
                        prometheus_parse::Value::Histogram(values) => Some(
                            values
                                .iter()
                                .map(|x| (x.less_than.to_string(), x.count as usize))
                                .collect(),
                        ),
                        _ => None,
                    })
                    .unwrap_or_default();

                let sum = workload_samples
                    .iter()
                    .find(|x| x.metric == M::LATENCY_SUM)
                    .and_then(|sample| match sample.value {
                        prometheus_parse::Value::Untyped(value) => {
                            Some(Duration::from_secs_f64(value))
                        }
                        _ => None,
                    })
                    .unwrap_or_default();

                let count = workload_samples
                    .iter()
                    .find(|x| x.metric == M::TOTAL_TRANSACTIONS)
                    .and_then(|sample| match sample.value {
                        prometheus_parse::Value::Untyped(value) => Some(value as usize),
                        _ => None,
                    })
                    .unwrap_or_default();

                let squared_sum = workload_samples
                    .iter()
                    .find(|x| x.metric == M::LATENCY_SQUARED_SUM)
                    .and_then(|sample| match sample.value {
                        prometheus_parse::Value::Counter(value) => {
                            Some(Duration::from_secs_f64(value))
                        }
                        _ => None,
                    })
                    .unwrap_or_default();

                // Try to get workload-specific timestamp, fall back to global timestamp
                let timestamp = workload_samples
                    .iter()
                    .find(|x| x.metric == M::BENCHMARK_DURATION)
                    .and_then(|sample| match sample.value {
                        prometheus_parse::Value::Gauge(value) => {
                            Some(Duration::from_secs(value as u64))
                        }
                        _ => None,
                    })
                    .unwrap_or(global_timestamp);

                let measurement = Self {
                    workload: workload.clone(),
                    timestamp,
                    buckets,
                    sum,
                    count,
                    squared_sum,
                };

                (workload, measurement)
            })
            .collect()
    }

    /// Compute the tps.
    /// NOTE: Do not use `self.timestamp` as benchmark duration because some
    /// clients may be unable to submit transactions passed the first few
    /// seconds of the benchmark. This may happen as a result of a bad
    /// control system within the nodes.
    pub fn tps(&self, duration: &Duration) -> u64 {
        let tps = self.count.checked_div(duration.as_secs() as usize);
        tps.unwrap_or_default() as u64
    }

    /// Compute the average latency.
    pub fn average_latency(&self) -> Duration {
        self.sum.checked_div(self.count as u32).unwrap_or_default()
    }

    /// Compute the standard deviation from the sum of squared latencies:
    /// `stdev = sqrt( squared_sum / count - avg^2 )`
    pub fn stdev_latency(&self) -> Duration {
        // Compute `squared_sum / count`.
        let first_term = if self.count == 0 {
            0.0
        } else {
            self.squared_sum.as_secs_f64() / self.count as f64
        };

        // Compute `avg^2`.
        let squared_avg = self.average_latency().as_secs_f64().powf(2.0);

        // Compute `squared_sum / count - avg^2`.
        let variance = if squared_avg > first_term {
            0.0
        } else {
            first_term - squared_avg
        };

        // Compute `sqrt( squared_sum / count - avg^2 )`.
        let stdev = variance.sqrt();
        Duration::from_secs_f64(stdev)
    }

    #[cfg(test)]
    pub fn new_for_test(workload: String) -> Self {
        Self {
            workload,
            timestamp: Duration::from_secs(30),
            buckets: HashMap::new(),
            sum: Duration::from_secs(1265),
            count: 1860,
            squared_sum: Duration::from_secs(952),
        }
    }
}

/// The identifier of the scrapers collecting the prometheus metrics.
type ScraperId = usize;

#[derive(Serialize, Deserialize, Clone)]
pub struct MeasurementsCollection<T> {
    /// The machine / instance type.
    pub machine_specs: String,
    /// The commit of the codebase.
    pub commit: String,
    /// The benchmark parameters of the current run.
    pub parameters: BenchmarkParameters<T>,
    /// The data collected by each scraper, organized by workload.
    pub scrapers: HashMap<ScraperId, HashMap<String, Vec<Measurement>>>,
}

impl<T: BenchmarkType> MeasurementsCollection<T> {
    /// Create a new (empty) collection of measurements.
    pub fn new(settings: &Settings, parameters: BenchmarkParameters<T>) -> Self {
        Self {
            machine_specs: settings.node_specs.clone(),
            commit: settings.repository.commit.clone(),
            parameters,
            scrapers: HashMap::new(),
        }
    }

    /// Load a collection of measurement from a json file.
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, std::io::Error> {
        let data = fs::read(path)?;
        let measurements: Self = serde_json::from_slice(data.as_slice())?;
        Ok(measurements)
    }

    /// Add a new measurement to the collection.
    pub fn add(&mut self, scraper_id: ScraperId, measurements: HashMap<String, Measurement>) {
        let scraper_workloads = self.scrapers.entry(scraper_id).or_default();
        for (workload, workload_measurement) in measurements {
            scraper_workloads
                .entry(workload)
                .or_default()
                .push(workload_measurement);
        }
    }

    /// Return the transaction (input) load of the benchmark.
    pub fn transaction_load(&self) -> usize {
        self.parameters.load
    }

    /// Aggregate the benchmark duration of multiple data points by taking the
    /// max.
    pub fn benchmark_duration(&self) -> Duration {
        self.last_measurements_iter()
            .map(|x| x.timestamp)
            .max()
            .unwrap_or_default()
    }

    pub fn workload_tps(&self) -> HashMap<String, u64> {
        // Collect all last measurements
        let last_measurements: Vec<_> = self.last_measurements_iter().collect();

        // Get the maximum timestamp
        let duration = last_measurements
            .iter()
            .map(|x| x.timestamp)
            .max()
            .unwrap_or_default();

        last_measurements
            .into_iter()
            // Sum TPS for each workload across all scrapers
            .fold(HashMap::new(), |mut acc, measurement| {
                *acc.entry(measurement.workload.clone()).or_insert(0) += measurement.tps(&duration);
                acc
            })
    }

    /// Aggregate the tps of multiple data points by taking the sum.
    /// Calculates TPS for each workload separately, then sums across all
    /// workloads.
    pub fn aggregate_tps(&self) -> u64 {
        // Collect all last measurements
        let last_measurements: Vec<_> = self.last_measurements_iter().collect();

        // Get the maximum timestamp
        let duration = last_measurements
            .iter()
            .map(|x| x.timestamp)
            .max()
            .unwrap_or_default();

        // Calculate and sum TPS for each measurement
        last_measurements.iter().map(|x| x.tps(&duration)).sum()
    }

    pub fn workload_average_latency(&self) -> HashMap<String, Duration> {
        self.last_measurements_iter()
            // get the maximum latency of each workload across all scrapers
            .fold(HashMap::new(), |mut acc, measurement| {
                let latency = measurement.average_latency();
                acc.entry(measurement.workload.clone())
                    .and_modify(|max_latency| {
                        if latency > *max_latency {
                            *max_latency = latency;
                        }
                    })
                    .or_insert(latency);
                acc
            })
    }

    /// Aggregate the average latency of multiple data points by taking the
    /// average.
    pub fn aggregate_average_latency(&self) -> Duration {
        let last_measurements: Vec<_> = self.last_measurements_iter().collect();

        last_measurements
            .iter()
            .map(|x| x.average_latency())
            .sum::<Duration>()
            .checked_div(last_measurements.len() as u32)
            .unwrap_or_default()
    }

    pub fn workload_stdev_latency(&self) -> HashMap<String, Duration> {
        self.last_measurements_iter()
            // get the maximum stdev latency of each workload across all scrapers
            .fold(HashMap::new(), |mut acc, measurement| {
                let stdev = measurement.stdev_latency();
                acc.entry(measurement.workload.clone())
                    .and_modify(|max_stdev| {
                        if stdev > *max_stdev {
                            *max_stdev = stdev;
                        }
                    })
                    .or_insert(stdev);
                acc
            })
    }

    /// Aggregate the stdev latency of multiple data points by taking the max.
    pub fn aggregate_stdev_latency(&self) -> Duration {
        self.last_measurements_iter()
            .map(|x| x.stdev_latency())
            .max()
            .unwrap_or_default()
    }

    /// Save the collection of measurements as a json file.
    pub fn save<P: AsRef<Path>>(&self, path: P) {
        let json = serde_json::to_string_pretty(self).expect("Cannot serialize metrics");
        let file = path
            .as_ref()
            .join(format!("measurements-{:?}.json", self.parameters));
        fs::write(file, json).unwrap();
    }

    /// Display a summary of the measurements.
    pub fn display_summary(&self) {
        let duration = self.benchmark_duration();
        let workload_tps = self.workload_tps();
        let total_tps = self.aggregate_tps();
        let workload_latency = self.workload_average_latency();
        let average_latency = self.aggregate_average_latency();
        let workload_stdev_latency = self.workload_stdev_latency();
        let stdev_latency = self.aggregate_stdev_latency();

        let mut table = Table::new();
        table.set_format(display::default_table_format());

        table.set_titles(row![bH2->"Benchmark Summary"]);
        table.add_row(row![b->"Benchmark type:", self.parameters.benchmark_type]);
        table.add_row(row![bH2->""]);
        table.add_row(row![b->"Nodes:", self.parameters.nodes]);
        table.add_row(
            row![b->"Use internal IPs:", format!("{}", self.parameters.use_internal_ip_address)],
        );
        table.add_row(row![b->"Faults:", self.parameters.faults]);
        table.add_row(row![b->"Load:", format!("{} tx/s", self.parameters.load)]);
        table.add_row(row![b->"Duration:", format!("{} s", duration.as_secs())]);
        table.add_row(row![bH2->""]);
        table.add_row(row![b->"TPS:", format!("{total_tps} tx/s")]);
        for (workload, tps) in &workload_tps {
            table.add_row(row![b->format!("  {workload} TPS:"), format!("{tps} tx/s")]);
        }
        table.add_row(row![bH2->""]);

        table.add_row(row![b->"Latency (avg):", format!("{} ms", average_latency.as_millis())]);
        for (workload, latency) in &workload_latency {
            table.add_row(
                row![b->format!("  {workload} Latency:" ), format!("{} ms", latency.as_millis())],
            );
        }
        table.add_row(row![bH2->""]);

        table.add_row(row![b->"Latency (stdev):", format!("{} ms", stdev_latency.as_millis())]);
        for (workload, latency) in &workload_stdev_latency {
            table.add_row(
                row![b->format!("  {workload} Latency:"), format!("{} ms", latency.as_millis())],
            );
        }

        display::newline();
        table.printstd();
        display::newline();
    }

    // Get an iterator over the last measurements of all workloads across all
    // scrapers
    fn last_measurements_iter(&self) -> impl Iterator<Item = &Measurement> {
        self.scrapers
            .values()
            .flat_map(|workload_map| workload_map.values())
            .filter_map(|measurements| measurements.last())
    }
}

#[cfg(test)]
mod test {
    use std::{collections::HashMap, time::Duration};

    use super::{BenchmarkParameters, Measurement, MeasurementsCollection};
    use crate::{
        benchmark::test::TestBenchmarkType, protocol::test_protocol_metrics::TestProtocolMetrics,
        settings::Settings,
    };

    #[test]
    fn average_latency() {
        let data = Measurement {
            workload: "transfer_object".into(),
            timestamp: Duration::from_secs(10),
            buckets: HashMap::new(),
            sum: Duration::from_secs(2),
            count: 100,
            squared_sum: Duration::from_secs(0),
        };

        assert_eq!(data.average_latency(), Duration::from_millis(20));
    }

    #[test]
    fn stdev_latency() {
        let data = Measurement {
            workload: "transfer_object".into(),
            timestamp: Duration::from_secs(10),
            buckets: HashMap::new(),
            sum: Duration::from_secs(50),
            count: 100,
            squared_sum: Duration::from_secs(75),
        };

        // squared_sum / count
        assert_eq!(
            data.squared_sum.checked_div(data.count as u32),
            Some(Duration::from_secs_f64(0.75))
        );
        // avg^2
        assert_eq!(data.average_latency().as_secs_f64().powf(2.0), 0.25);
        // sqrt( squared_sum / count - avg^2 )
        let stdev = data.stdev_latency();
        assert_eq!((stdev.as_secs_f64() * 10.0).round(), 7.0);
    }

    #[test]
    fn prometheus_parse() {
        let report = r#"
            # HELP benchmark_duration Duration of the benchmark
            # TYPE benchmark_duration gauge
            benchmark_duration 30
            # HELP latency_s Total time in seconds to return a response
            # TYPE latency_s histogram
            latency_s_bucket{workload=transfer_object,le=0.1} 0
            latency_s_bucket{workload=transfer_object,le=0.25} 0
            latency_s_bucket{workload=transfer_object,le=0.5} 506
            latency_s_bucket{workload=transfer_object,le=0.75} 1282
            latency_s_bucket{workload=transfer_object,le=1} 1693
            latency_s_bucket{workload="transfer_object",le="1.25"} 1816
            latency_s_bucket{workload="transfer_object",le="1.5"} 1860
            latency_s_bucket{workload="transfer_object",le="1.75"} 1860
            latency_s_bucket{workload="transfer_object",le="2"} 1860
            latency_s_bucket{workload=transfer_object,le=2.5} 1860
            latency_s_bucket{workload=transfer_object,le=5} 1860
            latency_s_bucket{workload=transfer_object,le=10} 1860
            latency_s_bucket{workload=transfer_object,le=20} 1860
            latency_s_bucket{workload=transfer_object,le=30} 1860
            latency_s_bucket{workload=transfer_object,le=60} 1860
            latency_s_bucket{workload=transfer_object,le=90} 1860
            latency_s_bucket{workload=transfer_object,le=+Inf} 1860
            latency_s_sum{workload=transfer_object} 1265.287933130998
            latency_s_count{workload=transfer_object} 1860
            # HELP latency_squared_s Square of total time in seconds to return a response
            # TYPE latency_squared_s counter
            latency_squared_s{workload="transfer_object"} 952.8160642745289
        "#;

        let measurement = Measurement::from_prometheus::<TestProtocolMetrics>(report);
        let settings = Settings::new_for_test();
        let mut aggregator = MeasurementsCollection::<TestBenchmarkType>::new(
            &settings,
            BenchmarkParameters::default(),
        );
        let scraper_id = 1;
        aggregator.add(scraper_id, measurement);

        assert_eq!(aggregator.scrapers.len(), 1);
        let scraper_data = aggregator.scrapers.get(&scraper_id).unwrap();
        assert_eq!(scraper_data.len(), 1); // One workload

        let data_points = scraper_data.get("transfer_object").unwrap();
        assert_eq!(data_points.len(), 1);

        let data = &data_points[0];
        assert_eq!(
            data.buckets,
            ([
                ("0.1".into(), 0),
                ("0.25".into(), 0),
                ("0.5".into(), 506),
                ("0.75".into(), 1282),
                ("1".into(), 1693),
                ("1.25".into(), 1816),
                ("1.5".into(), 1860),
                ("1.75".into(), 1860),
                ("2".into(), 1860),
                ("2.5".into(), 1860),
                ("5".into(), 1860),
                ("10".into(), 1860),
                ("20".into(), 1860),
                ("30".into(), 1860),
                ("60".into(), 1860),
                ("90".into(), 1860),
                ("inf".into(), 1860)
            ])
            .iter()
            .cloned()
            .collect()
        );
        assert_eq!(data.sum.as_secs(), 1265);
        assert_eq!(data.count, 1860);
        assert_eq!(data.timestamp.as_secs(), 30);
        assert_eq!(data.squared_sum.as_secs(), 952);
    }

    #[test]
    fn prometheus_parse_multi_workloads() {
        let report = r#"
            # HELP benchmark_duration Duration of the benchmark
            # TYPE benchmark_duration gauge
            benchmark_duration 30
            # HELP latency_s Total time in seconds to return a response
            # TYPE latency_s histogram
            latency_s_bucket{workload=transfer_object,le=0.1} 0
            latency_s_bucket{workload=transfer_object,le=0.25} 0
            latency_s_bucket{workload=transfer_object,le=0.5} 506
            latency_s_bucket{workload=transfer_object,le=0.75} 1282
            latency_s_bucket{workload=transfer_object,le=1} 1693
            latency_s_bucket{workload="transfer_object",le="1.25"} 1816
            latency_s_bucket{workload="transfer_object",le="1.5"} 1860
            latency_s_bucket{workload="transfer_object",le="1.75"} 1860
            latency_s_bucket{workload="transfer_object",le="2"} 1860
            latency_s_bucket{workload=transfer_object,le=2.5} 1860
            latency_s_bucket{workload=transfer_object,le=5} 1860
            latency_s_bucket{workload=transfer_object,le=10} 1860
            latency_s_bucket{workload=transfer_object,le=20} 1860
            latency_s_bucket{workload=transfer_object,le=30} 1860
            latency_s_bucket{workload=transfer_object,le=60} 1860
            latency_s_bucket{workload=transfer_object,le=90} 1860
            latency_s_bucket{workload=transfer_object,le=+Inf} 1860
            latency_s_sum{workload=transfer_object} 1265.287933130998
            latency_s_count{workload=transfer_object} 1860
            # HELP latency_squared_s Square of total time in seconds to return a response
            # TYPE latency_squared_s counter
            latency_squared_s{workload="transfer_object"} 952.8160642745289
            latency_s_bucket{workload=shared_counter,le=0.1} 0
            latency_s_bucket{workload=shared_counter,le=0.25} 1
            latency_s_bucket{workload=shared_counter,le=0.5} 600
            latency_s_bucket{workload=shared_counter,le=0.75} 1200
            latency_s_bucket{workload=shared_counter,le=1} 1600
            latency_s_bucket{workload="shared_counter",le="1.25"} 1800
            latency_s_bucket{workload="shared_counter",le="1.5"} 1870
            latency_s_bucket{workload="shared_counter",le="1.75"} 1870
            latency_s_bucket{workload="shared_counter",le="2"} 1870
            latency_s_bucket{workload=shared_counter,le=2.5} 1870
            latency_s_bucket{workload=shared_counter,le=5} 1870
            latency_s_bucket{workload=shared_counter,le=10} 1870
            latency_s_bucket{workload=shared_counter,le=20} 1870
            latency_s_bucket{workload=shared_counter,le=30} 1870
            latency_s_bucket{workload=shared_counter,le=60} 1870
            latency_s_bucket{workload=shared_counter,le=90} 1870
            latency_s_bucket{workload=shared_counter,le=+Inf} 1870
            latency_s_sum{workload=shared_counter} 865.287933130998
            latency_s_count{workload=shared_counter} 1870
            # HELP latency_squared_s Square of total time in seconds to return a response
            # TYPE latency_squared_s counter
            latency_squared_s{workload="shared_counter"} 455.8160642745289
        "#;

        let measurements = Measurement::from_prometheus::<TestProtocolMetrics>(report);
        let settings = Settings::new_for_test();
        let mut aggregator = MeasurementsCollection::<TestBenchmarkType>::new(
            &settings,
            BenchmarkParameters::default(),
        );
        let scraper_id = 1;

        aggregator.add(scraper_id, measurements);

        assert_eq!(aggregator.scrapers.len(), 1);
        let scraper_data = aggregator.scrapers.get(&scraper_id).unwrap();
        assert_eq!(scraper_data.len(), 2); // Two workloads

        let data_points = scraper_data.get("transfer_object").unwrap();
        assert_eq!(data_points.len(), 1);

        let data = &data_points[0];
        assert_eq!(
            data.buckets,
            ([
                ("0.1".into(), 0),
                ("0.25".into(), 0),
                ("0.5".into(), 506),
                ("0.75".into(), 1282),
                ("1".into(), 1693),
                ("1.25".into(), 1816),
                ("1.5".into(), 1860),
                ("1.75".into(), 1860),
                ("2".into(), 1860),
                ("2.5".into(), 1860),
                ("5".into(), 1860),
                ("10".into(), 1860),
                ("20".into(), 1860),
                ("30".into(), 1860),
                ("60".into(), 1860),
                ("90".into(), 1860),
                ("inf".into(), 1860)
            ])
            .iter()
            .cloned()
            .collect()
        );
        assert_eq!(data.sum.as_secs(), 1265);
        assert_eq!(data.count, 1860);
        assert_eq!(data.timestamp.as_secs(), 30);
        assert_eq!(data.squared_sum.as_secs(), 952);

        let data_points = scraper_data.get("shared_counter").unwrap();
        assert_eq!(data_points.len(), 1);

        let data = &data_points[0];
        assert_eq!(
            data.buckets,
            ([
                ("0.1".into(), 0),
                ("0.25".into(), 1),
                ("0.5".into(), 600),
                ("0.75".into(), 1200),
                ("1".into(), 1600),
                ("1.25".into(), 1800),
                ("1.5".into(), 1870),
                ("1.75".into(), 1870),
                ("2".into(), 1870),
                ("2.5".into(), 1870),
                ("5".into(), 1870),
                ("10".into(), 1870),
                ("20".into(), 1870),
                ("30".into(), 1870),
                ("60".into(), 1870),
                ("90".into(), 1870),
                ("inf".into(), 1870)
            ])
            .iter()
            .cloned()
            .collect()
        );
        assert_eq!(data.sum.as_secs(), 865);
        assert_eq!(data.count, 1870);
        assert_eq!(data.timestamp.as_secs(), 30);
        assert_eq!(data.squared_sum.as_secs(), 455);
    }
}
