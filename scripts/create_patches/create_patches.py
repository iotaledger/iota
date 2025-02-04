# Copyright (c) 2025 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0
import sys, os, difflib, shutil, argparse
sys.path.append('../utils')
from codeowners import CodeOwners

# Create a patch file for the differences between source_file and target_file.
def create_patch(source_file, target_file, patch_file, relative_item, verbose):
    source_lines = []
    target_lines = []

    from_file = "/dev/null"
    to_file = "/dev/null"
    
    if os.path.exists(source_file):
        from_file = relative_item
        with open(source_file, 'r') as f:
            try:
                source_lines = f.readlines()
            except UnicodeDecodeError as e:
                if verbose:
                    print(f"Error: Unable to read source file {source_file}, {e}")
                return

    if os.path.exists(target_file):
        to_file = relative_item
        with open(target_file, 'r') as f:
            try:
                target_lines = f.readlines()
            except UnicodeDecodeError as e:
                if verbose:
                    print(f"Error: Unable to read target file {target_file}, {e}")
                return

    diff = difflib.unified_diff(
        source_lines,
        target_lines,
        fromfile=from_file,
        tofile=to_file
    )

    file_diff = ''.join(diff)
    if len(file_diff) == 0:
        # No differences found
        return

    # Create output folder if it doesn't exist
    if not os.path.exists(os.path.dirname(patch_file)):
        os.makedirs(os.path.dirname(patch_file))

    with open(patch_file, 'w') as patch:
        patch.write(file_diff)

# Recursively process folders and generate patch files for differing files.
def process_folder(source_folder_root, target_folder_root, output_folder_root, relative_folder, verbose):
    # Join relative folder path with folder paths
    source_folder = os.path.join(source_folder_root, relative_folder)
    target_folder = os.path.join(target_folder_root, relative_folder)
    output_folder = os.path.join(output_folder_root, relative_folder)

    if verbose:
        print(f"Processing folder: {output_folder}")
    
    # Get lists of files and subdirectories in both source and target
    source_items = set(os.listdir(source_folder)) if os.path.exists(source_folder) else set()
    target_items = set(os.listdir(target_folder)) if os.path.exists(target_folder) else set()

    # Combine source and target items
    all_items = source_items.union(target_items)

    for item in all_items:
        source_item = os.path.join(source_folder, item)
        target_item = os.path.join(target_folder, item)
        relative_item = os.path.join(relative_folder, item)

        if not os.path.exists(source_item) and not os.path.exists(target_item):
            raise Exception(f"Error: {os.path.join(relative_folder, item)} does not exist in either source or target.")

        if os.path.isdir(source_item) or os.path.isdir(target_item):
            # Recursively process subdirectories
            process_folder(source_folder_root, target_folder_root, output_folder_root, relative_item, verbose)
        else:
            # Process files
            patch_file = os.path.join(output_folder, f"{item}.patch")
            if verbose:
                print(f"Comparing: {item}")
            create_patch(source_item, target_item, patch_file, relative_item, verbose)
            if verbose:
                print(f"Patch created: {patch_file}")

# Process specified folders and generate patch files for differing files.
def process_folders(source_folder, target_folder, folders, output_folder, verbose):
    # remove the output folder if it exists
    if os.path.exists(output_folder):
        print(f"Removing existing output folder: {output_folder}")
        shutil.rmtree(output_folder)

    # create the output folder    
    os.makedirs(output_folder)

    for folder in folders:
        relative_folder = os.path.normpath(folder)
        process_folder(source_folder, target_folder, output_folder, relative_folder, verbose)

################################################################################
if __name__ == "__main__":
    # Argument parser setup
    parser = argparse.ArgumentParser(description="Generate patch files for differing files in specified subfolders.")
    parser.add_argument('--source-folder', required=True, help="The path to the source folder for comparison.")
    parser.add_argument('--target-folder', required=True, help="The path to the target folder for comparison.")
    parser.add_argument("--output-folder", required=True, help="The path to the output folder to save patch files.")
    parser.add_argument('--folders', nargs='+', help='Optionally filter by a list of folders relative to the project root (e.g., "crates/iota-core crates/iota-node").')
    parser.add_argument('--codeowners', nargs='+', help='Optionally filter folders by code owners (e.g., "@iotaledger/node @iotaledger/consensus").')
    parser.add_argument('--codeowners-file', default="", help="The path to the code owners file.")
    parser.add_argument('--verbose', action='store_true', help="Print verbose output.")
    args = parser.parse_args()

    # get the source folder
    source_folder = args.source_folder
    if not source_folder:
        raise Exception("Error: No source folder specified.")
    source_folder = os.path.abspath(os.path.expanduser(source_folder))
    if not os.path.exists(source_folder):
        raise Exception(f"Error: Source folder {source_folder} does not exist.")

    # get the target folder
    target_folder = args.target_folder
    if not target_folder:
        raise Exception("Error: No target folder specified.")
    target_folder = os.path.abspath(os.path.expanduser(target_folder))
    if not os.path.exists(target_folder):
        raise Exception(f"Error: Target folder {target_folder} does not exist.")

    # get the output folder
    output_folder = args.output_folder
    if not output_folder:
        raise Exception("Error: No output folder specified.")

    output_folder = os.path.abspath(os.path.expanduser(output_folder))

    folders = set()
    if args.folders:
        folders.update(args.folders)
    
    # check if we want to filter by code owners
    if args.codeowners:
        # get the folder the script is in
        script_folder = os.path.dirname(os.path.realpath(__file__))

        # load the code owners file
        code_owners_file = os.path.join(script_folder, "..", "..", ".github", "CODEOWNERS")
        if args.codeowners_file != "":
            code_owners_file = os.path.abspath(os.path.expanduser(args.codeowners_file))

        code_owners = CodeOwners(code_owners_file)

        # Get folders of the code owner
        paths = code_owners.match_paths_for_owners(source_folder, args.codeowners)
        folders.update(paths)
        paths = code_owners.match_paths_for_owners(target_folder, args.codeowners)
        folders.update(paths)
    
    process_folders(source_folder, target_folder, folders, output_folder, args.verbose)
