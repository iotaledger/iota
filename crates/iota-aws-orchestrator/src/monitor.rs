// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{fs, net::SocketAddr, path::PathBuf};

use crate::{
    client::Instance,
    error::{MonitorError, MonitorResult},
    protocol::ProtocolMetrics,
    ssh::{CommandContext, SshConnectionManager},
};

pub struct Monitor {
    instance: Instance,
    clients: Vec<Instance>,
    nodes: Vec<Instance>,
    ssh_manager: SshConnectionManager,
}

impl Monitor {
    /// Create a new monitor.
    pub fn new(
        instance: Instance,
        clients: Vec<Instance>,
        nodes: Vec<Instance>,
        ssh_manager: SshConnectionManager,
    ) -> Self {
        Self {
            instance,
            clients,
            nodes,
            ssh_manager,
        }
    }

    /// Dependencies to install.
    pub fn dependencies() -> Vec<&'static str> {
        let mut commands = Vec::new();
        commands.extend(Prometheus::install_commands());
        commands.extend(Grafana::install_commands());
        commands.extend(Tempo::install_commands());
        commands
    }

    /// Start a prometheus instance on each remote machine.
    pub async fn start_prometheus<P: ProtocolMetrics>(
        &self,
        protocol_commands: &P,
        use_internal_ip_address: bool,
        use_fullnode_for_execution: bool,
        snapshot_dir: Option<&str>,
    ) -> MonitorResult<()> {
        let instance = std::iter::once(self.instance.clone());
        let commands = Prometheus::setup_commands(
            self.clients.clone(),
            self.nodes.clone(),
            protocol_commands,
            use_internal_ip_address,
            use_fullnode_for_execution,
            snapshot_dir,
        );
        self.ssh_manager
            .execute(instance, commands, CommandContext::default())
            .await?;
        Ok(())
    }

    /// Start grafana on the local host.
    pub async fn start_grafana(&self) -> MonitorResult<()> {
        // Configure and reload grafana.
        let instance = std::iter::once(self.instance.clone());
        let commands = Grafana::setup_commands();
        self.ssh_manager
            .execute(instance, commands, CommandContext::default())
            .await?;

        Ok(())
    }

    /// The public address of the grafana instance.
    pub fn grafana_address(&self) -> String {
        format!("http://{}:{}", self.instance.main_ip, Grafana::DEFAULT_PORT)
    }

    pub async fn start_tempo(&self) -> MonitorResult<()> {
        let instance = std::iter::once(self.instance.clone());
        let commands = Tempo::setup_commands();
        self.ssh_manager
            .execute(instance, commands, CommandContext::default())
            .await?;
        Ok(())
    }
}

/// Generate the commands to setup prometheus on the given instances.
/// TODO: Modify the configuration to also get client metrics.
pub struct Prometheus;

impl Prometheus {
    /// The default prometheus configuration path.
    const DEFAULT_PROMETHEUS_CONFIG_PATH: &'static str = "/etc/prometheus/prometheus.yml";
    const PROMETHEUS_DAEMON_DIR: &'static str = "/etc/systemd/system/prometheus.service.d";
    /// The default prometheus port.
    pub const DEFAULT_PORT: u16 = 9090;

    /// The commands to install prometheus.
    pub fn install_commands() -> Vec<&'static str> {
        vec![
            "sudo apt-get update",
            "sudo apt-get -y install prometheus",
            "sudo chmod 777 -R /var/lib/prometheus/ /etc/prometheus/",
        ]
    }

    /// Generate the commands to update the prometheus configuration and restart
    /// prometheus.
    pub fn setup_commands<I, P>(
        clients: I,
        nodes: I,
        protocol: &P,
        use_internal_ip_address: bool,
        use_fullnode_for_execution: bool,
        snapshot_dir: Option<&str>,
    ) -> String
    where
        I: IntoIterator<Item = Instance>,
        P: ProtocolMetrics,
    {
        // Generate the prometheus' global configuration.
        let mut config = vec![Self::global_configuration()];

        // Add configurations to scrape the clients.
        let mut client_ips = vec![];
        let clients_metrics_path = protocol.clients_metrics_path(clients, use_internal_ip_address);
        for (i, (_, clients_metrics_path)) in clients_metrics_path.into_iter().enumerate() {
            let id = format!("client-{i}");
            let client_ip = clients_metrics_path.split(":").next().unwrap().to_string();
            client_ips.push(client_ip.clone());
            let scrape_config = Self::scrape_configuration(&id, &clients_metrics_path);
            config.push(scrape_config);

            // When using fullnode for execution, also scrape fullnode metrics on port 9184
            if use_fullnode_for_execution {
                let fullnode_metrics_path = format!("{client_ip}:9184/metrics");
                let fullnode_id = format!("fullnode-{i}");
                let fullnode_scrape_config =
                    Self::scrape_configuration(&fullnode_id, &fullnode_metrics_path);
                config.push(fullnode_scrape_config);
            }
        }
        // Add configurations to scrape the nodes.
        let mut node_ips = vec![];
        let nodes_metrics_path = protocol.nodes_metrics_path(nodes, use_internal_ip_address);
        for (i, (_, nodes_metrics_path)) in nodes_metrics_path.into_iter().enumerate() {
            let id = format!("node-{i}");
            let node_ip = nodes_metrics_path.split(":").next().unwrap().to_string();
            node_ips.push(node_ip);
            let scrape_config = Self::scrape_configuration(&id, &nodes_metrics_path);
            config.push(scrape_config);
        }

        // Add client prometheus exporter to the config only if dedicated clients are
        // used
        if !node_ips.contains(client_ips.first().unwrap()) {
            let prometheus_client_exporter_config =
                Self::node_exporter_configuration("prometheus_exporter_clients", client_ips, 9100);
            config.push(prometheus_client_exporter_config);
        }
        // Add configuration to scrape prometheus exporter metrics
        let prometheus_exporter_config =
            Self::node_exporter_configuration("prometheus_exporter", node_ips, 9100);
        config.push(prometheus_exporter_config);

        // Make the command to configure and restart prometheus.
        let mut commands = vec![];
        if let Some(tsdb) = snapshot_dir {
            commands.push(format!("sudo mkdir -p {}", Self::PROMETHEUS_DAEMON_DIR));
            // We need to pass '--web.enable-admin-api' flag to enable snapshots.
            // The safest way to do it is via override file.
            commands.push(format!("(echo \"[Service]\nExecStart=\nExecStart=/usr/bin/prometheus --web.enable-admin-api --storage.tsdb.path=/var/lib/prometheus/{}\" | sudo tee {}/override.conf > /dev/null)",
                tsdb,
                Self::PROMETHEUS_DAEMON_DIR,
            ));
            commands.push("sudo systemctl daemon-reload".to_string());
        }
        commands.push(format!(
            "sudo echo \"{}\" > {}",
            config.join("\n"),
            Self::DEFAULT_PROMETHEUS_CONFIG_PATH
        ));
        commands.push("sudo service prometheus restart".to_string());
        commands.join(" && ")
    }

    pub fn take_snapshot_command() -> String {
        format!(
            "curl -XPOST http://localhost:{}/api/v1/admin/tsdb/snapshot",
            Prometheus::DEFAULT_PORT
        )
    }

    /// Generate the global prometheus configuration.
    /// NOTE: The configuration file is a yaml file so spaces are important.
    fn global_configuration() -> String {
        [
            "global:",
            "  scrape_interval: 5s",
            "  evaluation_interval: 5s",
            "scrape_configs:",
        ]
        .join("\n")
    }

    /// Generate the prometheus configuration from the given metrics path.
    /// NOTE: The configuration file is a yaml file so spaces are important.
    fn scrape_configuration(id: &str, nodes_metrics_path: &str) -> String {
        let parts: Vec<_> = nodes_metrics_path.split('/').collect();
        let address = parts[0].parse::<SocketAddr>().unwrap();
        let ip = address.ip();
        let port = address.port();
        let path = parts[1];

        [
            &format!("  - job_name: {id}"),
            &format!("    metrics_path: /{path}"),
            "    static_configs:",
            "      - targets:",
            &format!("        - {ip}:{port}"),
            "        labels:",
            &format!("          host: {id}"),
        ]
        .join("\n")
    }

    fn node_exporter_configuration(
        id: &str,
        node_ips: Vec<String>,
        prometheus_exporter_port: u16,
    ) -> String {
        let mut configuration = vec![
            format!("  - job_name: {id}"),
            "    static_configs:".to_string(),
            "      - targets:".to_string(),
        ];
        let targets = node_ips
            .into_iter()
            .map(|path| format!("        - {path}:{prometheus_exporter_port}"))
            .collect::<Vec<_>>();
        configuration.extend(targets);
        configuration.join("\n")
    }
}

pub struct Grafana;

impl Grafana {
    /// The path to the datasources directory.
    const DATASOURCES_PATH: &'static str = "/etc/grafana/provisioning/datasources";
    /// The path to the dashboards directory.
    const DASHBOARDS_PATH: &'static str = "/etc/grafana/provisioning/dashboards";
    /// The default grafana port.
    pub const DEFAULT_PORT: u16 = 3000;

    /// The commands to install prometheus.
    pub fn install_commands() -> Vec<&'static str> {
        vec![
            "sudo apt-get install -y apt-transport-https software-properties-common wget",
            "sudo wget -q -O /usr/share/keyrings/grafana.key https://apt.grafana.com/gpg.key",
            "(sudo rm /etc/apt/sources.list.d/grafana.list || true)",
            "echo \"deb [signed-by=/usr/share/keyrings/grafana.key] https://apt.grafana.com stable main\" | sudo tee -a /etc/apt/sources.list.d/grafana.list",
            "sudo apt-get update",
            "sudo apt-get install -y grafana",
            "sudo chmod 777 -R /etc/grafana/",
        ]
    }

    /// Generate the commands to update the grafana datasource and restart
    /// grafana.
    pub fn setup_commands() -> String {
        [
            &format!("(rm -r {} || true)", Self::DATASOURCES_PATH),
            &format!("mkdir -p {}", Self::DATASOURCES_PATH),
            &format!(
                "sudo echo \"{}\" > {}/testbed.yml",
                Self::datasource(),
                Self::DATASOURCES_PATH
            ),
            &format!("(rm -r {} || true)", Self::DASHBOARDS_PATH),
            &format!("mkdir -p {}", Self::DASHBOARDS_PATH),
            &format!(
                "sudo echo \"{}\" > {}/dashboards.yml",
                Self::dashboard_provider(),
                Self::DASHBOARDS_PATH
            ),
            &format!(
                "sudo echo '{}' > {}/aws-dashboard.json",
                include_str!("../assets/grafana-dashboard.json"),
                Self::DASHBOARDS_PATH
            ),
            &format!(
                "sudo cp -f iota/dev-tools/grafana-local/dashboards/cluster-status-dashboard.json {}",
                Self::DASHBOARDS_PATH
            ),
            &format!(
                "sudo cp -f iota/dev-tools/grafana-local/dashboards/consensus-overview.json {}",
                Self::DASHBOARDS_PATH
            ),
            &format!(
                "sudo cp -f iota/dev-tools/grafana-local/dashboards/starfish-overview.json {}",
                Self::DASHBOARDS_PATH
            ),
            "sudo service grafana-server restart",
        ]
        .join(" && ")
    }

    /// Generate the content of the datasource file for the given instance.
    /// NOTE: The datasource file is a yaml file so spaces are important.
    fn datasource() -> String {
        [
            "apiVersion: 1",
            "deleteDatasources:",
            "  - name: testbed",
            "    orgId: 1",
            "  - name: tempo",
            "    orgId: 1",
            "datasources:",
            "  - name: testbed",
            "    type: prometheus",
            "    access: proxy",
            "    orgId: 1",
            &format!("    url: http://localhost:{}", Prometheus::DEFAULT_PORT),
            "    editable: true",
            "    uid: prometheus",
            "",
            "  - name: tempo",
            "    type: tempo",
            "    access: proxy",
            "    orgId: 1",
            &format!("    url: http://localhost:{}", Tempo::DEFAULT_PORT),
            "    editable: true",
            "    uid: tempo",
        ]
        .join("\n")
    }

    /// Generate the dashboard provider configuration.
    fn dashboard_provider() -> String {
        [
            "apiVersion: 1",
            "",
            "providers:",
            "  - name: \"testbed-dashboards\"",
            "    orgId: 1",
            "    folder: \"\"",
            "    type: file",
            "    disableDeletion: false",
            "    editable: true",
            "    allowUiUpdates: true",
            "    options:",
            &format!("      path: {}", Self::DASHBOARDS_PATH),
            "      updateIntervalSeconds: 30",
        ]
        .join("\n")
    }
}

/// Bootstrap the grafana with datasource to connect to the given instances.
/// NOTE: Only for macOS. Grafana must be installed through homebrew (and not
/// from source). Deeper grafana configuration can be done through the
/// grafana.ini file (/opt/homebrew/etc/grafana/grafana.ini) or the plist file
/// (~/Library/LaunchAgents/homebrew.mxcl.grafana.plist).
pub struct LocalGrafana;

#[expect(dead_code)]
impl LocalGrafana {
    /// The default grafana home directory (macOS, homebrew install).
    const DEFAULT_GRAFANA_HOME: &'static str = "/opt/homebrew/opt/grafana/share/grafana/";
    /// The path to the datasources directory.
    const DATASOURCES_PATH: &'static str = "conf/provisioning/datasources/";
    /// The default grafana port.
    pub const DEFAULT_PORT: u16 = 3000;

    /// Configure grafana to connect to the given instances. Only for macOS.
    pub fn run<I>(instances: I) -> MonitorResult<()>
    where
        I: IntoIterator<Item = Instance>,
    {
        let path: PathBuf = [Self::DEFAULT_GRAFANA_HOME, Self::DATASOURCES_PATH]
            .iter()
            .collect();

        // Remove the old datasources.
        fs::remove_dir_all(&path).unwrap();
        fs::create_dir(&path).unwrap();

        // Create the new datasources.
        for (i, instance) in instances.into_iter().enumerate() {
            let mut file = path.clone();
            file.push(format!("instance-{i}.yml"));
            fs::write(&file, Self::datasource(&instance, i)).map_err(|e| {
                MonitorError::Grafana(format!("Failed to write grafana datasource ({e})"))
            })?;
        }

        // Restart grafana.
        std::process::Command::new("brew")
            .arg("services")
            .arg("restart")
            .arg("grafana")
            .arg("-q")
            .spawn()
            .map_err(|e| MonitorError::Grafana(e.to_string()))?;

        Ok(())
    }

    /// Generate the content of the datasource file for the given instance. This
    /// grafana instance takes one datasource per instance and assumes one
    /// prometheus server runs per instance. NOTE: The datasource file is a
    /// yaml file so spaces are important.
    fn datasource(instance: &Instance, index: usize) -> String {
        [
            "apiVersion: 1",
            "deleteDatasources:",
            &format!("  - name: instance-{index}"),
            "    orgId: 1",
            "datasources:",
            &format!("  - name: instance-{index}"),
            "    type: prometheus",
            "    access: proxy",
            "    orgId: 1",
            &format!(
                "    url: http://{}:{}",
                instance.main_ip,
                Prometheus::DEFAULT_PORT
            ),
            "    editable: true",
            &format!("    uid: UID-{index}"),
        ]
        .join("\n")
    }
}

pub struct Tempo;

impl Tempo {
    pub const DEFAULT_PORT: u16 = 3200;
    const BASE_DIR: &'static str = "/etc/iota-monitoring/tempo";
    pub const OTLP_GRPC_PORT: u16 = 4317;
    pub const OTLP_HTTP_PORT: u16 = 4318;

    pub fn install_commands() -> Vec<&'static str> {
        vec![
            "sudo apt-get update",
            // docker-compose-v2 is required for the docker compose file, but it is not available
            // in all distributions, so we install it from the official docker repository. The
            // docker.io package is required as a dependency for docker-compose-v2.
            "sudo apt-get -y install software-properties-common",
            "sudo add-apt-repository -y universe || true",
            "sudo apt-get update",
            // Install docker and compose v2, not docker-compose-plugin
            "sudo apt-get -y install docker.io docker-compose-v2",
            "sudo systemctl enable --now docker",
            "sudo docker version",
            "sudo docker compose version || true",
        ]
    }

    pub fn setup_commands() -> String {
        let dir = Self::BASE_DIR;

        let docker_compose = format!(
            r#"version: "3.8"
services:
  tempo:
    image: grafana/tempo:2.9.0
    command: ["-config.file=/etc/tempo.yaml"]
    volumes:
      - ./tempo.yaml:/etc/tempo.yaml:ro
      - tempo-data:/var/tempo
    ports:
      - "127.0.0.1:{tempo}:{tempo}"

  otelcol:
    image: otel/opentelemetry-collector-contrib:0.90.0
    command: ["--config=/etc/otelcol.yaml"]
    volumes:
      - ./otel-collector.yaml:/etc/otelcol.yaml:ro
    ports:
      - "0.0.0.0:{grpc}:{grpc}"
      - "0.0.0.0:{http}:{http}"
    depends_on:
      - tempo

volumes:
  tempo-data: {{}}
"#,
            tempo = Self::DEFAULT_PORT,  // 3200
            grpc = Self::OTLP_GRPC_PORT, // 4317
            http = Self::OTLP_HTTP_PORT, // 4318
        );

        let tempo_yaml = format!(
            r#"server:
  http_listen_port: {tempo}

distributor:
  receivers:
    otlp:
      protocols:
        grpc:
          endpoint: 0.0.0.0:{grpc}
        http:
          endpoint: 0.0.0.0:{http}

storage:
  trace:
    backend: local
    wal:
      path: /var/tempo/wal
    local:
      path: /var/tempo/blocks

metrics_generator:
  storage:
    path: /var/tempo/generator/wal
  traces_storage:
    path: /var/tempo/generator/traces
  processor:
    local_blocks:
      filter_server_spans: false

# Solution to RATE_LIMITED for ingestion
overrides:
  defaults:
    metrics_generator:
      processors: [local-blocks]
    ingestion:
      rate_strategy: local
      rate_limit_bytes: 100000000   # 100 MB/s
      burst_size_bytes: 200000000   # 200 MB burst
"#,
            tempo = Self::DEFAULT_PORT,
            grpc = Self::OTLP_GRPC_PORT,
            http = Self::OTLP_HTTP_PORT,
        );

        let otel_yaml = r#"receivers:
  otlp:
    protocols:
      grpc:
        endpoint: 0.0.0.0:4317
      http:
        endpoint: 0.0.0.0:4318

processors:
  memory_limiter:
    check_interval: 1s
    limit_mib: 512
  batch:
    timeout: 1s
    send_batch_size: 256
    send_batch_max_size: 512

exporters:
  otlphttp:
    endpoint: http://tempo:4318

service:
  pipelines:
    traces:
      receivers: [otlp]
      processors: [memory_limiter, batch]
      exporters: [otlphttp]
"#;

        format!(
            r#"set -euo pipefail
sudo mkdir -p {dir}

sudo tee {dir}/docker-compose.yml >/dev/null <<'YAML'
{docker_compose}
YAML

sudo tee {dir}/tempo.yaml >/dev/null <<'YAML'
{tempo_yaml}
YAML

sudo tee {dir}/otel-collector.yaml >/dev/null <<'YAML'
{otel_yaml}
YAML

cd {dir}
sudo docker compose down --remove-orphans -v || true
sudo docker compose up -d --remove-orphans
"#,
            dir = dir,
            docker_compose = docker_compose,
            tempo_yaml = tempo_yaml,
            otel_yaml = otel_yaml,
        )
    }
}
