// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

//! The [`package_builder`] module provides the [`PackageBuilder`] struct, which is responsible for building and compiling Stardust native token packages.
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use fs_extra::dir::{copy, CopyOptions};
use tempfile::tempdir;

use crate::stardust::error::StardustError;
use sui_move_build::{BuildConfig, CompiledPackage};

use crate::stardust::native_token::package_data::NativeTokenPackageData;

/// Builds and compiles a Stardust native token package.
pub fn build_and_compile(package: NativeTokenPackageData) -> Result<CompiledPackage> {
    let crate_root_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let template_path = crate_root_path.join("src/stardust/native_token/package_template");

    // Set up a temporary directory to build the native token package
    let tmp_dir = tempdir()?;
    let package_path = tmp_dir.path().join("native_token_package");

    // Define the path to the framework packages directory
    let framework_packages_path = crate_root_path
        .parent()
        .expect("parent should exist")
        .join("sui-framework/packages");

    // Step 1: Copy the template package directory
    copy_template_dir(&template_path, &package_path)?;

    // Step 2: Adjust the Move.toml file
    adjust_move_toml(&package_path, &framework_packages_path, &package)?;

    // Step 3: Replace template variables in the .move file
    adjust_native_token_module(&package_path, &package)?;

    // Step 4: Compile the package
    let compiled_package = BuildConfig::default().build(package_path)?;

    // Clean up the temporary directory
    tmp_dir.close()?;

    Ok(compiled_package)
}

fn copy_template_dir(src_dir: &Path, target_dir: &Path) -> Result<()> {
    let copy_options = CopyOptions::new().overwrite(true).content_only(true);
    copy(src_dir, target_dir, &copy_options)?;

    Ok(())
}

// Adjusts the Move.toml file with the package name and alias address.
fn adjust_move_toml(
    package_path: &Path,
    framework_packages_path: &Path,
    package: &NativeTokenPackageData,
) -> Result<()> {
    let cargo_toml_path = package_path.join("Move.toml");
    let contents = fs::read_to_string(&cargo_toml_path)?;
    let new_contents = contents
        .replace("$PACKAGE_NAME", package.package_name())
        .replace(
            "$FRAMEWORK_PACKAGES_PATH",
            framework_packages_path
                .to_str()
                .ok_or(StardustError::FrameworkPackagesPathNotFound)?,
        );
    fs::write(&cargo_toml_path, new_contents)?;

    Ok(())
}

// Replaces template variables in the .move file with the actual values.
fn adjust_native_token_module(package_path: &Path, package: &NativeTokenPackageData) -> Result<()> {
    let old_move_file_path = package_path.join("sources/native_token_template.move");
    let new_move_file_name = format!("{}.move", package.module().module_name);
    let new_move_file_path = package_path.join("sources").join(new_move_file_name);

    // Rename the template .move file
    fs::rename(old_move_file_path, &new_move_file_path)?;

    let contents = fs::read_to_string(&new_move_file_path)?;

    let icon_url = match &package.module().icon_url {
        Some(url) => format!(
            "option::some<Url>(sui::url::new_unsafe_from_bytes(b\"{}\"))",
            url
        ),
        None => "option::none<Url>()".to_string(),
    };

    let new_contents = contents
        .replace("$MODULE_NAME", &package.module().module_name)
        .replace("$OTW", &package.module().otw_name)
        .replace("$COIN_DECIMALS", &package.module().decimals.to_string())
        .replace("$COIN_SYMBOL", &package.module().symbol)
        .replace(
            "$CIRCULATING_TOKENS",
            &package.module().circulating_tokens.to_string(),
        )
        .replace(
            "$MAXIMUM_SUPPLY",
            &package.module().maximum_supply.to_string(),
        )
        .replace("$COIN_NAME", &package.module().coin_name)
        .replace("$COIN_DESCRIPTION", &package.module().coin_description)
        .replace("$ICON_URL", &icon_url)
        .replace(
            "$ALIAS",
            // Remove the "0x" prefix
            &package.module().alias_address.to_string().replace("0x", ""),
        );

    fs::write(&new_move_file_path, new_contents)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs::{self, File};
    use std::io::Write;

    use crate::stardust::native_token::package_builder;
    use tempfile::tempdir;

    #[test]
    fn test_copy_template_dir_success() {
        // Set up a temporary directory as the environment for the test
        let tmp_dir = tempdir().unwrap();
        let test_package_path = tmp_dir.path().join("package_template");
        fs::create_dir_all(&test_package_path).unwrap();

        // Simulate existing files and directories that the function expects to copy
        let src_dir = test_package_path.join("sources");
        fs::create_dir_all(&src_dir).unwrap();
        let test_file_path = src_dir.join("test.move");
        let mut file = File::create(test_file_path).unwrap();
        writeln!(file, "0x0::test {{}}").unwrap();

        // Define the target directory for the files to be copied
        let target_dir = tmp_dir.path().join("target_package");

        // Copy the files
        let result = package_builder::copy_template_dir(test_package_path.as_path(), &target_dir);

        assert!(result.is_ok());
        assert!(target_dir.exists());
        assert!(target_dir.join("sources").join("test.move").exists());
    }
}
