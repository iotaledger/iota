// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
};

use anyhow::Result;
use capitalize::Capitalize;
use iota_move_build::{BuildConfig, IotaPackageHooks};
use move_binary_format::{CompiledModule, file_format::Visibility};
use move_compiler::editions::Edition;
use move_package::{BuildConfig as MoveBuildConfig, LintFlag};

const CRATE_ROOT: &str = env!("CARGO_MANIFEST_DIR");
const COMPILED_PACKAGES_DIR: &str = "packages_compiled";
const DOCS_DIR: &str = "../../docs/generated-docs/framework";
const PUBLISHED_API_FILE: &str = "published_api.txt";

#[test]
fn build_system_packages() {
    move_package::package_hooks::register_package_hooks(Box::new(IotaPackageHooks));
    let tempdir = tempfile::tempdir().unwrap();
    let out_dir = if std::env::var_os("UPDATE").is_some() {
        let crate_root = Path::new(CRATE_ROOT);
        let _ = std::fs::remove_dir_all(crate_root.join(COMPILED_PACKAGES_DIR));
        let _ = std::fs::remove_dir_all(DOCS_DIR);
        let _ = std::fs::remove_file(crate_root.join(PUBLISHED_API_FILE));
        crate_root
    } else {
        tempdir.path()
    };

    std::fs::create_dir_all(out_dir.join(COMPILED_PACKAGES_DIR)).unwrap();
    std::fs::create_dir_all(DOCS_DIR).unwrap();

    let packages_path = Path::new(CRATE_ROOT).join("packages");

    let bridge_path = packages_path.join("bridge");
    let iota_system_path = packages_path.join("iota-system");
    let iota_framework_path = packages_path.join("iota-framework");
    let move_stdlib_path = packages_path.join("move-stdlib");
    let stardust_path = packages_path.join("stardust");

    build_packages(
        &bridge_path,
        &iota_system_path,
        &iota_framework_path,
        &move_stdlib_path,
        &stardust_path,
        out_dir,
    );

    check_diff(Path::new(CRATE_ROOT), out_dir)
}

// Verify that checked-in values are the same as the generated ones
fn check_diff(checked_in: &Path, built: &Path) {
    for path in [COMPILED_PACKAGES_DIR, PUBLISHED_API_FILE] {
        let output = std::process::Command::new("diff")
            .args(["--brief", "--recursive"])
            .arg(checked_in.join(path))
            .arg(built.join(path))
            .output()
            .unwrap();
        if !output.status.success() {
            let header = "Generated and checked-in iota-framework packages do not match.\n\
                Re-run with `UPDATE=1` to update checked-in packages. e.g.\n\n\
                UPDATE=1 cargo test -p iota-framework --test build-system-packages";

            panic!(
                "{header}\n\n{}\n\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
}

fn build_packages(
    bridge_path: &Path,
    iota_system_path: &Path,
    iota_framework_path: &Path,
    stdlib_path: &Path,
    stardust_path: &Path,
    out_dir: &Path,
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
        bridge_path,
        iota_system_path,
        iota_framework_path,
        stdlib_path,
        stardust_path,
        out_dir,
        "bridge",
        "iota-system",
        "iota-framework",
        "move-stdlib",
        "stardust",
        config,
    );
}

fn build_packages_with_move_config(
    bridge_path: &Path,
    iota_system_path: &Path,
    iota_framework_path: &Path,
    stdlib_path: &Path,
    stardust_path: &Path,
    out_dir: &Path,
    bridge_dir: &str,
    system_dir: &str,
    framework_dir: &str,
    stdlib_dir: &str,
    stardust_dir: &str,
    config: MoveBuildConfig,
) {
    let stdlib_pkg = BuildConfig {
        config: config.clone(),
        run_bytecode_verifier: true,
        print_diags_to_stderr: false,
        chain_id: None, // Framework pkg addr is agnostic to chain, resolves from Move.toml
    }
    .build(stdlib_path)
    .unwrap();
    let framework_pkg = BuildConfig {
        config: config.clone(),
        run_bytecode_verifier: true,
        print_diags_to_stderr: false,
        chain_id: None, // Framework pkg addr is agnostic to chain, resolves from Move.toml
    }
    .build(iota_framework_path)
    .unwrap();
    let system_pkg = BuildConfig {
        config: config.clone(),
        run_bytecode_verifier: true,
        print_diags_to_stderr: false,
        chain_id: None, // Framework pkg addr is agnostic to chain, resolves from Move.toml
    }
    .build(iota_system_path)
    .unwrap();
    let bridge_pkg = BuildConfig {
        config: config.clone(),
        run_bytecode_verifier: true,
        print_diags_to_stderr: false,
        chain_id: None, // Framework pkg addr is agnostic to chain, resolves from Move.toml
    }
    .build(bridge_path)
    .unwrap();
    let stardust_pkg = BuildConfig {
        config,
        run_bytecode_verifier: true,
        print_diags_to_stderr: false,
        chain_id: None, // Framework pkg addr is agnostic to chain, resolves from Move.toml
    }
    .build(stardust_path)
    .unwrap();

    let move_stdlib = stdlib_pkg.get_stdlib_modules();
    let iota_system = system_pkg.get_iota_system_modules();
    let iota_framework = framework_pkg.get_iota_framework_modules();
    let bridge = bridge_pkg.get_bridge_modules();
    let stardust = stardust_pkg.get_stardust_modules();

    let compiled_packages_dir = out_dir.join(COMPILED_PACKAGES_DIR);

    let iota_system_members =
        serialize_modules_to_file(iota_system, &compiled_packages_dir.join(system_dir)).unwrap();
    let iota_framework_members =
        serialize_modules_to_file(iota_framework, &compiled_packages_dir.join(framework_dir))
            .unwrap();
    let bridge_members =
        serialize_modules_to_file(bridge, &compiled_packages_dir.join(bridge_dir)).unwrap();
    let stdlib_members =
        serialize_modules_to_file(move_stdlib, &compiled_packages_dir.join(stdlib_dir)).unwrap();
    let stardust_members =
        serialize_modules_to_file(stardust, &compiled_packages_dir.join(stardust_dir)).unwrap();

    // write out generated docs
    let docs_dir = PathBuf::from(DOCS_DIR);
    let mut files_to_write = BTreeMap::new();
    create_category_file(system_dir);
    relocate_docs(
        system_dir,
        &system_pkg.package.compiled_docs.unwrap(),
        &mut files_to_write,
    );
    create_category_file(framework_dir);
    relocate_docs(
        framework_dir,
        &framework_pkg.package.compiled_docs.unwrap(),
        &mut files_to_write,
    );
    create_category_file(stdlib_dir);
    relocate_docs(
        stdlib_dir,
        &stdlib_pkg.package.compiled_docs.unwrap(),
        &mut files_to_write,
    );
    create_category_file(bridge_dir);
    relocate_docs(
        bridge_dir,
        &bridge_pkg.package.compiled_docs.unwrap(),
        &mut files_to_write,
    );
    create_category_file(stardust_dir);
    relocate_docs(
        stardust_dir,
        &stardust_pkg.package.compiled_docs.unwrap(),
        &mut files_to_write,
    );
    for (fname, doc) in files_to_write {
        let dst_path = docs_dir.join(fname);
        fs::create_dir_all(dst_path.parent().unwrap()).unwrap();
        fs::write(dst_path, doc).unwrap();
    }

    let published_api = [
        iota_system_members.join("\n"),
        iota_framework_members.join("\n"),
        bridge_members.join("\n"),
        stdlib_members.join("\n"),
        stardust_members.join("\n"),
    ]
    .join("\n");

    fs::write(out_dir.join(PUBLISHED_API_FILE), published_api).unwrap();
}

/// Create a Docusaurus category file for the specified prefix.
fn create_category_file(prefix: &str) {
    let mut path = PathBuf::from(DOCS_DIR).join(prefix);
    fs::create_dir_all(path.clone()).unwrap();
    path.push("_category_.json");
    let label = prefix
        .split('-')
        .map(|w| w.capitalize())
        .collect::<Vec<_>>()
        .join(" ");
    fs::write(
        path,
        serde_json::json!({
            "label": label,
            "link": {
                "type": "generated-index",
                "slug": format!("/references/framework/{}", prefix),
                "description": format!(
                    "Documentation for the modules in the iota/crates/iota-framework/packages/{prefix} crate. Select a module from the list to see its details."
                )
            }
        }).to_string()
    ).unwrap()
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
    // of the file to beginning of the heading, then capture the heading as three
    // different parts and replace with the yaml tag for docusaurus, add the
    // Link import and the title anchor, so the tile can be linked to. E.g., ```
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
    let type_regex =
        regex::Regex::new(r"(\S*?)<(IOTA|0xabcded::soon::SOON|T|u8|u64|String)>").unwrap();
    let iota_system_regex = regex::Regex::new(r"((?:\.\.\/|\.\/)+)(iota_system)(\.md)").unwrap();

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

        // Replace a-tags with Link to register anchors in Docusaurus (we have to use
        // the `id` attribute as `name` is deprecated and not existing in Link
        // component)
        let content = link_from_regex.replace_all(file_content, r#"<Link id="$1"></Link>"#);

        // Replace a-tags with href for Link tags to enable link and anchor checking. We
        // need to make sure that `to` path don't contain extensions in a later step.
        let content = link_to_regex.replace_all(&content, r#"<Link to="$1">$2</Link>"#);

        // Escape `{` in multi-line <code> and add new lines as this is a requirement
        // from mdx
        let content = code_regex.replace_all(&content, |caps: &regex::Captures| {
            let match_content = caps.get(0).unwrap().as_str();
            let code_content = caps.get(1).unwrap().as_str();
            if match_content.lines().count() == 1 {
                return match_content.to_string();
            }
            format!("\n<code>\n{}</code>\n", code_content.replace('{', "\\{"))
        });

        // Wrap types like '<IOTA>', '<T>' and more in backticks as they are seen as
        // React components otherwise
        let content = type_regex.replace_all(&content, r#"`$1<$2>`"#);

        // Add the iota-system directory to links containing iota_system.md
        // This is a quite specific case, as docs of packages, that are not
        // dependencies, are created in root, but this script moves them to a
        // folder with the name of the package. So their links are not correct anymore.
        // We could improve this by checking all links that, are not from dependencies,
        // against a list of all paths and replace them accordingly.
        let content = iota_system_regex.replace_all(&content, r#"${1}iota-system/$2$3"#);

        let content = content
            .replace("../../", "../")
            .replace("../dependencies/", "../")
            .replace("dependencies/", "../")
            // Here we remove the extension from `to` property in Link tags
            .replace(".md", "");

        // Store all files in a map to deduplicate and change extension to mdx
        output.entry(format!("{}x", file_name)).or_insert_with(|| {
            title_regex.replace_all(&content, |caps: &regex::Captures| {
                    let title_type = caps.get(1).unwrap().as_str();
                    let package = caps.get(2).unwrap().as_str();
                    let anchor = package.replace("::", "_");
                    let name = package.split("::").last().unwrap();
                    // Remove backticks from title and add module name as sidebar label
                    // We also have to add a slug. Why? Let me tell you how stupid this is:
                    // When we have a folder which contains an markdown file with the same name (/framework/bridge/bridge.md)
                    // The url of that file will be /framework/bridge. Which will break anchors that are for example in that mentioned file
                    // and look like this: bridge#anchor for example. So we enforced docusaurus to keep the duplicate in the url by using a custom slug.
                    // Another alternative for later could be to fix this weird anchors in the first place by using relative paths.
                    format!("---\ntitle: {}{}\nsidebar_label: {}\nslug: {}\n---\nimport Link from '@docusaurus/Link';\n\n<Link id=\"{}\"/>", title_type, package, name, name, anchor)
            }).to_string()
        });
    }
}

fn serialize_modules_to_file<'a>(
    modules: impl Iterator<Item = &'a CompiledModule>,
    file: &Path,
) -> Result<Vec<String>> {
    let mut serialized_modules = Vec::new();
    let mut members = vec![];
    for module in modules {
        let module_name = module.self_id().short_str_lossless();
        for def in module.struct_defs() {
            let sh = module.datatype_handle_at(def.struct_handle);
            let sn = module.identifier_at(sh.name);
            members.push(format!("{sn}\n\tpublic struct\n\t{module_name}"));
        }

        for def in module.enum_defs() {
            let eh = module.datatype_handle_at(def.enum_handle);
            let en = module.identifier_at(eh.name);
            members.push(format!("{en}\n\tpublic enum\n\t{module_name}"));
        }

        for def in module.function_defs() {
            let fh = module.function_handle_at(def.function);
            let fn_ = module.identifier_at(fh.name);
            let viz = match def.visibility {
                Visibility::Public => "public ",
                Visibility::Friend => "public(package) ",
                Visibility::Private => "",
            };
            let entry = if def.is_entry { "entry " } else { "" };
            members.push(format!("{fn_}\n\t{viz}{entry}fun\n\t{module_name}"));
        }

        let mut buf = Vec::new();
        module.serialize_with_version(module.version, &mut buf)?;
        serialized_modules.push(buf);
    }
    assert!(
        !serialized_modules.is_empty(),
        "Failed to find system or framework or stdlib modules"
    );

    let binary = bcs::to_bytes(&serialized_modules)?;

    fs::write(file, binary)?;

    Ok(members)
}
