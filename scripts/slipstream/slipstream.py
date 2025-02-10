# Copyright (c) 2024 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0
import sys, os, re, json, shutil, subprocess, argparse, semver
sys.path.append('../utils')
from git_utils import clone_repo
from rust_utils import parse_rust_toolchain_version

# Extract the semantic version from a version string
def extract_sem_version(version_str):
    # Use regex to capture only the numeric version part (e.g., "1.30.0")
    match = re.search(r"(\d+\.\d+\.\d+)", version_str)
    if match:
        return semver.Version.parse(match.group(1))
    else:
        raise ValueError(f"Invalid version format: {version_str}")

# Load the configuration from config.json
def load_slipstream_config(file_path):
    print("Loading config file...")
    with open(file_path, 'r') as config_file:
        return json.load(config_file)

# Execute a function in all subfolders of the given folder (not recursive)
def execute_in_subfolders(root_folder, function, filter=None):
    # Remember the current working directory
    current_folder = os.getcwd()

    # Change working directory to the root folder
    os.chdir(root_folder)

    # Execute the function in each subfolder (not recursive)
    try:
        for item in os.listdir():
            if os.path.isdir(item) and (filter is None or item in filter):
                try:
                    os.chdir(item)
                    function()
                finally:
                    # we need to change back to the root folder otherwise the isdir check will fail
                    os.chdir(root_folder)
    finally:
        # Change working directory back to the current folder
        os.chdir(current_folder)

# Search the Cargo.toml file for external crates and clone them
def clone_external_crates(target_folder, config, main_repository_folder_name):
    print("Searching external crates to clone...")

    external_crates_configs = config["clone"]["external_crates"]

    # Compile the regex patterns for every repo in external_crates_configs
    for external_crates_name in external_crates_configs:
        external_crate_config = external_crates_configs[external_crates_name]
        external_crate_config["pattern"] = re.compile("git = \"" + external_crate_config["repo"] + "\"")

    # find all external crates to clone including their revision
    external_crates_to_clone = {}
    
    # Find all Cargo.toml files in the target folder
    for root, dirs, files in os.walk(os.path.join(target_folder, main_repository_folder_name)):
        for file_name in files:
            if file_name != "Cargo.toml":
                continue

            file_path = os.path.join(root, file_name)

            # Read the content of the Cargo.toml file
            content = None
            try:
                with open(file_path, 'r', encoding='utf-8') as file:
                    content = file.read()
            except UnicodeDecodeError:
                raise UnicodeError(f"file_path: {file_path}")

            content_new = ""
            for line in content.split("\n"):                
                for external_crates_name in external_crates_configs:
                    external_crate_config = external_crates_configs[external_crates_name]
                    
                    # check if the line contains the external crate repo
                    if not external_crate_config["pattern"].search(line):
                        continue

                    break_loop = False
                    for crate_name in external_crate_config["crates"]:
                        # we found a match, now we need to check if the line starts with the crate name
                        if not line.startswith(crate_name + " ="):
                            continue

                        # we found a match, now we need to extract the revision, only match a valid git revision
                        match = re.search(r'rev = "([A-Za-z0-9]*)"', line)
                        if not match:
                            raise ValueError(f"Revision not found for external crate: {crate_name}")
                        revision = match.group(1)
                        
                        # add the external crate to the list
                        if external_crates_name in external_crates_to_clone:
                            if external_crates_to_clone[external_crates_name]["revision"] != revision:
                                raise ValueError(f"External crate already exists with different revision: {external_crates_name}")
                        
                        external_crates_to_clone[external_crates_name] = {
                            "revision": revision,
                        }

                        crate_config = external_crate_config["crates"][crate_name]

                        # determine the relative path to the external crate if the external crate is in the parent folder of the target folder
                        relative_path = os.path.join(os.path.relpath(os.path.join(root, ".."), os.path.dirname(file_path)), external_crates_name)

                        if "additional_path" in crate_config:
                            relative_path = os.path.join(relative_path, crate_config["additional_path"])
                        
                        print(f"   Found external crate '{external_crates_name}' with revision '{revision}' in '{relative_path}'")

                        # replace the dependency path with the local path (remove the git and rev)
                        line = re.sub(r'git = "[^"]*", ', "", re.sub(r'rev = "([A-Za-z0-9]*)"', f"path = \"{relative_path}\"", line))
                        
                        content_new += line + "\n"

                        # break the loop, we found the external crate
                        break_loop = True
                        break
                    
                    if break_loop:
                        break
                else:
                    # no match found, just add the line without modification
                    content_new += line + "\n"

            # write the modified Cargo.toml    
            with open(file_path, 'w', encoding='utf-8') as file:
                file.write(content_new)

    # Clone the external crates
    for external_crates_name in external_crates_to_clone:
        print(f"   Cloning external crate '{external_crates_name}'...")

        external_crate_folder = os.path.join(target_folder, external_crates_name)

        # Remember the current working directory
        current_folder = os.getcwd()

        try:
            # Clone the external crate (this will change the working directory)
            clone_repo(external_crates_configs[external_crates_name]["repo"], external_crates_to_clone[external_crates_name]["revision"], False, external_crate_folder, [], [], [])
        finally:
            # Change working directory back to the original folder
            os.chdir(current_folder)

# Delete specified crates
def delete_crates(crates):
    print("Deleting crates...")

    for crate in crates:
        crate_folder = f"crates/{crate}"
        if os.path.exists(crate_folder) and os.path.isdir(crate_folder):
            print(f"   Deleting crate folder: {crate_folder}")
            shutil.rmtree(crate_folder)
    
    cargo_toml_content = None
    with open('Cargo.toml', 'r') as file:
        cargo_toml_content = file.readlines()

    with open('Cargo.toml', 'w') as file:
        for line in cargo_toml_content:
            stripped_line = line.strip()
            
            for crate in crates:
                if crate in stripped_line:
                    print(f"   Deleting line from Cargo.toml: {stripped_line}")
                    
                    # Skip the line
                    break
            else:
                file.write(line)

    cargo_lock_content = None
    with open('Cargo.lock', 'r') as file:
        cargo_lock_content = file.readlines()

    with open('Cargo.lock', 'w') as file:
        in_package_section = False
        package_line_added = True
        skip_section = False

        for line in cargo_lock_content:
            # Detect the start of a new package section
            if line.strip() == "[[package]]":
                in_package_section = True
                package_line_added = False
                continue

            # If in a package section, check if the crate name matches
            if in_package_section and line.strip().startswith("name ="):
                for crate in crates:
                    if crate in line:
                        print(f"   Deleting package from Cargo.lock: {crate}")
                        skip_section = True
                        break
                
            # If we've reached the end of the package section, reset the flags
            if in_package_section and line.strip() == "":
                in_package_section = False
                package_line_added = True
                skip_section = False
                file.write(line)
                continue

            # Only write the line if we're not skipping this section
            if not skip_section:
                if not package_line_added:
                    file.write("[[package]]\n")
                    package_line_added = True
                
                file.write(line)

# Delete specified folders
def delete_folders(folders, verbose):
    for folder in folders:
        if not os.path.exists(folder):
            raise FileNotFoundError(f"Folder not found: {folder}")
        
        if verbose:
            print(f"   Deleting folder: {folder}")
        
        shutil.rmtree(folder)

# Delete specified files
def delete_files(files, verbose):
    for file in files:
        if not os.path.exists(file):
            raise FileNotFoundError(f"File not found: {file}")
        
        if verbose:
            print(f"   Deleting file: {file}")
        
        os.remove(file)

# Apply path renames
def apply_path_renames(ignored_folders, ignored_files, ignored_file_types, rename_patterns, verbose):
    print("Renaming paths...")

    # Compile the regex patterns for ignored folders, files, and file types
    ignored_folders = [re.compile(pattern) for pattern in ignored_folders]
    ignored_files = [re.compile(pattern) for pattern in ignored_files]
    ignored_file_types = [re.compile(pattern) for pattern in ignored_file_types]

    def compile_ignore_pattern(ignored):
        return {    
            "files": [re.compile(pattern) for pattern in ignored.get("files", [])],
        }
   
    patterns = [{
        "regex": re.compile(pattern["regex"]),
        "replacement": pattern["replacement"],
        "ignore": compile_ignore_pattern(pattern.get("ignore", {
            "files": [],
        }))
    } for pattern in rename_patterns]

    # Helper function to check if a file is tracked by Git
    def is_file_tracked(file_path):
        try:
            # Check if the file is tracked by Git
            subprocess.run(['git', 'ls-files', '--error-unmatch', file_path], check=True, stderr=subprocess.DEVNULL)
            return True
        except subprocess.CalledProcessError:
            return False

    # Apply renames within code files based on regex
    for root, dirs, files in os.walk("."):
        # Skip the entire directory if the root matches any ignored folder pattern
        if any(pattern.search(root) for pattern in ignored_folders):
            print(f"   Skipping directory (regex): {root}")
            dirs.clear()    # Don't walk into the directory if it should be ignored
            continue

        # Ignore specified subfolders using regex patterns
        dirs[:] = [d for d in dirs if not any(pattern.search(os.path.relpath(os.path.join(root, d), os.getcwd())) for pattern in ignored_folders)]

        for file_name in files:
            # Ignore specified file types using regex patterns
            if any(pattern.search(os.path.splitext(file_name)[1]) for pattern in ignored_file_types):
                continue

            # Ignore specified files using regex patterns
            if any(pattern.search(file_name) for pattern in ignored_files):
                print(f"   Skipping file (regex): {file_name}")
                continue
            
            file_path = os.path.join(root, file_name)
            
            # Apply rename patterns
            new_path = file_path

            for pattern in patterns:
                # Skip the pattern if the file is in the ignore list
                if any(ignore.search(new_path) for ignore in pattern["ignore"]["files"]):
                    continue

                new_path = pattern["regex"].sub(pattern["replacement"], new_path)
            
            if new_path == file_path:
                continue

            if verbose:
                print(f"   Renaming: {file_path} -> {new_path}")

            os.makedirs(os.path.dirname(new_path), exist_ok=True)
            try:
                subprocess.run(['git', 'mv', file_path, new_path], check=True, stderr=subprocess.DEVNULL)
            except subprocess.CalledProcessError:
                if not is_file_tracked(file_path):
                    print(f"   Skipping file (not tracked by git): {file_path}")
                else:
                    raise Exception(f"Failed to rename file: {file_path} -> {new_path}")
        
        # After moving files, check if the directory is empty
        if not os.listdir(root):
            if verbose:
                print(f"   Directory is empty, deleting: {root}")
            os.rmdir(root)

            # Check if the parent directory is empty
            parent_dir = os.path.dirname(root)
            while not os.listdir(parent_dir):
                if verbose:
                    print(f"   Parent directory is empty, deleting: {parent_dir}")
                os.rmdir(parent_dir)        # This will panic if the parent dir is not empty, so we are safe
                parent_dir = os.path.dirname(parent_dir)

    # We need to fix symlinks after moving files
    for root, dirs, files in os.walk("."):
        for file_name in files:
            file_path = os.path.join(root, file_name)

            # Check if the file is a symbolic link
            if not os.path.islink(file_path):
                continue

            # Get the target of the symbolic link
            target = os.readlink(file_path)
            target_root = os.path.dirname(target)
            target_file_name = os.path.basename(target)

            # Check if the target might have been skipped during renaming because of the ignored folders patterns
            if any(pattern.search(target_root) for pattern in ignored_folders):
                print(f"   Symbolic links: Skipping directory (regex): {target_root}")
                continue

            # Check if the target might have been skipped during renaming because of the ignored files patterns
            if any(pattern.search(target_file_name) for pattern in ignored_files):
                print(f"   Symbolic links: Skipping file (regex): {target}")
                continue
            
            # Check if the target might have been skipped during renaming because of the ignored file types patterns
            if any(pattern.search(os.path.splitext(target_file_name)[1]) for pattern in ignored_file_types):
                continue

            # Apply rename patterns
            new_target = target

            for pattern in patterns:
                # Skip the pattern if the file is in the ignore list
                if any(ignore.search(new_target) for ignore in pattern["ignore"]["files"]):
                    continue

                new_target = pattern["regex"].sub(pattern["replacement"], new_target)

            # Skip the symbolic link if the target has not changed
            if new_target == target:
                continue
            
            # Update the symbolic link if the target has changed
            os.remove(file_path)
            os.symlink(new_target, file_path)

            print(f"   Updated symlink: {file_path}: {target} -> {new_target}")

# Apply code renames
def apply_code_renames(ignored_folders, ignored_files, ignored_file_types, rename_patterns, verbose):
    print("Applying code renames...")

    # Compile the regex patterns for ignored folders, files, and file types
    ignored_folders = [re.compile(pattern) for pattern in ignored_folders]
    ignored_files = [re.compile(pattern) for pattern in ignored_files]
    ignored_file_types = [re.compile(pattern) for pattern in ignored_file_types]
    
    def compile_ignore_pattern(ignored):
        return {    
            "files": [re.compile(pattern) for pattern in ignored.get("files", [])],
        }
   
    patterns = [{
        "regex": re.compile(pattern["regex"], re.MULTILINE),
        "replacement": pattern["replacement"],
        "ignore": compile_ignore_pattern(pattern.get("ignore", {
            "files": [],
        }))
    } for pattern in rename_patterns]
    
    # Apply renames within code files based on regex
    for root, dirs, files in os.walk("."):
        # Skip the entire directory if the root matches any ignored folder pattern
        if any(pattern.search(root) for pattern in ignored_folders):
            print(f"   Skipping directory (regex): {root}")
            dirs.clear()    # Don't walk into the directory if it should be ignored
            continue

        # Ignore specified subfolders using regex patterns
        dirs[:] = [d for d in dirs if not any(pattern.search(os.path.relpath(os.path.join(root, d), os.getcwd())) for pattern in ignored_folders)]

        for file_name in files:
            # Ignore specified file types using regex patterns
            if any(pattern.search(os.path.splitext(file_name)[1]) for pattern in ignored_file_types):
                continue

            file_path = os.path.join(root, file_name)
            
            # Ignore specified files using regex patterns
            if any(pattern.search(file_path) for pattern in ignored_files):
                print(f"   Skipping file (regex): {file_path}")
                continue
            
            # Read the file content
            content = None
            try:
                with open(file_path, 'r', encoding='utf-8') as file:
                    content = file.read()
            except UnicodeDecodeError:
                raise UnicodeError(f"file_path: {file_path}")

            # Apply rename patterns
            for pattern in patterns:
                # Skip the pattern if the file is in the ignore list
                if any(ignore.search(file_name) for ignore in pattern["ignore"]["files"]):
                    continue

                content = pattern["regex"].sub(pattern["replacement"], content)
            
            if verbose:
                print(f"   Renaming code in file: {file_path}")

            # Write the modified content back to the file
            with open(file_path, 'w', encoding='utf-8') as file:
                file.write(content)

# Skip an entry based on the min_sem_version and max_sem_version in the config
def skip_entry_by_version(sem_version, config_entry, name):
    if not config_entry:
        return False
    
    min_sem_version = config_entry.get("min_sem_version", None)
    if min_sem_version:
        min_sem_version = extract_sem_version(min_sem_version)
    
    if min_sem_version and sem_version < min_sem_version:
        print(f"   Skipping entry because min version not reached: \"{name}\" - current: {sem_version} < min: {min_sem_version}")
        return True
    
    max_sem_version = config_entry.get("max_sem_version", None)
    if max_sem_version:
        max_sem_version = extract_sem_version(max_sem_version)

    if max_sem_version and sem_version > max_sem_version:
        print(f"   Skipping entry because max version exceeded: \"{name}\" - current: {sem_version} > max: {max_sem_version}")
        return True
    
    return False

# Copy and overwrite files listed in the config
def copy_overwrites(script_folder, sem_version, overwrites_config):
    print("Copying overwrites...")

    for overwrite_config in overwrites_config:
        # Skip the overwrite if it should be skipped based on the config
        if skip_entry_by_version(sem_version, overwrite_config, overwrite_config["source"]):
            continue    

        source = os.path.abspath(os.path.expanduser(os.path.join(script_folder, overwrite_config["source"])))
        target = overwrite_config["destination"]

        if ("overwrite_only" in overwrite_config) and (overwrite_config.get("overwrite_only", False)):
            if not os.path.exists(target):
                print(f"   Skipping overwrite because target does not exist: {target}")
                continue

        if os.path.isdir(source):
            if os.path.exists(target):
                print(f"   Deleting existing directory: {target}")
                shutil.rmtree(target)

            print(f"   Copying directory: {source} -> {target}")

            # Copy the local folder to the target folder
            shutil.copytree(
                source,
                target,
                symlinks=True
            )
        else:
            print(f"   Copying file: {source} -> {target}")
            if not os.path.exists(source) and ("skip_on_missing" in overwrite_config) and (overwrite_config.get("skip_on_missing", False)):
                print(f"   File not found but skip_on_missing is set, skipping: {source}")
                continue

            # Copy the file to the target
            shutil.copy2(source, target)

# Apply all git patches from patches_config to the repository
def apply_git_patches(current_folder, patches_config, sem_version):
    print("Applying git patch files...")

    # Apply each patch file in the patches_config
    for patch_name in patches_config:
        patch_config = patches_config.get(patch_name)
        
        # Skip the patch if it should be skipped based on the config
        if skip_entry_by_version(sem_version, patch_config, patch_name):
            continue

        if "depends_on" in patch_config:
            depends_on = patch_config.get("depends_on")
            if not os.path.exists(depends_on):
                print(f"   Skipping patch {patch_name} because it depends on '{depends_on}' which is not found.")
                continue
        
        print(f"   Applying patch: {patch_name}")
        patch_path = os.path.join(current_folder, patch_config.get("path"))
        subprocess.run(['git', 'apply', '-C2', '--verbose', patch_path], check=True)

# Run fix typos
def run_fix_typos(panic_on_errors):
    print("Running fix typos...")
    # We won't check the return code because not all typos might be fixable
    subprocess.run(["typos", "--write-changes"], check=panic_on_errors)

# Run cargo fmt
def run_cargo_fmt(panic_on_errors):
    print("Running cargo fmt...")
    subprocess.run(["cargo", "+nightly", "fmt"], check=panic_on_errors)

# Run dprint fmt
def run_dprint_fmt(panic_on_errors):
    print("Running dprint fmt...")
    subprocess.run(["dprint", "fmt"], check=panic_on_errors)

# Prepare the Docker container for running turborepo
def prepare_docker_turborepo(script_folder):
    rust_toolchain_version = parse_rust_toolchain_version()

    # Remember the current working directory
    current_folder = os.getcwd()

    # Change working directory to the script folder
    os.chdir(script_folder)

    print("   building Docker container for turborepo...")
    try:
        # Run the "docker_turborepo/build.sh" script
        # cmd: docker build --build-arg RUST_VERSION=1.79 -t turborepo-image -f ./docker_turborepo/Dockerfile .
        subprocess.run(["docker", "build", "--build-arg", f"RUST_VERSION={rust_toolchain_version}", "-t", "turborepo-image", "-f", "./docker_turborepo/Dockerfile", "."], check=True)
    finally:
        # Change working directory back to the current folder
        os.chdir(current_folder)

    # Run the docker command for installing dependencies
    print("   run install dependencies...")
    # cmd: docker run --rm --name turborepo -v {repo_folder}:/home/node/app  --user 1000:1000 turborepo-image sh -c "pnpm i"
    subprocess.run([
        "docker", "run", "--rm",
        "--name", "turborepo",
        "-v", f"{current_folder}:/home/node/app",
        "--user", f"1000:1000",
        "turborepo-image",
        "sh", "-c", "pnpm i"
    ], check=True)

    print("   run install of additional dependencies...")
    # cmd: docker run --rm --name turborepo -v {repo_folder}:/home/node/app  --user 1000:1000 turborepo-image sh -c "pnpm i"
    subprocess.run([
        "docker", "run", "--rm",
        "--name", "turborepo",
        "-v", f"{current_folder}:/home/node/app",
        "--user", f"1000:1000",
        "turborepo-image",
        "sh", "-c", "pnpm add -w --save-dev eslint-config-next"
    ], check=True)

# Run pnpm prettier:fix using the turborepo docker container
def run_pnpm_prettier_fix(script_folder, panic_on_errors):
    print("Running pnpm prettier:fix...")

    current_folder = os.getcwd()

    prepare_docker_turborepo(script_folder)

    # Run the docker command for prettier:fix
    print("   run prettier:fix...")
    subprocess.run([
        "docker", "run", "--rm",
        "--name", "turborepo",
        "-v", f"{current_folder}:/home/node/app",
        "--user", f"1000:1000",
        "turborepo-image",
        "sh", "-c", "pnpm turbo prettier:fix"
    ], check=panic_on_errors)

# Run pnpm lint:fix using the turborepo docker container
def run_pnpm_lint_fix(script_folder, panic_on_errors):
    print("Running pnpm lint:fix...")

    current_folder = os.getcwd()

    prepare_docker_turborepo(script_folder)

    # Run the docker command for building
    print("   run turbo build...")
    subprocess.run([
        "docker", "run", "--rm",
        "--name", "turborepo",
        "-v", f"{current_folder}:/home/node/app",
        "--user", f"1000:1000",
        "turborepo-image",
        "sh", "-c", "pnpm turbo build"
    ], check=panic_on_errors)

    # Run the docker command for lint:fix
    print("   run turbo lint:fix...")
    subprocess.run([
        "docker", "run", "--rm",
        "--name", "turborepo",
        "-v", f"{current_folder}:/home/node/app",
        "--user", f"1000:1000",
        "turborepo-image",
        "sh", "-c", "pnpm turbo lint:fix"
    ], check=panic_on_errors)

# Run all shell commands
def run_shell_commands(commands):
    print(f"Applying shell commands in {0}...", os.getcwd())
    for command in commands:
        subprocess.run(command, shell=True, check=True)

# Run cargo clippy
def run_cargo_clippy(panic_on_errors):
    print("Running cargo clippy...")
    subprocess.run(["cargo", "clippy", "--fix"], check=panic_on_errors)

# Revert all git patches from patches_config that have the config entry set to revert
def revert_git_patches(current_folder, patches_config, sem_version):
    print("Reverting git patch files...")

    # Revert each patch file in the patches_config that has the config entry set to revert
    for patch_name in patches_config:
        patch_config = patches_config.get(patch_name)

        if not patch_config.get("revert", False):
            # not set to revert, skip
            continue

        # Skip the patch if it should be skipped based on the config
        if skip_entry_by_version(sem_version, patch_config, patch_name):
            continue

        if "depends_on" in patch_config:
            depends_on = patch_config.get("depends_on")
            if not os.path.exists(depends_on):
                print(f"   Skip reverting patch {patch_name} because it depends on '{depends_on}' which is not found.")
                continue
        
        print(f"   Reverting patch: {patch_name}")
        patch_path = os.path.join(current_folder, patch_config.get("path"))
        subprocess.run(['git', 'apply', '-R', '-C2', '--verbose', patch_path], check=True)

# Recompile the framework system packages and bytecode snapshots
def recompile_framework_packages(verbose):
    print("Recompiling framework system packages and bytecode snapshots...")

    # Delete the existing framework-snapshot manifest file
    delete_files(["crates/iota-framework-snapshot/manifest.json"], verbose)

    # Delete the existing framework-snapshot bytecode_snapshot folders
    delete_folders(["crates/iota-framework-snapshot/bytecode_snapshot"], verbose)

    # Run the cargo build command
    subprocess.run(["cargo", "build"], check=True)
    
    # Rebuild the framework system packages
    subprocess.run(["cargo", "test", "-p", "iota-framework", "--test", "build-system-packages"], env=dict(os.environ, UPDATE="1"), check=True)

    # Rebuild the framework bytecode snapshots
    subprocess.run(["cargo", "run", "--bin", "iota-framework-snapshot"], check=True)

# Commit changes
def commit_changes(commit_message):
    print(f"Committing changes... \"{commit_message}\"")

    # Add all changes to the staging area
    subprocess.run(["git", "add", "."], check=True)

    # Check if there are any staged changes
    result = subprocess.run(["git", "diff", "--cached", "--quiet"])
    
    # If there are staged changes, commit them
    if result.returncode != 0:
        subprocess.run(["git", "commit", "-q", "-m", commit_message], check=True)
    else:
        print("   No changes to commit.")

################################################################################
if __name__ == "__main__":
    # Argument parser setup
    parser = argparse.ArgumentParser(description="Use the slipstream to catch up...")
    parser.add_argument('--config', default="config.json", help="The path to the configuration file.")
    parser.add_argument('--verbose', action='store_true', help="Print verbose output.")
    parser.add_argument('--repo-url', default="git@github.com:MystenLabs/sui.git", help="The URL to the repository. Can also be a local folder.")
    parser.add_argument('--repo-tag', default="mainnet-v1.22.0", help="The tag to checkout in the repository.")
    parser.add_argument('--version', default=None, help="The semantic version to filter overwrites/patches if not found in the repo-tag.")
    parser.add_argument('--target-folder', default="result", help="The path to the target folder.")
    parser.add_argument('--main-repository-folder-name', default="main", help="The name of the main repository folder (subfolder of target-folder).")
    parser.add_argument('--target-branch', default=None, help="The branch to create and checkout in the target folder.")
    parser.add_argument('--commit-between-steps', action='store_true', help="Create a commit between each step.")
    parser.add_argument('--panic-on-linter-errors', action='store_true', help="Panic on linter errors (typos, cargo fmt, dprint, pnpm lint, cargo clippy).")
    parser.add_argument('--clone-source', action='store_true', help="Clone the upstream repository.")
    parser.add_argument('--clone-history', action='store_true', help="Clone the complete history of the upstream repository.")
    parser.add_argument('--create-branch', action='store_true', help="Create a new branch in the target folder.")
    parser.add_argument('--delete', action='store_true', help="Delete files or folders based on the rules in the config.")
    parser.add_argument('--apply-path-renames', action='store_true', help="Apply path renames based on the rules in the config.")
    parser.add_argument('--apply-code-renames', action='store_true', help="Apply code renames based on the rules in the config.")
    parser.add_argument('--copy-overwrites', action='store_true', help="Copy and overwrite files listed in the config.")
    parser.add_argument('--apply-patches', action='store_true', help="Apply git patches from the patches folder.")
    parser.add_argument('--run-fix-typos', action='store_true', help="Run script to fix typos.")
    parser.add_argument('--run-cargo-fmt', action='store_true', help="Run cargo fmt.")
    parser.add_argument('--run-dprint-fmt', action='store_true', help="Run dprint fmt.")
    parser.add_argument('--run-pnpm-prettier-fix', action='store_true', help="Run pnpm prettier:fix.")
    parser.add_argument('--run-pnpm-lint-fix', action='store_true', help="Run pnpm lint:fix.")
    parser.add_argument('--run-shell-commands', action='store_true', help="Run shell commands listed in the config.")
    parser.add_argument('--run-cargo-clippy', action='store_true', help="Run cargo clippy.")
    parser.add_argument('--recompile-framework-packages', action='store_true', help="Recompile the framework system packages and bytecode snapshots.")
    
    # get the folder the script is in
    script_folder = os.path.dirname(os.path.realpath(__file__))

    # get the target folder
    args = parser.parse_args()
    target_folder = args.target_folder
    target_folder = os.path.abspath(os.path.expanduser(target_folder))

    # get the current working directory
    workdir_at_start = os.getcwd()

    # Check if the repository URL and tag are set
    if args.clone_source or args.copy_overwrites or args.apply_patches:
        if args.repo_url is None or args.repo_url == "":
            raise ValueError("The repository URL must be set.")
        if args.repo_tag is None or args.repo_tag == "":
            raise ValueError("The repository tag must be set.")

    # Get the current version from the tag
    sem_version = None
    if args.copy_overwrites or args.apply_patches:
        try:
            sem_version = extract_sem_version(args.repo_tag)
        except ValueError as e:
            try:
                sem_version = extract_sem_version(args.version)
            except:
                print(f"Version not found in tag: \"{args.repo_tag}\", please provide a valid \"--version\" argument.")
                exit(1)
    
    # Load the configuration
    config = load_slipstream_config(args.config)
    
    if args.clone_source:
        # remove the target folder if it exists
        if os.path.exists(target_folder):
            shutil.rmtree(target_folder)

        # Clone the repository
        clone_repo(
            args.repo_url,
            args.repo_tag,
            args.clone_history,
            os.path.join(target_folder, args.main_repository_folder_name),        # clone the main repository into the "main" folder
            config["clone"]["ignore"]["folders"],
            config["clone"]["ignore"]["files"],
            config["clone"]["ignore"]["file_types"],
        )
        
        # Clone external crates if needed
        clone_external_crates(target_folder, config=config, main_repository_folder_name=args.main_repository_folder_name)
    
    # Change working directory to the target folder
    os.chdir(target_folder)
    
    if args.create_branch:
        # Check if the target branch was set, if not, panic
        if args.target_branch is None:
            raise ValueError("The target branch argument must be set if a new branch should be created.")

        # Create a new branch
        execute_in_subfolders(target_folder, lambda: subprocess.run(["git", "checkout", "-b", args.target_branch], check=True))
    
    if args.delete:
        def delete_func():
            # Delete specified crates
            delete_crates(config["deletions"]["crates"])

            # Delete specified folders
            print("Deleting folders...")
            delete_folders(config["deletions"]["folders"], args.verbose)

            # Delete specified files
            print("Deleting files...")
            delete_files(config["deletions"]["files"], args.verbose)

            if args.commit_between_steps:
                commit_changes("fix: deleted unused folders and files")    

        # Execute the delete function in the main folder
        execute_in_subfolders(target_folder, delete_func, filter=[args.main_repository_folder_name])

    if args.apply_path_renames:
        def path_renames_func():
            # Apply path renames
            apply_path_renames(
                config["path_renames"]["ignore"]["folders"],
                config["path_renames"]["ignore"]["files"],
                config["path_renames"]["ignore"]["file_types"],
                config["path_renames"]["patterns"],
                args.verbose,
            )

            if args.commit_between_steps:
                commit_changes("fix: renamed paths")

        # Execute the path renames function in all subfolders
        execute_in_subfolders(target_folder, path_renames_func)
    
    if args.apply_code_renames:
        def code_renames_func():
            # Apply code renames
            apply_code_renames(
                config["code_renames"]["ignore"]["folders"],
                config["code_renames"]["ignore"]["files"],
                config["code_renames"]["ignore"]["file_types"],
                config["code_renames"]["patterns"],
                args.verbose,
            )

            if args.commit_between_steps:
                commit_changes("fix: renamed code")
        
        # Execute the code renames function in all subfolders
        execute_in_subfolders(target_folder, code_renames_func)

    if args.copy_overwrites:
        def copy_overwrites_func():
            # Copy and overwrite files listed in the config
            copy_overwrites(script_folder, sem_version, config["overwrites"])

            if args.commit_between_steps:
                commit_changes("fix: copied overwrites")
        
        # Execute the copy overwrites function in the main folder
        execute_in_subfolders(target_folder, copy_overwrites_func, filter=[args.main_repository_folder_name])

    if args.apply_patches:
        def apply_patches_func():
            # Apply git patch files
            apply_git_patches(workdir_at_start, config["patches"], sem_version)

            if args.commit_between_steps:
                commit_changes("fix: applied patches")
        
        # Execute the apply patches function in the main folder
        execute_in_subfolders(target_folder, apply_patches_func, filter=[args.main_repository_folder_name])
        
    if args.run_fix_typos:
        def fix_typos_func():
            run_fix_typos(args.panic_on_linter_errors)

            if args.commit_between_steps:
                commit_changes("fix: ran typos")
        
        # Execute the fix typos function in the main folder
        execute_in_subfolders(target_folder, fix_typos_func, filter=[args.main_repository_folder_name])

    if args.run_cargo_fmt:
        def cargo_fmt_func():
            run_cargo_fmt(args.panic_on_linter_errors)

            if args.commit_between_steps:
                commit_changes("fix: ran cargo fmt")
        
        # Execute the cargo fmt function in the main folder
        execute_in_subfolders(target_folder, cargo_fmt_func, filter=[args.main_repository_folder_name])

    if args.run_dprint_fmt:
        def dprint_fmt_func():
            run_dprint_fmt(args.panic_on_linter_errors)
        
            if args.commit_between_steps:
                commit_changes("fix: ran dprint fmt")
        
        # Execute the dprint fmt function in the main folder
        execute_in_subfolders(target_folder, dprint_fmt_func, filter=[args.main_repository_folder_name])


    if args.run_pnpm_prettier_fix:
        def pnpm_prettier_fix_func():
            run_pnpm_prettier_fix(script_folder, args.panic_on_linter_errors)

            if args.commit_between_steps:
                commit_changes("fix: ran pnpm prettier:fix")

        # Execute the pnpm prettier:fix function in the main folder
        execute_in_subfolders(target_folder, pnpm_prettier_fix_func, filter=[args.main_repository_folder_name])

    if args.run_pnpm_lint_fix:
        def pnpm_lint_fix_func():
            run_pnpm_lint_fix(script_folder, args.panic_on_linter_errors)

            if args.commit_between_steps:
                commit_changes("fix: ran pnpm lint:fix")
        
        # Execute the pnpm lint:fix function in the main folder
        execute_in_subfolders(target_folder, pnpm_lint_fix_func, filter=[args.main_repository_folder_name])

    if args.run_shell_commands:
        def shell_commands_func():
            run_shell_commands(config["commands"])

            if args.commit_between_steps:
                commit_changes("fix: ran additional shell commands")
        
        # Execute the shell commands function in the main folder
        execute_in_subfolders(target_folder, shell_commands_func, filter=[args.main_repository_folder_name])
    
    if args.run_cargo_clippy:
        def cargo_clippy_func():
            run_cargo_clippy(args.panic_on_linter_errors)

            if args.commit_between_steps:
                commit_changes("fix: ran cargo clippy")
        
        # Execute the cargo clippy function in the main folder
        execute_in_subfolders(target_folder, cargo_clippy_func, filter=[args.main_repository_folder_name])

    if args.apply_patches:
        def revert_patches_func():
            # Revert git patch files
            revert_git_patches(workdir_at_start, config["patches"], sem_version)

            if args.commit_between_steps:
                commit_changes("fix: reverted patches")
        
        # Execute the revert patches function in the main folder
        execute_in_subfolders(target_folder, revert_patches_func, filter=[args.main_repository_folder_name])

    if args.recompile_framework_packages:
        def recompile_framework_packages_func():
            # Recompile the framework system packages and bytecode snapshots
            recompile_framework_packages(args.verbose)

            if args.commit_between_steps:
                commit_changes("fix: recompiled framework system packages and bytecode snapshots")
        
        # Execute the recompile framework packages function in the main folder
        execute_in_subfolders(target_folder, recompile_framework_packages_func, filter=[args.main_repository_folder_name])
