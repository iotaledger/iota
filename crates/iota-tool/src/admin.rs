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
use iota_config::node::default_admin_interface_address;
use iota_traffic_controller::canonical_access_rules;
use iota_types::traffic_control::{
    AccessRuleChangeQuery, AccessRuleList, AccessRules, BlockedClients, BlocklistQuery,
    ClientUsage, ClientUsageQuery, DEFAULT_DENY_WARNING, DefaultAction, ManualBlockQuery,
    PolicyConfig, TRAFFIC_CONTROL_BLOCK_ROUTE, TRAFFIC_CONTROL_BLOCKLIST_ROUTE,
    TRAFFIC_CONTROL_CLIENTS_ROUTE, TRAFFIC_CONTROL_CONFIG_ROUTE, TRAFFIC_CONTROL_ROUTE,
    TRAFFIC_CONTROL_RULES_ADD_ROUTE, TRAFFIC_CONTROL_RULES_REMOVE_ROUTE,
    TRAFFIC_CONTROL_RULES_ROUTE, TRAFFIC_CONTROL_UNBLOCK_ROUTE, TrafficControlReconfigParams,
    TrafficControlStatus, UnblockQuery,
};
use reqwest::{StatusCode, Url};
use serde::{Serialize, de::DeserializeOwned};

const SAVE_HINT: &str = "Run `iota-tool admin traffic-control save --config <file>` to write \
    it to the node config.";

/// Call the admin API of a running node.
#[derive(Debug, Parser)]
pub struct AdminCommand {
    /// The admin API of the node: its `admin-interface-address`, such as
    /// `127.0.0.1:1337`, or a URL.
    #[arg(long, default_value_t = default_admin_interface_address().to_string(), global = true)]
    admin_url: String,
    /// Print JSON, for scripts: what the node answers to a read, the settings
    /// in force for `set`, and `{"answer": ...}` for the other changes. `save`
    /// always prints text.
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: AdminSubcommand,
}

#[derive(Debug, Subcommand)]
enum AdminSubcommand {
    /// Read and change the traffic control of the node.
    #[command(subcommand, name = "traffic-control")]
    TrafficControl(TrafficControlCommand),
}

#[derive(Debug, Subcommand)]
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
        /// How many clients to show at most. Without it, the node shows its
        /// default number.
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Block a client: refuse the requests whose client it is. The client of
    /// a request is the address that connects to the node, or the one that
    /// `client-id-source` names. A block does not refuse requests that reach
    /// the node through a fullnode. An IPv6 address is blocked with its whole
    /// `ipv6-prefix-len` prefix.
    Block {
        /// The IP address, or the IPv6 prefix as the blocklist names it, such
        /// as `2001:db8:1:2::/64`.
        client: String,
        /// How long to block the client, in seconds.
        #[arg(long)]
        ttl: u64,
    },
    /// Remove the blocks of a client. Also reset what the spam and error
    /// policies counted for it, and its simulation gas balance, also when it
    /// had no block.
    Unblock {
        /// The IP address or IPv6 prefix to unblock.
        client: String,
    },
    /// Show the clients with the lowest simulation gas balances, and the
    /// blocked clients.
    Clients {
        /// How many clients of each kind to show at most. Without it, the node
        /// shows its default number of each.
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

#[derive(Args, Debug)]
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
    /// Sets how long the node blocks a client that breaches the spam or error
    /// policy, in seconds. The client is the address that connects to the
    /// node, or the one that `client-id-source` names.
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

#[derive(Clone, Copy, Debug, ValueEnum)]
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

#[derive(Debug, Subcommand)]
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

#[derive(Debug, Subcommand)]
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
type AskOperator<'a> = &'a dyn Fn(&str) -> Result<bool>;

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

    async fn run(self, output: &mut Output<'_>, confirm: AskOperator<'_>) -> Result<()> {
        let admin_api = AdminClient::new(&self.admin_url)?;
        let AdminSubcommand::TrafficControl(command) = self.command;
        command.run(&admin_api, output, confirm).await
    }
}

impl TrafficControlCommand {
    async fn run(
        self,
        admin_api: &AdminClient,
        output: &mut Output<'_>,
        confirm: AskOperator<'_>,
    ) -> Result<()> {
        match self {
            Self::Status => {
                let status: TrafficControlStatus =
                    admin_api.get(TRAFFIC_CONTROL_ROUTE, &()).await?;
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
                    && !confirm(&format!("{DEFAULT_DENY_WARNING} Continue?"))?
                {
                    writeln!(output.notes, "Nothing changed.")?;
                    return Ok(());
                }
                // The answer holds the warning for the default action deny, and
                // says how long the change lasts.
                let answer = admin_api.post(TRAFFIC_CONTROL_ROUTE, &params).await?;
                write!(output.notes, "{answer}")?;
                let status: TrafficControlStatus =
                    admin_api.get(TRAFFIC_CONTROL_ROUTE, &()).await.context(
                        "the node accepted the request, but reading the settings in force \
                         failed; a node of an older version may have ignored the settings it \
                         does not know",
                    )?;
                print_status(&status, output)?;
                writeln!(output.notes, "{SAVE_HINT}")?;
            }
            Self::Rules(RulesCommand::List) => {
                let rules: AccessRules = admin_api.get(TRAFFIC_CONTROL_RULES_ROUTE, &()).await?;
                print_rules(&rules, output)?;
            }
            Self::Rules(RulesCommand::Allow(change)) => {
                change_rule(admin_api, AccessRuleList::Allow, change, output).await?;
            }
            Self::Rules(RulesCommand::Deny(change)) => {
                change_rule(admin_api, AccessRuleList::Deny, change, output).await?;
            }
            Self::Blocklist { limit } => {
                let blocklist: BlockedClients = admin_api
                    .get(TRAFFIC_CONTROL_BLOCKLIST_ROUTE, &BlocklistQuery { limit })
                    .await?;
                print_blocklist(&blocklist, output)?;
            }
            Self::Block { client, ttl } => {
                let answer = admin_api
                    .post(
                        TRAFFIC_CONTROL_BLOCK_ROUTE,
                        &ManualBlockQuery {
                            client,
                            ttl_sec: ttl,
                        },
                    )
                    .await?;
                print_answer(&answer, output)?;
            }
            Self::Unblock { client } => {
                let answer = admin_api
                    .post(TRAFFIC_CONTROL_UNBLOCK_ROUTE, &UnblockQuery { client })
                    .await?;
                print_answer(&answer, output)?;
            }
            Self::Clients { top } => {
                let usage: ClientUsage = admin_api
                    .get(TRAFFIC_CONTROL_CLIENTS_ROUTE, &ClientUsageQuery { top })
                    .await?;
                print_client_usage(&usage, output)?;
            }
            Self::Save {
                config,
                output: output_path,
                yes,
            } => {
                let policy = parse_policy_config_without_loss(
                    admin_api.get(TRAFFIC_CONTROL_CONFIG_ROUTE, &()).await?,
                )?;
                save(
                    &config,
                    output_path.as_deref(),
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

async fn change_rule(
    admin_api: &AdminClient,
    list: AccessRuleList,
    change: RuleChange,
    output: &mut Output<'_>,
) -> Result<()> {
    let (path, client) = match change {
        RuleChange::Add { client } => (TRAFFIC_CONTROL_RULES_ADD_ROUTE, client),
        RuleChange::Remove { client } => (TRAFFIC_CONTROL_RULES_REMOVE_ROUTE, client),
    };
    let answer = admin_api
        .post(path, &AccessRuleChangeQuery { list, client })
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
/// a setting it does not know.
fn parse_policy_config_without_loss(policy: serde_json::Value) -> Result<PolicyConfig> {
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
/// old file next to it, as `<file>.<unix seconds>.bak`. With `output_path`,
/// writes the new config there instead and leaves `path` as it is. The new
/// file keeps the mode and, where it may, the owner of the old one, and a
/// symbolic link to the file stays one.
fn save(
    path: &Path,
    output_path: Option<&Path>,
    policy: &PolicyConfig,
    yes: bool,
    out: &mut dyn Write,
    confirm: AskOperator<'_>,
) -> Result<()> {
    let path = &resolve_node_config(path, output_path)?;
    let old = std::fs::read_to_string(path)
        .with_context(|| format!("cannot read the node config {}", path.display()))?;
    let Some(change) = PolicyConfigChange::new(path, &old, policy)? else {
        writeln!(out, "The node config already holds the settings in force.")?;
        return Ok(());
    };
    let target = output_path.unwrap_or(path);
    change.show(path, target, out)?;
    if !yes && !confirm(&format!("Write this change to {}?", target.display()))? {
        writeln!(out, "Nothing written.")?;
        return Ok(());
    }
    if std::fs::read_to_string(path).ok().as_deref() != Some(old.as_str()) {
        bail!(
            "{} changed since it was read; run save again",
            path.display()
        );
    }
    write_saved_config(path, output_path, &change, out)
}

/// The node config file that `path` names, with symbolic links resolved.
/// Fails when `output_path` names the same file.
fn resolve_node_config(path: &Path, output_path: Option<&Path>) -> Result<PathBuf> {
    let path = std::fs::canonicalize(path)
        .with_context(|| format!("cannot read the node config {}", path.display()))?;
    if output_path.is_some_and(|output_path| {
        std::fs::canonicalize(output_path).is_ok_and(|output_path| output_path == path)
    }) {
        bail!("--output names the node config; leave it out to write the config in place");
    }
    Ok(path)
}

/// What `save` changes in the text of a node config.
struct PolicyConfigChange {
    old_config: String,
    new_config: String,
    /// The `policy-config` of the old config as YAML, empty when it has
    /// none.
    old_policy_config: String,
    new_policy_config: String,
}

impl PolicyConfigChange {
    /// The change that puts `policy` into `old_config`, the text of the node
    /// config at `path`, or `None` when the config already means the same.
    fn new(path: &Path, old_config: &str, policy: &PolicyConfig) -> Result<Option<Self>> {
        let mut config: serde_yaml::Value = serde_yaml::from_str(old_config)
            .with_context(|| format!("{} is not YAML", path.display()))?;
        let mapping = config
            .as_mapping_mut()
            .with_context(|| format!("{} is not a node config", path.display()))?;
        let key = serde_yaml::Value::from("policy-config");
        let policy = serde_yaml::to_value(policy)?;
        let old_policy = mapping.get(&key).cloned();
        if same_policy(old_policy.as_ref(), &policy) {
            return Ok(None);
        }
        mapping.insert(key, policy.clone());
        Ok(Some(Self {
            old_config: old_config.to_string(),
            new_config: serde_yaml::to_string(&config)?,
            old_policy_config: old_policy
                .as_ref()
                .map(serde_yaml::to_string)
                .transpose()?
                .unwrap_or_default(),
            new_policy_config: serde_yaml::to_string(&policy)?,
        }))
    }

    /// Prints the change of the `policy-config`, then of the whole file at
    /// `path` as it is written to `target`, which loses its comments.
    fn show(&self, path: &Path, target: &Path, out: &mut dyn Write) -> Result<()> {
        writeln!(out, "The policy-config changes:")?;
        write!(
            out,
            "{}",
            similar::TextDiff::from_lines(&self.old_policy_config, &self.new_policy_config)
                .unified_diff()
                .header("policy-config", "policy-config")
        )?;
        writeln!(
            out,
            "The whole file, written again (its comments are not kept):"
        )?;
        write!(
            out,
            "{}",
            similar::TextDiff::from_lines(&self.old_config, &self.new_config)
                .unified_diff()
                .header(&path.display().to_string(), &target.display().to_string())
        )?;
        Ok(())
    }
}

/// Writes the new config of `change` to `output_path`, or, without it, over
/// the node config at `path` after keeping its old config as a backup, and
/// says where it went.
fn write_saved_config(
    path: &Path,
    output_path: Option<&Path>,
    change: &PolicyConfigChange,
    out: &mut dyn Write,
) -> Result<()> {
    let name = path.display();
    match output_path {
        Some(output_path) => {
            replace_file_atomically(output_path, &change.new_config, path)?;
            writeln!(
                out,
                "Wrote {}. Move it over {name} for the node to read it at its next start.",
                output_path.display()
            )?;
        }
        None => {
            let seconds = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
            let backup = backup(path, &change.old_config, seconds)?;
            replace_file_atomically(path, &change.new_config, path)?;
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
/// holds none, means the same as `policy`. Access rules that mean the same,
/// such as `203.0.113.7` and `203.0.113.7/32`, count as the same.
fn same_policy(old: Option<&serde_yaml::Value>, policy: &serde_yaml::Value) -> bool {
    let old = match old {
        Some(old) => serde_yaml::from_value::<Option<PolicyConfig>>(old.clone()),
        None => Ok(Some(PolicyConfig::default_dos_protection_policy())),
    };
    let (Ok(Some(old)), Ok(new)) = (old, serde_yaml::from_value::<PolicyConfig>(policy.clone()))
    else {
        return false;
    };
    let with_canonical_rules = |mut policy: PolicyConfig| {
        policy.access_rules = canonical_access_rules(&policy.access_rules).ok()?;
        serde_yaml::to_value(policy).ok()
    };
    with_canonical_rules(old).is_some_and(|old| with_canonical_rules(new) == Some(old))
}

/// Writes `old`, the contents of `path`, to a new file next to it,
/// `<file>.<seconds>.bak`, or `<file>.<seconds>.<n>.bak` when that name is
/// taken, with the mode and, where it may, the owner of `path`. Returns its
/// path.
fn backup(path: &Path, old: &str, seconds: u64) -> Result<PathBuf> {
    let metadata = mode_and_owner(path)?;
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
                let write_result = copy_mode_and_owner(&file, &metadata)
                    .and_then(|()| file.write_all(old.as_bytes()))
                    .and_then(|()| file.sync_all());
                if let Err(err) = write_result {
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

/// The metadata of `path`, which holds its mode and owner.
fn mode_and_owner(path: &Path) -> Result<std::fs::Metadata> {
    std::fs::metadata(path)
        .with_context(|| format!("cannot read the mode and owner of {}", path.display()))
}

/// Gives `file` the mode and, where it may, the owner of the file `metadata`
/// describes.
fn copy_mode_and_owner(file: &File, metadata: &std::fs::Metadata) -> std::io::Result<()> {
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
/// `mode_and_owner_of`. The contents go to a file of their own next to
/// `target` first, which is then moved over it, so that a failed write leaves
/// `target` as it was.
fn replace_file_atomically(target: &Path, contents: &str, mode_and_owner_of: &Path) -> Result<()> {
    let folder = match target.parent() {
        Some(folder) if !folder.as_os_str().is_empty() => folder,
        _ => Path::new("."),
    };
    let failed = || format!("cannot write {}", target.display());
    // Created readable by its owner only, so that no one else reads the
    // config before its mode is set.
    let mut file = tempfile::NamedTempFile::new_in(folder).with_context(failed)?;
    copy_mode_and_owner(file.as_file(), &mode_and_owner(mode_and_owner_of)?)
        .with_context(failed)?;
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
        /// How long to wait for the admin API to accept a connection.
        const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
        /// How long to wait for the whole answer of the admin API.
        const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
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
        let response = require_success(response).await?;
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
        require_success(response)
            .await?
            .text()
            .await
            .with_context(|| {
                format!("the node made the change, but reading its answer to {path} failed")
            })
    }
}

/// Fails with what the node answers when it refuses a request.
async fn require_success(response: reqwest::Response) -> Result<reqwest::Response> {
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

fn value_or_dash(value: Option<impl ToString>) -> String {
    value.map_or_else(|| "-".to_string(), |value| value.to_string())
}

fn print_status(status: &TrafficControlStatus, output: &mut Output<'_>) -> Result<()> {
    if output.json {
        return print_json(status, output.out);
    }
    let mut table = Table::new();
    table.set_header(["Setting", "Value"]);
    for (setting, value) in [
        ("spam threshold", value_or_dash(status.spam_threshold)),
        ("error threshold", value_or_dash(status.error_threshold)),
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
            value_or_dash(
                status
                    .simulation_gas_limit
                    .as_ref()
                    .map(|limit| limit.gas_per_sec),
            ),
        ),
        (
            "simulation burst gas",
            value_or_dash(
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

fn print_client_usage(usage: &ClientUsage, output: &mut Output<'_>) -> Result<()> {
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
        Router,
        extract::{Request, State},
        middleware::{self, Next},
        response::Response,
        routing::{get, post},
    };
    use iota_config::{Config, NodeConfig};
    use iota_core::authority::{AuthorityState, test_authority_builder::TestAuthorityBuilder};
    use iota_node::admin::traffic_control_router;
    use iota_types::traffic_control::{
        BlockReason, BlockedClient, ClientGasBalance, DEFAULT_DENY_WARNING,
        SimulationGasLimitConfig, Weight,
    };

    use super::*;

    /// The requests the admin server got, as `<method> <path>?<query>`.
    type Requests = Arc<Mutex<Vec<String>>>;

    /// How the node ends its answer to a traffic control change.
    const UNTIL_RESTART: &str = "The change lasts until the node restarts.\n";

    const FULLNODE_CONFIG_TEMPLATE: &str =
        include_str!("../../iota-config/data/fullnode-template.yaml");

    fn policy() -> PolicyConfig {
        PolicyConfig {
            dry_run: false,
            access_rules: AccessRules {
                allow: vec!["198.51.100.0/24".to_string()],
                ..Default::default()
            },
            simulation_gas_limit: Some(SimulationGasLimitConfig {
                gas_per_sec: 1_000_000,
                burst_gas: 20_000_000,
            }),
            ..PolicyConfig::default_dos_protection_policy()
        }
    }

    /// The traffic control routes of a node, served on a free port of the
    /// loopback address.
    struct AdminServer {
        url: String,
        state: Arc<AuthorityState>,
        requests: Requests,
    }

    impl AdminServer {
        fn requests(&self) -> Vec<String> {
            self.requests.lock().unwrap().clone()
        }

        fn status(&self) -> TrafficControlStatus {
            self.state.traffic_control_status().unwrap()
        }
    }

    async fn record_request(
        State(requests): State<Requests>,
        request: Request,
        next: Next,
    ) -> Response {
        requests.lock().unwrap().push(format!(
            "{} {}?{}",
            request.method(),
            request.uri().path(),
            request.uri().query().unwrap_or_default()
        ));
        next.run(request).await
    }

    /// Starts the real traffic control routes of a node that runs
    /// `policy_config`, with one more route, `/text`, that answers text that
    /// is not JSON.
    async fn start_admin_server(policy_config: PolicyConfig) -> AdminServer {
        let state = TestAuthorityBuilder::new()
            .with_policy_config(policy_config)
            .build()
            .await;
        let requests = Requests::default();
        let router = traffic_control_router(state.clone())
            .route("/text", get(|| async { "not JSON" }))
            .layer(middleware::from_fn_with_state(
                requests.clone(),
                record_request,
            ));
        AdminServer {
            url: serve_on_a_free_port(router).await,
            state,
            requests,
        }
    }

    /// Serves `router` on a free port of the loopback address, and returns
    /// its URL.
    async fn serve_on_a_free_port(router: Router) -> String {
        let listener = tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
            .await
            .unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, router).await });
        url
    }

    /// What a command printed: its result, and its notes.
    struct Printed {
        out: String,
        notes: String,
    }

    /// Runs `iota-tool admin` with `args` against `url`, and fails the test
    /// if the command asks to confirm.
    async fn run(url: &str, args: &[&str]) -> Result<Printed> {
        run_asking(url, args, &|question: &str| {
            panic!("the command asked {question:?}")
        })
        .await
    }

    /// Runs `iota-tool admin` with `args` against `url`, and confirms what
    /// the command asks.
    async fn run_confirming(url: &str, args: &[&str]) -> Result<Printed> {
        run_asking(url, args, &|_: &str| Ok(true)).await
    }

    /// Runs `iota-tool admin` with `args` against `url`, and declines what
    /// the command asks.
    async fn run_declining(url: &str, args: &[&str]) -> Result<Printed> {
        run_asking(url, args, &|_: &str| Ok(false)).await
    }

    async fn run_asking(url: &str, args: &[&str], confirm: AskOperator<'_>) -> Result<Printed> {
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
        command.run(&mut output, confirm).await?;
        Ok(Printed {
            out: String::from_utf8(out)?,
            notes: String::from_utf8(notes)?,
        })
    }

    #[tokio::test]
    async fn set_sends_only_the_given_settings_and_prints_the_result() {
        let node = start_admin_server(policy()).await;
        let printed = run(
            &node.url,
            &[
                "traffic-control",
                "set",
                "--spam-threshold",
                "500",
                "--dry-run",
                "true",
                "--simulation-gas-per-sec",
                "2000000",
            ],
        )
        .await
        .unwrap();
        assert_eq!(
            node.requests(),
            [
                "POST /traffic-control?spam_threshold=500&dry_run=true&simulation_gas_per_sec=2000000",
                "GET /traffic-control?",
            ]
        );
        let status = node.status();
        assert_eq!(status.spam_threshold, Some(500));
        assert!(status.dry_run);
        assert_eq!(
            status.simulation_gas_limit.map(|limit| limit.gas_per_sec),
            Some(2_000_000)
        );
        assert!(printed.out.contains("spam threshold"), "{}", printed.out);
        assert!(printed.out.contains("2000000"), "{}", printed.out);
        assert!(
            printed
                .notes
                .starts_with("Traffic control configured with:\n"),
            "{}",
            printed.notes
        );
        assert!(
            printed
                .notes
                .ends_with(&format!("{UNTIL_RESTART}{SAVE_HINT}\n")),
            "{}",
            printed.notes
        );
    }

    #[tokio::test]
    async fn set_passes_each_setting_to_the_node() {
        let node = start_admin_server(policy()).await;
        run(
            &node.url,
            &[
                "traffic-control",
                "set",
                "--spam-threshold",
                "501",
                "--error-threshold",
                "51",
                "--dry-run",
                "true",
                "--connection-blocklist-ttl-sec",
                "61",
                "--proxy-blocklist-ttl-sec",
                "62",
                "--default-action",
                "deny",
                "--simulation-gas-per-sec",
                "1000001",
                "--simulation-burst-gas",
                "20000001",
                "--yes",
            ],
        )
        .await
        .unwrap();
        assert_eq!(
            node.status(),
            TrafficControlStatus {
                error_threshold: Some(51),
                spam_threshold: Some(501),
                dry_run: true,
                connection_blocklist_ttl_sec: 61,
                proxy_blocklist_ttl_sec: 62,
                default_action: DefaultAction::Deny,
                allow_rules: 1,
                deny_rules: 0,
                blocked_clients: 0,
                simulation_gas_limit: Some(SimulationGasLimitConfig {
                    gas_per_sec: 1_000_001,
                    burst_gas: 20_000_001,
                }),
            }
        );
    }

    #[tokio::test]
    async fn set_against_an_older_node_says_that_it_may_have_ignored_settings() {
        // A node of a version without `GET /traffic-control` answers the change
        // with success, and ignores the settings it does not know.
        let url = serve_on_a_free_port(Router::new().route(
            TRAFFIC_CONTROL_ROUTE,
            post(|| async { "Traffic control configured with:\n" }),
        ))
        .await;
        let error = run(
            &url,
            &["traffic-control", "set", "--default-action", "limit"],
        )
        .await
        .err()
        .unwrap();
        assert!(
            error
                .to_string()
                .contains("may have ignored the settings it does not know"),
            "{error:#}"
        );
    }

    #[tokio::test]
    async fn set_takes_at_least_one_setting() {
        let node = start_admin_server(policy()).await;
        assert!(run(&node.url, &["traffic-control", "set"]).await.is_err());
        assert!(
            run(&node.url, &["traffic-control", "set", "--yes"])
                .await
                .is_err()
        );
        assert!(node.requests().is_empty());
    }

    #[tokio::test]
    async fn json_prints_only_json() {
        let node = start_admin_server(policy()).await;
        let printed = run(
            &node.url,
            &[
                "--json",
                "traffic-control",
                "set",
                "--spam-threshold",
                "500",
            ],
        )
        .await
        .unwrap();
        assert_eq!(
            serde_json::from_str::<TrafficControlStatus>(&printed.out).unwrap(),
            node.status()
        );
        assert!(
            printed
                .notes
                .starts_with("Traffic control configured with:\n"),
            "{}",
            printed.notes
        );

        let printed = run(
            &node.url,
            &["--json", "traffic-control", "unblock", "203.0.113.9"],
        )
        .await
        .unwrap();
        let printed: serde_json::Value = serde_json::from_str(&printed.out).unwrap();
        let printed = printed.as_object().unwrap();
        assert_eq!(printed.len(), 1, "{printed:?}");
        assert!(
            printed["answer"]
                .as_str()
                .unwrap()
                .ends_with(UNTIL_RESTART.trim_end()),
            "{printed:?}"
        );
    }

    #[tokio::test]
    async fn the_default_action_deny_shows_the_warning_and_takes_a_confirmation() {
        let node = start_admin_server(policy()).await;
        let args = ["traffic-control", "set", "--default-action", "deny"];
        let questions = Mutex::new(Vec::new());
        let printed = run_asking(&node.url, &args, &|question: &str| {
            questions.lock().unwrap().push(question.to_string());
            Ok(false)
        })
        .await
        .unwrap();
        assert_eq!(printed.notes, "Nothing changed.\n");
        assert!(node.requests().is_empty());
        assert_eq!(node.status().default_action, DefaultAction::Limit);
        let [question] = questions.into_inner().unwrap().try_into().unwrap();
        assert_eq!(question, format!("{DEFAULT_DENY_WARNING} Continue?"));

        let warning = format!("Warning: {DEFAULT_DENY_WARNING}\n");
        let printed = run_confirming(&node.url, &args).await.unwrap();
        assert!(printed.notes.contains(&warning), "{}", printed.notes);
        assert_eq!(node.status().default_action, DefaultAction::Deny);
        let mut with_yes = args.to_vec();
        with_yes.push("--yes");
        let printed = run(&node.url, &with_yes).await.unwrap();
        assert!(printed.notes.contains(&warning), "{}", printed.notes);
        let posts = node
            .requests()
            .into_iter()
            .filter(|request| request.starts_with("POST"))
            .collect::<Vec<_>>();
        assert_eq!(posts, ["POST /traffic-control?default_action=deny"; 2]);
    }

    #[tokio::test]
    async fn rule_and_unblock_commands_send_their_queries() {
        let node = start_admin_server(policy()).await;
        let printed = run(
            &node.url,
            &["traffic-control", "rules", "allow", "add", "203.0.113.0/24"],
        )
        .await
        .unwrap();
        assert_eq!(
            printed.out,
            format!("Added 203.0.113.0/24 to the allow rules.\n{UNTIL_RESTART}")
        );
        assert_eq!(printed.notes, format!("{SAVE_HINT}\n"));
        let printed = run(
            &node.url,
            &["traffic-control", "rules", "deny", "remove", "192.0.2.0/24"],
        )
        .await
        .unwrap();
        assert_eq!(
            printed.out,
            format!("The deny rules do not hold 192.0.2.0/24.\n{UNTIL_RESTART}")
        );
        assert_eq!(printed.notes, format!("{SAVE_HINT}\n"));
        assert_eq!(
            node.state.traffic_control_access_rules().unwrap().allow,
            ["198.51.100.0/24", "203.0.113.0/24"]
        );
        // A block is not part of the config, so there is nothing to save.
        let printed = run(&node.url, &["traffic-control", "unblock", "203.0.113.9"])
            .await
            .unwrap();
        assert!(printed.out.ends_with(UNTIL_RESTART), "{}", printed.out);
        assert_eq!(printed.notes, "");
        assert_eq!(
            node.requests(),
            [
                "POST /traffic-control/rules/add?list=allow&client=203.0.113.0%2F24",
                "POST /traffic-control/rules/remove?list=deny&client=192.0.2.0%2F24",
                "POST /traffic-control/unblock?client=203.0.113.9",
            ]
        );
    }

    #[tokio::test]
    async fn a_refusal_of_the_node_is_an_error_with_its_answer() {
        let node = start_admin_server(policy()).await;
        let error = run(
            &node.url,
            &[
                "traffic-control",
                "block",
                "203.0.113.9",
                "--ttl",
                "9999999",
            ],
        )
        .await
        .err()
        .unwrap();
        assert!(
            error.to_string().starts_with(
                "the node answered 400 Bad Request: Invalid admin request: The TTL of a manual \
                 block must be between 1 and "
            ),
            "{error}"
        );
        let missing = AdminClient::new(&node.url)
            .unwrap()
            .get::<serde_json::Value>("/no-such-route", &())
            .await
            .unwrap_err();
        assert!(
            missing.to_string().contains("without this admin route"),
            "{missing}"
        );
        let text = AdminClient::new(&node.url)
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
    async fn a_change_whose_answer_breaks_off_says_that_the_node_made_it() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
            .await
            .unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        // Answers with success, then closes the connection before the end of
        // the announced body.
        tokio::spawn(async move {
            let (mut connection, _) = listener.accept().await.unwrap();
            let mut request = [0; 4096];
            let _ = connection.read(&mut request).await.unwrap();
            connection
                .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 100\r\n\r\nUnblocked")
                .await
                .unwrap();
        });
        let error = run(&url, &["traffic-control", "unblock", "203.0.113.9"])
            .await
            .err()
            .unwrap();
        assert!(
            error
                .to_string()
                .starts_with("the node made the change, but reading its answer"),
            "{error:#}"
        );
    }

    #[test]
    fn the_admin_url_is_by_default_the_default_admin_address_of_a_node() {
        let command = AdminCommand::try_parse_from(["admin", "traffic-control", "status"]).unwrap();
        assert_eq!(
            command.admin_url,
            default_admin_interface_address().to_string()
        );
    }

    #[tokio::test]
    async fn the_admin_url_may_be_an_address() {
        let node = start_admin_server(policy()).await;
        let address = node.url.trim_start_matches("http://");
        run(address, &["traffic-control", "status"]).await.unwrap();
        assert_eq!(node.requests(), ["GET /traffic-control?"]);
        let error = AdminClient::new("http://[not an address").err().unwrap();
        assert!(error.to_string().contains("is not an address"), "{error}");
    }

    #[tokio::test]
    async fn block_and_listings_send_their_queries() {
        let node = start_admin_server(policy()).await;
        let printed = run(
            &node.url,
            &["traffic-control", "block", "203.0.113.9", "--ttl", "3600"],
        )
        .await
        .unwrap();
        assert!(
            printed.out.starts_with("Blocked 203.0.113.9 for "),
            "{}",
            printed.out
        );
        assert!(printed.out.ends_with(UNTIL_RESTART), "{}", printed.out);
        run(
            &node.url,
            &["traffic-control", "blocklist", "--limit", "100"],
        )
        .await
        .unwrap();
        let out = run(&node.url, &["traffic-control", "clients", "--top", "5"])
            .await
            .unwrap()
            .out;
        assert!(out.contains("203.0.113.9"), "{out}");
        run(&node.url, &["traffic-control", "clients"])
            .await
            .unwrap();
        assert_eq!(
            node.requests(),
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
        let node = start_admin_server(policy()).await;
        for client in ["203.0.113.9", "203.0.113.10", "203.0.113.11"] {
            run(
                &node.url,
                &["traffic-control", "block", client, "--ttl", "3600"],
            )
            .await
            .unwrap();
        }
        let out = run(&node.url, &["traffic-control", "blocklist", "--limit", "1"])
            .await
            .unwrap()
            .out;
        assert!(out.contains("manual"), "{out}");
        assert!(
            out.ends_with("2 more blocked clients not shown.\n"),
            "{out}"
        );

        let out = run(&node.url, &["traffic-control", "status"])
            .await
            .unwrap()
            .out;
        assert!(out.contains("default action"), "{out}");
        assert!(out.contains("20000000"), "{out}");
        let out = run(&node.url, &["--json", "traffic-control", "status"])
            .await
            .unwrap()
            .out;
        assert_eq!(
            serde_json::from_str::<TrafficControlStatus>(&out).unwrap(),
            node.status()
        );

        let out = run(&node.url, &["traffic-control", "rules", "list"])
            .await
            .unwrap()
            .out;
        assert!(out.contains("198.51.100.0/24"), "{out}");
        assert!(out.ends_with("Default action: limit\n"), "{out}");
    }

    /// The value in the row of `setting` of the status table `table`.
    fn status_row(table: &str, setting: &str) -> String {
        table
            .lines()
            .find_map(|line| {
                let cells = line
                    .split(['|', '│'])
                    .map(str::trim)
                    .filter(|cell| !cell.is_empty())
                    .collect::<Vec<_>>();
                (cells.first() == Some(&setting)).then(|| cells[1].to_string())
            })
            .unwrap_or_else(|| panic!("no row {setting:?} in\n{table}"))
    }

    #[test]
    fn the_status_table_prints_each_setting_in_its_own_row() {
        let status = TrafficControlStatus {
            error_threshold: Some(50),
            spam_threshold: Some(1000),
            dry_run: true,
            connection_blocklist_ttl_sec: 60,
            proxy_blocklist_ttl_sec: 70,
            default_action: DefaultAction::Deny,
            allow_rules: 2,
            deny_rules: 3,
            blocked_clients: 4,
            simulation_gas_limit: Some(SimulationGasLimitConfig {
                gas_per_sec: 1_000_000,
                burst_gas: 20_000_000,
            }),
        };
        let without_limits = TrafficControlStatus {
            error_threshold: None,
            spam_threshold: None,
            simulation_gas_limit: None,
            ..status.clone()
        };
        for (status, rows) in [
            (
                status,
                [
                    ("spam threshold", "1000"),
                    ("error threshold", "50"),
                    ("simulation gas per sec", "1000000"),
                    ("simulation burst gas", "20000000"),
                ],
            ),
            (
                without_limits,
                [
                    ("spam threshold", "-"),
                    ("error threshold", "-"),
                    ("simulation gas per sec", "-"),
                    ("simulation burst gas", "-"),
                ],
            ),
        ] {
            let (mut out, mut notes) = (Vec::new(), Vec::new());
            print_status(
                &status,
                &mut Output {
                    out: &mut out,
                    notes: &mut notes,
                    json: false,
                },
            )
            .unwrap();
            let table = String::from_utf8(out).unwrap();
            for (setting, value) in rows.into_iter().chain([
                ("dry run", "true"),
                ("connection blocklist TTL (s)", "60"),
                ("proxy blocklist TTL (s)", "70"),
                ("default action", "deny"),
                ("allow rules", "2"),
                ("deny rules", "3"),
                ("blocked clients", "4"),
            ]) {
                assert_eq!(status_row(&table, setting), value, "{table}");
            }
        }
    }

    #[test]
    fn the_client_usage_names_the_shared_balance_and_counts_the_entries_left_out() {
        let usage = ClientUsage {
            gas_balances: vec![ClientGasBalance {
                client: None,
                gas: 42,
            }],
            omitted_gas_balances: 1,
            blocked: BlockedClients {
                clients: vec![BlockedClient {
                    client: "203.0.113.9".to_string(),
                    proxied: false,
                    reason: BlockReason::Manual,
                    seconds_left: 3600,
                }],
                omitted: 2,
            },
        };
        let (mut out, mut notes) = (Vec::new(), Vec::new());
        print_client_usage(
            &usage,
            &mut Output {
                out: &mut out,
                notes: &mut notes,
                json: false,
            },
        )
        .unwrap();
        let out = String::from_utf8(out).unwrap();
        assert!(out.contains("(no resolved IP)"), "{out}");
        assert!(out.contains("1 more gas balances not shown."), "{out}");
        assert!(out.contains("203.0.113.9"), "{out}");
        assert!(
            out.ends_with("2 more blocked clients not shown.\n"),
            "{out}"
        );
        assert!(notes.is_empty());
    }

    /// The `yes` of `save`: it writes without asking, as with `--yes`.
    const SKIP_THE_QUESTION: bool = true;
    /// The `yes` of `save`: it asks before it writes.
    const ASK_THE_QUESTION: bool = false;

    fn find_backups(dir: &Path) -> Vec<PathBuf> {
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
        let node = start_admin_server(policy()).await;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fullnode.yaml");
        std::fs::write(&path, FULLNODE_CONFIG_TEMPLATE).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            // Not the mode a backup is created with, to show that it is copied.
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o640)).unwrap();
        }
        let path_arg = path.to_str().unwrap();

        // Not confirmed: the file stays as it was.
        let out = run_declining(
            &node.url,
            &["traffic-control", "save", "--config", path_arg],
        )
        .await
        .unwrap()
        .out;
        assert!(out.starts_with("The policy-config changes:\n"), "{out}");
        assert!(out.contains("+policy-config:"), "{out}");
        assert!(out.ends_with("Nothing written.\n"), "{out}");
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            FULLNODE_CONFIG_TEMPLATE
        );

        let out = run(
            &node.url,
            &["traffic-control", "save", "--config", path_arg, "--yes"],
        )
        .await
        .unwrap()
        .out;
        assert!(out.contains("kept the old file"), "{out}");
        let [backup] = find_backups(dir.path()).try_into().unwrap();
        assert_eq!(
            std::fs::read_to_string(&backup).unwrap(),
            FULLNODE_CONFIG_TEMPLATE
        );
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
            &node.url,
            &["traffic-control", "save", "--config", path_arg],
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
        let node = start_admin_server(policy()).await;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fullnode.yaml");
        let old_policy = PolicyConfig {
            dry_run: true,
            ..policy()
        };
        let mut old: serde_yaml::Mapping = serde_yaml::from_str(FULLNODE_CONFIG_TEMPLATE).unwrap();
        old.insert(
            "policy-config".into(),
            serde_yaml::to_value(&old_policy).unwrap(),
        );
        std::fs::write(&path, serde_yaml::to_string(&old).unwrap()).unwrap();

        let out = run(
            &node.url,
            &[
                "traffic-control",
                "save",
                "--config",
                path.to_str().unwrap(),
                "--yes",
            ],
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
    async fn save_finds_no_change_in_rules_that_the_node_holds_in_canonical_form() {
        let plain_rules = PolicyConfig {
            access_rules: AccessRules {
                allow: vec![
                    "203.0.113.7".to_string(),
                    "::ffff:198.51.100.0/120".to_string(),
                ],
                deny: vec!["2001:db8::1".to_string()],
                ..Default::default()
            },
            ..policy()
        };
        let node = start_admin_server(plain_rules.clone()).await;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fullnode.yaml");
        let mut config: serde_yaml::Mapping =
            serde_yaml::from_str(FULLNODE_CONFIG_TEMPLATE).unwrap();
        config.insert(
            "policy-config".into(),
            serde_yaml::to_value(&plain_rules).unwrap(),
        );
        let config_text = serde_yaml::to_string(&config).unwrap();
        std::fs::write(&path, &config_text).unwrap();

        let out = run_declining(
            &node.url,
            &[
                "traffic-control",
                "save",
                "--config",
                path.to_str().unwrap(),
            ],
        )
        .await
        .unwrap()
        .out;
        assert_eq!(
            out,
            "The node config already holds the settings in force.\n"
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), config_text);
    }

    #[tokio::test]
    async fn save_with_an_output_leaves_the_config_as_it_is() {
        let node = start_admin_server(policy()).await;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fullnode.yaml");
        let output_path = dir.path().join("new.yaml");
        std::fs::write(&path, FULLNODE_CONFIG_TEMPLATE).unwrap();

        let out = run(
            &node.url,
            &[
                "traffic-control",
                "save",
                "--config",
                path.to_str().unwrap(),
                "--output",
                output_path.to_str().unwrap(),
                "--yes",
            ],
        )
        .await
        .unwrap()
        .out;
        assert!(
            out.ends_with(&format!(
                "Wrote {}. Move it over {} for the node to read it at its next start.\n",
                output_path.display(),
                std::fs::canonicalize(&path).unwrap().display()
            )),
            "{out}"
        );
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            FULLNODE_CONFIG_TEMPLATE
        );
        assert_eq!(find_backups(dir.path()), Vec::<PathBuf>::new());
        assert_eq!(
            load(&output_path).policy_config.unwrap().access_rules,
            policy().access_rules
        );

        let error = run(
            &node.url,
            &[
                "traffic-control",
                "save",
                "--config",
                path.to_str().unwrap(),
                "--output",
                path.to_str().unwrap(),
                "--yes",
            ],
        )
        .await
        .err()
        .unwrap();
        assert!(
            error.to_string().contains("names the node config"),
            "{error}"
        );
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            FULLNODE_CONFIG_TEMPLATE
        );
    }

    #[test]
    fn a_save_whose_write_fails_changes_no_file_and_leaves_no_temporary_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fullnode.yaml");
        std::fs::write(&path, FULLNODE_CONFIG_TEMPLATE).unwrap();
        // A file cannot replace a directory, so the write fails after the new
        // config is written to its temporary file.
        let output_path = dir.path().join("new.yaml");
        std::fs::create_dir(&output_path).unwrap();
        std::fs::write(output_path.join("kept"), "kept").unwrap();

        let error = save(
            &path,
            Some(&output_path),
            &policy(),
            SKIP_THE_QUESTION,
            &mut Vec::new(),
            &|_: &str| Ok(true),
        )
        .unwrap_err();
        assert!(error.to_string().contains("cannot write"), "{error}");
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            FULLNODE_CONFIG_TEMPLATE
        );
        assert_eq!(
            std::fs::read_to_string(output_path.join("kept")).unwrap(),
            "kept"
        );
        let mut entries = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>();
        entries.sort();
        assert_eq!(entries, ["fullnode.yaml", "new.yaml"]);
    }

    #[test]
    fn save_leaves_a_config_that_takes_the_settings_in_force_by_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fullnode.yaml");
        std::fs::write(&path, FULLNODE_CONFIG_TEMPLATE).unwrap();
        let mut out = Vec::new();
        save(
            &path,
            None,
            &PolicyConfig::default_dos_protection_policy(),
            SKIP_THE_QUESTION,
            &mut out,
            &|_: &str| Ok(true),
        )
        .unwrap();
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "The node config already holds the settings in force.\n"
        );
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            FULLNODE_CONFIG_TEMPLATE
        );
    }

    /// Output that a test can read while the code under test still holds it.
    #[derive(Clone, Default)]
    struct SharedOutput(Arc<Mutex<Vec<u8>>>);

    impl SharedOutput {
        fn text(&self) -> String {
            String::from_utf8(self.0.lock().unwrap().clone()).unwrap()
        }
    }

    impl Write for SharedOutput {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn save_shows_the_lost_comments_before_it_asks() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fullnode.yaml");
        std::fs::write(&path, FULLNODE_CONFIG_TEMPLATE).unwrap();
        let shown = SharedOutput::default();
        let shown_when_asked = Mutex::new(None);
        save(
            &path,
            None,
            &policy(),
            ASK_THE_QUESTION,
            &mut shown.clone(),
            &|_: &str| {
                *shown_when_asked.lock().unwrap() = Some(shown.text());
                Ok(false)
            },
        )
        .unwrap();
        let shown_when_asked = shown_when_asked.into_inner().unwrap().expect("save asks");
        assert!(
            shown_when_asked.contains("(its comments are not kept)"),
            "{shown_when_asked}"
        );
        assert!(
            shown_when_asked.contains(
                "\n-# Update this value to the location you want IOTA to store its database\n"
            ),
            "{shown_when_asked}"
        );
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            FULLNODE_CONFIG_TEMPLATE
        );
    }

    #[test]
    fn save_refuses_a_config_that_changed_while_it_asked() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fullnode.yaml");
        std::fs::write(&path, FULLNODE_CONFIG_TEMPLATE).unwrap();
        let edited = format!("{FULLNODE_CONFIG_TEMPLATE}\n# edited\n");
        let error = save(
            &path,
            None,
            &policy(),
            ASK_THE_QUESTION,
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
        assert_eq!(find_backups(dir.path()), Vec::<PathBuf>::new());
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
            parse_policy_config_without_loss(answered.clone())
                .unwrap()
                .access_rules,
            policy().access_rules
        );
        answered["new-setting"] = serde_json::json!(true);
        let error = parse_policy_config_without_loss(answered).unwrap_err();
        assert!(
            error.to_string().contains("read the policy-config"),
            "{error}"
        );
    }

    #[test]
    fn a_config_that_disappears_before_its_mode_is_read_is_named_in_the_error() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("fullnode.yaml");
        let missing_name = missing.display().to_string();
        let error = backup(&missing, "old", 7).unwrap_err();
        assert!(error.to_string().contains(&missing_name), "{error:#}");
        let target = dir.path().join("new.yaml");
        let error = replace_file_atomically(&target, "new", &missing).unwrap_err();
        assert!(error.to_string().contains(&missing_name), "{error:#}");
        assert!(!target.exists());
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
        let node = start_admin_server(policy()).await;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fullnode.yaml");
        let link = dir.path().join("config.yaml");
        std::fs::write(&path, FULLNODE_CONFIG_TEMPLATE).unwrap();
        std::os::unix::fs::symlink(&path, &link).unwrap();

        run(
            &node.url,
            &[
                "traffic-control",
                "save",
                "--config",
                link.to_str().unwrap(),
                "--yes",
            ],
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
