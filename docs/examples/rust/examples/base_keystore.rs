use std::{fs, path::PathBuf};

use iota_keys::keystore::FileBasedKeystore;
use iota_sdk::IotaClientBuilder;

/// Creates a temporary keystore
fn setup_keystore() -> Result<FileBasedKeystore, anyhow::Error> {
    // Create a temporary keystore
    let keystore_path = PathBuf::from("iotatempdb");
    if !keystore_path.exists() {
        let keystore = FileBasedKeystore::new(&keystore_path)?;
        keystore.save()?;
    }
    // Read the iota keystore
    Ok(FileBasedKeystore::new(&keystore_path)?)
}

fn clean_keystore() -> Result<(), anyhow::Error> {
    // Remove the keystore files
    fs::remove_file("iotatempdb")?;
    fs::remove_file("iotatempdb.aliases")?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // Build an iota client for a local network
    let _iota_client = IotaClientBuilder::default().build_localnet().await?;

    // Setup a temporary file based keystore
    let _keystore = setup_keystore()?;

    // Finish and clean the temporary keystore file
    clean_keystore()
}
