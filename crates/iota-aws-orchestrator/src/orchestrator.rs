// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::HashSet,
    fs::{self},
    marker::PhantomData,
    path::{Path, PathBuf},
    time::Duration,
};

use tokio::time::{self, Instant};

use crate::{
    benchmark::{BenchmarkParameters, BenchmarkParametersGenerator, BenchmarkType},
    build_cache::BuildCacheService,
    client::Instance,
    display,
    error::TestbedResult,
    faults::CrashRecoverySchedule,
    logs::LogsAnalyzer,
    measurement::{Measurement, MeasurementsCollection},
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
}

impl<P: ProtocolCommands<T> + ProtocolMetrics, T: BenchmarkType> Orchestrator<P, T> {
    /// Boot one node per instance.
    async fn boot_nodes(
        &self,
        instances: Vec<Instance>,
        parameters: &BenchmarkParameters<T>,
    ) -> TestbedResult<()> {
        // Run one node per instance.
        let targets = self
            .protocol_commands
            .node_command(instances.clone(), parameters);

        let repo = self.settings.repository_name();
        let context = CommandContext::new()
            .run_background("node".into())
            .with_log_file("~/node.log".into())
            .with_execute_from_path(repo.into());
        if parameters.use_internal_ip_address {
            if let Some(latency_topology) = parameters.latency_topology.clone() {
                let latency_context = CommandContext::default();
                let latency_commands = NetworkLatencyCommandBuilder::new(&instances)
                    .with_perturbation_spec(parameters.perturbation_spec.clone())
                    .with_topology_layout(latency_topology)
                    .with_max_latency(parameters.maximum_latency)
                    .build_network_latency_matrix();
                self.ssh_manager
                    .execute_per_instance(latency_commands, latency_context)
                    .await?;
            }
        }
        self.ssh_manager
            .execute_per_instance(targets, context)
            .await?;

        // Wait until all nodes are reachable.
        let commands = self
            .protocol_commands
            .nodes_metrics_command(instances.clone(), parameters);
        self.ssh_manager.wait_for_success(commands).await;

        Ok(())
    }

    /// Install the codebase and its dependencies on the testbed.
    pub async fn install(&self) -> TestbedResult<()> {
        display::action("Installing dependencies on all machines");

        let working_dir = self.settings.working_dir.display();
        let url = &self.settings.repository.url;

        let use_precompiled_binaries = self.settings.build_cache_enabled();

        let working_dir_cmd = format!("mkdir -p {working_dir}");
        let git_clone_cmd = format!("(git clone {url} || true)");

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

        let mut basic_commands = vec![
            "sudo apt-get update",
            "sudo apt-get -y upgrade",
            "sudo apt-get -y autoremove",
            // Disable "pending kernel upgrade" message.
            "sudo apt-get -y remove needrestart",
            "sudo apt-get -y install curl git ca-certificates",
            // Create the working directory.
            working_dir_cmd.as_str(),
            // Clone the repo.
            git_clone_cmd.as_str(),
        ];

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

        let context = CommandContext::default();
        self.ssh_manager
            .execute(self.instances(), command, context.clone())
            .await?;
        if !self.skip_monitoring {
            let metrics_instance = self
                .metrics_instance
                .clone()
                .expect("No metrics instance available");
            let monitor_command = Monitor::dependencies().join(" && ");
            self.ssh_manager
                .execute(vec![metrics_instance], monitor_command, context)
                .await?;
        }

        display::done();
        Ok(())
    }

    /// Reload prometheus on all instances.
    pub async fn start_monitoring(&self, parameters: &BenchmarkParameters<T>) -> TestbedResult<()> {
        if let Some(instance) = &self.metrics_instance {
            display::action("Configuring monitoring instance");

            let monitor = Monitor::new(
                instance.clone(),
                self.client_instances.clone(),
                self.node_instances.clone(),
                self.ssh_manager.clone(),
            );
            monitor
                .start_prometheus(&self.protocol_commands, parameters)
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

        // Update all active instances.
        let commit = &self.settings.repository.commit;
        let git_update_command = [
            &format!("git fetch origin {commit} --force"),
            &format!("(git reset --hard origin/{commit} || git checkout --force {commit})"),
            "git clean -fd -e target",
        ]
        .join(" && ");

        let id = "git update";
        let repo_name = self.settings.repository_name();
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

        let build_groups = self.settings.build_groups();

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
        display::action(format!("Genesis command: {command}"));
        let repo_name = self.settings.repository_name();
        let context = CommandContext::new().with_execute_from_path(repo_name.into());
        self.ssh_manager
            .execute(self.instances_without_metrics(), command, context)
            .await?;

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

        display::done();
        Ok(())
    }

    /// Deploy the nodes.
    pub async fn run_nodes(&self, parameters: &BenchmarkParameters<T>) -> TestbedResult<()> {
        display::action("Deploying validators");

        // Boot one node per instance.
        self.boot_nodes(self.node_instances.clone(), parameters)
            .await?;

        display::done();
        Ok(())
    }

    /// Deploy the load generators.
    pub async fn run_clients(&self, parameters: &BenchmarkParameters<T>) -> TestbedResult<()> {
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
        let commands = self
            .protocol_commands
            .clients_metrics_command(self.client_instances.clone(), parameters);
        self.ssh_manager.wait_for_success(commands).await;

        display::done();
        Ok(())
    }

    /// Collect metrics from the load generators.
    pub async fn run(
        &self,
        parameters: &BenchmarkParameters<T>,
    ) -> TestbedResult<MeasurementsCollection<T>> {
        display::action(format!(
            "Scraping metrics (at least {}s)",
            parameters.duration.as_secs()
        ));

        // Regularly scrape the client
        let metrics_commands = self
            .protocol_commands
            .clients_metrics_command(self.client_instances.clone(), parameters);

        let mut aggregator = MeasurementsCollection::new(&self.settings, parameters.clone());
        let mut metrics_interval = time::interval(self.scrape_interval);
        metrics_interval.tick().await; // The first tick returns immediately.

        let faults_type = parameters.faults.clone();
        let mut faults_schedule =
            CrashRecoverySchedule::new(faults_type, self.node_instances.clone());
        let mut faults_interval = time::interval(self.crash_interval);
        faults_interval.tick().await; // The first tick returns immediately.

        let start = Instant::now();
        loop {
            tokio::select! {
                // Scrape metrics.
                now = metrics_interval.tick() => {
                    let elapsed = now.duration_since(start).as_secs_f64().ceil() as u64;
                    display::status(format!("{elapsed}s"));

                    let stdio = self
                        .ssh_manager
                        .execute_per_instance(metrics_commands.clone(), CommandContext::default())
                        .await?;
                    for (i, (stdout, _stderr)) in stdio.iter().enumerate() {
                        display::action(format!("Processing metrics from client {}\n", i));
                        let measurement = Measurement::from_prometheus::<P>(stdout);
                        aggregator.add(i, measurement);
                    }

                    if elapsed > parameters.duration .as_secs() {
                        break;
                    }
                },

                // Kill and recover nodes according to the input schedule.
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

        let results_directory = &self.settings.results_dir;
        let commit = &self.settings.repository.commit;
        let path: PathBuf = [results_directory, &format!("results-{commit}").into()]
            .iter()
            .collect();
        fs::create_dir_all(&path).expect("Failed to create log directory");
        aggregator.save(&path);

        if self.settings.enable_flamegraph {
            self.fetch_flamegraphs(
                parameters,
                self.node_instances.clone(),
                &path,
                "?svg=true",
                "flamegraph",
            )
            .await?;
        }

        display::done();
        Ok(aggregator)
    }

    async fn fetch_flamegraphs(
        &self,
        parameters: &BenchmarkParameters<T>,
        nodes: Vec<Instance>,
        path: &Path,
        query: &str,
        file_prefix: &str,
    ) -> TestbedResult<()> {
        let flamegraph_commands = self
            .protocol_commands
            .nodes_flamegraph_command(nodes, parameters, query);
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

    /// Download the log files from the nodes and clients.
    pub async fn download_logs(
        &self,
        parameters: &BenchmarkParameters<T>,
    ) -> TestbedResult<LogsAnalyzer> {
        // Create a log sub-directory for this run.
        let commit = &self.settings.repository.commit;
        let path: PathBuf = [
            &self.settings.logs_dir,
            &format!("logs-{commit}").into(),
            &format!("logs-{parameters:?}").into(),
        ]
        .iter()
        .collect();
        fs::create_dir_all(&path).expect("Failed to create log directory");

        // NOTE: Our ssh library does not seem to be able to transfers files in parallel
        // reliably.
        let mut log_parsers = Vec::new();

        // Download the clients log files.
        display::action("Downloading clients logs");
        for (i, instance) in self.client_instances.iter().enumerate() {
            display::status(format!("{}/{}", i + 1, self.client_instances.len()));

            let connection = self.ssh_manager.connect(instance.ssh_address()).await?;
            let client_log_content = connection.download("client.log").await?;

            let client_log_file = [path.clone(), format!("client-{i}.log").into()]
                .iter()
                .collect::<PathBuf>();
            fs::write(&client_log_file, client_log_content.as_bytes())
                .expect("Cannot write log file");

            let mut log_parser = LogsAnalyzer::default();
            log_parser.set_client_errors(&client_log_content);
            log_parsers.push(log_parser)
        }
        display::done();

        display::action("Downloading nodes logs");
        for (i, instance) in self.node_instances.iter().enumerate() {
            display::status(format!("{}/{}", i + 1, self.node_instances.len()));

            let connection = self.ssh_manager.connect(instance.ssh_address()).await?;
            let node_log_content = connection.download("node.log").await?;

            let node_log_file = [path.clone(), format!("node-{i}.log").into()]
                .iter()
                .collect::<PathBuf>();
            fs::write(&node_log_file, node_log_content.as_bytes()).expect("Cannot write log file");

            let mut log_parser = LogsAnalyzer::default();
            log_parser.set_node_errors(&node_log_content);
            log_parsers.push(log_parser)
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

        // Update the software on all instances.
        if !self.skip_testbed_update {
            self.install().await?;
            self.update().await?;
        }

        // Run all benchmarks.
        let mut i = 1;
        let mut latest_committee_size = 0;
        while let Some(parameters) = generator.next() {
            display::header(format!("Starting benchmark {i}"));
            display::config("Benchmark type", &parameters.benchmark_type);
            display::config("Parameters", &parameters);
            display::newline();

            // Cleanup the testbed (in case the previous run was not completed).
            self.cleanup(true).await?;
            // Start the instance monitoring tools.
            self.start_monitoring(&parameters).await?;

            // Configure all instances (if needed).
            if !self.skip_testbed_configuration && latest_committee_size != parameters.nodes {
                self.configure(&parameters).await?;
                latest_committee_size = parameters.nodes;
            }

            // Deploy the validators.
            self.run_nodes(&parameters).await?;

            // Deploy the load generators.
            self.run_clients(&parameters).await?;

            // Wait for the benchmark to terminate. Then save the results and print a
            // summary.
            let aggregator = self.run(&parameters).await?;
            aggregator.display_summary();
            generator.register_result(aggregator);
            // drop(monitor);

            // Kill the nodes and clients (without deleting the log files).
            self.cleanup(false).await?;

            // Download the log files.
            if self.log_processing {
                let error_counter = self.download_logs(&parameters).await?;
                error_counter.print_summary();
            }

            i += 1;
        }

        display::header("Benchmark completed");
        Ok(())
    }
}
