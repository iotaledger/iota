// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The `package_builder` module provides functions for building and
//! compiling Stardust native token packages.
use std::{fs, path::Path};

use anyhow::Result;
use iota_move_build::{BuildConfig, CompiledPackage, IotaPackageHooks};
use tempfile::tempdir;

use crate::stardust::native_token::package_data::NativeTokenPackageData;

const IOTA_FRAMEWORK_GENESIS_REVISION: &str = "framework/genesis/mainnet";

const MODULE_CONTENT: &str = r#"// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[allow(lint(share_owned))]
module 0x0::$MODULE_NAME {
    use iota::coin;
    use iota::coin_manager;
    use iota::url::Url;

    /// The type identifier of coin. The coin will have a type
    /// tag of kind: `Coin<package_object::$MODULE_NAME::$OTW`
    /// Make sure that the name of the type matches the module's name.
    public struct $OTW has drop {}

    /// Module initializer is called once on module publish. A treasury
    /// cap is sent to the publisher, who then controls minting and burning
    fun init(witness: $OTW, ctx: &mut TxContext) {
        let icon_url = $ICON_URL;

        // Create the currency
        let (mut treasury_cap, metadata) = coin::create_currency<$OTW>(
            witness,
            $COIN_DECIMALS,
            b"$COIN_SYMBOL",
            $COIN_NAME,
            $COIN_DESCRIPTION,
            icon_url,
            ctx
        );

        // Mint the tokens and transfer them to the publisher
        let minted_coins = coin::mint(&mut treasury_cap, $CIRCULATING_SUPPLY, ctx);
        transfer::public_transfer(minted_coins, ctx.sender());

        // Create a coin manager
        let (cm_treasury_cap, cm_metadata_cap, mut coin_manager) = coin_manager::new(treasury_cap, metadata, ctx);
        cm_treasury_cap.enforce_maximum_supply(&mut coin_manager, $MAXIMUM_SUPPLY);

        // Make the metadata immutable
        cm_metadata_cap.renounce_metadata_ownership(&mut coin_manager);

        // Publicly sharing the `CoinManager` object for convenient usage by anyone interested
        transfer::public_share_object(coin_manager);

        // Transfer the coin manager treasury capability to the alias address
        transfer::public_transfer(cm_treasury_cap, iota::address::from_ascii_bytes(&b"$ALIAS"));
    }
}
"#;

const TOML_CONTENT: &str = r#"[package]
name = "$PACKAGE_NAME"
version = "0.0.1"
edition = "2024.beta"

[dependencies]
Iota = { git = "https://github.com/iotaledger/iota.git", subdir = "crates/iota-framework/packages/iota-framework", rev = "$GENESIS_REVISION" }
"#;

/// Builds and compiles a Stardust native token package.
pub fn build_and_compile(package: NativeTokenPackageData) -> Result<CompiledPackage> {
    // Set up a temporary directory to build the native token package
    let tmp_dir = tempdir()?;
    let package_path = tmp_dir.path().join("native_token_package");
    fs::create_dir_all(&package_path).expect("Failed to create native_token_package directory");

    // Write and replace template variables in the Move.toml file
    write_move_toml(&package_path, &package, IOTA_FRAMEWORK_GENESIS_REVISION)?;

    // Write and replace template variables in the .move file
    write_native_token_module(&package_path, &package)?;

    // Compile the package
    move_package::package_hooks::register_package_hooks(Box::new(IotaPackageHooks));
    let compiled_package = BuildConfig::default().build(&package_path)?;

    // Step 5: Clean up the temporary directory
    tmp_dir.close()?;

    Ok(compiled_package)
}

// Write the Move.toml file with the package name and alias address.
fn write_move_toml(
    package_path: &Path,
    package: &NativeTokenPackageData,
    iota_framework_genesis_revision: &str,
) -> Result<()> {
    let cargo_toml_path = package_path.join("Move.toml");
    let new_contents = TOML_CONTENT
        .replace("$PACKAGE_NAME", package.package_name())
        .replace("$GENESIS_REVISION", iota_framework_genesis_revision);
    fs::write(&cargo_toml_path, new_contents)?;

    Ok(())
}

// Replaces template variables in the .move file with the actual values.
fn write_native_token_module(package_path: &Path, package: &NativeTokenPackageData) -> Result<()> {
    let move_source_path = package_path.join("sources");
    fs::create_dir_all(&move_source_path).expect("Failed to create sources directory");
    let new_move_file_name = format!("{}.move", package.module().module_name);
    let new_move_file_path = move_source_path.join(new_move_file_name);

    let icon_url = match &package.module().icon_url {
        Some(url) => format!(
            "option::some<Url>(iota::url::new_unsafe_from_bytes({}))",
            format_string_as_move_vector(url.as_str())
        ),
        None => "option::none<Url>()".to_string(),
    };

    let new_contents = MODULE_CONTENT
        .replace("$MODULE_NAME", &package.module().module_name)
        .replace("$OTW", &package.module().otw_name)
        .replace("$COIN_DECIMALS", &package.module().decimals.to_string())
        .replace("$COIN_SYMBOL", &package.module().symbol)
        .replace(
            "$CIRCULATING_SUPPLY",
            &package.module().circulating_supply.to_string(),
        )
        .replace(
            "$MAXIMUM_SUPPLY",
            &package.module().maximum_supply.to_string(),
        )
        .replace(
            "$COIN_NAME",
            format_string_as_move_vector(package.module().coin_name.as_str()).as_str(),
        )
        .replace(
            "$COIN_DESCRIPTION",
            format_string_as_move_vector(package.module().coin_description.as_str()).as_str(),
        )
        .replace("$ICON_URL", &icon_url)
        .replace(
            "$ALIAS",
            // Remove the "0x" prefix
            &package.module().alias_address.to_string().replace("0x", ""),
        );

    fs::write(&new_move_file_path, new_contents)?;

    Ok(())
}

/// Converts a string x to a string y representing the bytes of x as hexadecimal
/// values, which can be used as a piece of Move code.
///
/// Example: It converts "abc" to "vector<u8>[0x61, 0x62, 0x63]" plus the
/// original human-readable string in a comment.
fn format_string_as_move_vector(string: &str) -> String {
    let mut byte_string = String::new();
    byte_string.push_str("/* The utf-8 bytes of '");
    byte_string.push_str(string);
    byte_string.push_str("' */\n");

    byte_string.push_str("            vector<u8>[");

    for (idx, byte) in string.as_bytes().iter().enumerate() {
        byte_string.push_str(&format!("{byte:#x}"));

        if idx != string.len() - 1 {
            byte_string.push_str(", ");
        }
    }

    byte_string.push(']');

    byte_string
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn string_to_move_vector() {
        let tests = [
            ("", "vector<u8>[]"),
            ("a", "vector<u8>[0x61]"),
            ("ab", "vector<u8>[0x61, 0x62]"),
            ("abc", "vector<u8>[0x61, 0x62, 0x63]"),
            (
                "\nöäü",
                "vector<u8>[0xa, 0xc3, 0xb6, 0xc3, 0xa4, 0xc3, 0xbc]",
            ),
        ];

        for (test_input, expected_result) in tests {
            let move_string = format_string_as_move_vector(test_input);
            // Ignore the comment and whitespace.
            let actual_result = move_string.split('\n').next_back().unwrap().trim_start();
            assert_eq!(expected_result, actual_result);
        }
    }
}
