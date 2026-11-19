// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! `iota-tool admin`: commands that call the admin API of a running node.

use std::{
    ffi::OsString,
    fs::{File, OpenOptions},
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail};
use clap::{ArgAction, Args, Parser, Subcommand, ValueEnum};
use comfy_table::Table;
use iota_types::traffic_control::{
    AccessRuleList, AccessRules, BlockedClients, ClientUsage, DefaultAction, PolicyConfig,
    TrafficControlReconfigParams, TrafficControlStatus,
};
use reqwest::{StatusCode, Url};
use serde::{Serialize, de::DeserializeOwned};

const UNTIL_RESTART: &str = "The change lasts until the node restarts.";
const SAVE_HINT: &str = "Run `iota-tool admin traffic-control save --config <file>` to write \
    it to the node config.";
/// How long to wait for the admin API to accept a connection.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// How long to wait for the whole answer of the admin API.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

/// Call the admin API of a running node.
#[derive(Parser)]
pub struct AdminCommand {
    /// The admin API of the node: its `admin-interface-address`, such as
    /// `127.0.0.1:1337`, or a URL.
    #[arg(long, default_value = "http://127.0.0.1:1337", global = true)]
    admin_url: String,
    /// Print JSON, for scripts: what the node answers to a read, the settings
    /// in force for `set`, and `{"answer": ...}` for the other changes. `save`
    /// always prints text.
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: AdminSubcommand,
}

#[derive(Subcommand)]
enum AdminSubcommand {
    /// Read and change the traffic control of the node.
    #[command(subcommand, name = "traffic-control")]
    TrafficControl(TrafficControlCommand),
}

#[derive(Subcommand)]
enum TrafficControlCommand {
    /// Show the settings in force.
    Status,
    /// Change settings. A setting left out keeps its value.
    Set(SetArgs),
    /// Show, add and remove access rules.
    #[command(subcommand)]
    Rules(RulesCommand),
    /// Show the blocked clients.
    Blocklist {
        /// How many clients to show at most. The node shows 200 by default.
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Block a client that connects to the node: an IP address, or an IPv6
    /// prefix as the blocklist names it. An IPv6 address is blocked with its
    /// whole `ipv6-prefix-len` prefix.
    Block {
        /// The IP address or IPv6 prefix to block.
        client: String,
        /// How long to block the client, in seconds.
        #[arg(long)]
        ttl: u64,
    },
    /// Remove the blocks of a client, and reset what the spam and error
    /// policies counted for it and its simulation gas balance.
    Unblock {
        /// The IP address or IPv6 prefix to unblock.
        client: String,
    },
    /// Show the clients with the lowest simulation gas balances, and the
    /// blocked clients.
    Clients {
        /// How many clients of each kind to show at most. The node shows 200 by
        /// default.
        #[arg(long)]
        top: Option<usize>,
    },
    /// Write the settings in force to the `policy-config` of a node config
    /// file, which the node reads at its next start. Shows the change and
    /// asks to confirm it. The file is written again as a whole, so its
    /// comments are lost.
    Save {
        /// The node config file.
        #[arg(long)]
        config: PathBuf,
        /// Write the new config to this file instead, and leave `--config` as
        /// it is, for a config file that cannot be written in place.
        #[arg(long)]
        output: Option<PathBuf>,
        /// Do not ask to confirm.
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Args)]
#[group(skip)]
#[command(group(clap::ArgGroup::new("settings").required(true).multiple(true)))]
struct SetArgs {
    /// Sets the request threshold of the spam policy.
    #[arg(long, group = "settings")]
    spam_threshold: Option<u64>,
    /// Sets the request threshold of the error policy.
    #[arg(long, group = "settings")]
    error_threshold: Option<u64>,
    /// Turns dry run on or off. Under dry run the node serves every client.
    #[arg(long, action = ArgAction::Set, group = "settings")]
    dry_run: Option<bool>,
    /// Sets how long a client that connects to the node stays blocked, in
    /// seconds.
    #[arg(long, group = "settings")]
    connection_blocklist_ttl_sec: Option<u64>,
    /// Sets how long a client behind a fullnode stays blocked, in seconds.
    #[arg(long, group = "settings")]
    proxy_blocklist_ttl_sec: Option<u64>,
    /// Sets the default action of the access rules. Outside dry run, `deny`
    /// refuses every client that no allow rule names. On a validator this also
    /// refuses the other nodes that send it requests.
    #[arg(long, group = "settings")]
    default_action: Option<DefaultActionArg>,
    /// Sets `gas-per-sec` of the simulation gas limit. Turning the limit on
    /// takes this and `--simulation-burst-gas` together.
    #[arg(long, group = "settings")]
    simulation_gas_per_sec: Option<u64>,
    /// Sets `burst-gas` of the simulation gas limit.
    #[arg(long, group = "settings")]
    simulation_burst_gas: Option<u64>,
    /// Do not ask to confirm `--default-action deny`.
    #[arg(long)]
    yes: bool,
}

#[derive(Clone, Copy, ValueEnum)]
enum DefaultActionArg {
    Limit,
    Deny,
}

impl From<DefaultActionArg> for DefaultAction {
    fn from(action: DefaultActionArg) -> Self {
        match action {
            DefaultActionArg::Limit => Self::Limit,
            DefaultActionArg::Deny => Self::Deny,
        }
    }
}

#[derive(Subcommand)]
enum RulesCommand {
    /// Show the access rules.
    List,
    /// Add or remove an allow rule.
    #[command(subcommand)]
    Allow(RuleChange),
    /// Add or remove a deny rule.
    #[command(subcommand)]
    Deny(RuleChange),
}

#[derive(Subcommand)]
enum RuleChange {
    /// Add a rule.
    Add {
        /// An IP address or a CIDR network.
        client: String,
    },
    /// Remove a rule.
    Remove {
        /// An IP address or a CIDR network.
        client: String,
    },
}

/// Asks the operator to confirm a change, and returns whether they did.
type Confirm<'a> = &'a dyn Fn(&str) -> Result<bool>;

/// Where a command prints: `out` for its result, `notes` for what a script
/// does not read.
struct Output<'a> {
    out: &'a mut dyn Write,
    notes: &'a mut dyn Write,
    json: bool,
}

impl AdminCommand {
    pub async fn execute(self) -> Result<()> {
        let confirm = |message: &str| {
            inquire::Confirm::new(message)
                .with_default(false)
                .prompt()
                .context("cannot ask to confirm; pass --yes to go on without asking")
        };
        let mut output = Output {
            out: &mut std::io::stdout(),
            notes: &mut std::io::stderr(),
            json: self.json,
        };
        self.run(&mut output, &confirm).await
    }

    async fn run(self, output: &mut Output<'_>, confirm: Confirm<'_>) -> Result<()> {
        let client = AdminClient::new(&self.admin_url)?;
        let AdminSubcommand::TrafficControl(command) = self.command;
        command.run(&client, output, confirm).await
    }
}

impl TrafficControlCommand {
    async fn run(
        self,
        client: &AdminClient,
        output: &mut Output<'_>,
        confirm: Confirm<'_>,
    ) -> Result<()> {
        match self {
            Self::Status => {
                let status: TrafficControlStatus = client.get("/traffic-control", &()).await?;
                print_status(&status, output)?;
            }
            Self::Set(args) => {
                let params = TrafficControlReconfigParams {
                    error_threshold: args.error_threshold,
                    spam_threshold: args.spam_threshold,
                    dry_run: args.dry_run,
                    connection_blocklist_ttl_sec: args.connection_blocklist_ttl_sec,
                    proxy_blocklist_ttl_sec: args.proxy_blocklist_ttl_sec,
                    simulation_gas_per_sec: args.simulation_gas_per_sec,
                    simulation_burst_gas: args.simulation_burst_gas,
                    default_action: args.default_action.map(DefaultAction::from),
                };
                if params.default_action == Some(DefaultAction::Deny)
                    && !args.yes
                    && !confirm(
                        "Outside dry run, the default action deny refuses every client that \
                         no allow rule names. On a validator this also refuses the other nodes \
                         that send it requests, so add allow rules for them first. Continue?",
                    )?
                {
                    writeln!(output.notes, "Nothing changed.")?;
                    return Ok(());
                }
                client.post("/traffic-control", &params).await?;
                let status: TrafficControlStatus =
                    client.get("/traffic-control", &()).await.context(
                        "the node made the change, but reading the settings in force failed",
                    )?;
                print_status(&status, output)?;
                writeln!(output.notes, "{UNTIL_RESTART} {SAVE_HINT}")?;
            }
            Self::Rules(RulesCommand::List) => {
                let rules: AccessRules = client.get("/traffic-control/rules", &()).await?;
                print_rules(&rules, output)?;
            }
            Self::Rules(RulesCommand::Allow(change)) => {
                change_rule(client, AccessRuleList::Allow, change, output).await?;
            }
            Self::Rules(RulesCommand::Deny(change)) => {
                change_rule(client, AccessRuleList::Deny, change, output).await?;
            }
            Self::Blocklist { limit } => {
                let blocklist: BlockedClients = client
                    .get("/traffic-control/blocklist", &given("limit", limit))
                    .await?;
                print_blocklist(&blocklist, output)?;
            }
            Self::Block {
                client: blocked,
                ttl,
            } => {
                let answer = client
                    .post(
                        "/traffic-control/block",
                        &[("client", blocked), ("ttl_sec", ttl.to_string())],
                    )
                    .await?;
                print_answer(&answer, output)?;
            }
            Self::Unblock { client: unblocked } => {
                let answer = client
                    .post("/traffic-control/unblock", &[("client", unblocked)])
                    .await?;
                print_answer(&answer, output)?;
            }
            Self::Clients { top } => {
                let usage: ClientUsage = client
                    .get("/traffic-control/clients", &given("top", top))
                    .await?;
                print_usage(&usage, output)?;
            }
            Self::Save {
                config,
                output: written,
                yes,
            } => {
                let policy = known_policy(client.get("/traffic-control/config", &()).await?)?;
                save(
                    &config,
                    written.as_deref(),
                    &policy,
                    yes,
                    output.out,
                    confirm,
                )?;
            }
        }
        Ok(())
    }
}

/// The query that names `value`, or none.
fn given(name: &'static str, value: Option<usize>) -> Vec<(&'static str, usize)> {
    value.map(|value| (name, value)).into_iter().collect()
}

async fn change_rule(
    client: &AdminClient,
    list: AccessRuleList,
    change: RuleChange,
    output: &mut Output<'_>,
) -> Result<()> {
    let (path, rule) = match change {
        RuleChange::Add { client } => ("/traffic-control/rules/add", client),
        RuleChange::Remove { client } => ("/traffic-control/rules/remove", client),
    };
    let list = match list {
        AccessRuleList::Allow => "allow",
        AccessRuleList::Deny => "deny",
    };
    let answer = client
        .post(path, &[("list", list.to_string()), ("client", rule)])
        .await?;
    print_answer(&answer, output)?;
    writeln!(output.notes, "{SAVE_HINT}")?;
    Ok(())
}

/// Prints the text the node answers to a change.
fn print_answer(answer: &str, output: &mut Output<'_>) -> Result<()> {
    if output.json {
        return print_json(
            &serde_json::json!({ "answer": answer.trim_end() }),
            output.out,
        );
    }
    write!(output.out, "{answer}")?;
    Ok(())
}

/// `policy`, the `policy-config` the node answers, read as this tool knows it.
/// Fails when this tool reads it differently from what the node holds, such as
/// a setting it does not know, since `save` would then write other settings.
fn known_policy(policy: serde_json::Value) -> Result<PolicyConfig> {
    const OTHER_VERSION: &str = "the node and this iota-tool read the policy-config \
        differently; use the iota-tool of the same version as the node";
    let known: PolicyConfig = serde_json::from_value(policy.clone()).context(OTHER_VERSION)?;
    // Compared as JSON text read again, as the node's answer was, so that a
    // number reads the same on both sides.
    let again: serde_json::Value = serde_json::from_str(&serde_json::to_string(&known)?)?;
    if again != policy {
        bail!(OTHER_VERSION);
    }
    Ok(known)
}

/// Writes `policy`, the `policy-config` the node holds, to the node config
/// file at `path`, once the operator confirmed the change it shows. Keeps the
/// old file next to it, as `<file>.<unix seconds>.bak`. With `written`, writes
/// the new config there instead and leaves `path` as it is. The new file
/// keeps the mode and, where it may, the owner of the old one, and a symbolic
/// link to the file stays one.
fn save(
    path: &Path,
    written: Option<&Path>,
    policy: &PolicyConfig,
    yes: bool,
    out: &mut dyn Write,
    confirm: Confirm<'_>,
) -> Result<()> {
    let path = &std::fs::canonicalize(path)
        .with_context(|| format!("cannot read the node config {}", path.display()))?;
    if written
        .is_some_and(|written| std::fs::canonicalize(written).is_ok_and(|written| written == *path))
    {
        bail!("--output names the node config; leave it out to write the config in place");
    }
    let old = std::fs::read_to_string(path)
        .with_context(|| format!("cannot read the node config {}", path.display()))?;
    let mut config: serde_yaml::Value =
        serde_yaml::from_str(&old).with_context(|| format!("{} is not YAML", path.display()))?;
    let mapping = config
        .as_mapping_mut()
        .with_context(|| format!("{} is not a node config", path.display()))?;
    let key = serde_yaml::Value::from("policy-config");
    let policy = serde_yaml::to_value(policy)?;
    let old_policy = mapping.get(&key).cloned();
    if same_policy(old_policy.as_ref(), &policy) {
        writeln!(out, "The node config already holds the settings in force.")?;
        return Ok(());
    }
    mapping.insert(key, policy.clone());
    let new = serde_yaml::to_string(&config)?;
    let name = path.display().to_string();
    writeln!(out, "The policy-config changes:")?;
    let old_section = old_policy
        .as_ref()
        .map(serde_yaml::to_string)
        .transpose()?
        .unwrap_or_default();
    write!(
        out,
        "{}",
        similar::TextDiff::from_lines(&old_section, &serde_yaml::to_string(&policy)?)
            .unified_diff()
            .header("policy-config", "policy-config")
    )?;
    let target = written.unwrap_or(path);
    writeln!(
        out,
        "The whole file, written again (its comments are not kept):"
    )?;
    write!(
        out,
        "{}",
        similar::TextDiff::from_lines(&old, &new)
            .unified_diff()
            .header(&name, &target.display().to_string())
    )?;
    if !yes && !confirm(&format!("Write this change to {}?", target.display()))? {
        writeln!(out, "Nothing written.")?;
        return Ok(());
    }
    if std::fs::read_to_string(path).ok().as_deref() != Some(old.as_str()) {
        bail!("{name} changed since it was read; run save again");
    }
    match written {
        Some(written) => {
            write_like(path, written, &new)?;
            writeln!(
                out,
                "Wrote {}. Move it over {name} for the node to read it at its next start.",
                written.display()
            )?;
        }
        None => {
            let seconds = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
            let backup = backup(path, &old, seconds)?;
            write_like(path, path, &new)?;
            writeln!(
                out,
                "Wrote {name}, and kept the old file at {}. The node reads it at its next start.",
                backup.display()
            )?;
        }
    }
    Ok(())
}

/// Whether the `policy-config` a file holds, or the default it takes when it
/// holds none, means the same as `policy`.
fn same_policy(old: Option<&serde_yaml::Value>, policy: &serde_yaml::Value) -> bool {
    let old = match old {
        Some(old) => serde_yaml::from_value::<Option<PolicyConfig>>(old.clone()),
        None => Ok(Some(PolicyConfig::default_dos_protection_policy())),
    };
    let (Ok(Some(old)), Ok(new)) = (old, serde_yaml::from_value::<PolicyConfig>(policy.clone()))
    else {
        return false;
    };
    serde_yaml::to_value(old).ok() == serde_yaml::to_value(new).ok()
}

/// Writes `old`, the contents of `path`, to a new file next to it,
/// `<file>.<seconds>.bak`, or `<file>.<seconds>.<n>.bak` when that name is
/// taken, with the mode and, where it may, the owner of `path`. Returns its
/// path.
fn backup(path: &Path, old: &str, seconds: u64) -> Result<PathBuf> {
    let metadata = std::fs::metadata(path)?;
    let mut n = 0;
    loop {
        let mut name = OsString::from(path.as_os_str());
        match n {
            0 => name.push(format!(".{seconds}.bak")),
            n => name.push(format!(".{seconds}.{n}.bak")),
        }
        let backup = PathBuf::from(name);
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        // Created readable by its owner only, as the config may hold keys.
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        match options.open(&backup) {
            Ok(mut file) => {
                let written = take_like(&file, &metadata)
                    .and_then(|()| file.write_all(old.as_bytes()))
                    .and_then(|()| file.sync_all());
                if let Err(err) = written {
                    let _ = std::fs::remove_file(&backup);
                    return Err(err)
                        .with_context(|| format!("cannot keep a backup at {}", backup.display()));
                }
                return Ok(backup);
            }
            Err(err) if err.kind() == ErrorKind::AlreadyExists => n += 1,
            Err(err) => {
                return Err(err)
                    .with_context(|| format!("cannot keep a backup at {}", backup.display()));
            }
        }
    }
}

/// Gives `file` the mode and, where it may, the owner of the file `metadata`
/// describes.
fn take_like(file: &File, metadata: &std::fs::Metadata) -> std::io::Result<()> {
    file.set_permissions(metadata.permissions())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        // Only root may give a file away; as anyone else the file stays theirs.
        let _ = std::os::unix::fs::fchown(file, Some(metadata.uid()), Some(metadata.gid()));
    }
    Ok(())
}

/// Writes `contents` to `target` with the mode and, where it may, the owner of
/// `like`. The contents go to a file of their own next to `target` first,
/// which is then moved over it, so that a failed write leaves `target` as it
/// was.
fn write_like(like: &Path, target: &Path, contents: &str) -> Result<()> {
    let folder = match target.parent() {
        Some(folder) if !folder.as_os_str().is_empty() => folder,
        _ => Path::new("."),
    };
    let failed = || format!("cannot write {}", target.display());
    // Created readable by its owner only, so that no one else reads the
    // config before its mode is set.
    let mut file = tempfile::NamedTempFile::new_in(folder).with_context(failed)?;
    take_like(file.as_file(), &std::fs::metadata(like)?).with_context(failed)?;
    file.write_all(contents.as_bytes())
        .and_then(|()| file.as_file().sync_all())
        .with_context(failed)?;
    file.persist(target)
        .map_err(|err| err.error)
        .with_context(failed)?;
    // The move lasts through a crash only once the folder is on disk too. The
    // file is written by now, so a folder that cannot be synced fails nothing.
    #[cfg(unix)]
    let _ = File::open(folder).and_then(|folder| folder.sync_all());
    Ok(())
}

/// The admin API of a node.
struct AdminClient {
    http: reqwest::Client,
    url: Url,
}

impl AdminClient {
    /// `url` is a URL, or an address such as `127.0.0.1:1337`.
    fn new(url: &str) -> Result<Self> {
        let with_scheme = if url.contains("://") {
            url.to_string()
        } else {
            format!("http://{url}")
        };
        let url = Url::parse(&with_scheme)
            .with_context(|| format!("{url:?} is not an address or a URL of the admin API"))?;
        // The workspace builds `reqwest` without a crypto provider, and a client
        // cannot be built without one. Another may be installed already.
        let _ = rustls::crypto::ring::default_provider().install_default();
        Ok(Self {
            http: reqwest::Client::builder()
                .connect_timeout(CONNECT_TIMEOUT)
                .timeout(REQUEST_TIMEOUT)
                .build()?,
            url,
        })
    }

    fn no_answer(&self) -> String {
        format!(
            "the admin API at {} did not answer in time, or cannot be reached",
            self.url
        )
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.url.as_str().trim_end_matches('/'))
    }

    async fn get<T: DeserializeOwned>(&self, path: &str, query: &impl Serialize) -> Result<T> {
        let response = self
            .http
            .get(self.url(path))
            .query(query)
            .send()
            .await
            .with_context(|| self.no_answer())?;
        let response = check(response).await?;
        response
            .json()
            .await
            .with_context(|| format!("cannot read the answer of the node to {path}"))
    }

    /// Sends a change, and returns what the node answers.
    async fn post(&self, path: &str, query: &impl Serialize) -> Result<String> {
        let response = self
            .http
            .post(self.url(path))
            .query(query)
            .send()
            .await
            .with_context(|| self.no_answer())?;
        Ok(check(response).await?.text().await?)
    }
}

/// Fails with what the node answers when it refuses a request.
async fn check(response: reqwest::Response) -> Result<reqwest::Response> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    let body = response.text().await.unwrap_or_default();
    if matches!(
        status,
        StatusCode::NOT_FOUND | StatusCode::METHOD_NOT_ALLOWED
    ) {
        bail!(
            "the node answered {status}; a node of a version without this admin route answers so"
        );
    }
    bail!("the node answered {status}: {}", body.trim_end())
}

fn print_json(value: &impl Serialize, out: &mut dyn Write) -> Result<()> {
    writeln!(out, "{}", serde_json::to_string_pretty(value)?)?;
    Ok(())
}

fn optional(value: Option<impl ToString>) -> String {
    value.map_or_else(|| "-".to_string(), |value| value.to_string())
}

fn print_status(status: &TrafficControlStatus, output: &mut Output<'_>) -> Result<()> {
    if output.json {
        return print_json(status, output.out);
    }
    let mut table = Table::new();
    table.set_header(["Setting", "Value"]);
    for (setting, value) in [
        ("spam threshold", optional(status.spam_threshold)),
        ("error threshold", optional(status.error_threshold)),
        ("dry run", status.dry_run.to_string()),
        (
            "connection blocklist TTL (s)",
            status.connection_blocklist_ttl_sec.to_string(),
        ),
        (
            "proxy blocklist TTL (s)",
            status.proxy_blocklist_ttl_sec.to_string(),
        ),
        ("default action", default_action(status.default_action)),
        ("allow rules", status.allow_rules.to_string()),
        ("deny rules", status.deny_rules.to_string()),
        ("blocked clients", status.blocked_clients.to_string()),
        (
            "simulation gas per sec",
            optional(
                status
                    .simulation_gas_limit
                    .as_ref()
                    .map(|limit| limit.gas_per_sec),
            ),
        ),
        (
            "simulation burst gas",
            optional(
                status
                    .simulation_gas_limit
                    .as_ref()
                    .map(|limit| limit.burst_gas),
            ),
        ),
    ] {
        table.add_row([setting.to_string(), value]);
    }
    writeln!(output.out, "{table}")?;
    Ok(())
}

fn default_action(action: DefaultAction) -> String {
    match action {
        DefaultAction::Limit => "limit",
        DefaultAction::Deny => "deny",
    }
    .to_string()
}

fn print_rules(rules: &AccessRules, output: &mut Output<'_>) -> Result<()> {
    if output.json {
        return print_json(rules, output.out);
    }
    let mut table = Table::new();
    table.set_header(["List", "Rule"]);
    for (list, rules) in [("allow", &rules.allow), ("deny", &rules.deny)] {
        for rule in rules {
            table.add_row([list, rule]);
        }
    }
    writeln!(output.out, "{table}")?;
    writeln!(
        output.out,
        "Default action: {}",
        default_action(rules.default)
    )?;
    Ok(())
}

fn blocklist_table(blocklist: &BlockedClients) -> Table {
    let mut table = Table::new();
    table.set_header(["Client", "Through a fullnode", "Reason", "Seconds left"]);
    for block in &blocklist.clients {
        table.add_row([
            block.client.clone(),
            block.proxied.to_string(),
            block.reason.to_string(),
            block.seconds_left.to_string(),
        ]);
    }
    table
}

fn print_blocklist(blocklist: &BlockedClients, output: &mut Output<'_>) -> Result<()> {
    if output.json {
        return print_json(blocklist, output.out);
    }
    writeln!(output.out, "{}", blocklist_table(blocklist))?;
    if blocklist.omitted > 0 {
        writeln!(
            output.out,
            "{} more blocked clients not shown.",
            blocklist.omitted
        )?;
    }
    Ok(())
}

fn print_usage(usage: &ClientUsage, output: &mut Output<'_>) -> Result<()> {
    if output.json {
        return print_json(usage, output.out);
    }
    let mut table = Table::new();
    table.set_header(["Client", "Simulation gas balance"]);
    for balance in &usage.gas_balances {
        table.add_row([
            balance
                .client
                .clone()
                .unwrap_or_else(|| "(no resolved IP)".to_string()),
            balance.gas.to_string(),
        ]);
    }
    writeln!(output.out, "{table}")?;
    if usage.omitted_gas_balances > 0 {
        writeln!(
            output.out,
            "{} more gas balances not shown.",
            usage.omitted_gas_balances
        )?;
    }
    print_blocklist(&usage.blocked, output)
}

#[cfg(test)]
mod tests {
    use std::{
        net::SocketAddr,
        sync::{Arc, Mutex},
    };

    use axum::{
        Json, Router,
        extract::State,
        http::{Method, StatusCode, Uri},
        routing::{get, post},
    };
    use iota_config::{Config, NodeConfig};
    use iota_types::traffic_control::{
        BlockReason, BlockedClient, ClientGasBalance, SimulationGasLimitConfig, Weight,
    };

    use super::*;

    /// The requests the stub node got, as `<method> <path>?<query>`.
    type Requests = Arc<Mutex<Vec<String>>>;

    /// What the node answers to a change.
    const CHANGED: &str = "Changed.\nThe change lasts until the node restarts.\n";

    const TEMPLATE: &str = include_str!("../../iota-config/data/fullnode-template.yaml");

    fn status() -> TrafficControlStatus {
        TrafficControlStatus {
            error_threshold: Some(50),
            spam_threshold: Some(1000),
            dry_run: false,
            connection_blocklist_ttl_sec: 60,
            proxy_blocklist_ttl_sec: 60,
            default_action: DefaultAction::Limit,
            allow_rules: 1,
            deny_rules: 0,
            blocked_clients: 1,
            simulation_gas_limit: Some(SimulationGasLimitConfig {
                gas_per_sec: 1_000_000,
                burst_gas: 20_000_000,
            }),
        }
    }

    fn policy() -> PolicyConfig {
        PolicyConfig {
            dry_run: false,
            access_rules: AccessRules {
                allow: vec!["198.51.100.0/24".to_string()],
                ..Default::default()
            },
            ..PolicyConfig::default_dos_protection_policy()
        }
    }

    fn blocklist() -> BlockedClients {
        BlockedClients {
            clients: vec![BlockedClient {
                client: "203.0.113.9".to_string(),
                proxied: false,
                reason: BlockReason::Manual,
                seconds_left: 3600,
            }],
            omitted: 2,
        }
    }

    fn record(requests: &Requests, method: &Method, uri: &Uri) {
        requests.lock().unwrap().push(format!(
            "{method} {}?{}",
            uri.path(),
            uri.query().unwrap_or_default()
        ));
    }

    async fn changed(State(requests): State<Requests>, method: Method, uri: Uri) -> &'static str {
        record(&requests, &method, &uri);
        CHANGED
    }

    async fn get_status(
        State(requests): State<Requests>,
        method: Method,
        uri: Uri,
    ) -> Json<TrafficControlStatus> {
        record(&requests, &method, &uri);
        Json(status())
    }

    async fn get_blocklist(
        State(requests): State<Requests>,
        method: Method,
        uri: Uri,
    ) -> Json<BlockedClients> {
        record(&requests, &method, &uri);
        Json(blocklist())
    }

    async fn get_clients(
        State(requests): State<Requests>,
        method: Method,
        uri: Uri,
    ) -> Json<ClientUsage> {
        record(&requests, &method, &uri);
        Json(ClientUsage {
            gas_balances: vec![ClientGasBalance {
                client: None,
                gas: 42,
            }],
            omitted_gas_balances: 1,
            blocked: blocklist(),
        })
    }

    /// Refuses `ttl_sec=9999999`, as the node refuses a TTL above its maximum.
    async fn block(
        State(requests): State<Requests>,
        method: Method,
        uri: Uri,
    ) -> (StatusCode, &'static str) {
        record(&requests, &method, &uri);
        if uri
            .query()
            .is_some_and(|query| query.ends_with("ttl_sec=9999999"))
        {
            (StatusCode::BAD_REQUEST, "The TTL is too long\n")
        } else {
            (StatusCode::OK, CHANGED)
        }
    }

    /// Starts a stub of the admin API of a node, which records each request
    /// other than reads of the config and the rules.
    async fn stub_node() -> (String, Requests) {
        let requests = Requests::default();
        let router = Router::new()
            .route("/traffic-control", get(get_status).post(changed))
            .route("/traffic-control/config", get(|| async { Json(policy()) }))
            .route(
                "/traffic-control/rules",
                get(|| async {
                    Json(AccessRules {
                        allow: vec!["198.51.100.0/24".to_string()],
                        ..Default::default()
                    })
                }),
            )
            .route("/traffic-control/rules/add", post(changed))
            .route("/traffic-control/rules/remove", post(changed))
            .route("/traffic-control/blocklist", get(get_blocklist))
            .route("/traffic-control/clients", get(get_clients))
            .route("/traffic-control/block", post(block))
            .route("/traffic-control/unblock", post(changed))
            .route("/text", get(|| async { "not JSON" }))
            .with_state(requests.clone());
        let listener = tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
            .await
            .unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, router).await });
        (url, requests)
    }

    /// What a command printed: its result, and its notes.
    struct Printed {
        out: String,
        notes: String,
    }

    async fn run(url: &str, args: &[&str], confirmed: bool) -> Result<Printed> {
        let mut argv = vec!["admin", "--admin-url", url];
        argv.extend_from_slice(args);
        let command = AdminCommand::try_parse_from(argv)?;
        let (mut out, mut notes) = (Vec::new(), Vec::new());
        let json = command.json;
        let mut output = Output {
            out: &mut out,
            notes: &mut notes,
            json,
        };
        command.run(&mut output, &|_: &str| Ok(confirmed)).await?;
        Ok(Printed {
            out: String::from_utf8(out)?,
            notes: String::from_utf8(notes)?,
        })
    }

    #[tokio::test]
    async fn set_sends_only_the_given_settings_and_prints_the_result() {
        let (url, requests) = stub_node().await;
        let printed = run(
            &url,
            &[
                "traffic-control",
                "set",
                "--spam-threshold",
                "500",
                "--dry-run",
                "false",
                "--simulation-gas-per-sec",
                "1000000",
            ],
            false,
        )
        .await
        .unwrap();
        assert_eq!(
            *requests.lock().unwrap(),
            [
                "POST /traffic-control?spam_threshold=500&dry_run=false&simulation_gas_per_sec=1000000",
                "GET /traffic-control?",
            ]
        );
        assert!(printed.out.contains("spam threshold"), "{}", printed.out);
        assert!(printed.out.contains("1000000"), "{}", printed.out);
        assert_eq!(printed.notes, format!("{UNTIL_RESTART} {SAVE_HINT}\n"));
    }

    #[tokio::test]
    async fn set_takes_at_least_one_setting() {
        let (url, requests) = stub_node().await;
        assert!(run(&url, &["traffic-control", "set"], false).await.is_err());
        assert!(
            run(&url, &["traffic-control", "set", "--yes"], false)
                .await
                .is_err()
        );
        assert!(requests.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn json_prints_only_json() {
        let (url, _) = stub_node().await;
        let printed = run(
            &url,
            &[
                "--json",
                "traffic-control",
                "set",
                "--spam-threshold",
                "500",
            ],
            false,
        )
        .await
        .unwrap();
        assert_eq!(
            serde_json::from_str::<TrafficControlStatus>(&printed.out).unwrap(),
            status()
        );
        assert!(
            printed.notes.starts_with(UNTIL_RESTART),
            "{}",
            printed.notes
        );

        let printed = run(
            &url,
            &["--json", "traffic-control", "unblock", "203.0.113.9"],
            false,
        )
        .await
        .unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&printed.out).unwrap(),
            serde_json::json!({ "answer": CHANGED.trim_end() })
        );
    }

    #[tokio::test]
    async fn the_default_action_deny_takes_a_confirmation() {
        let (url, requests) = stub_node().await;
        let args = ["traffic-control", "set", "--default-action", "deny"];
        let printed = run(&url, &args, false).await.unwrap();
        assert_eq!(printed.notes, "Nothing changed.\n");
        assert!(requests.lock().unwrap().is_empty());

        run(&url, &args, true).await.unwrap();
        let mut with_yes = args.to_vec();
        with_yes.push("--yes");
        run(&url, &with_yes, false).await.unwrap();
        let posts = requests
            .lock()
            .unwrap()
            .iter()
            .filter(|request| request.starts_with("POST"))
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(posts, ["POST /traffic-control?default_action=deny"; 2]);
    }

    #[tokio::test]
    async fn rule_and_unblock_commands_send_their_queries() {
        let (url, requests) = stub_node().await;
        for args in [
            &[
                "traffic-control",
                "rules",
                "allow",
                "add",
                "198.51.100.0/24",
            ][..],
            &["traffic-control", "rules", "deny", "remove", "192.0.2.0/24"],
        ] {
            let printed = run(&url, args, false).await.unwrap();
            assert_eq!(printed.out, CHANGED);
            assert_eq!(printed.notes, format!("{SAVE_HINT}\n"));
        }
        // A block is not part of the config, so there is nothing to save.
        let printed = run(&url, &["traffic-control", "unblock", "203.0.113.9"], false)
            .await
            .unwrap();
        assert_eq!(
            (printed.out.as_str(), printed.notes.as_str()),
            (CHANGED, "")
        );
        assert_eq!(
            *requests.lock().unwrap(),
            [
                "POST /traffic-control/rules/add?list=allow&client=198.51.100.0%2F24",
                "POST /traffic-control/rules/remove?list=deny&client=192.0.2.0%2F24",
                "POST /traffic-control/unblock?client=203.0.113.9",
            ]
        );
    }

    #[tokio::test]
    async fn a_refusal_of_the_node_is_an_error_with_its_answer() {
        let (url, _) = stub_node().await;
        let error = run(
            &url,
            &[
                "traffic-control",
                "block",
                "203.0.113.9",
                "--ttl",
                "9999999",
            ],
            false,
        )
        .await
        .err()
        .unwrap();
        assert_eq!(
            error.to_string(),
            "the node answered 400 Bad Request: The TTL is too long"
        );
        let missing = AdminClient::new(&url)
            .unwrap()
            .get::<serde_json::Value>("/no-such-route", &())
            .await
            .unwrap_err();
        assert!(
            missing.to_string().contains("without this admin route"),
            "{missing}"
        );
        let text = AdminClient::new(&url)
            .unwrap()
            .get::<serde_json::Value>("/text", &())
            .await
            .unwrap_err();
        assert!(
            text.to_string().contains("cannot read the answer"),
            "{text}"
        );
        // Nothing listens on port 1.
        let down = AdminClient::new("127.0.0.1:1")
            .unwrap()
            .get::<serde_json::Value>("/traffic-control", &())
            .await
            .unwrap_err();
        assert!(down.to_string().contains("cannot be reached"), "{down}");
    }

    #[tokio::test]
    async fn the_admin_url_may_be_an_address() {
        let (url, requests) = stub_node().await;
        let address = url.trim_start_matches("http://");
        run(address, &["traffic-control", "status"], false)
            .await
            .unwrap();
        assert_eq!(*requests.lock().unwrap(), ["GET /traffic-control?"]);
        let error = AdminClient::new("http://[not an address").err().unwrap();
        assert!(error.to_string().contains("is not an address"), "{error}");
    }

    #[tokio::test]
    async fn block_and_listings_send_their_queries() {
        let (url, requests) = stub_node().await;
        let printed = run(
            &url,
            &["traffic-control", "block", "203.0.113.9", "--ttl", "3600"],
            false,
        )
        .await
        .unwrap();
        assert_eq!(printed.out, CHANGED);
        run(
            &url,
            &["traffic-control", "blocklist", "--limit", "100"],
            false,
        )
        .await
        .unwrap();
        let out = run(&url, &["traffic-control", "clients", "--top", "5"], false)
            .await
            .unwrap()
            .out;
        assert!(out.contains("(no resolved IP)"), "{out}");
        assert!(out.contains("1 more gas balances not shown."), "{out}");
        assert!(out.contains("203.0.113.9"), "{out}");
        run(&url, &["traffic-control", "clients"], false)
            .await
            .unwrap();
        assert_eq!(
            *requests.lock().unwrap(),
            [
                "POST /traffic-control/block?client=203.0.113.9&ttl_sec=3600",
                "GET /traffic-control/blocklist?limit=100",
                "GET /traffic-control/clients?top=5",
                "GET /traffic-control/clients?",
            ]
        );
    }

    #[tokio::test]
    async fn listings_print_as_tables_or_json() {
        let (url, _) = stub_node().await;
        let out = run(&url, &["traffic-control", "blocklist"], false)
            .await
            .unwrap()
            .out;
        assert!(out.contains("203.0.113.9"), "{out}");
        assert!(out.contains("manual"), "{out}");
        assert!(
            out.ends_with("2 more blocked clients not shown.\n"),
            "{out}"
        );

        let out = run(&url, &["traffic-control", "status"], false)
            .await
            .unwrap()
            .out;
        assert!(out.contains("default action"), "{out}");
        assert!(out.contains("20000000"), "{out}");
        let out = run(&url, &["--json", "traffic-control", "status"], false)
            .await
            .unwrap()
            .out;
        assert_eq!(
            serde_json::from_str::<TrafficControlStatus>(&out).unwrap(),
            status()
        );

        let out = run(&url, &["traffic-control", "rules", "list"], false)
            .await
            .unwrap()
            .out;
        assert!(out.contains("198.51.100.0/24"), "{out}");
        assert!(out.ends_with("Default action: limit\n"), "{out}");
    }

    fn backups(dir: &Path) -> Vec<PathBuf> {
        let mut backups = std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|entry| {
                entry
                    .extension()
                    .is_some_and(|extension| extension == "bak")
            })
            .collect::<Vec<_>>();
        backups.sort();
        backups
    }

    /// Loads and checks `path` as the node does at its start.
    fn load(path: &Path) -> NodeConfig {
        let config = NodeConfig::load(path).unwrap();
        config.validate().unwrap();
        config
    }

    #[tokio::test]
    async fn save_shows_the_change_keeps_a_backup_and_writes_a_config_that_loads() {
        let (url, _) = stub_node().await;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fullnode.yaml");
        std::fs::write(&path, TEMPLATE).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            // Not the mode a backup is created with, to show that it is copied.
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o640)).unwrap();
        }
        let path_arg = path.to_str().unwrap();

        // Not confirmed: the file stays as it was.
        let out = run(
            &url,
            &["traffic-control", "save", "--config", path_arg],
            false,
        )
        .await
        .unwrap()
        .out;
        assert!(out.starts_with("The policy-config changes:\n"), "{out}");
        assert!(out.contains("+policy-config:"), "{out}");
        assert!(out.ends_with("Nothing written.\n"), "{out}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), TEMPLATE);

        let out = run(
            &url,
            &["traffic-control", "save", "--config", path_arg, "--yes"],
            false,
        )
        .await
        .unwrap()
        .out;
        assert!(out.contains("kept the old file"), "{out}");
        let [backup] = backups(dir.path()).try_into().unwrap();
        assert_eq!(std::fs::read_to_string(&backup).unwrap(), TEMPLATE);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            for file in [&path, &backup] {
                let mode = std::fs::metadata(file).unwrap().permissions().mode();
                assert_eq!(mode & 0o777, 0o640, "{}", file.display());
            }
        }
        let saved = load(&path)
            .policy_config
            .expect("the policy config is saved");
        assert_eq!(saved.access_rules, policy().access_rules);
        assert!(!saved.dry_run);

        // A second save finds nothing to change.
        let out = run(
            &url,
            &["traffic-control", "save", "--config", path_arg],
            false,
        )
        .await
        .unwrap()
        .out;
        assert_eq!(
            out,
            "The node config already holds the settings in force.\n"
        );
    }

    #[tokio::test]
    async fn save_replaces_the_policy_config_the_file_holds() {
        let (url, _) = stub_node().await;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fullnode.yaml");
        let old_policy = PolicyConfig {
            dry_run: true,
            ..policy()
        };
        let mut old: serde_yaml::Mapping = serde_yaml::from_str(TEMPLATE).unwrap();
        old.insert(
            "policy-config".into(),
            serde_yaml::to_value(&old_policy).unwrap(),
        );
        std::fs::write(&path, serde_yaml::to_string(&old).unwrap()).unwrap();

        let out = run(
            &url,
            &[
                "traffic-control",
                "save",
                "--config",
                path.to_str().unwrap(),
                "--yes",
            ],
            false,
        )
        .await
        .unwrap()
        .out;
        assert!(
            out.contains("--- policy-config\n+++ policy-config\n"),
            "{out}"
        );
        assert!(out.contains("\n-dry-run: true\n"), "{out}");
        assert!(out.contains("\n+dry-run: false\n"), "{out}");
        let mut new: serde_yaml::Mapping =
            serde_yaml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let saved = new.remove(&"policy-config".into()).unwrap();
        old.remove(&"policy-config".into());
        assert_eq!(new, old);
        assert!(
            !serde_yaml::from_value::<PolicyConfig>(saved)
                .unwrap()
                .dry_run
        );
        load(&path);
    }

    #[tokio::test]
    async fn save_with_an_output_leaves_the_config_as_it_is() {
        let (url, _) = stub_node().await;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fullnode.yaml");
        let written = dir.path().join("new.yaml");
        std::fs::write(&path, TEMPLATE).unwrap();

        let out = run(
            &url,
            &[
                "traffic-control",
                "save",
                "--config",
                path.to_str().unwrap(),
                "--output",
                written.to_str().unwrap(),
                "--yes",
            ],
            false,
        )
        .await
        .unwrap()
        .out;
        assert!(
            out.ends_with(&format!(
                "Wrote {}. Move it over {} for the node to read it at its next start.\n",
                written.display(),
                std::fs::canonicalize(&path).unwrap().display()
            )),
            "{out}"
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), TEMPLATE);
        assert_eq!(backups(dir.path()), Vec::<PathBuf>::new());
        assert_eq!(
            load(&written).policy_config.unwrap().access_rules,
            policy().access_rules
        );

        let error = run(
            &url,
            &[
                "traffic-control",
                "save",
                "--config",
                path.to_str().unwrap(),
                "--output",
                path.to_str().unwrap(),
                "--yes",
            ],
            false,
        )
        .await
        .err()
        .unwrap();
        assert!(
            error.to_string().contains("names the node config"),
            "{error}"
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), TEMPLATE);
    }

    #[test]
    fn save_leaves_a_config_that_takes_the_settings_in_force_by_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fullnode.yaml");
        std::fs::write(&path, TEMPLATE).unwrap();
        let mut out = Vec::new();
        save(
            &path,
            None,
            &PolicyConfig::default_dos_protection_policy(),
            true,
            &mut out,
            &|_: &str| Ok(true),
        )
        .unwrap();
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "The node config already holds the settings in force.\n"
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), TEMPLATE);
    }

    #[test]
    fn save_refuses_a_config_that_changed_while_it_asked() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fullnode.yaml");
        std::fs::write(&path, TEMPLATE).unwrap();
        let edited = format!("{TEMPLATE}\n# edited\n");
        let error = save(
            &path,
            None,
            &policy(),
            false,
            &mut Vec::new(),
            &|_: &str| {
                std::fs::write(&path, &edited).unwrap();
                Ok(true)
            },
        )
        .unwrap_err();
        assert!(
            error.to_string().contains("changed since it was read"),
            "{error}"
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), edited);
        assert_eq!(backups(dir.path()), Vec::<PathBuf>::new());
    }

    #[test]
    fn save_refuses_settings_this_tool_does_not_know() {
        // The node answers JSON text; 0.2 is not exact as an `f32`.
        let held = PolicyConfig {
            spam_sample_rate: Weight::new(0.2).unwrap(),
            ..policy()
        };
        let text = serde_json::to_string(&held).unwrap();
        assert!(text.contains("\"spam-sample-rate\":0.2"), "{text}");
        let mut answered: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(
            known_policy(answered.clone()).unwrap().access_rules,
            policy().access_rules
        );
        answered["new-setting"] = serde_json::json!(true);
        let error = known_policy(answered).unwrap_err();
        assert!(
            error.to_string().contains("read the policy-config"),
            "{error}"
        );
    }

    #[test]
    fn a_backup_never_replaces_another() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fullnode.yaml");
        std::fs::write(&path, "config").unwrap();
        let first = backup(&path, "first", 7).unwrap();
        let second = backup(&path, "second", 7).unwrap();
        assert_eq!(first, dir.path().join("fullnode.yaml.7.bak"));
        assert_eq!(second, dir.path().join("fullnode.yaml.7.1.bak"));
        assert_eq!(std::fs::read_to_string(first).unwrap(), "first");
        assert_eq!(std::fs::read_to_string(second).unwrap(), "second");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn save_through_a_symbolic_link_writes_the_file_it_names() {
        let (url, _) = stub_node().await;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fullnode.yaml");
        let link = dir.path().join("config.yaml");
        std::fs::write(&path, TEMPLATE).unwrap();
        std::os::unix::fs::symlink(&path, &link).unwrap();

        run(
            &url,
            &[
                "traffic-control",
                "save",
                "--config",
                link.to_str().unwrap(),
                "--yes",
            ],
            false,
        )
        .await
        .unwrap();
        assert!(
            std::fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(
            load(&path).policy_config.unwrap().access_rules,
            policy().access_rules
        );
    }
}
