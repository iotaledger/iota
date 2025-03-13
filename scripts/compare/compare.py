# Copyright (c) 2024 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0
import os, sys, subprocess, argparse, tempfile
sys.path.append('../utils')
from codeowners import CodeOwners

def renamed_folders_mapping_func(str):
    premap = {
        'crates/iota-rpc-api': 'crates/iota-rest-api',
    }

    for key in premap:
        if key in str:
            return str.replace(key, premap[key])

    return str

def renamed_folders_mapping_func_reverse(str):
    premap = {
        'crates/iota-rest-api': 'crates/iota-rpc-api',
    }

    for key in premap:
        if key in str:
            return str.replace(key, premap[key])

    return str

# create symlinks in target for the matching folders of the code owners
def create_symlinks(source, target, code_owners, owners, ignore_folders):
    # check if the target directory already exists
    if os.path.exists(target):
        raise Exception(f"Target directory '{target}' already exists.")
    
    # get matching folders for the codeowners
    matching_paths = code_owners.match_paths_for_owners(source, owners)
    if not matching_paths:
        raise Exception("No matching folders found for the code owners.")
    
    # create target directory
    os.makedirs(target)

    # create symlinks for the matching folders
    for path in matching_paths:
        if ignore_folders and any(path.startswith(folder) for folder in ignore_folders):
            print(f"Ignoring folder '{path}'.")
            continue

        source_path = os.path.abspath(os.path.join(source, path))
        if not os.path.exists(source_path):
            # try to map the source path to a renamed folder
            source_path = renamed_folders_mapping_func_reverse(source_path)
        
        if not os.path.exists(source_path):
            print(f"WARNING: Folder '{source_path}' does not exist.")
            continue

        target_path = os.path.abspath(os.path.join(target, path))
        os.makedirs(os.path.dirname(target_path), exist_ok=True)
        os.symlink(source_path, target_path, target_is_directory=True)

    print(f"Symlinks created in '{target}'.")

# open tool for comparison
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
    parser = argparse.ArgumentParser(description="Repository comparison tool with optional code ownership filter.")
    parser.add_argument('--source-folder', required=True, help="The path to the source folder for comparison.")
    parser.add_argument('--target-folder', required=True, help="The path to the target folder for comparison.")
    parser.add_argument('--codeowners', nargs='+', help='Optionally filter folders by code owners (e.g., "@iotaledger/node @iotaledger/consensus").')
    parser.add_argument('--codeowners-file', default="", help="The path to the code owners file.")
    parser.add_argument('--compare-tool-binary', default="meld", help="The binary to use for comparison.")
    parser.add_argument('--compare-tool-arguments', default="", help="The arguments to use for comparison.")
    parser.add_argument('--ignore-folders', nargs='+', help='Optionally ignore folders (e.g., "node_modules").')
    args = parser.parse_args()
    
    # get the folder paths
    source_folder = os.path.abspath(os.path.expanduser(args.source_folder))
    target_folder = os.path.abspath(os.path.expanduser(args.target_folder))

    # check if we want to filter by code owners
    if args.codeowners:
        # get the folder the script is in
        script_folder = os.path.dirname(os.path.realpath(__file__))

        # load the code owners file
        code_owners_file = os.path.join(script_folder, "..", "..", ".github", "CODEOWNERS")
        if args.codeowners_file != "":
            code_owners_file = os.path.abspath(os.path.expanduser(args.codeowners_file))

        code_owners = CodeOwners(code_owners_file, path_mapping_func=renamed_folders_mapping_func)

        results = os.path.abspath(os.path.join(script_folder, "results"))
        os.makedirs(results, exist_ok=True)

        # Create temporary root folder for the target with random name suffix
        with tempfile.TemporaryDirectory(prefix="source_", dir=results) as temp_dir_source:
            create_symlinks(source_folder, os.path.join(temp_dir_source, "main"), code_owners, args.codeowners, args.ignore_folders)

            with tempfile.TemporaryDirectory(prefix="target_", dir=results) as temp_dir_target:
                create_symlinks(target_folder, os.path.join(temp_dir_target, "main"), code_owners, args.codeowners, args.ignore_folders)
                
                run_compare_tool(args.compare_tool_binary, args.compare_tool_arguments, temp_dir_source, temp_dir_target)
    else:
        run_compare_tool(args.compare_tool_binary, args.compare_tool_arguments, source_folder, target_folder)
