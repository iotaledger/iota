// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
};

use anyhow::Result;
use iota_move_build::{BuildConfig, IotaPackageHooks};
use move_binary_format::CompiledModule;
use move_compiler::editions::Edition;
use move_package::{BuildConfig as MoveBuildConfig, LintFlag};

const DOCS_DIR: &str = "docs";

/// Save revision info to environment variable
fn main() {
    move_package::package_hooks::register_package_hooks(Box::new(IotaPackageHooks));
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let packages_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("packages");

    let deepbook_path = packages_path.join("deepbook");
    let iota_system_path = packages_path.join("iota-system");
    let iota_framework_path = packages_path.join("iota-framework");
    let stardust_path = packages_path.join("stardust");
    let move_stdlib_path = packages_path.join("move-stdlib");

    build_packages(
        deepbook_path.clone(),
        iota_system_path.clone(),
        iota_framework_path.clone(),
        stardust_path.clone(),
        out_dir,
    );

    println!("cargo:rerun-if-changed=build.rs");
    println!(
        "cargo:rerun-if-changed={}",
        deepbook_path.join("Move.toml").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        deepbook_path.join("sources").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        iota_system_path.join("Move.toml").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        iota_system_path.join("sources").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        iota_framework_path.join("Move.toml").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        iota_framework_path.join("sources").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        move_stdlib_path.join("Move.toml").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        move_stdlib_path.join("sources").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        stardust_path.join("Move.toml").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        stardust_path.join("sources").display()
    );
}

fn build_packages(
    deepbook_path: PathBuf,
    iota_system_path: PathBuf,
    iota_framework_path: PathBuf,
    stardust_path: PathBuf,
    out_dir: PathBuf,
) {
    let config = MoveBuildConfig {
        generate_docs: true,
        warnings_are_errors: true,
        install_dir: Some(PathBuf::from(".")),
        lint_flag: LintFlag::LEVEL_NONE,
        default_edition: Some(Edition::E2024_BETA),
        ..Default::default()
    };
    debug_assert!(!config.test_mode);
    build_packages_with_move_config(
        deepbook_path.clone(),
        iota_system_path.clone(),
        iota_framework_path.clone(),
        stardust_path.clone(),
        out_dir.clone(),
        "deepbook",
        "iota-system",
        "iota-framework",
        "move-stdlib",
        "stardust",
        config,
        true,
    );
    let config = MoveBuildConfig {
        generate_docs: true,
        test_mode: true,
        warnings_are_errors: true,
        install_dir: Some(PathBuf::from(".")),
        lint_flag: LintFlag::LEVEL_NONE,
        default_edition: Some(Edition::E2024_BETA),
        ..Default::default()
    };
    build_packages_with_move_config(
        deepbook_path,
        iota_system_path,
        iota_framework_path,
        stardust_path,
        out_dir,
        "deepbook-test",
        "iota-system-test",
        "iota-framework-test",
        "move-stdlib-test",
        "stardust-test",
        config,
        false,
    );
}

fn build_packages_with_move_config(
    deepbook_path: PathBuf,
    iota_system_path: PathBuf,
    iota_framework_path: PathBuf,
    stardust_path: PathBuf,
    out_dir: PathBuf,
    deepbook_dir: &str,
    system_dir: &str,
    framework_dir: &str,
    stdlib_dir: &str,
    stardust_dir: &str,
    config: MoveBuildConfig,
    write_docs: bool,
) {
    let framework_pkg = BuildConfig {
        config: config.clone(),
        run_bytecode_verifier: true,
        print_diags_to_stderr: false,
    }
    .build(iota_framework_path)
    .unwrap();
    let system_pkg = BuildConfig {
        config: config.clone(),
        run_bytecode_verifier: true,
        print_diags_to_stderr: false,
    }
    .build(iota_system_path)
    .unwrap();
    let deepbook_pkg = BuildConfig {
        config: config.clone(),
        run_bytecode_verifier: true,
        print_diags_to_stderr: false,
    }
    .build(deepbook_path)
    .unwrap();
    let stardust_pkg = BuildConfig {
        config: config.clone(),
        run_bytecode_verifier: true,
        print_diags_to_stderr: false,
    }
    .build(stardust_path)
    .unwrap();

    let iota_system = system_pkg.get_iota_system_modules();
    let iota_framework = framework_pkg.get_iota_framework_modules();
    let deepbook = deepbook_pkg.get_deepbook_modules();
    let move_stdlib = framework_pkg.get_stdlib_modules();
    let stardust = stardust_pkg.get_stardust_modules();

    serialize_modules_to_file(iota_system, &out_dir.join(system_dir)).unwrap();
    serialize_modules_to_file(iota_framework, &out_dir.join(framework_dir)).unwrap();
    serialize_modules_to_file(deepbook, &out_dir.join(deepbook_dir)).unwrap();
    serialize_modules_to_file(move_stdlib, &out_dir.join(stdlib_dir)).unwrap();
    serialize_modules_to_file(stardust, &out_dir.join(stardust_dir)).unwrap();
    // write out generated docs
    if write_docs {
        // Remove the old docs directory -- in case there was a module that was deleted
        // (could happen during development).
        if Path::new(DOCS_DIR).exists() {
            std::fs::remove_dir_all(DOCS_DIR).unwrap();
        }
        let mut files_to_write = BTreeMap::new();
        relocate_docs(
            deepbook_dir,
            &deepbook_pkg.package.compiled_docs.unwrap(),
            &mut files_to_write,
        );
        relocate_docs(
            system_dir,
            &system_pkg.package.compiled_docs.unwrap(),
            &mut files_to_write,
        );
        relocate_docs(
            framework_dir,
            &framework_pkg.package.compiled_docs.unwrap(),
            &mut files_to_write,
        );
        relocate_docs(
            stardust_dir,
            &stardust_pkg.package.compiled_docs.unwrap(),
            &mut files_to_write,
        );
        for (fname, doc) in files_to_write {
            let mut dst_path = PathBuf::from(DOCS_DIR);
            dst_path.push(fname);
            fs::create_dir_all(dst_path.parent().unwrap()).unwrap();
            fs::write(dst_path, doc).unwrap();
        }
    }
}

/// Post process the generated docs so that they are in a format that can be
/// consumed by docusaurus.
/// * Flatten out the tree-like structure of the docs directory that we generate
///   for a package into a flat list of packages;
/// * Deduplicate packages (since multiple packages could share dependencies);
///   and
/// * Replace html tags and use Docusaurus components where needed.
fn relocate_docs(prefix: &str, files: &[(String, String)], output: &mut BTreeMap<String, String>) {
    // Turn on multi-line mode so that `.` matches newlines, consume from the start
    // of the file to beginning of the heading, then capture the heading as three different parts and
    // replace with the yaml tag for docusaurus, add the Link import and the title anchor,
    // so the tile can be linked to. E.g., ```
    // -<a name="0x2_display"></a>
    // -
    // -# Module `0x2::display`
    // -
    // +---
    // +title: Module `0x2::display`
    // +---
    // +
    // +import Link from '@docusaurus/Link';
    // +<Link id="0x2::display"/>
    //```
    let title_regex = regex::Regex::new(r"(?s).*\n#\s+(.*?)`(\S*?)`\n").unwrap();
    let link_from_regex = regex::Regex::new(r#"<a name=\"([^\"]+)\"></a>"#).unwrap();
    let link_to_regex = regex::Regex::new(r#"<a href="(\S*)">([\s\S]*?)</a>"#).unwrap();
    let code_regex = regex::Regex::new(r"<code>([\s\S]*?)<\/code>").unwrap();
    let type_regex = regex::Regex::new(r"(\S*?)<(IOTA|SMR|0xabcded::soon::SOON|T)>").unwrap();

    for (file_name, file_content) in files {
        let path = PathBuf::from(file_name);
        let top_level = path.components().count() == 1;
        let file_name = if top_level {
            let mut new_path = PathBuf::from(prefix);
            new_path.push(file_name);
            new_path.to_string_lossy().to_string()
        } else {
            let mut new_path = PathBuf::new();
            new_path.push(path.components().skip(1).collect::<PathBuf>());
            new_path.to_string_lossy().to_string()
        };

        // Replace a-tags with Link to register anchors in Docusaurus (we have to use the `id` attribute as `name` is deprecated and not existing in Link component)
        let content = link_from_regex.replace_all(&file_content, r#"<Link id="$1"></Link>"#);

        // Replace a-tags with href for Link tags to enable link and anchor checking. We need to make sure that `to` path don't contain extensions in a later step.
        let content = link_to_regex.replace_all(&content, r#"<Link to="$1">$2</Link>"#);

        // Escape `{` in <code> and add new lines as this is a requirement from mdx
        let content = code_regex.replace_all(&content, |caps: &regex::Captures| {
            let code_content = caps.get(1).unwrap().as_str();
            format!("<code>\n{}</code>", code_content.replace("{", "\\{"))
        });

        // Wrap types like '<IOTA>', '<T>' and more in backticks as they are seen as
        // React components otherwise
        let content = type_regex.replace_all(&content, r#"`$1<$2>`"#);

        let content = content
            .replace("../../dependencies/", "../")
            .replace("dependencies/", "../")
            // Here we remove the extension from `to` property in Link tags
            .replace(".md", "");

        // Store all files in a map to deduplicate and change extension to mdx
        output.entry(format!("{}x", file_name)).or_insert_with(|| {
            title_regex.replace_all(&content, |caps: &regex::Captures| {
                    let title_type = caps.get(1).unwrap().as_str();
                    let name = caps.get(2).unwrap().as_str();
                    let anchor = name.replace("::", "_");
                    format!("---\ntitle: {}`{}`\n---\nimport Link from '@docusaurus/Link';\n\n<Link id=\"{}\"/>", title_type, name, anchor)
        }).to_string()
        });
    }
}

fn serialize_modules_to_file<'a>(
    modules: impl Iterator<Item = &'a CompiledModule>,
    file: &Path,
) -> Result<()> {
    let mut serialized_modules = Vec::new();
    for module in modules {
        let mut buf = Vec::new();
        module.serialize(&mut buf)?;
        serialized_modules.push(buf);
    }
    assert!(
        !serialized_modules.is_empty(),
        "Failed to find system or framework or stdlib modules"
    );

    let binary = bcs::to_bytes(&serialized_modules)?;

    fs::write(file, binary)?;

    Ok(())
}
