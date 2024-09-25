import os, re, json, shutil, subprocess, argparse, semver

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

# Clone a repository (either from a URL or a local folder)
def clone_repo(repo_url, repo_tag, clone_history, target_folder, ignored_folders, ignored_files, ignored_file_types):
    print(f"Cloning '{repo_url}' with tag '{repo_tag}' to '{target_folder}'...")
    repo_url_exp = os.path.expanduser(repo_url)

    # Check if the repository is a git repository or a local folder
    if os.path.exists(repo_url_exp):
        # Fetch the latest changes
        subprocess.run(["git", "fetch", "--all"], cwd=repo_url_exp, check=True)

        # Checkout the tag in the source folder
        subprocess.run(["git", "checkout", repo_tag], cwd=repo_url_exp, check=True)

        # helper function to check if the current reference is following a branch
        def is_following_branch():
            # Run the git command to check the current reference
            result = subprocess.run(
                ['git', 'rev-parse', '--abbrev-ref', 'HEAD'],
                cwd=repo_url_exp,
                capture_output=True,
                text=True,
                check=True
            )
            # If the result is 'HEAD', we're not following a branch (detached state or similar)
            return result.stdout.strip() != "HEAD"

        # Pull the latest changes
        if is_following_branch():
            print("   Pulling latest changes...")
            subprocess.run(["git", "pull"], cwd=repo_url_exp, check=True)

        # Check if the local folder equals the target folder
        if os.path.abspath(repo_url_exp) != os.path.abspath(target_folder):

            # Compile the regex patterns for ignored folders, files, and file types
            ignored_folders = [re.compile(pattern) for pattern in ignored_folders]
            ignored_files = [re.compile(pattern) for pattern in ignored_files]
            ignored_file_types = [re.compile(pattern) for pattern in ignored_file_types]

            # helper function to filter ignored items
            def filter_ignored_items(dir, contents):
                dir = os.path.relpath(dir, os.path.abspath(repo_url_exp))

                if any(pattern.search(dir) for pattern in ignored_folders):
                    print(f"   Skipping directory (regex): {dir}")
                    return contents
                
                ignored = []

                # Check if the item is a folder and matches any ignored folder pattern
                for item_name in contents:
                    item_path = os.path.join(dir, item_name)

                    if os.path.isdir(item_path):
                        if any(pattern.search(item_path) for pattern in ignored_folders):
                            print(f"   Skipping directory (regex): {item_path}")
                            ignored.append(item_name)
                            continue
                    else:
                        # Ignore specified file types using regex patterns
                        if any(pattern.search(os.path.splitext(item_name)[1]) for pattern in ignored_file_types):
                            print(f"   Skipping file (type regex): {item_path}")
                            ignored.append(item_name)
                            continue

                        # Ignore specified files using regex patterns
                        if any(pattern.search(item_name) for pattern in ignored_files):
                            print(f"   Skipping file (regex): {item_path}")
                            ignored.append(item_name)
                            continue

                return ignored

            # Copy the local folder to the target folder
            shutil.copytree(
                repo_url_exp,
                target_folder,
                ignore=filter_ignored_items,
                symlinks=True
            )
    else:
        # Clone the repository, the tag can be used as the branch name directly to checkout a specific tag in one step
        cmd = ["git", "clone"]
        if not clone_history:
            cmd += ["--depth", "1"]
        cmd += ["--single-branch", "--branch", repo_tag, repo_url, target_folder]

        subprocess.run(cmd, check=True)

    # Change working directory to the cloned repo
    os.chdir(target_folder)

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

    for overwrite in overwrites_config:
        # Skip the overwrite if it should be skipped based on the config
        if skip_entry_by_version(sem_version, overwrite, overwrite["source"]):
            continue    

        source = os.path.abspath(os.path.expanduser(os.path.join(script_folder, overwrite["source"])))
        target = overwrite["destination"]

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

            # Copy the file to the target
            shutil.copy2(source, target)

# Apply all git patches from patches_folder to the repository
def apply_git_patches(patches_folder, sem_version, patches_config):
    print("Applying git patch files...")

    # Apply each patch file in the patches_folder
    for patch_file in os.listdir(patches_folder):
        if patch_file.endswith('.patch'):
            patch_name = os.path.splitext(patch_file)[0]

            # Skip the patch if it should be skipped based on the config
            if skip_entry_by_version(sem_version, patches_config.get(patch_name, None), patch_file):
                continue    

            patch_path = os.path.join(patches_folder, patch_file)
            print(f"   Applying patch: {patch_file}")
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

# Parse the rust-toolchain.toml file to get the Rust version
def parse_rust_toolchain_version():
    try:
        content = None
        with open('rust-toolchain.toml', 'r') as file:
            content = file.read()

        # Regex to find the Rust version
        match = re.search(r'(?<=channel = ").*(?=")', content)
        if not match:
            raise Exception("Rust version not found in rust-toolchain.toml.")
        
        return match.group(0)

    except FileNotFoundError:
        raise FileNotFoundError("rust-toolchain.toml not found.")

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

    print("   run install of additional depedencies...")
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

# Revert all git patches from patches_folder that have the config entry set to revert
def revert_git_patches(patches_folder, sem_version, patches_config):
    print("Reverting git patch files...")

    # Revert each patch file in the patches_folder that has the config entry set to revert
    for patch_file in os.listdir(patches_folder):
        if patch_file.endswith('.patch'):
            patch_name = os.path.splitext(patch_file)[0]

            config_entry = patches_config.get(patch_name, None)
            if not config_entry:
                # no config entry found, skip
                continue

            if not config_entry.get("revert", False):
                # not set to revert, skip
                continue

            # Skip the patch if it should be skipped based on the config
            if skip_entry_by_version(sem_version, config_entry, patch_file):
                continue    

            patch_path = os.path.join(patches_folder, patch_file)
            print(f"   Reverting patch: {patch_file}")
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

# Open tool for comparison
def run_compare_tool(compare_tool_binary, compare_tool_arguments, source_folder, target_folder):
    print(f"Opening {compare_tool_binary} for comparison between {source_folder} and {target_folder}...")
    
    cmd = [compare_tool_binary]
    if compare_tool_arguments:
        cmd = cmd + compare_tool_arguments.split(" ")
    cmd = cmd + [source_folder, target_folder]
    
    subprocess.run(cmd)

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
    parser.add_argument('--target-branch', default=None, help="The branch to create and checkout in the target folder.")
    parser.add_argument('--patches-folder', default="patches", help="The path to the patches folder.")
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
    parser.add_argument('--compare-results', action='store_true', help="Open tool for comparison.")
    parser.add_argument('--compare-source-folder', help="The path to the source folder for comparison.")
    parser.add_argument('--compare-tool-binary', default="meld", help="The binary to use for comparison.")
    parser.add_argument('--compare-tool-arguments', default="", help="The arguments to use for comparison.")
    
    # get the folder the script is in
    script_folder = os.path.dirname(os.path.realpath(__file__))

    # get the target folder
    args = parser.parse_args()
    target_folder = args.target_folder
    target_folder = os.path.abspath(os.path.expanduser(target_folder))

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
    
    # get current root folder
    source_folder = os.path.abspath(os.path.join(os.getcwd(), "..", ".."))
    if args.compare_source_folder:
        source_folder = os.path.abspath(args.compare_source_folder)
    patches_folder = os.path.abspath(args.patches_folder)

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
            target_folder,
            config["clone"]["ignore"]["folders"],
            config["clone"]["ignore"]["files"],
            config["clone"]["ignore"]["file_types"],
        )
    else:
        # Change working directory to the target folder
        os.chdir(target_folder)

    if args.create_branch:
        # Check if the target branch was set, if not, panic
        if args.target_branch is None:
            raise ValueError("The target branch argument must be set if a new branch should be created.")

        # Create a new branch
        subprocess.run(["git", "checkout", "-b", args.target_branch], check=True)
    
    if args.delete:
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

    if args.apply_path_renames:
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
    
    if args.apply_code_renames:
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

    if args.copy_overwrites:
        copy_overwrites(script_folder, sem_version, config["overwrites"])

        if args.commit_between_steps:
            commit_changes("fix: copied overwrites")

    if args.apply_patches:
        # Apply git patch files
        apply_git_patches(patches_folder, sem_version, config["patches"])

        if args.commit_between_steps:
            commit_changes("fix: applied patches")

    if args.run_fix_typos:
        run_fix_typos(args.panic_on_linter_errors)

        if args.commit_between_steps:
            commit_changes("fix: ran typos")

    if args.run_cargo_fmt:
        run_cargo_fmt(args.panic_on_linter_errors)
        
        if args.commit_between_steps:
            commit_changes("fix: ran cargo fmt")

    if args.run_dprint_fmt:
        run_dprint_fmt(args.panic_on_linter_errors)
        
        if args.commit_between_steps:
            commit_changes("fix: ran dprint fmt")

    if args.run_pnpm_prettier_fix:
        run_pnpm_prettier_fix(script_folder, args.panic_on_linter_errors)

        if args.commit_between_steps:
            commit_changes("fix: ran pnpm prettier:fix")

    if args.run_pnpm_lint_fix:
        run_pnpm_lint_fix(script_folder, args.panic_on_linter_errors)

        if args.commit_between_steps:
            commit_changes("fix: ran pnpm lint:fix")

    if args.run_shell_commands:
        # Apply shell commands
        run_shell_commands(config["commands"])

        if args.commit_between_steps:
            commit_changes("fix: ran additional shell commands")
    
    if args.run_cargo_clippy:
        run_cargo_clippy(args.panic_on_linter_errors)
        
        if args.commit_between_steps:
            commit_changes("fix: ran cargo clippy")

    if args.apply_patches:
        # Revert git patch files
        revert_git_patches(patches_folder, sem_version, config["patches"])

        if args.commit_between_steps:
            commit_changes("fix: reverted patches")

    if args.recompile_framework_packages:
        # Recompile the framework system packages and bytecode snapshots
        recompile_framework_packages(args.verbose)
        
        if args.commit_between_steps:
            commit_changes("fix: recompiled framework system packages and bytecode snapshots")

    if args.compare_results:
        # Open tool for comparison
        run_compare_tool(args.compare_tool_binary, args.compare_tool_arguments, source_folder, target_folder)
