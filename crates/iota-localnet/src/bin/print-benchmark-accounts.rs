//! Generate the deterministic benchmark gas accounts derived from
//! `GenesisConfig::benchmark_gas_keys()` and write the matching keystore.
//!
//! Used by `dev-tools/iota-private-network/bootstrap.sh` to produce the
//! gas account block for the genesis template plus the keystore file the
//! validators will load. Keeps the addresses and keys in sync (both derive
//! from the same seeded RNG).
//!
//! Example:
//!   cargo run --release -p iota-localnet --bin print-benchmark-accounts -- \
//!     --count 96 \
//!     --gas-per-account 140000000000000000 \
//!     --keystore-path dev-tools/iota-private-network/configs/genesis/benchmark.keystore \
//!     > /tmp/accounts.yaml

use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use iota_keys::keystore::{AccountKeystore, FileBasedKeystore};
use iota_swarm_config::genesis_config::GenesisConfig;
use iota_types::base_types::IotaAddress;

#[derive(Parser)]
struct Cli {
    /// Number of benchmark gas accounts to generate.
    #[arg(long)]
    count: usize,
    /// Per-account gas amount in nanos (single gas object per account).
    #[arg(long)]
    gas_per_account: u64,
    /// Path where the keystore will be written (overwrites if exists).
    #[arg(long)]
    keystore_path: PathBuf,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.keystore_path.exists() {
        std::fs::remove_file(&cli.keystore_path)?;
    }
    if let Some(parent) = cli.keystore_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut keystore = FileBasedKeystore::new(&cli.keystore_path)?;
    let keys = GenesisConfig::benchmark_gas_keys(cli.count);

    for gas_key in keys.into_iter() {
        let addr = IotaAddress::from(&gas_key.public());
        println!("  - address: \"{}\"", addr);
        println!("    gas_amounts: [{}]", cli.gas_per_account);
        keystore.add_key(None, gas_key)?;
    }
    keystore.save()?;

    eprintln!(
        "Wrote {} keys to {}",
        cli.count,
        cli.keystore_path.display()
    );
    Ok(())
}
