// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{fs::File, path::PathBuf};

use anyhow::Result;
use camino::Utf8PathBuf;
use clap::Parser;
use fastcrypto::encoding::{Encoding, Hex};
use iota_config::{
    IOTA_GENESIS_FILENAME,
    genesis::{Delegations, TokenDistributionScheduleBuilder, UnsignedGenesis},
};
use iota_genesis_builder::{
    Builder, GENESIS_BUILDER_PARAMETERS_FILE, SnapshotSource, SnapshotUrl,
    genesis_build_effects::GenesisBuildEffects,
};
use iota_keys::keypair_file::{
    read_authority_keypair_from_file, read_keypair_from_file, read_network_keypair_from_file,
};
use iota_protocol_config::MAX_PROTOCOL_VERSION;
use iota_types::{
    base_types::IotaAddress,
    committee::ProtocolVersion,
    crypto::{
        AuthorityKeyPair, IotaKeyPair, KeypairTraits, NetworkKeyPair, generate_proof_of_possession,
    },
    message_envelope::Message,
    multiaddr::Multiaddr,
};

use crate::genesis_inspector::examine_genesis_checkpoint;

#[derive(Parser)]
pub struct Ceremony {
    /// The directory where the Genesis builder will be stored. Defaults to the
    /// current directory.
    #[arg(long)]
    path: Option<PathBuf>,
    /// The protocol version to use for this snapshot.
    #[arg(long, default_value_t = MAX_PROTOCOL_VERSION)]
    protocol_version: u64,
    #[command(subcommand)]
    command: CeremonyCommand,
}

impl Ceremony {
    pub async fn run(self) -> Result<()> {
        run(self).await
    }
}

#[derive(Parser)]
pub enum CeremonyCommand {
    /// Initialize a Genesis builder which can be configured with validators.
    Init,
    /// Validate the current state of the Genesis builder.
    ValidateState,
    /// Add a validator to the Genesis builder.
    AddValidator {
        /// The name of the validator.
        #[arg(long)]
        name: String,
        /// The path to the BLS12381 authority key file for the validator.
        #[arg(long)]
        authority_key_file: PathBuf,
        /// The path to the Ed25519 network key file for the consensus protocol.
        #[arg(long)]
        protocol_key_file: PathBuf,
        /// The path to the Ed25519 network key file for the account.
        #[arg(long)]
        account_key_file: PathBuf,
        /// The path to the Ed25519 network key file.
        #[arg(long)]
        network_key_file: PathBuf,
        /// The network address. This must be a TCP address in ASCII format.
        #[arg(long)]
        network_address: Multiaddr,
        /// The peer-to-peer address. This must be a UDP address in ASCII
        /// format.
        #[arg(long)]
        p2p_address: Multiaddr,
        /// The primary address. This must be a UDP address in ASCII
        /// format.
        #[arg(long)]
        primary_address: Multiaddr,
        /// An optional description of the validator.
        #[arg(long)]
        description: Option<String>,
        /// An optional URL pointing to an image for the validator.
        #[arg(long)]
        image_url: Option<String>,
        /// An optional URL pointing to the validator webpage.
        #[arg(long)]
        project_url: Option<String>,
    },
    /// Initialize token distribution schedule.
    InitTokenDistributionSchedule {
        #[arg(
            long,
            help = "The path to the csv file with the token allocations",
            name = "token_allocations.csv"
        )]
        token_allocations_path: PathBuf,
    },
    /// List the current validators in the Genesis builder.
    ListValidators,
    /// Initialize the validator delegations.
    InitDelegations {
        #[arg(long, help = "Path to the delegations file.", name = "delegations.csv")]
        delegations_path: PathBuf,
    },
    /// Build the Genesis checkpoint.
    BuildUnsignedCheckpoint {
        #[arg(
            long,
            help = "Define paths to local migration snapshots.",
            name = "path",
            num_args(0..)
        )]
        local_migration_snapshots: Vec<PathBuf>,
        #[arg(long, name = "iota|<full-url>", help = "Remote migration snapshots.", num_args(0..))]
        remote_migration_snapshots: Vec<SnapshotUrl>,
    },
    /// Examine the details of the built Genesis checkpoint.
    ExamineGenesisCheckpoint,
    /// Verify and sign the built Genesis checkpoint.
    VerifyAndSign {
        /// The path to a key file which will be used to sign the checkpoint.
        #[arg(long)]
        key_file: PathBuf,
    },
    /// Create the Genesis blob file from the current configuration.
    Finalize,
}

pub async fn run(cmd: Ceremony) -> Result<()> {
    let dir = if let Some(path) = cmd.path {
        path
    } else {
        std::env::current_dir()?
    };
    let dir = Utf8PathBuf::try_from(dir)?;

    let protocol_version = ProtocolVersion::new(cmd.protocol_version);

    match cmd.command {
        CeremonyCommand::Init => {
            let builder = Builder::new().with_protocol_version(protocol_version);
            builder.save(&dir)?;
            println!(
                "Initialized ceremony builder at {}",
                dir.join(GENESIS_BUILDER_PARAMETERS_FILE)
            );
        }

        CeremonyCommand::ValidateState => {
            let builder = Builder::load(&dir).await?;
            builder.validate()?;
            println!(
                "Successfully validated ceremony builder at {}",
                dir.join(GENESIS_BUILDER_PARAMETERS_FILE)
            );
        }

        CeremonyCommand::InitTokenDistributionSchedule {
            token_allocations_path,
        } => {
            let mut builder = Builder::load(&dir).await?;
            let mut schedule_builder = TokenDistributionScheduleBuilder::new();

            let token_allocations_csv = File::open(token_allocations_path)?;
            let mut reader = csv::Reader::from_reader(token_allocations_csv);
            for allocation in reader.deserialize() {
                schedule_builder.add_allocation(allocation?);
            }

            builder = builder.with_token_distribution_schedule(schedule_builder.build());
            builder.save(dir)?;
        }

        CeremonyCommand::AddValidator {
            name,
            authority_key_file,
            protocol_key_file,
            account_key_file,
            network_key_file,
            network_address,
            p2p_address,
            primary_address,
            description,
            image_url,
            project_url,
        } => {
            let mut builder = Builder::load(&dir).await?;
            let authority_keypair: AuthorityKeyPair =
                read_authority_keypair_from_file(authority_key_file)?;
            let account_keypair: IotaKeyPair = read_keypair_from_file(account_key_file)?;
            let protocol_keypair: NetworkKeyPair =
                read_network_keypair_from_file(protocol_key_file)?;
            let network_keypair: NetworkKeyPair = read_network_keypair_from_file(network_key_file)?;
            let pop = generate_proof_of_possession(
                &authority_keypair,
                (&account_keypair.public()).into(),
            );
            builder = builder.add_validator(
                iota_genesis_builder::validator_info::ValidatorInfo {
                    name,
                    authority_key: authority_keypair.public().into(),
                    protocol_key: protocol_keypair.public().clone(),
                    account_address: IotaAddress::from(&account_keypair.public()),
                    network_key: network_keypair.public().clone(),
                    gas_price: iota_config::node::DEFAULT_VALIDATOR_GAS_PRICE,
                    commission_rate: iota_config::node::DEFAULT_COMMISSION_RATE,
                    network_address,
                    p2p_address,
                    primary_address,
                    description: description.unwrap_or_default(),
                    image_url: image_url.unwrap_or_default(),
                    project_url: project_url.unwrap_or_default(),
                },
                pop,
            );
            builder.save(dir)?;
            println!("Successfully added validator");
        }

        CeremonyCommand::ListValidators => {
            let builder = Builder::load(&dir).await?;

            let mut validators = builder
                .validators()
                .values()
                .map(|v| {
                    (
                        v.info.name().to_lowercase(),
                        v.info.account_address.to_string(),
                    )
                })
                .collect::<Vec<_>>();

            let max_width = validators
                .iter()
                .max_by_key(|v| &v.0)
                .map(|(n, _)| n.len().max(14))
                .unwrap_or(14);

            validators.sort_by(|v1, v2| v1.0.cmp(&v2.0));

            println!(
                "{:<width$} Account Address",
                "Validator Name",
                width = max_width
            );
            println!("{:-<width$} {:-<66}", "", "", width = max_width);

            for (name, address) in validators {
                println!("{name:<width$} {address}", width = max_width);
            }
        }

        CeremonyCommand::InitDelegations { delegations_path } => {
            let mut builder = Builder::load(&dir).await?;
            let file = File::open(delegations_path)?;
            let delegations = Delegations::from_csv(file)?;
            builder = builder.with_delegations(delegations);
            builder.save(dir)?;
        }

        CeremonyCommand::BuildUnsignedCheckpoint {
            local_migration_snapshots,
            remote_migration_snapshots,
        } => {
            let local_snapshots = local_migration_snapshots
                .into_iter()
                .map(SnapshotSource::Local);
            let remote_snapshots = remote_migration_snapshots
                .into_iter()
                .map(SnapshotSource::S3);

            let mut builder = Builder::load(&dir).await?;
            for source in local_snapshots.chain(remote_snapshots) {
                builder = builder.add_migration_source(source);
            }

            tokio::task::spawn_blocking(move || {
                let UnsignedGenesis { checkpoint, .. } = builder.get_or_build_unsigned_genesis();
                println!(
                    "Successfully built unsigned checkpoint: {}",
                    checkpoint.digest()
                );

                builder.save(dir)
            })
            .await??;
        }

        CeremonyCommand::ExamineGenesisCheckpoint => {
            let builder = Builder::load(&dir).await?;

            let Some(unsigned_genesis) = builder.unsigned_genesis_checkpoint() else {
                return Err(anyhow::anyhow!(
                    "Unable to examine genesis checkpoint; try running `build-unsigned-checkpoint`"
                ));
            };

            examine_genesis_checkpoint(unsigned_genesis);
        }

        CeremonyCommand::VerifyAndSign { key_file } => {
            let keypair: AuthorityKeyPair = read_authority_keypair_from_file(key_file)?;

            let mut builder = Builder::load(&dir).await?;

            check_protocol_version(&builder, protocol_version)?;

            // Don't sign unless the unsigned checkpoint has already been created
            if builder.unsigned_genesis_checkpoint().is_none() {
                return Err(anyhow::anyhow!(
                    "Unable to verify and sign genesis checkpoint; try running `build-unsigned-checkpoint`"
                ));
            }

            builder = builder.add_validator_signature(&keypair);
            let UnsignedGenesis { checkpoint, .. } = builder.unsigned_genesis_checkpoint().unwrap();
            builder.save(dir)?;

            println!(
                "Successfully verified and signed genesis checkpoint: {}",
                checkpoint.digest()
            );
        }

        CeremonyCommand::Finalize => {
            let builder = Builder::load(&dir).await?;

            check_protocol_version(&builder, protocol_version)?;

            let GenesisBuildEffects { genesis, .. } = builder.build();
            genesis.save(dir.join(IOTA_GENESIS_FILENAME))?;

            println!("Successfully built {IOTA_GENESIS_FILENAME}");
            println!(
                "{IOTA_GENESIS_FILENAME} blake2b-256: {}",
                Hex::encode(genesis.hash())
            );
        }
    }

    Ok(())
}

fn check_protocol_version(builder: &Builder, protocol_version: ProtocolVersion) -> Result<()> {
    // It is entirely possible for the user to sign a genesis blob with an unknown
    // protocol version, but if this happens there is almost certainly some
    // confusion (e.g. using a `iota` binary built at the wrong commit).
    if builder.protocol_version() != protocol_version {
        return Err(anyhow::anyhow!(
            "Serialized protocol version does not match local --protocol-version argument. ({:?} vs {:?})",
            builder.protocol_version(),
            protocol_version
        ));
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use iota_config::local_ip_utils;
    use iota_genesis_builder::validator_info::ValidatorInfo;
    use iota_keys::keypair_file::{write_authority_keypair_to_file, write_keypair_to_file};
    use iota_macros::nondeterministic;
    use iota_types::crypto::{
        AccountKeyPair, AuthorityKeyPair, IotaKeyPair, get_key_pair_from_rng,
    };

    use super::*;

    #[tokio::test]
    #[cfg_attr(msim, ignore)]
    async fn ceremony() -> Result<()> {
        let dir = nondeterministic!(tempfile::TempDir::new().unwrap());

        let validators = (0..10)
            .map(|i| {
                let authority_keypair: AuthorityKeyPair =
                    get_key_pair_from_rng(&mut rand::rngs::OsRng).1;
                let protocol_keypair: NetworkKeyPair =
                    get_key_pair_from_rng(&mut rand::rngs::OsRng).1;
                let network_keypair: NetworkKeyPair =
                    get_key_pair_from_rng(&mut rand::rngs::OsRng).1;
                let account_keypair: AccountKeyPair =
                    get_key_pair_from_rng(&mut rand::rngs::OsRng).1;
                let info = ValidatorInfo {
                    name: format!("validator-{i}"),
                    authority_key: authority_keypair.public().into(),
                    protocol_key: protocol_keypair.public().clone(),
                    account_address: IotaAddress::from(account_keypair.public()),
                    network_key: network_keypair.public().clone(),
                    gas_price: iota_config::node::DEFAULT_VALIDATOR_GAS_PRICE,
                    commission_rate: iota_config::node::DEFAULT_COMMISSION_RATE,
                    network_address: local_ip_utils::new_local_tcp_address_for_testing(),
                    p2p_address: local_ip_utils::new_local_udp_address_for_testing(),
                    primary_address: local_ip_utils::new_local_udp_address_for_testing(),
                    description: String::new(),
                    image_url: String::new(),
                    project_url: String::new(),
                };
                let authority_key_file = dir.path().join(format!("{}-0.key", info.name));
                write_authority_keypair_to_file(&authority_keypair, &authority_key_file).unwrap();

                let protocol_key_file = dir.path().join(format!("{}.key", info.name));
                write_keypair_to_file(&IotaKeyPair::Ed25519(protocol_keypair), &protocol_key_file)
                    .unwrap();

                let network_key_file = dir.path().join(format!("{}-1.key", info.name));
                write_keypair_to_file(&IotaKeyPair::Ed25519(network_keypair), &network_key_file)
                    .unwrap();

                let account_key_file = dir.path().join(format!("{}-2.key", info.name));
                write_keypair_to_file(&IotaKeyPair::Ed25519(account_keypair), &account_key_file)
                    .unwrap();

                (
                    authority_key_file,
                    protocol_key_file,
                    network_key_file,
                    account_key_file,
                    info,
                )
            })
            .collect::<Vec<_>>();

        // Initialize
        let command = Ceremony {
            path: Some(dir.path().into()),
            protocol_version: MAX_PROTOCOL_VERSION,
            command: CeremonyCommand::Init,
        };
        command.run().await?;

        // Add the validators
        for (
            authority_key_file,
            protocol_key_file,
            network_key_file,
            account_key_file,
            validator,
        ) in &validators
        {
            let command = Ceremony {
                path: Some(dir.path().into()),
                protocol_version: MAX_PROTOCOL_VERSION,
                command: CeremonyCommand::AddValidator {
                    name: validator.name().to_owned(),
                    authority_key_file: authority_key_file.into(),
                    protocol_key_file: protocol_key_file.into(),
                    network_key_file: network_key_file.into(),
                    account_key_file: account_key_file.into(),
                    network_address: validator.network_address().to_owned(),
                    p2p_address: validator.p2p_address().to_owned(),
                    primary_address: validator.primary_address.clone(),
                    description: None,
                    image_url: None,
                    project_url: None,
                },
            };
            command.run().await?;

            Ceremony {
                path: Some(dir.path().into()),
                protocol_version: MAX_PROTOCOL_VERSION,
                command: CeremonyCommand::ValidateState,
            }
            .run()
            .await?;
        }

        // Build the unsigned checkpoint
        let command = Ceremony {
            path: Some(dir.path().into()),
            protocol_version: MAX_PROTOCOL_VERSION,
            command: CeremonyCommand::BuildUnsignedCheckpoint {
                local_migration_snapshots: vec![],
                remote_migration_snapshots: vec![],
            },
        };
        command.run().await?;

        // Have all the validators verify and sign genesis
        for (authority_key, _protocol_key, _network_key, _account_key, _validator) in &validators {
            let command = Ceremony {
                path: Some(dir.path().into()),
                protocol_version: MAX_PROTOCOL_VERSION,
                command: CeremonyCommand::VerifyAndSign {
                    key_file: authority_key.into(),
                },
            };
            command.run().await?;

            Ceremony {
                path: Some(dir.path().into()),
                protocol_version: MAX_PROTOCOL_VERSION,
                command: CeremonyCommand::ValidateState,
            }
            .run()
            .await?;
        }

        // Finalize the Ceremony and build the Genesis object
        let command = Ceremony {
            path: Some(dir.path().into()),
            protocol_version: MAX_PROTOCOL_VERSION,
            command: CeremonyCommand::Finalize,
        };
        command.run().await?;

        Ok(())
    }
}
