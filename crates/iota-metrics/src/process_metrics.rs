// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Metrics of the node process, read from `/proc`.
//!
//! The standard `process_*` metrics come from the Prometheus process
//! collector. `process_open_fds_by_kind` splits the open file descriptors by
//! what they refer to, which the standard collector does not do.
//!
//! Both collectors register at the `Info` level. Set `runtime: info`, or the
//! override `iota_metrics::process_metrics=info`, to show them. The seven
//! standard metrics share one level.

use prometheus_filtered::Registry;

/// Registers the process metrics in `registry`. Does nothing outside Linux and
/// in the simulator, where `/proc` describes the host rather than the
/// simulated node.
pub fn register_process_metrics(registry: &Registry) -> prometheus_filtered::Result<()> {
    #[cfg(all(target_os = "linux", not(msim)))]
    {
        linux::register(registry)
    }
    #[cfg(not(all(target_os = "linux", not(msim))))]
    {
        let _ = registry;
        Ok(())
    }
}

#[cfg(all(target_os = "linux", not(msim)))]
mod linux {
    use std::{
        fs, io,
        path::{Path, PathBuf},
        sync::Arc,
        time::{Duration, Instant},
    };

    use parking_lot::Mutex;
    use prometheus_filtered::{
        IntGaugeVec, MetricLevel, Opts, Registry,
        core::{Collector, Desc},
        process_collector::ProcessCollector,
        proto::MetricFamily,
    };
    use strum::{EnumCount, EnumIter, IntoEnumIterator, IntoStaticStr};

    /// Filter key of the standard collector. It is not a metric name: the
    /// collector exposes several `process_*` metrics, and a directive naming
    /// one of them does not hide it.
    const STANDARD_COLLECTOR_KEY: &str = "process";
    const OPEN_FDS_BY_KIND: &str = "process_open_fds_by_kind";
    const MODULE: &str = module_path!();
    const COUNT_INTERVAL: Duration = Duration::from_secs(60);
    const FD_DIR: &str = "/proc/self/fd";

    pub(super) fn register(registry: &Registry) -> prometheus_filtered::Result<()> {
        registry.register_filtered(
            STANDARD_COLLECTOR_KEY,
            MODULE,
            MetricLevel::Info,
            StandardProcessMetrics(Arc::new(ProcessCollector::for_self())),
        )?;
        registry.register_filtered(
            OPEN_FDS_BY_KIND,
            MODULE,
            MetricLevel::Info,
            OpenFdsByKind::new(PathBuf::from(FD_DIR)),
        )?;
        Ok(())
    }

    /// `ProcessCollector` is not `Clone`, which `register_filtered` needs.
    #[derive(Clone)]
    struct StandardProcessMetrics(Arc<ProcessCollector>);

    impl Collector for StandardProcessMetrics {
        fn desc(&self) -> Vec<&Desc> {
            self.0.desc()
        }

        fn collect(&self) -> Vec<MetricFamily> {
            self.0.collect()
        }
    }

    /// What an open file descriptor refers to.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, EnumCount, EnumIter, IntoStaticStr)]
    #[strum(serialize_all = "snake_case")]
    enum FdKind {
        Socket,
        File,
        Other,
    }

    impl FdKind {
        /// Classifies the target of a `/proc/self/fd` link: `socket:[inode]`
        /// for a socket, an absolute path for a file, and `pipe:[..]`,
        /// `anon_inode:[..]` and the like for the rest.
        fn of_link_target(target: &Path) -> Self {
            if target.starts_with("/") {
                Self::File
            } else if target
                .as_os_str()
                .as_encoded_bytes()
                .starts_with(b"socket:")
            {
                Self::Socket
            } else {
                Self::Other
            }
        }
    }

    type FdCounts = [i64; FdKind::COUNT];

    /// Counts the descriptors listed in `dir` by kind, without the descriptor
    /// that the listing itself opens.
    fn count_fds(dir: &Path) -> io::Result<FdCounts> {
        let own_dir = fs::canonicalize(dir)?;
        let mut counts = [0; FdKind::COUNT];
        for entry in fs::read_dir(dir)? {
            // A descriptor that closed while counting has no link any more.
            let Ok(target) = fs::read_link(entry?.path()) else {
                continue;
            };
            if target == own_dir {
                continue;
            }
            counts[FdKind::of_link_target(&target) as usize] += 1;
        }
        Ok(counts)
    }

    /// Open descriptors by kind. A collection recounts only when the last
    /// count is older than `COUNT_INTERVAL`. A collection that finds a count
    /// in progress returns the previous values instead of waiting for it.
    #[derive(Clone)]
    struct OpenFdsByKind {
        gauge: IntGaugeVec,
        dir: PathBuf,
        last_count: Arc<Mutex<Option<Instant>>>,
    }

    impl OpenFdsByKind {
        fn new(dir: PathBuf) -> Self {
            let help = format!(
                "Number of open file descriptors by kind: socket, file (a path in the file \
                 system) or other (pipe, eventfd, epoll). Recounted at most once per {} seconds.",
                COUNT_INTERVAL.as_secs()
            );
            let gauge = IntGaugeVec::new(Opts::new(OPEN_FDS_BY_KIND, help), &["kind"])
                .expect("valid gauge options");
            Self {
                gauge,
                dir,
                last_count: Arc::default(),
            }
        }

        /// A failed count removes the series, because the previous values
        /// would be wrong, and is not retried before `COUNT_INTERVAL` passes.
        fn collect_at(&self, now: Instant) -> Vec<MetricFamily> {
            if let Some(mut last_count) = self.last_count.try_lock() {
                let fresh =
                    last_count.is_some_and(|at| now.saturating_duration_since(at) < COUNT_INTERVAL);
                if !fresh {
                    match count_fds(&self.dir) {
                        Ok(counts) => {
                            for kind in FdKind::iter() {
                                self.gauge
                                    .with_label_values(&[<&str>::from(kind)])
                                    .set(counts[kind as usize]);
                            }
                        }
                        Err(e) => {
                            self.gauge.reset();
                            tracing::warn!("failed to count open file descriptors: {e}");
                        }
                    }
                    *last_count = Some(now);
                }
            }
            self.gauge.collect()
        }
    }

    impl Collector for OpenFdsByKind {
        fn desc(&self) -> Vec<&Desc> {
            self.gauge.desc()
        }

        fn collect(&self) -> Vec<MetricFamily> {
            self.collect_at(Instant::now())
        }
    }

    #[cfg(test)]
    mod tests {
        use std::{
            fs::File,
            os::{fd::AsRawFd, unix::fs::symlink},
            sync::mpsc,
        };

        use tempfile::TempDir;

        use super::*;
        use crate::metric_groups::MetricGroups;

        const STANDARD_METRICS: [&str; 7] = [
            "process_cpu_seconds_total",
            "process_open_fds",
            "process_max_fds",
            "process_virtual_memory_bytes",
            "process_resident_memory_bytes",
            "process_start_time_seconds",
            "process_threads",
        ];

        fn registry_with_filter(env: Option<&str>) -> Registry {
            let (filter, errors) = MetricGroups::default().startup_filter(env);
            assert!(errors.is_empty(), "{errors:?}");
            Registry::new_custom(None, None, Some(Arc::new(filter))).unwrap()
        }

        fn visible_registry() -> Registry {
            let registry = registry_with_filter(Some("iota_metrics::process_metrics=info"));
            register(&registry).unwrap();
            registry
        }

        fn kind_series(families: &[MetricFamily], kind: &str) -> Option<i64> {
            let family = families
                .iter()
                .find(|family| family.name() == OPEN_FDS_BY_KIND)?;
            let metric = family.get_metric().iter().find(|metric| {
                metric
                    .get_label()
                    .iter()
                    .any(|pair| pair.name() == "kind" && pair.value() == kind)
            })?;
            Some(metric.get_gauge().value() as i64)
        }

        /// A directory of links like `/proc/self/fd`: two sockets, two files,
        /// two others, the link of the directory itself, and an entry without
        /// a link, like a descriptor closed while counting.
        fn fd_dir() -> TempDir {
            let dir = tempfile::tempdir().unwrap();
            let link = |name: &str, target: &str| symlink(target, dir.path().join(name)).unwrap();
            link("0", "socket:[1]");
            link("1", "socket:[7]");
            link("2", "/some/file");
            link("3", "/missing/file");
            link("4", "pipe:[2]");
            link("5", "anon_inode:[eventpoll]");
            link("6", dir.path().canonicalize().unwrap().to_str().unwrap());
            File::create(dir.path().join("7")).unwrap();
            dir
        }

        fn add_socket(dir: &Path, name: &str) {
            symlink("socket:[99]", dir.join(name)).unwrap();
        }

        fn kind_of_fd(fd: &impl AsRawFd) -> FdKind {
            let target = fs::read_link(format!("/proc/self/fd/{}", fd.as_raw_fd())).unwrap();
            FdKind::of_link_target(&target)
        }

        #[test]
        fn real_descriptors_are_classified_by_what_they_refer_to() {
            let socket = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
            let file = File::open("/proc/self/status").unwrap();
            let (pipe_reader, _pipe_writer) = std::io::pipe().unwrap();

            assert_eq!(kind_of_fd(&socket), FdKind::Socket);
            assert_eq!(kind_of_fd(&file), FdKind::File);
            assert_eq!(kind_of_fd(&pipe_reader), FdKind::Other);
        }

        #[test]
        fn kind_labels_are_stable() {
            let labels: Vec<&str> = FdKind::iter().map(<&str>::from).collect();
            assert_eq!(labels, ["socket", "file", "other"]);
        }

        #[test]
        fn count_fds_counts_each_kind_and_skips_its_own_directory() {
            let dir = fd_dir();
            assert_eq!(count_fds(dir.path()).unwrap(), [2, 2, 2]);
        }

        #[test]
        fn the_counts_are_reported_by_kind() {
            let dir = fd_dir();
            let collector = OpenFdsByKind::new(dir.path().to_path_buf());
            let families = collector.collect_at(Instant::now());
            assert_eq!(kind_series(&families, "socket"), Some(2));
            assert_eq!(kind_series(&families, "file"), Some(2));
            assert_eq!(kind_series(&families, "other"), Some(2));
        }

        #[test]
        fn open_sockets_are_counted() {
            const SOCKETS: usize = 64;
            let registry = visible_registry();
            let sockets: Vec<_> = (0..SOCKETS)
                .map(|_| std::net::UdpSocket::bind("127.0.0.1:0").unwrap())
                .collect();
            let counted = kind_series(&registry.gather(), "socket").unwrap();
            assert!(counted >= SOCKETS as i64, "{counted} sockets");
            drop(sockets);
        }

        #[test]
        fn standard_process_metrics_are_exposed() {
            let families = visible_registry().gather();
            for name in STANDARD_METRICS {
                let family = families
                    .iter()
                    .find(|family| family.name() == name)
                    .unwrap_or_else(|| panic!("{name} is not exposed"));
                let metric = &family.get_metric()[0];
                let value = if metric.get_counter().has_value() {
                    metric.get_counter().value()
                } else {
                    metric.get_gauge().value()
                };
                if name != "process_cpu_seconds_total" {
                    assert!(value > 0.0, "{name} is {value}");
                }
            }
        }

        #[test]
        fn count_fds_matches_an_independent_count_of_the_real_descriptors() {
            // Under `cargo test`, other tests of the process open and close
            // descriptors, so only an attempt with a steady count is compared.
            let listed = || fs::read_dir(FD_DIR).unwrap().count() - 1;
            let mut seen = Vec::new();
            for _ in 0..50 {
                let before = listed();
                let counted: i64 = count_fds(Path::new(FD_DIR)).unwrap().iter().sum();
                let after = listed();
                if before == after && counted == before as i64 {
                    return;
                }
                seen.push((before, counted, after));
            }
            panic!("no attempt had equal counts (before, counted, after): {seen:?}");
        }

        #[test]
        fn nothing_is_exposed_by_default() {
            let registry = registry_with_filter(None);
            register(&registry).unwrap();
            assert!(registry.gather().is_empty());
        }

        #[test]
        fn the_module_override_exposes_every_metric_of_the_module() {
            let families = visible_registry().gather();
            assert!(kind_series(&families, "socket").is_some());
            for name in STANDARD_METRICS {
                assert!(
                    families.iter().any(|family| family.name() == name),
                    "{name}"
                );
            }
        }

        #[test]
        fn a_hidden_metric_does_not_count_the_descriptors() {
            let dir = fd_dir();
            for (env, counted) in [
                (None, false),
                (Some("iota_metrics::process_metrics=info"), true),
            ] {
                let registry = registry_with_filter(env);
                let collector = OpenFdsByKind::new(dir.path().to_path_buf());
                registry
                    .register_filtered(
                        OPEN_FDS_BY_KIND,
                        MODULE,
                        MetricLevel::Info,
                        collector.clone(),
                    )
                    .unwrap();
                let families = registry.gather();
                assert_eq!(collector.last_count.lock().is_some(), counted);
                assert_eq!(kind_series(&families, "socket").is_some(), counted);
            }
        }

        #[test]
        fn counting_is_repeated_only_after_the_interval() {
            let dir = fd_dir();
            let collector = OpenFdsByKind::new(dir.path().to_path_buf());
            let sockets = |at| kind_series(&collector.collect_at(at), "socket").unwrap();

            let start = Instant::now();
            assert_eq!(sockets(start), 2);
            add_socket(dir.path(), "8");
            assert_eq!(sockets(start + COUNT_INTERVAL - Duration::from_secs(1)), 2);
            assert_eq!(sockets(start + COUNT_INTERVAL), 3);
        }

        #[test]
        fn a_failed_count_removes_the_series_and_is_not_retried_at_once() {
            let dir = fd_dir();
            let collector = OpenFdsByKind::new(dir.path().to_path_buf());
            let start = Instant::now();
            assert_eq!(kind_series(&collector.collect_at(start), "socket"), Some(2));

            fs::remove_dir_all(dir.path()).unwrap();
            let failed_at = start + COUNT_INTERVAL;
            assert_eq!(
                kind_series(&collector.collect_at(failed_at), "socket"),
                None
            );
            assert_eq!(*collector.last_count.lock(), Some(failed_at));

            fs::create_dir(dir.path()).unwrap();
            add_socket(dir.path(), "0");
            let soon = failed_at + Duration::from_secs(1);
            assert_eq!(kind_series(&collector.collect_at(soon), "socket"), None);
            let later = failed_at + COUNT_INTERVAL;
            assert_eq!(kind_series(&collector.collect_at(later), "socket"), Some(1));
        }

        #[test]
        fn a_concurrent_collection_returns_previous_values_without_waiting() {
            let dir = fd_dir();
            let collector = OpenFdsByKind::new(dir.path().to_path_buf());
            let start = Instant::now();
            collector.collect_at(start);
            add_socket(dir.path(), "8");

            // The held lock stands for a count in progress.
            let count_in_progress = collector.last_count.lock();
            let (done_tx, done_rx) = mpsc::channel();
            let concurrent = {
                let collector = collector.clone();
                std::thread::spawn(move || {
                    let families = collector.collect_at(start + COUNT_INTERVAL);
                    done_tx.send(kind_series(&families, "socket")).unwrap();
                })
            };
            let seen = done_rx.recv_timeout(Duration::from_secs(10));
            drop(count_in_progress);
            concurrent.join().unwrap();

            assert_eq!(seen, Ok(Some(2)));
            assert_eq!(*collector.last_count.lock(), Some(start));
        }
    }
}
