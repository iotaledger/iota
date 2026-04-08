// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    io,
    io::{Write, stdout},
    path::{Path, PathBuf},
};

use anyhow::{Context, bail};
use clap::*;
use colored::Colorize;
use iota_config::{Config, IOTA_CLIENT_CONFIG, IOTA_KEYSTORE_FILENAME, iota_config_dir};
use iota_keys::keystore::{AccountKeystore, FileBasedKeystore, Keystore};
use iota_move::{
    self, Command as MoveCommand, execute_move_command, manage_package::resolve_lock_file_path,
};
use iota_move_build::{
    BuildConfig as IotaBuildConfig, IotaPackageHooks, check_conflicting_addresses,
    check_invalid_dependencies, check_unpublished_dependencies, implicit_deps,
};
use iota_package_management::system_package_versions::latest_system_packages;
use iota_sdk::{
    iota_client_config::{IotaClientConfig, IotaEnv},
    wallet_context::WalletContext,
};
use iota_types::crypto::SignatureScheme;
use move_analyzer::analyzer;
use move_core_types::account_address::AccountAddress;
use move_package::BuildConfig;
use serde_json::json;
use url::Url;

#[cfg(feature = "iota-names")]
use crate::name_commands;
use crate::{
    PrintableResult,
    client_commands::{IotaClientCommands, implicit_deps_for_protocol_version, pkg_tree_shake},
    keytool::KeyToolCommand,
    validator_commands::IotaValidatorCommand,
};

#[derive(Parser, Debug)]
#[clap(rename_all = "kebab-case")]
pub struct IotaEnvConfig {
    /// Sets the file storing the state of our user accounts (an empty one will
    /// be created if missing)
    #[clap(long = "client.config")]
    pub config: Option<PathBuf>,
    /// The IOTA environment to use. This must be present in the current config
    /// file.
    #[clap(long = "client.env")]
    pub env: Option<String>,
}

#[derive(Parser)]
pub enum IotaCommand {
    /// Deprecated: use `iota-localnet start` instead.
    // Remove with v1.21.0: <https://github.com/iotaledger/iota/issues/10732>
    #[command(hide = true)]
    Start {},
    /// Deprecated: use `iota-localnet genesis` instead.
    // Remove with v1.21.0: <https://github.com/iotaledger/iota/issues/10732>
    #[command(hide = true)]
    Genesis {},
    /// Deprecated: use `iota-tool genesis-ceremony` instead.
    // Remove with v1.21.0: <https://github.com/iotaledger/iota/issues/10732>
    #[command(hide = true)]
    GenesisCeremony {},
    /// IOTA keystore tool.
    #[command(name = "keytool")]
    KeyTool {
        #[arg(long)]
        keystore_path: Option<PathBuf>,
        /// Return command outputs in json format
        #[arg(long, global = true)]
        json: bool,
        /// Subcommands.
        #[command(subcommand)]
        cmd: KeyToolCommand,
    },
    /// Client for interacting with the IOTA network.
    Client {
        #[clap(flatten)]
        config: IotaEnvConfig,
        #[command(subcommand)]
        cmd: Option<IotaClientCommands>,
        /// Return command outputs in json format.
        #[arg(long, global = true)]
        json: bool,
        #[arg(short = 'y', long = "yes")]
        accept_defaults: bool,
    },
    /// A tool for validators and validator candidates.
    Validator {
        /// Sets the file storing the state of our user accounts (an empty one
        /// will be created if missing)
        #[arg(long = "client.config")]
        config: Option<PathBuf>,
        #[command(subcommand)]
        cmd: Option<IotaValidatorCommand>,
        /// Return command outputs in json format.
        #[arg(long, global = true)]
        json: bool,
        #[arg(short = 'y', long = "yes")]
        accept_defaults: bool,
    },
    /// Tool to build and test Move applications.
    Move {
        /// Path to a package which the command should be run with respect to.
        #[arg(long = "path", short = 'p', global = true)]
        package_path: Option<PathBuf>,
        #[clap(flatten)]
        config: IotaEnvConfig,
        /// Package build options
        #[command(flatten)]
        build_config: BuildConfig,
        /// Subcommands.
        #[command(subcommand)]
        cmd: MoveCommand,
    },
    #[cfg(feature = "iota-names")]
    /// Manage names registered in IOTA-Names.
    /// By using this service, you agree to the Terms & Conditions:
    /// iotanames.com/terms-of-service."
    Name {
        #[clap(flatten)]
        config: IotaEnvConfig,
        /// Return command outputs in json format.
        #[arg(long, global = true)]
        json: bool,
        #[command(subcommand)]
        cmd: name_commands::NameCommand,
    },
    /// Deprecated: use `iota-tool fire-drill` instead.
    // Remove with v1.21.0: <https://github.com/iotaledger/iota/issues/10732>
    #[command(hide = true)]
    FireDrill {},
    /// Invoke IOTA's move-analyzer via CLI
    #[command(hide = true)]
    Analyzer,
    /// Generate completion files for various shells
    #[cfg(feature = "gen-completions")]
    GenerateCompletions(crate::completions::GenerateCompletionsCommand),
}

impl IotaCommand {
    pub async fn execute(self) -> Result<(), anyhow::Error> {
        move_package::package_hooks::register_package_hooks(Box::new(IotaPackageHooks));
        match self {
            IotaCommand::Start {} => {
                eprintln!(
                    "{}",
                    "The `start` command has been moved to the `iota-localnet` binary.\n\
                     Please use `iota-localnet start` instead."
                        .yellow()
                        .bold()
                );
                std::process::exit(1);
            }
            IotaCommand::Genesis {} => {
                eprintln!(
                    "{}",
                    "The `genesis` command has been moved to the `iota-localnet` binary.\n\
                     Please use `iota-localnet genesis` instead."
                        .yellow()
                        .bold()
                );
                std::process::exit(1);
            }
            IotaCommand::GenesisCeremony {} => {
                eprintln!(
                    "{}",
                    "The `genesis-ceremony` command has been moved to `iota-tool`.\n\
                     Please use `iota-tool genesis-ceremony` instead."
                        .yellow()
                        .bold()
                );
                std::process::exit(1);
            }
            IotaCommand::KeyTool {
                keystore_path,
                json,
                cmd,
            } => {
                let keystore_path =
                    keystore_path.unwrap_or(iota_config_dir()?.join(IOTA_KEYSTORE_FILENAME));
                let mut keystore = Keystore::from(FileBasedKeystore::new(&keystore_path)?);
                cmd.execute(&mut keystore).await?.print(!json);
                Ok(())
            }
            IotaCommand::Client {
                config,
                cmd,
                json,
                accept_defaults,
            } => {
                let config_path = config
                    .config
                    .unwrap_or(iota_config_dir()?.join(IOTA_CLIENT_CONFIG));
                prompt_if_no_config(
                    &config_path,
                    accept_defaults,
                    !matches!(cmd, Some(IotaClientCommands::NewEnv { .. })),
                    !matches!(cmd, Some(IotaClientCommands::NewAddress { .. })),
                )?;
                if let Some(cmd) = cmd {
                    let mut context = WalletContext::new(&config_path)?;
                    if let Some(env_override) = config.env {
                        context = context.with_env_override(env_override);
                    }
                    cmd.execute(&mut context).await?.print(!json);
                } else {
                    // Print help
                    let mut app: Command = IotaCommand::command();
                    app.build();
                    app.find_subcommand_mut("client").unwrap().print_help()?;
                }
                Ok(())
            }
            IotaCommand::Validator {
                config,
                cmd,
                json,
                accept_defaults,
            } => {
                let config_path = config.unwrap_or(iota_config_dir()?.join(IOTA_CLIENT_CONFIG));
                prompt_if_no_config(&config_path, accept_defaults, true, true)?;
                let mut context = WalletContext::new(&config_path)?;
                if let Some(cmd) = cmd {
                    cmd.execute(&mut context, json).await?.print(!json);
                } else {
                    // Print help
                    let mut app: Command = IotaCommand::command();
                    app.build();
                    app.find_subcommand_mut("validator").unwrap().print_help()?;
                }
                Ok(())
            }
            IotaCommand::Move {
                package_path,
                build_config,
                mut cmd,
                config: client_config,
            } => {
                match cmd {
                    iota_move::Command::Build(build) if build.dump_bytecode_as_base64 => {
                        // `iota move build` does not ordinarily require a network connection.
                        // The exception is when --dump-bytecode-as-base64 is specified: In this
                        // case, we should resolve the correct addresses for the respective chain
                        // (e.g., testnet, mainnet) from the Move.lock under automated address
                        // management. In addition, tree shaking also
                        // requires a network as it needs to fetch
                        // on-chain linkage table of package dependencies.
                        let config = client_config
                            .config
                            .unwrap_or(iota_config_dir()?.join(IOTA_CLIENT_CONFIG));
                        prompt_if_no_config(&config, false, true, true)?;
                        let mut context = WalletContext::new(&config)?;

                        if let Some(env_override) = client_config.env {
                            context = context.with_env_override(env_override);
                        }

                        let Ok(client) = context.get_client().await else {
                            bail!(
                                "`iota move build --dump-bytecode-as-base64` requires a connection to the network. Current active network is {} but failed to connect to it.",
                                context.active_env().as_ref().unwrap()
                            );
                        };
                        let read_api = client.read_api();

                        if let Err(e) = client.check_api_version() {
                            eprintln!("{}", format!("[warning] {e}").yellow().bold());
                        }

                        let chain_id = if build.ignore_chain {
                            // for tests it's useful to ignore the chain id!
                            None
                        } else {
                            read_api.get_chain_identifier().await.ok()
                        };

                        let rerooted_path = move_cli::base::reroot_path(package_path.as_deref())?;
                        let mut build_config =
                            resolve_lock_file_path(build_config, Some(&rerooted_path))?;

                        let previous_id = if let Some(ref chain_id) = chain_id {
                            iota_package_management::set_package_id(
                                &rerooted_path,
                                build_config.install_dir.clone(),
                                chain_id,
                                AccountAddress::ZERO,
                            )?
                        } else {
                            None
                        };

                        let protocol_config = read_api.get_protocol_config(None).await?;
                        build_config.implicit_dependencies =
                            implicit_deps_for_protocol_version(protocol_config.protocol_version)?;
                        let mut pkg = IotaBuildConfig {
                            config: build_config.clone(),
                            run_bytecode_verifier: true,
                            print_diags_to_stderr: true,
                            chain_id: chain_id.clone(),
                        }
                        .build(&rerooted_path)?;

                        // Restore original ID, then check result.
                        if let (Some(chain_id), Some(previous_id)) = (chain_id, previous_id) {
                            let _ = iota_package_management::set_package_id(
                                &rerooted_path,
                                build_config.install_dir.clone(),
                                &chain_id,
                                previous_id,
                            )?;
                        }

                        let with_unpublished_deps = build.with_unpublished_dependencies;

                        check_conflicting_addresses(&pkg.dependency_ids.conflicting, true)?;
                        check_invalid_dependencies(&pkg.dependency_ids.invalid)?;

                        if !with_unpublished_deps {
                            check_unpublished_dependencies(&pkg.dependency_ids.unpublished)?;
                        }

                        pkg_tree_shake(read_api, with_unpublished_deps, &mut pkg).await?;

                        println!(
                            "{}",
                            json!({
                                "modules": pkg.get_package_base64(with_unpublished_deps),
                                "dependencies": pkg.get_dependency_storage_package_ids(),
                                "digest": pkg.get_package_digest(with_unpublished_deps),
                            })
                        );
                        return Ok(());
                    }
                    _ => (),
                }

                // If a specific environment is specified for the build command we set the chain
                // ID to the one that is specified.
                if client_config.env.is_some() && matches!(cmd, MoveCommand::Build(_)) {
                    // TODO replace with get_chain_id_and_client when https://github.com/iotaledger/iota/issues/10215 is done
                    let mut context = WalletContext::new(
                        &client_config
                            .config
                            .unwrap_or(iota_config_dir()?.join(IOTA_CLIENT_CONFIG)),
                    )?;
                    if let Some(env_override) = &client_config.env {
                        context = context.with_env_override(env_override.clone());
                    }
                    let Ok(client) = context.get_client().await else {
                        bail!(
                            "`iota move build` requires a connection to the network. Current active network is {} but failed to connect to it.",
                            context.active_env().as_ref().unwrap()
                        );
                    };
                    let chain_id = client.read_api().get_chain_identifier().await.ok();

                    let MoveCommand::Build(build_config) = &mut cmd else {
                        unreachable!("We checked for Build above, so this should never happen");
                    };

                    build_config.chain_id = chain_id;
                }

                execute_move_command(package_path.as_deref(), build_config, cmd)
            }
            #[cfg(feature = "iota-names")]
            IotaCommand::Name { config, json, cmd } => {
                eprintln!(
                    "{}",
                    "By using this service, you agree to the Terms & Conditions: iotanames.com/terms-of-service"
                        .bold()
                        .yellow()
                );
                let config_path = config
                    .config
                    .unwrap_or(iota_config_dir()?.join(IOTA_CLIENT_CONFIG));
                prompt_if_no_config(&config_path, false, true, true)?;
                let mut context = WalletContext::new(&config_path)?;
                cmd.execute(&mut context).await?.print(!json);
                Ok(())
            }
            IotaCommand::FireDrill {} => {
                eprintln!(
                    "{}",
                    "The `fire-drill` command has been moved to `iota-tool`.\n\
                     Please use `iota-tool fire-drill` instead."
                        .yellow()
                        .bold()
                );
                std::process::exit(1);
            }
            IotaCommand::Analyzer => {
                analyzer::run(implicit_deps(latest_system_packages()));
                Ok(())
            }
            #[cfg(feature = "gen-completions")]
            IotaCommand::GenerateCompletions(cmd) => cmd.run(),
        }
    }
}

fn prompt_for_environment(
    wallet_conf_path: &Path,
    accept_defaults: bool,
) -> anyhow::Result<IotaEnv> {
    if let Some(v) = std::env::var_os("IOTA_CONFIG_WITH_RPC_URL") {
        return Ok(IotaEnv::new("custom", v.into_string().unwrap()));
    }

    if accept_defaults {
        print!(
            "Creating config file [{wallet_conf_path:?}] with default (Testnet) full node server and ed25519 key scheme."
        );
        return Ok(IotaEnv::testnet());
    }

    print!(
        "Select a default network [mainnet|testnet|devnet|localnet], or enter a custom IOTA full node server URL (defaults to testnet if not specified): "
    );
    match read_line()?.trim().to_lowercase().as_str() {
        "mainnet" => Ok(IotaEnv::mainnet()),
        "testnet" | "" => Ok(IotaEnv::testnet()),
        "devnet" => Ok(IotaEnv::devnet()),
        "localnet" => Ok(IotaEnv::localnet()),
        input => {
            if Url::parse(input).is_ok() {
                print!("Environment alias for [{input}]: ");
                let alias = read_line()?;
                let alias = if alias.trim().is_empty() {
                    "custom".to_string()
                } else {
                    alias
                };
                Ok(IotaEnv::new(alias, input))
            } else {
                bail!("invalid custom URL: {input}");
            }
        }
    }
}

fn prompt_if_no_config(
    wallet_conf_path: &Path,
    accept_defaults: bool,
    prompt_for_env: bool,
    generate_address: bool,
) -> anyhow::Result<()> {
    // Prompt user for connect to devnet fullnode if config does not exist.
    if !wallet_conf_path.exists() {
        let keystore_path = match wallet_conf_path.parent() {
            // Wallet config was created in the current directory as a relative path.
            Some(parent) if parent.as_os_str().is_empty() => std::env::current_dir()
                .context("Could not find current directory for iota config")?,
            // Wallet config was given a path with some parent (could be relative or absolute).
            Some(parent) => parent
                .canonicalize()
                .context("Could not find iota config directory")?,
            // No parent component and the wallet config was the empty string, use the default
            // config.
            None if wallet_conf_path.as_os_str().is_empty() => iota_config_dir()?,
            // Wallet config was requested at the root of the file system for some reason.
            None => wallet_conf_path.to_owned(),
        }
        .join(IOTA_KEYSTORE_FILENAME);
        let keystore = Keystore::from(FileBasedKeystore::new(&keystore_path)?);
        let mut config = IotaClientConfig::new(keystore).with_default_envs();
        if prompt_for_env {
            let env = prompt_for_environment(wallet_conf_path, accept_defaults)?;
            let alias = env.alias().clone();
            config.set_env(env);
            config.set_active_env(alias);
        }
        // Get an existing address or generate a new one
        if let Some(existing_address) = config.keystore().addresses().first() {
            println!("Using existing address {existing_address} as active address.");
            config = config.with_active_address(*existing_address);
        } else if generate_address {
            let key_scheme = if accept_defaults {
                SignatureScheme::ED25519
            } else {
                print!(
                    "Select key scheme to generate keypair (0 for ed25519, 1 for secp256k1, 2: for secp256r1): "
                );
                match SignatureScheme::from_flag(read_line()?.trim()) {
                    Ok(s) => s,
                    Err(e) => bail!("{e}"),
                }
            };
            let (new_address, phrase, scheme) = config
                .keystore_mut()
                .generate_and_add_new_key(key_scheme, None, None, None)?;
            let alias = config.keystore().get_alias_by_address(&new_address)?;
            println!(
                "Generated new keypair and alias for address with scheme {:?}:\n[{alias}: {new_address}]",
                scheme.to_string()
            );
            println!("Secret Recovery Phrase:\n[{phrase}]");
            config = config.with_active_address(new_address);
        }
        config.persisted(wallet_conf_path).save()?;
    }
    Ok(())
}

fn read_line() -> Result<String, anyhow::Error> {
    let mut s = String::new();
    let _ = stdout().flush();
    io::stdin().read_line(&mut s)?;
    Ok(s.trim_end().to_string())
}
