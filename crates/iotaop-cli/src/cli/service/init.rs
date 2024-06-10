// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    fs,
    fs::{create_dir_all, File},
    io::prelude::*,
    path::Path,
};

use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use include_dir::{include_dir, Dir};
use tracing::{debug, info};

// include the boilerplate code in this binary

// hardcode the 'boilerplate' symlink in the Windows compilation for now
#[cfg(target_os = "windows")]
static PROJECT_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/../mysten-service-boilerplate");
#[cfg(not(target_os = "windows"))]
static PROJECT_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/boilerplate");

#[derive(ValueEnum, Parser, Debug, Clone)]
pub enum ServiceLanguage {
    Rust,
    Typescript,
}

pub fn bootstrap_service(lang: &ServiceLanguage, path: &Path) -> Result<()> {
    match lang {
        ServiceLanguage::Rust => create_rust_service(path),
        ServiceLanguage::Typescript => todo!(),
    }
}

/// Add the new service to the iota-services dockerfile in the iota repository
fn add_to_iota_dockerfile(path: &Path) -> Result<()> {
    let path = path.canonicalize().context("canonicalizing service path")?;
    let crates_dir = path.parent().unwrap();
    if !crates_dir.ends_with("iota/crates") {
        panic!("directory wasn't in the iota repo");
    }
    let iota_services_dockerfile_path = &crates_dir.join("../docker/iota-services/Dockerfile");
    // read the dockerfile
    let dockerfile = fs::read_to_string(iota_services_dockerfile_path)
        .context("reading iota-services dockerfile")?;

    // find the line with the build cmd
    let build_line = dockerfile
        .lines()
        .enumerate()
        .find(|(_, line)| line.contains("RUN cargo build --release \\"))
        .expect("couldn't find build line in iota-services dockerfile")
        .0;
    // update with the new service
    let mut final_dockerfile = dockerfile.lines().collect::<Vec<_>>();
    let bin_str = format!(
        "    --bin {} \\",
        path.file_name()
            .expect("getting the project name from the given path")
            .to_str()
            .unwrap()
    );
    final_dockerfile.insert(build_line + 1, &bin_str);
    // write the file back
    fs::write(iota_services_dockerfile_path, final_dockerfile.join("\n"))
        .context("writing iota-services dockerfile after modifying it")?;

    Ok(())
}

fn add_member_to_workspace(path: &Path) -> Result<()> {
    // test
    let path = path.canonicalize().context("canonicalizing service path")?;
    let crates_dir = path.parent().unwrap();
    if !crates_dir.ends_with("iota/crates") {
        panic!("directory wasn't in the iota repo");
    }
    let workspace_toml_path = &crates_dir.join("../Cargo.toml");
    // read the workspace toml
    let toml_content = fs::read_to_string(workspace_toml_path)?;
    let mut toml = toml_content.parse::<toml_edit::Document>()?;
    toml["workspace"]["members"]
        .as_array_mut()
        .unwrap()
        .push_formatted(toml_edit::Value::String(toml_edit::Formatted::new(
            Path::new("crates/")
                .join(
                    path.file_name()
                        .expect("getting the project name from the given path"),
                )
                .to_str()
                .expect("converting the path to a str")
                .to_string(),
        )));
    fs::write(workspace_toml_path, toml.to_string())
        .context("failed to write workspace Cargo.toml back after update")?;
    Ok(())
}

fn create_rust_service(path: &Path) -> Result<()> {
    info!("creating rust service in {}", path.to_string_lossy());
    // create the dir to ensure we can canonicalize any relative paths
    create_dir_all(path)?;
    let is_iota_service = path
        // expand relative paths and symlinks
        .canonicalize()
        .context("canonicalizing service path")?
        .to_string_lossy()
        .contains("iota/crates");
    debug!("iota service: {:?}", is_iota_service);
    let cargo_toml_path = if is_iota_service {
        "Cargo.toml"
    } else {
        "Cargo-external.toml"
    };
    let cargo_toml = PROJECT_DIR.get_file(cargo_toml_path).unwrap();
    let main_rs = PROJECT_DIR.get_file("src/main.rs").unwrap();
    let main_body = main_rs.contents();
    let cargo_body = std::str::from_utf8(cargo_toml.contents())?;
    let mut toml_content = cargo_body.parse::<toml_edit::Document>()?;
    toml_content["package"]["name"] = toml_edit::value(path.file_name().unwrap().to_str().unwrap());
    create_dir_all(path.join("src"))?;
    let mut main_file = File::create(path.join("src/main.rs"))?;
    main_file.write_all(main_body)?;
    let mut cargo_file = File::create(path.join("Cargo.toml"))?;
    cargo_file.write_all(toml_content.to_string().as_bytes())?;

    // add the project as a member of the cargo workspace
    if is_iota_service {
        add_member_to_workspace(path)?;
    }
    // now that the source directory works, let's update/add a dockerfile
    if is_iota_service {
        // update iota-services dockerfile
        add_to_iota_dockerfile(path)?;
    } else {
        // TODO: create a new dockerfile where the user designates
    }

    Ok(())
}
