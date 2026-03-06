// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::HashSet,
    fs::{self, OpenOptions},
    future::Future,
    io::Write,
    marker::PhantomData,
    path::Path,
    time::Duration,
};

use chrono;
use futures;
use tokio::time::{self, Instant};
use tracing::info;

use crate::{
    benchmark::{BenchmarkParameters, BenchmarkParametersGenerator, BenchmarkType, RunInterval},
    build_cache::BuildCacheService,
    client::Instance,
    display,
    error::{TestbedError, TestbedResult},
    faults::CrashRecoverySchedule,
    logger::{IS_LOOP, SwappableWriter},
    logs::LogsAnalyzer,
    measurement::MeasurementsCollection,
    monitor::{Monitor, Prometheus},
    net_latency::NetworkLatencyCommandBuilder,
    protocol::{ProtocolCommands, ProtocolMetrics},
    settings::{BuildGroups, Settings, build_cargo_command},
    ssh::{CommandContext, CommandStatus, SshConnectionManager},
};

/// An orchestrator to run benchmarks on a testbed.
pub struct Orchestrator<P, T> {
    /// The testbed's settings.
    settings: Settings,
    /// Node instances
    node_instances: Vec<Instance>,
    // Client (Load Generator) instances
    client_instances: Vec<Instance>,
    // Dedicated Metrics instance
    metrics_instance: Option<Instance>,
    /// The type of the benchmark parameters.
    benchmark_type: PhantomData<T>,
    /// Provider-specific commands to install on the instance.
    instance_setup_commands: Vec<String>,
    /// Protocol-specific commands generator to generate the protocol
    /// configuration files, boot clients and nodes, etc.
    protocol_commands: P,
    /// The interval between measurements collection.
    scrape_interval: Duration,
    /// The interval to crash nodes.
    crash_interval: Duration,
    /// Handle ssh connections to instances.
    ssh_manager: SshConnectionManager,
    /// Whether to skip testbed updates before running benchmarks.
    skip_testbed_update: bool,
    /// Whether to skip testbed configuration before running benchmarks.
    skip_testbed_configuration: bool,
    /// Whether to downloading and analyze the client and node log files.
    log_processing: bool,
    /// Number of instances running only load generators (not nodes). If this
    /// value is set to zero, the orchestrator runs a load generate
    /// collocated with each node.
    dedicated_clients: usize,
    /// Whether to forgo a grafana and prometheus instance and leave the testbed
    /// unmonitored.
    skip_monitoring: bool,
    /// Directory to store benchmark results.
    benchmark_dir: std::path::PathBuf,
    /// Writer for benchmark logs.
    benchmark_writer: crate::logger::SwappableWriter,
}

impl<P, T> Orchestrator<P, T> {
    /// The default interval between measurements collection.
    const DEFAULT_SCRAPE_INTERVAL: Duration = Duration::from_secs(15);
    /// The default interval to crash nodes.
    const DEFAULT_CRASH_INTERVAL: Duration = Duration::from_secs(60);

    /// Make a new orchestrator.
    pub fn new(
        settings: Settings,
        node_instances: Vec<Instance>,
        client_instances: Vec<Instance>,
        metrics_instance: Option<Instance>,
        instance_setup_commands: Vec<String>,
        protocol_commands: P,
        ssh_manager: SshConnectionManager,
    ) -> Self {
        Self {
            settings,
            node_instances,
            client_instances,
            metrics_instance,
            benchmark_type: PhantomData,
            instance_setup_commands,
            protocol_commands,
            ssh_manager,
            scrape_interval: Self::DEFAULT_SCRAPE_INTERVAL,
            crash_interval: Self::DEFAULT_CRASH_INTERVAL,
            skip_testbed_update: false,
            skip_testbed_configuration: false,
            log_processing: false,
            dedicated_clients: 0,
            skip_monitoring: false,
            benchmark_dir: Path::new("benchmark_results").to_path_buf(),
            benchmark_writer: SwappableWriter::new(),
        }
    }

    /// Set interval between measurements collection.
    pub fn with_scrape_interval(mut self, scrape_interval: Duration) -> Self {
        self.scrape_interval = scrape_interval;
        self
    }

    /// Set interval with which to crash nodes.
    pub fn with_crash_interval(mut self, crash_interval: Duration) -> Self {
        self.crash_interval = crash_interval;
        self
    }

    /// Set whether to skip testbed updates before running benchmarks.
    pub fn skip_testbed_updates(mut self, skip_testbed_update: bool) -> Self {
        self.skip_testbed_update = skip_testbed_update;
        self
    }

    /// Whether to skip testbed configuration before running benchmarks.
    pub fn skip_testbed_configuration(mut self, skip_testbed_configuration: bool) -> Self {
        self.skip_testbed_configuration = skip_testbed_configuration;
        self
    }

    /// Set whether to download and analyze the client and node log files.
    pub fn with_log_processing(mut self, log_processing: bool) -> Self {
        self.log_processing = log_processing;
        self
    }

    /// Set the number of instances running exclusively load generators.
    pub fn with_dedicated_clients(mut self, dedicated_clients: usize) -> Self {
        self.dedicated_clients = dedicated_clients;
        self
    }

    pub fn with_benchmark_dir_and_writer<F: AsRef<Path>>(
        mut self,
        benchmark_dir: F,
        writer: crate::logger::SwappableWriter,
    ) -> Self {
        self.benchmark_dir = benchmark_dir.as_ref().to_path_buf();
        self.benchmark_writer = writer;
        self
    }

    /// Set whether to boot grafana on the local machine to monitor the nodes.
    pub fn skip_monitoring(mut self, skip_monitoring: bool) -> Self {
        self.skip_monitoring = skip_monitoring;
        self
    }

    pub fn instances_without_metrics(&self) -> Vec<Instance> {
        let mut instances = self.node_instances.clone();

        if self.dedicated_clients > 0 {
            instances.extend(self.client_instances.clone());
        }
        instances
    }

    /// Returns all the instances combined
    pub fn instances(&self) -> Vec<Instance> {
        let mut instances = self.instances_without_metrics();
        if let Some(metrics_instance) = &self.metrics_instance {
            instances.push(metrics_instance.clone());
        }
        instances
    }

    fn effective_scrape_interval(&self, parameters: &BenchmarkParameters<T>) -> Duration {
        const MIN: Duration = Duration::from_secs(1);
        // upper bound: current self.scrape_interval (e.g., 15s)
        let max = self.scrape_interval;

        match parameters.run_interval {
            RunInterval::Time(_) => self.scrape_interval,

            RunInterval::Count(tx_count) => {
                // Evaluate the scrape interval based on the estimated benchmark duration,
                // aiming for around 30 samples, but never less than 1s and never more than
                // self.scrape_interval.
                let qps = parameters.load.max(1) as u64; // protect against division by zero, even if load is set to 0
                let est_secs = tx_count.div_ceil(qps);

                let target_samples = 30u64;
                let raw = (est_secs / target_samples).max(1);
                let candidate = Duration::from_secs(raw);

                candidate.clamp(MIN, max)
            }
        }
    }
}

impl<P: ProtocolCommands<T> + ProtocolMetrics, T: BenchmarkType> Orchestrator<P, T> {
    /// Boot one node per instance.
    async fn boot_nodes(
        &self,
        instances: Vec<Instance>,
        parameters: &BenchmarkParameters<T>,
    ) -> TestbedResult<()> {
        if parameters.use_internal_ip_address {
            if let Some(latency_topology) = parameters.latency_topology.clone() {
                let latency_commands = NetworkLatencyCommandBuilder::new(&instances)
                    .with_perturbation_spec(parameters.perturbation_spec.clone())
                    .with_topology_layout(latency_topology)
                    .with_max_latency(parameters.maximum_latency)
                    .build_network_latency_matrix();
                self.ssh_manager
                    .execute_per_instance(latency_commands, CommandContext::default())
                    .await?;
            }
        }

        // Run one node per instance.
        let targets = self
            .protocol_commands
            .node_command(instances.clone(), parameters);

        let repo = self.settings.repository_name();
        let node_context = CommandContext::new()
            .run_background("node".into())
            .with_log_file("~/node.log".into())
            .with_execute_from_path(repo.into());
        self.ssh_manager
            .execute_per_instance(targets, node_context)
            .await?;

        // Wait until all nodes are reachable.
        let commands = self
            .protocol_commands
            .nodes_metrics_command(instances.clone(), parameters.use_internal_ip_address);
        self.wait_for_success(commands, &parameters.benchmark_dir)
            .await;

        Ok(())
    }

    /// Install the codebase and its dependencies on the testbed.
    pub async fn install(&self) -> TestbedResult<()> {
        display::action("Installing dependencies on all machines");

        let working_dir = self.settings.working_dir.display();
        let url = &self.settings.repository.url;

        let use_precompiled_binaries = self.settings.build_cache_enabled();

        let working_dir_cmd = format!("mkdir -p {working_dir}");
        let git_clone_cmd = format!("(git clone --depth=1 {url} || true)");

        let mut basic_commands = vec![
            "sudo apt-get update",
            "sudo apt-get -y upgrade",
            "sudo apt-get -y autoremove",
            // Disable "pending kernel upgrade" message.
            "sudo apt-get -y remove needrestart",
            "sudo apt-get -y install curl git ca-certificates",
            // Increase open file limits to prevent "Too many open files" errors
            "echo '* soft nofile 1048576' | sudo tee -a /etc/security/limits.conf",
            "echo '* hard nofile 1048576' | sudo tee -a /etc/security/limits.conf",
            "echo 'root soft nofile 1048576' | sudo tee -a /etc/security/limits.conf",
            "echo 'root hard nofile 1048576' | sudo tee -a /etc/security/limits.conf",
            // Set system-wide file descriptor limits
            "echo 'fs.file-max = 2097152' | sudo tee -a /etc/sysctl.conf",
            "sudo sysctl -p",
            // Set limits for current session
            "ulimit -n 1048576 || true",
            // set network buffer sizes
            "sudo sysctl -w net.core.rmem_max=104857600",
            "sudo sysctl -w net.core.wmem_max=104857600",
            "sudo sysctl -w net.ipv4.tcp_rmem=\"8192 262144 104857600\"",
            "sudo sysctl -w net.ipv4.tcp_wmem=\"8192 262144 104857600\"",
            // Create the working directory.
            working_dir_cmd.as_str(),
            // Clone the repo.
            git_clone_cmd.as_str(),
        ];

        // Collect all unique non-"stable" rust toolchains from build configs
        let toolchain_cmds: Vec<String> = if !use_precompiled_binaries {
            self.settings
                .build_configs
                .values()
                .filter_map(|config| {
                    config
                        .toolchain
                        .as_ref()
                        .filter(|t| t.as_str() != "stable")
                        .cloned()
                })
                .collect::<HashSet<String>>()
                .into_iter()
                .map(|toolchain| format!("rustup toolchain install {toolchain}"))
                .collect()
        } else {
            vec![]
        };

        if !use_precompiled_binaries {
            // If not using precompiled binaries, install rustup.
            basic_commands.extend([
                // The following dependencies:
                // * build-essential: prevent the error: [error: linker `cc` not found].
                "sudo apt-get -y install build-essential cmake clang lld protobuf-compiler pkg-config nvme-cli",
                // Install rust (non-interactive).
                "curl --proto \"=https\" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y",
                "echo \"source $HOME/.cargo/env\" | tee -a ~/.bashrc",
                "source $HOME/.cargo/env",
                "rustup default stable",
            ]);

            // Add the toolchain install commands to basic_commands
            for cmd in &toolchain_cmds {
                basic_commands.push(cmd.as_str());
            }
        } else {
            // Create cargo env file if using precompiled binaries, so that the source
            // commands don't fail.
            basic_commands.push("mkdir -p $HOME/.cargo/ && touch $HOME/.cargo/env");
        }

        let cloud_provider_specific_dependencies: Vec<_> = self
            .instance_setup_commands
            .iter()
            .map(|x| x.as_str())
            .collect();

        let protocol_dependencies = self.protocol_commands.protocol_dependencies();

        let command = [
            &basic_commands[..],
            &Prometheus::install_commands(),
            &cloud_provider_specific_dependencies[..],
            &protocol_dependencies[..],
        ]
        .concat()
        .join(" && ");

        self.ssh_manager
            .execute(self.instances(), command, CommandContext::default())
            .await?;

        if !self.skip_monitoring {
            let metrics_instance = self
                .metrics_instance
                .clone()
                .expect("No metrics instance available");
            let monitor_command = Monitor::dependencies().join(" && ");
            self.ssh_manager
                .execute(
                    vec![metrics_instance],
                    monitor_command,
                    CommandContext::default(),
                )
                .await?;
        }

        display::done();
        Ok(())
    }

    /// Reload prometheus on all instances.
    pub async fn start_monitoring(
        &self,
        use_internal_ip_address: bool,
        timestamp: &str,
    ) -> TestbedResult<()> {
        if self.skip_monitoring {
            display::warn("Monitoring is skipped, not starting Prometheus, Tempo and Grafana");
            return Ok(());
        }
        if let Some(instance) = &self.metrics_instance {
            display::action("Configuring monitoring instance");

            let monitor = Monitor::new(
                instance.clone(),
                self.client_instances.clone(),
                self.node_instances.clone(),
                self.ssh_manager.clone(),
            );
            // When prometheus snapshots are enabled, pass the timestamp as snapshot
            // directory
            let snapshot_dir = self
                .settings
                .enable_prometheus_snapshots
                .then_some(timestamp);
            monitor.start_tempo().await?;
            monitor
                .start_prometheus(
                    &self.protocol_commands,
                    use_internal_ip_address,
                    self.settings.use_fullnode_for_execution,
                    snapshot_dir,
                )
                .await?;
            monitor.start_grafana().await?;

            display::done();
            display::config("Grafana address", monitor.grafana_address());
            display::newline();
        }

        Ok(())
    }

    /// Update all instances to use the version of the codebase specified in the
    /// setting file.
    pub async fn update(&self) -> TestbedResult<()> {
        display::action("Updating all instances");

        let commit = &self.settings.repository.commit;
        let repo_name = self.settings.repository_name();
        let build_groups = self.settings.build_groups();

        // we need to fetch and checkout the commit even if using precompiled binaries
        // because the iota-framework submodule, the examples/move folder, or the
        // dev-tools/grafana-local folder might be used.
        let git_update_command = [
            &format!("git fetch origin {commit} --force"),
            &format!("(git reset --hard origin/{commit} || git checkout --force {commit})"),
            "git clean -fd -e target",
        ]
        .join(" && ");

        let id = "git update";
        let context = CommandContext::new()
            .run_background(id.into())
            .with_execute_from_path(repo_name.clone().into());

        // Execute and wait for the git update command on all instances (including
        // metrics)
        display::action(format!("update command: {git_update_command}"));
        self.ssh_manager
            .execute(self.instances(), git_update_command, context)
            .await?;
        self.ssh_manager
            .wait_for_command(self.instances(), id, CommandStatus::Terminated)
            .await?;

        // Check if build cache is enabled
        if self.settings.build_cache_enabled() {
            display::action("Using build cache for binary distribution");
            let build_cache_service = BuildCacheService::new(&self.settings, &self.ssh_manager);
            build_cache_service
                .update_with_build_cache(
                    commit,
                    &build_groups,
                    self.instances_without_metrics(),
                    repo_name.clone(),
                )
                .await?;
        } else {
            self.update_with_local_build(build_groups).await?;
        }

        display::done();
        Ok(())
    }

    /// Update instances with local build (fallback, if build cache is not used)
    /// Execute and wait for the cargo build command on all instances except the
    /// metrics one. This requires compiling the codebase in release
    /// (which may take a long time) so we run the command in the background
    /// to avoid keeping alive many ssh connections for too long.
    async fn update_with_local_build(&self, build_groups: BuildGroups) -> TestbedResult<()> {
        let without_metrics = self.instances_without_metrics();
        let repo_name = self.settings.repository_name();

        // Build each group separately
        for (i, (group, binary_names)) in build_groups.iter().enumerate() {
            // Build arguments
            let build_command = build_cargo_command(
                "build",
                group.toolchain.clone(),
                group.features.clone(),
                binary_names,
                &[] as &[&str],
                &[] as &[&str],
            );

            // print the full command for logging
            display::action(format!(
                "Running build command {}/{}: \"{build_command}\" in \"{repo_name}\"",
                i + 1,
                build_groups.len()
            ));

            let context = CommandContext::new().with_execute_from_path(repo_name.clone().into());

            self.ssh_manager
                .execute(without_metrics.clone(), build_command, context)
                .await?;
        }

        Ok(())
    }

    /// Configure the instances with the appropriate configuration files.
    pub async fn configure(&self, parameters: &BenchmarkParameters<T>) -> TestbedResult<()> {
        display::action("Configuring instances");

        // Generate the genesis configuration file and the keystore allowing access to
        // gas objects.
        let command = self
            .protocol_commands
            .genesis_command(self.node_instances.iter(), parameters);
        display::action(format!("\nGenesis command: {command}\n\n"));
        let repo_name = self.settings.repository_name();
        let context = CommandContext::new().with_execute_from_path(repo_name.into());
        self.ssh_manager
            .execute(self.instances_without_metrics(), command, context)
            .await?;

        display::action("Configuration of all instances completed");
        display::done();
        Ok(())
    }

    /// Cleanup all instances and optionally delete their log files.
    pub async fn cleanup(&self, cleanup: bool) -> TestbedResult<()> {
        display::action("Cleaning up testbed");

        // Kill all tmux servers and delete the nodes dbs. Optionally clear logs.
        let mut command = vec!["(tmux kill-server || true)".into()];
        for path in self.protocol_commands.db_directories() {
            command.push(format!("(rm -rf {} || true)", path.display()));
        }
        if cleanup {
            command.push("(rm -rf ~/*log* || true)".into());
        }
        let command = command.join(" ; ");

        // Execute the deletion on all machines.
        let active = self.instances().into_iter().filter(|x| x.is_active());
        let context = CommandContext::default();
        self.ssh_manager.execute(active, command, context).await?;

        display::action("Cleanup of all instances completed");
        display::done();
        Ok(())
    }

    /// Deploy the nodes.
    pub async fn run_nodes(&self, parameters: &BenchmarkParameters<T>) -> TestbedResult<()> {
        display::action("Deploying validators");

        // Boot one node per instance.
        self.boot_nodes(self.node_instances.clone(), parameters)
            .await?;

        display::action("Deployment of validators completed");
        display::done();
        Ok(())
    }

    /// Deploy the load generators.
    pub async fn run_clients(&self, parameters: &BenchmarkParameters<T>) -> TestbedResult<()> {
        display::action("Starting deployment of load generators");
        if self.settings.use_fullnode_for_execution {
            display::action("Setting up full nodes");

            // Deploy the fullnodes.
            let targets = self
                .protocol_commands
                .fullnode_command(self.client_instances.clone(), parameters);

            let repo = self.settings.repository_name();
            let context = CommandContext::new()
                .run_background("fullnode".into())
                .with_log_file("~/fullnode.log".into())
                .with_execute_from_path(repo.into());
            self.ssh_manager
                .execute_per_instance(targets, context)
                .await?;

            // Wait until all fullnodes are fully started by querying the latest checkpoint
            // (otherwise clients might fail when a fullnode is not listening yet).
            display::action("Await fullnode ready...");
            let commands = self
                .client_instances
                .iter()
                .cloned()
                .map(|i| (i, "curl http://127.0.0.1:9000 -H 'Content-Type: application/json' -d '{\"jsonrpc\":\"2.0\",\"method\":\"iota_getLatestCheckpointSequenceNumber\",\"params\":[],\"id\":1}'".to_owned()));
            self.ssh_manager.wait_for_success(commands).await;

            display::done();
        }

        display::action("Setting up load generators");

        // Deploy the load generators.
        let targets = self
            .protocol_commands
            .client_command(self.client_instances.clone(), parameters);

        let repo = self.settings.repository_name();
        let context = CommandContext::new()
            .run_background("client".into())
            .with_log_file("~/client.log".into())
            .with_execute_from_path(repo.into());
        self.ssh_manager
            .execute_per_instance(targets, context)
            .await?;

        // Wait until all load generators are reachable.
        let commands = self.protocol_commands.clients_metrics_command(
            self.client_instances.clone(),
            parameters.use_internal_ip_address,
        );
        self.wait_for_success(commands, &parameters.benchmark_dir)
            .await;

        // Start background metrics collection service on each client instance.
        display::action("\n\nStarting background metrics collection service");
        let metrics_script =
            self.metrics_collection_script_command(parameters.use_internal_ip_address);
        let metrics_context = CommandContext::new().run_background("metrics-collector".into());
        self.ssh_manager
            .execute_per_instance(metrics_script.clone(), metrics_context)
            .await?;

        display::action("Background metrics collection service started");
        display::action("Deployment of load generators completed");
        display::done();
        Ok(())
    }

    /// Create a background metrics collection script that runs on each
    /// client instance.
    fn metrics_collection_script_command(
        &self,
        use_internal_ip_address: bool,
    ) -> Vec<(Instance, String)> {
        // We need to get the metrics path from clients_metrics_command
        self.protocol_commands
            .clients_metrics_command(self.client_instances.clone(), use_internal_ip_address)
            .into_iter()
            .map(|(instance, cmd)| {
                (
                    instance,
                    format!(
                        r#"while true; do
    {cmd} >> ~/metrics.log 2>&1
    sleep 15
done"#
                    ),
                )
            })
            .collect::<Vec<_>>()
    }

    /// Collect metrics from the load generators.
    pub async fn run(&self, parameters: &BenchmarkParameters<T>) -> TestbedResult<()> {
        let run_label = match parameters.run_interval {
            RunInterval::Time(d) => format!("at least {}s", d.as_secs()),
            RunInterval::Count(n) => format!("until {n} tx executed"),
        };
        display::action(format!("Running benchmark ({run_label})"));

        let scrape_every = self.effective_scrape_interval(parameters);
        let mut metrics_interval = time::interval(scrape_every);
        metrics_interval.tick().await;
        let faults_type = parameters.faults.clone();
        let mut faults_schedule =
            CrashRecoverySchedule::new(faults_type, self.node_instances.clone());

        let mut faults_interval = time::interval(self.crash_interval);
        faults_interval.tick().await;

        // In Count-mode we should stop when the client tmux session terminates,
        // not when "elapsed seconds" reaches some value.
        let is_count_mode = matches!(parameters.run_interval, RunInterval::Count(_));
        let clients = self.client_instances.clone();
        let ssh = self.ssh_manager.clone();

        let wait_clients_future: std::pin::Pin<Box<dyn Future<Output = TestbedResult<()>> + Send>> =
            if is_count_mode && !clients.is_empty() {
                Box::pin(async move {
                    ssh.wait_for_command(clients, "client", CommandStatus::Terminated)
                        .await
                        .map_err(Into::into)
                })
            } else {
                Box::pin(std::future::pending::<TestbedResult<()>>())
            };
        tokio::pin!(wait_clients_future);

        let start = Instant::now();

        loop {
            tokio::select! {
                _ = metrics_interval.tick() => {
                    let elapsed = Instant::now().duration_since(start).as_secs_f64().ceil() as u64;
                    display::status(format!("{elapsed}s"));

                    if let Some(limit) = parameters.run_interval.time_limit_secs() {
                        if elapsed >= limit {
                            break;
                        }
                    }
                }

                res = &mut wait_clients_future => {
                    // Work only count-based benchmarks, for time-based the future is pending and will never complete
                    res?;
                    break;
                }

                _ = faults_interval.tick() => {
                    let  action = faults_schedule.update();
                    if !action.kill.is_empty() {
                        self.ssh_manager.kill(action.kill.clone(), "node").await?;
                    }
                    if !action.boot.is_empty() {
                        self.boot_nodes(action.boot.clone(), parameters).await?;
                    }
                    if !action.kill.is_empty() || !action.boot.is_empty() {
                        display::newline();
                        display::config("Testbed update", action);
                    }
                 }
            }
        }

        if self.settings.enable_flamegraph {
            let flamegraphs_dir = parameters.benchmark_dir.join("flamegraphs");
            fs::create_dir_all(&flamegraphs_dir).expect("Failed to create flamegraphs directory");

            self.fetch_flamegraphs(
                self.instances_without_metrics().clone(),
                &flamegraphs_dir,
                "?svg=true",
                "flamegraph",
            )
            .await?;

            if self
                .settings
                .build_configs
                .get("iota-node")
                .is_some_and(|config| config.features.iter().any(|f| f == "flamegraph-alloc"))
            {
                self.fetch_flamegraphs(
                    self.instances_without_metrics().clone(),
                    &flamegraphs_dir,
                    "?svg=true&mem=true",
                    "flamegraph-alloc",
                )
                .await?;
            }
        }

        display::action("Benchmark run completed");
        display::done();
        Ok(())
    }

    async fn fetch_flamegraphs(
        &self,
        nodes: Vec<Instance>,
        path: &Path,
        query: &str,
        file_prefix: &str,
    ) -> TestbedResult<()> {
        let flamegraph_commands = self
            .protocol_commands
            .nodes_flamegraph_command(nodes, query);
        let stdio = self
            .ssh_manager
            .execute_per_instance(flamegraph_commands, CommandContext::default())
            .await?;
        for (i, (stdout, stderr)) in stdio.into_iter().enumerate() {
            if !stdout.is_empty() {
                let file = path.join(format!("{file_prefix}-{i}.svg"));
                fs::write(file, stdout).unwrap();
            }
            if !stderr.is_empty() {
                let file = path.join(format!("{file_prefix}-{i}.log"));
                fs::write(file, stderr).unwrap();
            }
        }
        Ok(())
    }

    pub async fn wait_for_success<I, S>(&self, instances: I, _benchmark_dir: &Path)
    where
        I: IntoIterator<Item = (Instance, S)> + Clone,
        S: Into<String> + Send + 'static + Clone,
    {
        match self
            .ssh_manager
            .execute_per_instance(
                instances.clone(),
                CommandContext::default().with_retries(10),
            )
            .await
        {
            Ok(_) => {}
            Err(e) => {
                // Handle failure case
                panic!("Command execution failed on one or more instances: {e}");
            }
        }
    }

    pub async fn download_benchmark_stats(
        &self,
        benchmark_stats_path: &str,
        parameters: &BenchmarkParameters<T>,
    ) -> TestbedResult<()> {
        let path = parameters.benchmark_dir.join("benchmark-stats");
        fs::create_dir_all(&path).expect("Failed to create benchmark-stats directory");

        display::action("Downloading benchmark stats");

        let mut downloaded = 0usize;

        for (i, instance) in self.client_instances.iter().enumerate() {
            display::status(format!("{}/{}", i + 1, self.client_instances.len()));

            // Support per-client template path, e.g.
            // "/home/ubuntu/benchmark_stats_{i}.json"
            let remote_path_raw = benchmark_stats_path.replace("{i}", &i.to_string());

            // SFTP/download often does not expand "~", so normalize it.
            let remote_path = if let Some(rest) = remote_path_raw.strip_prefix("~/") {
                format!("/home/ubuntu/{rest}")
            } else {
                remote_path_raw
            };

            let local_file_name = Path::new(&remote_path)
                .file_name()
                .and_then(|s| s.to_str())
                .map(|name| format!("client-{i}-{name}"))
                .unwrap_or_else(|| format!("client-{i}-benchmark-stats.json"));

            let result: TestbedResult<()> = async {
                let connection = self.ssh_manager.connect(instance.ssh_address()).await?;
                let content = connection.download(&remote_path).await?;

                let local_file = path.join(local_file_name);
                fs::write(local_file, content.as_bytes())
                    .expect("Cannot write benchmark stats file");
                Ok(())
            }
            .await;

            match result {
                Ok(_) => {
                    downloaded += 1;
                }
                Err(e) => {
                    display::warn(format!(
                        "Failed to download benchmark stats from client {i} ({}) at '{}': {e}",
                        instance.ssh_address(),
                        remote_path
                    ));
                }
            }
        }

        if downloaded == 0 {
            display::warn(format!(
                "No benchmark stats files downloaded (remote path template: '{}')",
                benchmark_stats_path
            ));
        } else {
            display::config("Downloaded benchmark stats files", downloaded);
        }

        display::done();
        Ok(())
    }

    /// Download the metrics logs from clients.
    pub async fn download_metrics_logs(&self, benchmark_dir: &Path) -> TestbedResult<()> {
        let path = benchmark_dir.join("logs");
        fs::create_dir_all(&path).expect("Failed to create logs directory");

        // Download the clients log files and metrics.
        display::action("Downloading metrics logs");
        for (i, instance) in self.client_instances.iter().enumerate() {
            display::status(format!("{}/{}", i + 1, self.client_instances.len()));

            let _: TestbedResult<()> = async {
                let connection = self.ssh_manager.connect(instance.ssh_address()).await?;

                // Download metrics file if it exists
                match connection.download("metrics.log").await {
                    Ok(metrics_content) => {
                        let metrics_file = path.join(format!("metrics-{i}.log"));
                        fs::write(metrics_file, metrics_content.as_bytes())
                            .expect("Cannot write metrics file");
                    }
                    Err(_) => {
                        display::warn(format!("Metrics file not found for client {i}"));
                    }
                }
                Ok(())
            }
            .await;
        }
        display::done();

        Ok(())
    }

    pub async fn download_prometheus_snapshot(
        &self,
        benchmark_dir: &Path,
        timestamp: &str,
    ) -> TestbedResult<()> {
        if let Some(instance) = &self.metrics_instance {
            display::action("Taking prometheus snapshot");
            let command = Prometheus::take_snapshot_command();

            // prometheus snapshot response structure
            #[derive(serde::Deserialize)]
            struct ResponseData {
                // snapshot directory name
                name: String,
            }
            #[derive(serde::Deserialize)]
            struct Response {
                #[allow(dead_code)]
                status: String,
                data: ResponseData,
            }

            let response = self
                .ssh_manager
                .execute(
                    std::iter::once(instance.clone()),
                    command.clone(),
                    CommandContext::default(),
                )
                .await?
                .into_iter()
                .next()
                .ok_or_else(|| {
                    TestbedError::SshCommandFailed(
                        instance.clone(),
                        command.clone(),
                        "No response from command".into(),
                    )
                })?
                .0;
            let response: Response = serde_json::from_str(&response).map_err(|e| {
                TestbedError::SshCommandFailed(
                    instance.clone(),
                    command.clone(),
                    format!("Failed to parse response: {e}"),
                )
            })?;
            display::done();

            let snapshot_name = response.data.name;
            display::config("Created prometheus snapshot", &snapshot_name);
            display::newline();

            display::action("Downloading prometheus snapshot");
            let snapshot_dir = benchmark_dir.join("snapshot").display().to_string();
            let rsync_args = vec![
                // options: recursive, verbose, compress, override ssh to use key file and disable
                // host key checking
                "-rvze".to_string(),
                // let rsync use ssh with the specified private key file
                format!(
                    "ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -i {}",
                    self.settings.ssh_private_key_file.display()
                ),
                // remote snapshot path: /var/lib/prometheus/<timestamp>/snapshots/<snapshot_name>
                format!(
                    "ubuntu@{}:/var/lib/prometheus/{}/snapshots/{}/",
                    instance.main_ip, timestamp, snapshot_name
                ),
                // local snapshot path: <benchmark_dir>/snapshot
                snapshot_dir,
            ];

            let instance = instance.clone();
            tokio::task::spawn_blocking(move || -> TestbedResult<()> {
                match std::process::Command::new("rsync")
                    .args(&rsync_args)
                    .status()
                {
                    Ok(status) if status.success() => Ok(()),
                    Ok(status) => Err(TestbedError::SshCommandFailed(
                        instance,
                        "rsync ".to_string() + &rsync_args.join(" "),
                        format!("rsync failed with status: {}", status),
                    )),
                    Err(e) => Err(TestbedError::SshCommandFailed(
                        instance,
                        "rsync ".to_string() + &rsync_args.join(" "),
                        format!("rsync failed with error: {}", e),
                    )),
                }
            })
            .await
            .unwrap()?;

            display::done();
            display::status("Downloaded prometheus snapshot");
            display::newline();
        }

        Ok(())
    }

    /// Download the log files from the nodes and clients.
    pub async fn download_logs(&self, benchmark_dir: &Path) -> TestbedResult<LogsAnalyzer> {
        // Create a logs sub-directory for this run.
        let path = benchmark_dir.join("logs");
        fs::create_dir_all(&path).expect("Failed to create logs directory");

        // NOTE: Our ssh library does not seem to be able to transfers files in parallel
        // reliably.
        let mut log_parsers = Vec::new();

        // Download the clients log files.
        display::action("Downloading clients logs");
        for (i, instance) in self.client_instances.iter().enumerate() {
            display::status(format!("{}/{}", i + 1, self.client_instances.len()));

            let _: TestbedResult<()> = async {
                let connection = self.ssh_manager.connect(instance.ssh_address()).await?;

                if self.settings.use_fullnode_for_execution {
                    let fullnode_log_content = connection.download("fullnode.log").await?;
                    let fullnode_log_file = path.join(format!("fullnode-{i}.log"));
                    fs::write(fullnode_log_file, fullnode_log_content.as_bytes())
                        .expect("Cannot write log file");
                }

                let client_log_content = connection.download("client.log").await?;

                let client_log_file = path.join(format!("client-{i}.log"));
                fs::write(client_log_file, client_log_content.as_bytes())
                    .expect("Cannot write log file");

                let mut log_parser = LogsAnalyzer::default();
                log_parser.set_client_errors(&client_log_content);
                log_parsers.push(log_parser);
                Ok(())
            }
            .await;
        }

        display::done();

        display::action("Downloading nodes logs");
        let download_tasks: Vec<_> = self
            .node_instances
            .iter()
            .enumerate()
            .map(|(i, instance)| {
                let ssh_manager = self.ssh_manager.clone();
                let path = path.clone();
                let ssh_address = instance.ssh_address();

                async move {
                    let connection = ssh_manager.connect(ssh_address).await?;
                    let node_log_content = connection.download("node.log").await?;

                    let node_log_file = path.join(format!("node-{i}.log"));
                    fs::write(node_log_file, node_log_content.as_bytes())
                        .expect("Cannot write log file");

                    let mut log_parser = LogsAnalyzer::default();
                    log_parser.set_node_errors(&node_log_content);
                    Ok::<LogsAnalyzer, TestbedError>(log_parser)
                }
            })
            .collect();

        let results = futures::future::join_all(download_tasks).await;
        for (idx, result) in results.into_iter().enumerate() {
            display::status(format!("{}/{}", idx + 1, self.node_instances.len()));
            match result {
                Ok(log_parser) => log_parsers.push(log_parser),
                Err(e) => display::warn(format!("Failed to download node log: {e}")),
            }
        }
        display::done();

        Ok(LogsAnalyzer::aggregate(log_parsers))
    }

    /// Run all the benchmarks specified by the benchmark generator.
    pub async fn run_benchmarks(
        &mut self,
        mut generator: BenchmarkParametersGenerator<T>,
    ) -> TestbedResult<()> {
        display::header("Preparing testbed");
        display::config("Commit", format!("'{}'", &self.settings.repository.commit));
        display::newline();

        // Cleanup the testbed (in case the previous run was not completed).
        self.cleanup(true).await?;
        let timestamp = chrono::Local::now().format("%y%m%d_%H%M%S").to_string();

        display::config("dedicated_clients", self.dedicated_clients);
        display::config("nodes", self.node_instances.len());
        display::config("clients", self.client_instances.len());
        display::config("metrics", self.metrics_instance.is_some());

        display::config(
            "nodes",
            self.node_instances
                .iter()
                .map(|i| i.ssh_address().to_string())
                .collect::<Vec<_>>()
                .join(", "),
        );
        display::config(
            "clients",
            self.client_instances
                .iter()
                .map(|i| i.ssh_address().to_string())
                .collect::<Vec<_>>()
                .join(", "),
        );
        display::config(
            "metrics",
            self.metrics_instance
                .as_ref()
                .map(|i| i.ssh_address().to_string())
                .unwrap_or("<none>".to_string()),
        );

        // Update the software on all instances.
        if !self.skip_testbed_update {
            self.install().await?;
            self.update().await?;
        }

        // Start the instance monitoring tools.
        self.start_monitoring(generator.use_internal_ip_address, &timestamp)
            .await?;

        display::action("Testbed ready!");

        // Run all benchmarks.
        let mut i = 1;
        let mut latest_committee_size = 0;
        let result: TestbedResult<()> = IS_LOOP
            .scope(true, async {
                while let Some(mut parameters) = generator.next() {
                    display::header(format!("Starting benchmark {i}"));
                    display::config("Benchmark type", &parameters.benchmark_type);
                    display::config("Parameters", &parameters);
                    display::newline();

                    parameters.benchmark_dir = self.benchmark_dir.join(format!("{parameters:?}"));

                    if !self.skip_monitoring {
                        if let Some(metrics) = &self.metrics_instance {
                            let host_ip = if generator.use_internal_ip_address {
                                metrics.private_ip
                            } else {
                                metrics.main_ip
                            };

                            parameters.otel.get_or_insert(crate::benchmark::OtelConfig {
                                otlp_endpoint: format!(
                                    "http://{}:{}",
                                    host_ip,
                                    crate::monitor::Tempo::OTLP_GRPC_PORT
                                ),
                                protocol: "grpc".to_string(),
                                sampler: "parentbased_traceidratio".to_string(),
                                sampler_arg: "0.1".to_string(),
                            });
                        }
                    }

                    // Cleanup the testbed (in case the previous run was not completed).
                    self.cleanup(true).await?;
                    // Create benchmark directory.
                    fs::create_dir_all(&parameters.benchmark_dir)
                        .expect("Failed to create benchmark directory");

                    // Swap the loop logger to write to this benchmark's log file
                    let log_file_path = parameters.benchmark_dir.join("logs.log");
                    let new_file = OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(log_file_path)
                        .expect("Failed to open log file");
                    self.benchmark_writer
                        .swap(Box::new(new_file))
                        .expect("Failed to swap log writer");

                    crate::logger::log(
                        &chrono::Local::now()
                            .format("Started %y-%m-%d:%H-%M-%S\n")
                            .to_string(),
                    );

                    let benchmark_result = async {
                        // Configure all instances (if not skipped).
                        if !self.skip_testbed_configuration {
                            self.configure(&parameters).await?;
                            latest_committee_size = parameters.nodes;
                        }

                        // Deploy the validators.
                        self.run_nodes(&parameters).await?;

                        // Deploy the load generators.
                        self.run_clients(&parameters).await?;

                        // Wait for the benchmark to terminate. Then save the results and
                        // print a summary.
                        self.run(&parameters).await?;

                        // Collect and aggregate metrics
                        let mut aggregator =
                            MeasurementsCollection::new(&self.settings, parameters.clone());
                        self.download_metrics_logs(&parameters.benchmark_dir)
                            .await?;

                        // Parse and aggregate metrics from downloaded files
                        aggregator.aggregates_metrics_from_files::<P>(
                            self.client_instances.len(),
                            &parameters.benchmark_dir.join("logs"),
                        );

                        aggregator.display_summary();
                        aggregator.save(&parameters.benchmark_dir);
                        generator.register_result(aggregator);

                        // Flush any remaining logs to the benchmark log file
                        let _ = self.benchmark_writer.flush();

                        TestbedResult::Ok(())
                    }
                    .await;

                    // Download benchmark stats if metadata path is provided
                    if let Some(benchmark_stats_path) = &parameters.benchmark_stats_path {
                        self.download_benchmark_stats(benchmark_stats_path, &parameters)
                            .await?;
                    }

                    // Kill the nodes and clients (without deleting the log files).
                    self.cleanup(false).await?;

                    // Download the log files.
                    if self.log_processing {
                        let error_counter = self.download_logs(&parameters.benchmark_dir).await?;
                        error_counter.print_summary();
                    }

                    // Close the per-benchmark log file
                    crate::logger::log(
                        &chrono::Local::now()
                            .format("Finished %y-%m-%d:%H-%M-%S\n")
                            .to_string(),
                    );

                    // Propagate any error that occurred
                    benchmark_result?;

                    i += 1;
                }

                Ok(())
            })
            .await;

        result?;

        if self.settings.enable_prometheus_snapshots {
            info!("Downloading prometheus snapshot");
            if let Err(e) = self
                .download_prometheus_snapshot(&self.benchmark_dir, &timestamp)
                .await
            {
                display::error(format!("Failed to download prometheus snapshot: {}", e));
            }
            info!("Prometheus snapshot download completed");
        }

        display::header("Benchmark completed");
        Ok(())
    }
}
