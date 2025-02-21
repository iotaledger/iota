# Copyright (c) 2024 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0
import os, re
from tqdm import tqdm
from concurrent.futures import ProcessPoolExecutor, as_completed

# Cache for the crate names
crate_name_cache = {}

# Gets the name of the crate
def get_crate_name(crate_path):
    # Check if the crate name is already cached
    if crate_path in crate_name_cache:
        return crate_name_cache[crate_path]
    
    # get the current working directory
    cwd = os.getcwd()

    try:
        # change the current working directory
        os.chdir(crate_path)

        # get the output of the command "cargo read-manifest"
        cargo_manifest = os.popen("cargo read-manifest").read()

        # parse the "name" field from the json output
        component_name = re.search(r'"name":\s*"([^"]+)"', cargo_manifest).group(1)

        # cache the crate name
        crate_name_cache[crate_path] = component_name

        return component_name
    except:
        # cache the crate name as "Unknown"
        crate_name_cache[crate_path] = "Unknown"
        return "Unknown"
    finally:
        # change back to the original working directory
        os.chdir(cwd)

# Searches for given files that match the pattern and are not in the ignored directories, 
# and calls the search_in_file_func for each file to search for the pattern in parallel.
def search_files_in_parallel(target_dir, ignored_dirs, file_pattern, search_in_file_func, init_results_func, merge_results_func, post_process_results_func):
    all_files = []

    # Collect all files that match the pattern
    for root, _, files in os.walk(target_dir):
        if any(ignored_dir in root for ignored_dir in ignored_dirs):
            continue
        for file in files:
            if file_pattern.search(file):
                all_files.append(os.path.join(root, file))

    total_files = len(all_files)

    # Initialize the results
    results = init_results_func()
    
    # Use ProcessPoolExecutor to parallelize file searching
    with ProcessPoolExecutor() as executor:
        futures = {executor.submit(search_in_file_func, file): file for file in all_files}

        # Wrap the progress bar around the "as_completed" generator
        with tqdm(total=total_files, desc="Processing files", unit="file") as progress_bar:
            for future in as_completed(futures):
                result = future.result()
                merge_results_func(results, result)
                progress_bar.update(1)  # Update progress bar for each completed file

    # Post-process the results
    if post_process_results_func:
        post_process_results_func(results)

    return results