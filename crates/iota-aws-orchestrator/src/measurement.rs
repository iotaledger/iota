// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashMap, fs, io::BufRead, path::Path, time::Duration};

use prettytable::{Table, row};
use prometheus_parse::Scrape;
use serde::{Deserialize, Serialize};

use crate::{
    IotaBenchmarkType,
    benchmark::{BenchmarkParameters, BenchmarkType, RunInterval},
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

        last_measurements
            .into_iter()
            // Sum TPS for each workload across all scrapers
            .fold(HashMap::new(), |mut acc, measurement| {
                *acc.entry(measurement.workload.clone()).or_insert(0) +=
                    measurement.tps(&measurement.timestamp);
                acc
            })
    }

    /// Aggregate the tps of multiple data points by taking the sum.
    /// Calculates TPS for each workload separately, then sums across all
    /// workloads.
    pub fn aggregate_tps(&self) -> u64 {
        // Collect all last measurements
        let last_measurements: Vec<_> = self.last_measurements_iter().collect();

        // Calculate and sum TPS for each measurement
        last_measurements.iter().map(|x| x.tps(&x.timestamp)).sum()
    }

    pub fn workload_average_latency(&self) -> HashMap<String, Duration> {
        // Collect sum and count for each workload across all scrapers
        let mut workload_data: HashMap<String, (Duration, usize)> = HashMap::new();

        for measurement in self.last_measurements_iter() {
            workload_data
                .entry(measurement.workload.clone())
                .and_modify(|(sum, count)| {
                    *sum += measurement.sum;
                    *count += measurement.count;
                })
                .or_insert((measurement.sum, measurement.count));
        }

        // Calculate average for each workload
        workload_data
            .into_iter()
            .map(|(workload, (sum, count))| {
                let avg = if count == 0 {
                    Duration::default()
                } else {
                    sum.checked_div(count as u32).unwrap_or_default()
                };
                (workload, avg)
            })
            .collect()
    }

    /// Aggregate the average latency of multiple data points by taking the
    /// weighted average based on transaction counts.
    /// This computes: (sum of all latency_sum) / (sum of all counts)
    pub fn aggregate_average_latency(&self) -> Duration {
        let last_measurements: Vec<_> = self.last_measurements_iter().collect();

        let total_sum: Duration = last_measurements.iter().map(|x| x.sum).sum();
        let total_count: usize = last_measurements.iter().map(|x| x.count).sum();

        if total_count == 0 {
            return Duration::default();
        }

        total_sum
            .checked_div(total_count as u32)
            .unwrap_or_default()
    }

    pub fn workload_stdev_latency(&self) -> HashMap<String, Duration> {
        // Collect sum, squared_sum, and count for each workload across all scrapers
        let mut workload_data: HashMap<String, (Duration, Duration, usize)> = HashMap::new();

        for measurement in self.last_measurements_iter() {
            workload_data
                .entry(measurement.workload.clone())
                .and_modify(|(sum, squared_sum, count)| {
                    *sum += measurement.sum;
                    *squared_sum += measurement.squared_sum;
                    *count += measurement.count;
                })
                .or_insert((measurement.sum, measurement.squared_sum, measurement.count));
        }

        // Calculate stdev for each workload from aggregated data
        workload_data
            .into_iter()
            .map(|(workload, (sum, squared_sum, count))| {
                let stdev = if count == 0 {
                    Duration::default()
                } else {
                    let first_term = squared_sum.as_secs_f64() / count as f64;
                    let avg = sum.as_secs_f64() / count as f64;
                    let variance = if avg.powf(2.0) > first_term {
                        0.0
                    } else {
                        first_term - avg.powf(2.0)
                    };
                    Duration::from_secs_f64(variance.sqrt())
                };
                (workload, stdev)
            })
            .collect()
    }

    /// Aggregate the stdev latency by combining all squared sums, sums, and
    /// counts. Uses the pooled variance formula: sqrt((Σsquared_sum /
    /// Σcount) - (Σsum / Σcount)^2)
    pub fn aggregate_stdev_latency(&self) -> Duration {
        let last_measurements: Vec<_> = self.last_measurements_iter().collect();

        let total_sum: Duration = last_measurements.iter().map(|x| x.sum).sum();
        let total_squared_sum: Duration = last_measurements.iter().map(|x| x.squared_sum).sum();
        let total_count: usize = last_measurements.iter().map(|x| x.count).sum();

        if total_count == 0 {
            return Duration::default();
        }

        let first_term = total_squared_sum.as_secs_f64() / total_count as f64;
        let avg = total_sum.as_secs_f64() / total_count as f64;
        let variance = if avg.powf(2.0) > first_term {
            0.0
        } else {
            first_term - avg.powf(2.0)
        };

        Duration::from_secs_f64(variance.sqrt())
    }

    pub fn workload_p50_latency(&self) -> HashMap<String, Duration> {
        // Aggregate buckets and counts for each workload across all scrapers
        let mut workload_data: HashMap<String, (HashMap<BucketId, usize>, usize)> = HashMap::new();

        for measurement in self.last_measurements_iter() {
            workload_data
                .entry(measurement.workload.clone())
                .and_modify(|(buckets, count)| {
                    // Sum bucket counts
                    for (bucket_id, bucket_count) in &measurement.buckets {
                        *buckets.entry(bucket_id.clone()).or_insert(0) += bucket_count;
                    }
                    *count += measurement.count;
                })
                .or_insert((measurement.buckets.clone(), measurement.count));
        }

        // Calculate P50 for each workload from aggregated buckets
        workload_data
            .into_iter()
            .map(|(workload, (buckets, count))| {
                let p50 = p50_latency(&buckets, count);
                (workload, p50)
            })
            .collect()
    }

    /// Aggregate the P50 latency by combining all histogram buckets and
    /// calculating P50 from the combined histogram.
    pub fn aggregate_p50_latency(&self) -> Duration {
        let last_measurements: Vec<_> = self.last_measurements_iter().collect();

        // Aggregate all buckets across all workloads and scrapers
        let mut combined_buckets: HashMap<BucketId, usize> = HashMap::new();
        let mut total_count = 0;

        for measurement in &last_measurements {
            for (bucket_id, bucket_count) in &measurement.buckets {
                *combined_buckets.entry(bucket_id.clone()).or_insert(0) += bucket_count;
            }
            total_count += measurement.count;
        }

        // Calculate P50 from combined histogram
        p50_latency(&combined_buckets, total_count)
    }

    pub fn workload_p99_latency(&self) -> HashMap<String, Duration> {
        // Aggregate buckets and counts for each workload across all scrapers
        let mut workload_data: HashMap<String, (HashMap<BucketId, usize>, usize)> = HashMap::new();

        for measurement in self.last_measurements_iter() {
            workload_data
                .entry(measurement.workload.clone())
                .and_modify(|(buckets, count)| {
                    // Sum bucket counts
                    for (bucket_id, bucket_count) in &measurement.buckets {
                        *buckets.entry(bucket_id.clone()).or_insert(0) += bucket_count;
                    }
                    *count += measurement.count;
                })
                .or_insert((measurement.buckets.clone(), measurement.count));
        }

        // Calculate P99 for each workload from aggregated buckets
        workload_data
            .into_iter()
            .map(|(workload, (buckets, count))| {
                let p99 = p99_latency(&buckets, count);
                (workload, p99)
            })
            .collect()
    }

    /// Aggregate the P99 latency by combining all histogram buckets and
    /// calculating P99 from the combined histogram.
    pub fn aggregate_p99_latency(&self) -> Duration {
        let last_measurements: Vec<_> = self.last_measurements_iter().collect();

        // Aggregate all buckets across all workloads and scrapers
        let mut combined_buckets: HashMap<BucketId, usize> = HashMap::new();
        let mut total_count = 0;

        for measurement in &last_measurements {
            for (bucket_id, bucket_count) in &measurement.buckets {
                *combined_buckets.entry(bucket_id.clone()).or_insert(0) += bucket_count;
            }
            total_count += measurement.count;
        }

        p99_latency(&combined_buckets, total_count)
    }

    /// Save the collection of measurements as a json file.
    pub fn save<P: AsRef<Path>>(&self, path: P) {
        let json = serde_json::to_string_pretty(self).expect("Cannot serialize metrics");
        let file = path
            .as_ref()
            .join(format!("measurements-{:?}.json", self.parameters));
        fs::write(file, json).unwrap();
    }

    pub fn aggregates_metrics_from_files<M: ProtocolMetrics>(
        &mut self,
        num_clients: usize,
        log_dir: &Path,
    ) {
        display::action("Processing metrics files");

        // IMPORTANT:
        // - Time-mode: keep only samples within [0 ..= duration_secs]
        // - Count-mode: do NOT cut by time (the run ends by tx-count, not wall clock)
        let time_limit_secs: Option<u64> = match self.parameters.run_interval {
            RunInterval::Time(d) => Some(d.as_secs()),
            RunInterval::Count(_) => None,
        };

        for i in 0..num_clients {
            let metrics_file = log_dir.join(format!("metrics-{i}.log"));

            if !metrics_file.exists() {
                continue;
            }

            match fs::read_to_string(&metrics_file) {
                Ok(content) => {
                    display::action(format!("Processing: {}\n", metrics_file.display()));

                    let chunks = self.split_into_chunks(&content);
                    for chunk in &chunks {
                        let mut measurements: HashMap<String, Measurement> =
                            Measurement::from_prometheus::<M>(chunk);

                        if let Some(limit) = time_limit_secs {
                            // Retain only measurements within the benchmark duration (seconds since
                            // start).
                            measurements.retain(|_, m| m.timestamp.as_secs() <= limit);
                        }

                        self.add(i, measurements);
                    }

                    display::action(format!("Processed metrics for client {i}\n"));
                }
                Err(e) => display::warn(format!("Failed to read metrics file {i}: {e}")),
            }
        }

        display::done();
    }

    /// Split metrics content into chunks separated by "# HELP
    /// benchmark_duration" lines
    fn split_into_chunks(&self, text: &str) -> Vec<String> {
        let mut chunks = Vec::new();
        let mut current_chunk = String::new();
        let mut found_first_help = false;

        for line in text.lines() {
            let trimmed = line.trim();

            // Skip everything until we find the first "# HELP benchmark_duration"
            if trimmed.starts_with("# HELP benchmark_duration") {
                if found_first_help && !current_chunk.is_empty() {
                    // We've found another chunk boundary, save the previous one
                    chunks.push(current_chunk);
                    current_chunk = String::new();
                }
                found_first_help = true;
            }

            if found_first_help {
                current_chunk.push_str(line);
                current_chunk.push('\n');
            }
        }

        // Add the last chunk
        if !current_chunk.is_empty() {
            chunks.push(current_chunk);
        }

        chunks
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
        let workload_p50_latency = self.workload_p50_latency();
        let p50_latency = self.aggregate_p50_latency();
        let workload_p99_latency = self.workload_p99_latency();
        let p99_latency = self.aggregate_p99_latency();

        let target = self.parameters.load as f64;
        let achieved = total_tps as f64;
        let efficiency = if target > 0.0 {
            100.0 * achieved / target
        } else {
            0.0
        };

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

        // Workload config
        table.add_row(row![b->"Load (target):", format!("{} tx/s", self.parameters.load)]);
        table.add_row(row![b->"Duration:", format!("{} s", duration.as_secs())]);

        // Efficiency / saturation signal
        table.add_row(row![b->"Achieved TPS:", format!("{total_tps} tx/s")]);
        table.add_row(row![b->"Efficiency:", format!("{:.1}%", efficiency)]);
        table.add_row(row![bH2->""]);

        // AA-specific block

        if self.parameters.benchmark_type.to_string()
            == IotaBenchmarkType::AbstractAccountBench.to_string()
        {
            table.add_row(row![bH2->"AA config"]);
            table.add_row(row![b->"Authenticator:", self.parameters.aa_authenticator.to_string()]);
            table.add_row(row![b->"Stress workers:", self.parameters.stress_num_workers]);
            table.add_row(
                row![b->"Stress in-flight ratio:", self.parameters.stress_in_flight_ratio],
            );

            table.add_row(row![b->"AA split amount:", self.parameters.aa_split_amount]);

            table.add_row(
                row![b->"Stress client threads:", self.parameters.stress_num_client_threads],
            );
            table.add_row(
                row![b->"Stress server threads:", self.parameters.stress_num_server_threads],
            );
            table.add_row(row![bH2->""]);
        }

        println!("Grafana UI:");
        println!(
            "ssh -i /Users/pk/.ssh/aws_orchestrator -L 3000:127.0.0.1:3000 <ubuntu@<metrics_public_ip>"
        );

        table.add_row(row![bH2->"Per-workload throughput"]);
        for (workload, tps) in &workload_tps {
            table.add_row(row![b->format!("  {workload} TPS:"), format!("{tps} tx/s")]);
        }
        table.add_row(row![bH2->""]);

        table.add_row(row![bH2->"Latency"]);
        table.add_row(row![b->"Latency (avg):", format!("{} ms", average_latency.as_millis())]);
        table.add_row(row![b->"Latency (stdev):", format!("{} ms", stdev_latency.as_millis())]);

        table.add_row(row![bH2->""]);
        table.add_row(row![bH2->"Per-workload average latency"]);
        for (workload, latency) in &workload_latency {
            table.add_row(
                row![b->format!("  {workload} avg:"), format!("{} ms", latency.as_millis())],
            );
        }
        table.add_row(row![bH2->""]);

        table.add_row(row![b->"Latency (p50):", format!("{} ms", p50_latency.as_millis())]);
        for (workload, latency) in &workload_p50_latency {
            table.add_row(
                row![b->format!("  {workload} p50 Latency:"), format!("{} ms", latency.as_millis())],
            );
        }
        table.add_row(row![bH2->""]);

        table.add_row(row![b->"Latency (p99):", format!("{} ms", p99_latency.as_millis())]);
        for (workload, latency) in &workload_p99_latency {
            table.add_row(
                row![b->format!("  {workload} p99 Latency:"), format!("{} ms", latency.as_millis())],
            );
        }
        table.add_row(row![bH2->""]);

        table.add_row(row![b->"Latency (stdev):", format!("{} ms", stdev_latency.as_millis())]);
        for (workload, latency) in &workload_stdev_latency {
            table.add_row(
                row![b->format!("  {workload} stdev:"), format!("{} ms", latency.as_millis())],
            );
        }

        display::newline();
        table.printstd();

        // Also log the table to file
        let table_string = format!("{}", table);
        crate::logger::log(&table_string);

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

/// Compute the P50 (median) latency from histogram buckets.
fn p50_latency(buckets: &HashMap<BucketId, usize>, count: usize) -> Duration {
    histogram_quantile(buckets, count, 0.5)
}

/// Compute the P99 latency from histogram buckets using.
fn p99_latency(buckets: &HashMap<BucketId, usize>, count: usize) -> Duration {
    histogram_quantile(buckets, count, 0.99)
}

/// Calculate a quantile from histogram buckets using linear interpolation,
/// matching Prometheus's histogram_quantile behavior.
fn histogram_quantile(buckets: &HashMap<BucketId, usize>, count: usize, quantile: f64) -> Duration {
    if count == 0 || !(0.0..=1.0).contains(&quantile) {
        return Duration::default();
    }

    // Parse and sort buckets by boundary
    let mut buckets: Vec<(f64, usize)> = buckets
        .iter()
        .filter_map(|(bucket, count)| {
            let bound = if bucket == "inf" {
                f64::INFINITY
            } else {
                bucket.parse::<f64>().ok()?
            };
            Some((bound, *count))
        })
        .collect();

    if buckets.is_empty() {
        return Duration::default();
    }

    buckets.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    // The rank we're looking for (0.5 for P50 means the middle observation)
    let rank = quantile * count as f64;

    // Handle edge cases
    if rank < 0.0 {
        return Duration::default();
    }

    // Find the two buckets between which the quantile falls
    //
    // Example: Calculate P50 (median) with 1000 total observations
    // Buckets: [(0.5s, 400), (1.0s, 800), (2.0s, 1000)]
    // This means: 400 observations ≤ 0.5s, 800 observations ≤ 1.0s, 1000
    // observations ≤ 2.0s
    //
    // rank = 0.5 * 1000 = 500 (we want the 500th observation)
    //
    // Iteration 1: bound=0.5s, count=400
    //   - 400 < 500, so P50 is not in this bucket, continue
    //   - prev_count=400, prev_bound=0.5
    //
    // Iteration 2: bound=1.0s, count=800
    //   - 800 >= 500, so P50 is in this bucket (between observations 400-800)
    //   - Linear interpolation: fraction = (500 - 400) / (800 - 400) = 100 / 400 =
    //     0.25 interpolated = 0.5 + (1.0 - 0.5) * 0.25 = 0.5 + 0.125 = 0.625s
    //   - The 500th observation is estimated at 0.625s
    let mut prev_count = 0.0;
    let mut prev_bound = 0.0;

    for (bound, count) in buckets {
        let count_f64 = count as f64;

        // If this bucket contains our quantile
        if count_f64 >= rank {
            // If this is the first bucket or all observations are in this bucket
            if prev_count == 0.0 || count_f64 == prev_count {
                return Duration::from_secs_f64(bound);
            }

            // Linear interpolation between prev_bound and bound
            // Formula: prev_bound + (bound - prev_bound) * ((rank - prev_count) / (count -
            // prev_count))
            let fraction = (rank - prev_count) / (count_f64 - prev_count);
            let interpolated = prev_bound + (bound - prev_bound) * fraction;

            return Duration::from_secs_f64(interpolated);
        }

        prev_count = count_f64;
        prev_bound = bound;
    }

    // If we get here, return the last finite bucket boundary
    Duration::from_secs_f64(prev_bound)
}

#[cfg(test)]
mod test {
    use std::{collections::HashMap, time::Duration};

    use super::{BenchmarkParameters, Measurement, MeasurementsCollection};
    use crate::{
        benchmark::{RunInterval, test::TestBenchmarkType},
        protocol::test_protocol_metrics::TestProtocolMetrics,
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
    fn p50_latency() {
        // Test with the example histogram from prometheus_parse test
        // Total count: 1860, P50 should be at observation 930
        // Buckets show: 506 at 0.5s, 1282 at 0.75s
        // So P50 falls between 0.5s and 0.75s buckets
        // Linear interpolation: 0.5 + (0.75 - 0.5) * ((930 - 506) / (1282 - 506))
        //                     = 0.5 + 0.25 * (424 / 776)
        //                     = 0.5 + 0.25 * 0.5464
        //                     = 0.5 + 0.1366
        //                     = 0.6366s ≈ 637ms
        let data = Measurement {
            workload: "transfer_object".into(),
            timestamp: Duration::from_secs(30),
            buckets: [
                ("0.1".into(), 0),
                ("0.25".into(), 0),
                ("0.5".into(), 506),
                ("0.75".into(), 1282),
                ("1".into(), 1693),
                ("1.25".into(), 1816),
                ("1.5".into(), 1860),
                ("inf".into(), 1860),
            ]
            .iter()
            .cloned()
            .collect(),
            sum: Duration::from_secs(1265),
            count: 1860,
            squared_sum: Duration::from_secs(952),
        };

        let p50 = super::p50_latency(&data.buckets, data.count);
        // Should be around 636-637ms
        assert!(p50.as_millis() >= 636 && p50.as_millis() <= 637);
    }

    #[test]
    fn aggregate_average_latency_weighted() {
        // Test that aggregate average is properly weighted
        let settings = Settings::new_for_test();
        let mut aggregator = MeasurementsCollection::<TestBenchmarkType>::new(
            &settings,
            BenchmarkParameters::default(),
        );

        // Scraper 1: 100 transactions with 2s total = 20ms avg
        let measurement1 = HashMap::from([(
            "test".to_string(),
            Measurement {
                workload: "test".into(),
                timestamp: Duration::from_secs(10),
                buckets: HashMap::new(),
                sum: Duration::from_secs(2),
                count: 100,
                squared_sum: Duration::from_secs(0),
            },
        )]);

        // Scraper 2: 200 transactions with 10s total = 50ms avg
        let measurement2 = HashMap::from([(
            "test".to_string(),
            Measurement {
                workload: "test".into(),
                timestamp: Duration::from_secs(10),
                buckets: HashMap::new(),
                sum: Duration::from_secs(10),
                count: 200,
                squared_sum: Duration::from_secs(0),
            },
        )]);

        aggregator.add(1, measurement1);
        aggregator.add(2, measurement2);

        // Weighted average should be (2 + 10) / (100 + 200) = 12 / 300 = 0.04s = 40ms
        let avg = aggregator.aggregate_average_latency();
        assert_eq!(avg.as_millis(), 40);
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

    #[test]
    #[ignore]
    // This test could be used to debug / test existed metrics aggregation
    fn debug_real_metrics_aggregation() {
        use std::{path::PathBuf, time::Duration};

        // Put the path to the metrics log directory
        let metrics_dir = PathBuf::from("PATH/TO/YOUR/METRICS/DIR");

        println!("\n\n========== METRICS AGGREGATION DEBUG ==========\n");
        println!("Reading metrics from: {}\n", metrics_dir.display());

        let settings = Settings::new_for_test();
        let num_clients = 10;
        // Define benchmark parameters matching the real benchmark
        let benchmark_parameters = BenchmarkParameters {
            run_interval: RunInterval::Time(Duration::from_secs(180)),
            load: 1000,
            nodes: num_clients,
            ..Default::default()
        };

        let mut aggregator =
            MeasurementsCollection::<TestBenchmarkType>::new(&settings, benchmark_parameters);

        // Parse all metrics files
        aggregator.aggregates_metrics_from_files::<TestProtocolMetrics>(num_clients, &metrics_dir);

        println!("========== DISPLAY SUMMARY ==========\n");
        aggregator.display_summary();
    }

    #[test]
    #[ignore]
    // Load measurements from measurement-*.json and parse associated metrics files
    fn debug_metrics_from_saved_measurements() {
        use std::{fs, path::PathBuf};

        use crate::IotaBenchmarkType;

        let benchmark_dir = PathBuf::from("PATH/TO/YOUR/SAVED/MEASUREMENTS/DIR");

        // Find and parse the measurement-*.json file to get parameters
        let mut aggregator: Option<MeasurementsCollection<IotaBenchmarkType>> = None;
        if let Ok(entries) = fs::read_dir(&benchmark_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if let Some(filename) = path.file_name() {
                    let filename_str = filename.to_string_lossy();

                    if filename_str.starts_with("measurements-") {
                        match MeasurementsCollection::<IotaBenchmarkType>::load(&path) {
                            Ok(loaded) => {
                                println!("Loaded parameters from: {}\n", filename_str);
                                aggregator = Some(loaded);
                                break;
                            }
                            Err(e) => {
                                println!("Failed to load {}: {}\n", filename_str, e);
                            }
                        }
                    }
                }
            }
        }

        let aggregator = match aggregator {
            Some(agg) => agg,
            None => {
                panic!("No measurement-*.json file found or failed to load");
            }
        };

        println!("========== DISPLAY SUMMARY ==========\n");
        aggregator.display_summary();
    }
}
