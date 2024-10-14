import os, re, argparse
from collections import defaultdict
from concurrent.futures import ProcessPoolExecutor, as_completed
from tqdm import tqdm

crate_name_cache = {}

# Top-level function to initialize defaultdict items (replaces the lambda)
def default_occurrence():
    return {"count": 0, "locations": [], "crates": {}}

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

# Function to search for occurrences of the regex pattern in a single file
def search_in_file(file_path, pattern, ignored_matches):
    occurrences = defaultdict(default_occurrence)

    with open(file_path, 'r', encoding='utf-8') as f:
        for line_num, line in enumerate(f, 1):
            matches = pattern.findall(line)
            if matches:
                for match in matches:
                    if match in ignored_matches:
                        continue
                    occurrences[match]["count"] += 1
                    occurrences[match]["locations"].append((file_path, line_num, line.strip()))
                    occurrences[match]["crates"][get_crate_name(os.path.dirname(file_path))] = None
    
    return occurrences

# Function to merge results from different parallel processes
def merge_results(total_occurrences, file_occurrences):
    for match_name, data in file_occurrences.items():
        total_occurrences[match_name]["count"] += data["count"]
        total_occurrences[match_name]["locations"].extend(data["locations"])
        total_occurrences[match_name]["crates"].update(data["crates"])

# Function to parallelize the search
def search_files_in_parallel(target_dir, pattern, ignored_dirs, ignored_matches, output_path, verbose, debug):
    total_occurrences = defaultdict(default_occurrence)
    all_files = []

    # Collect all *.rs files
    for root, _, files in os.walk(target_dir):
        if any(ignored_dir in root for ignored_dir in ignored_dirs):
            continue
        for file in files:
            if file.endswith('.rs'):
                all_files.append(os.path.join(root, file))

    total_files = len(all_files)

    # Use ProcessPoolExecutor to parallelize file searching
    with ProcessPoolExecutor() as executor:
        futures = {executor.submit(search_in_file, file, pattern, ignored_matches): file for file in all_files}

        # Wrap the progress bar around the "as_completed" generator
        with tqdm(total=total_files, desc="Processing files", unit="file") as progress_bar:
            for future in as_completed(futures):
                file_occurrences = future.result()
                merge_results(total_occurrences, file_occurrences)
                progress_bar.update(1)  # Update progress bar for each completed file

    # sort the results by name
    total_occurrences = dict(sorted(total_occurrences.items(), key=lambda x: x[0]))

    # Open the output file to save the results
    with open(output_path, 'w', encoding='utf-8') as f:
        for match_name, data in total_occurrences.items():
            # print the match name and the number of occurrences in red color
            print(f"\033[91m{match_name:90}\033[0m {data['count']}: occurrence(s)")
            # print the crates where the match was found in light blue color
            print(f"  - Crates: \033[94m{', '.join(data['crates'].keys())}\033[0m")
            f.write(f"{match_name:90}: {data['count']} occurrence(s)\n")
            for file_path, line_num, line in data["locations"]:
                f.write(f"  - {file_path}, line {line_num}\n")
                if verbose:
                    # print the the file path and line number in white color
                    print(f"    - \033[97m{file_path}, line {line_num}\033[0m")
                    if debug:
                        # print the line in green color
                        print(f"        => \033[92m{line}\033[0m")
                    
if __name__ == "__main__":
    parser = argparse.ArgumentParser(description='Search for occurrences of function or variable names ending with V(\\d+) or v(\\d+) in Rust (*.rs) files.')
    parser.add_argument('--target', default="../../", help='Target directory to search in.')
    parser.add_argument('--output', default="output.txt", help='Output file to save the results.')
    parser.add_argument('--verbose', action='store_true', help='Display detailed file path and line number for each occurrence.')
    parser.add_argument('--debug', action='store_true', help='Display the line where the occurrence was found.')
    
    args = parser.parse_args()

    # Compile the pattern once
    pattern = re.compile(r'(?:\w|::)*(?:[Vv])\d+\b')

    ignored_dirs = [
        '.git',
        'scripts',
        'node_modules',
        'target',
        '.pnpm_store',
        'unit_tests',
        'move-compiler',
        'move-vm-types',
        'move-bytecode-verifier',
        'move-binary-format',
        'move-core-types',
        'move-ir-types',
        'move-model',
        'move-prover',
        'move-vm-integration-tests',
    ]

    ignored_matches = {
        "Uuid::new_v4",
        "recv0",
        "recv1",
        "IpAddr::V4",
        "IpAddr::V6",
        "icmpv6",
        "tlsv1",
        "multi_addr_ipv4",
        "multi_addr_ipv6",
        "socket_addr_ipv4",
        "socket_addr_ipv6",
        "SocketAddr::V4",
        "SocketAddr::V6",
        "SocketAddrV4",
        "SocketAddrV6",
        "ipv4",
        "ipv6",
        "IpProto::Ipv4",
        "IpProto::Ipv6",
        "eval_ipv6",
        "EtherType::Ipv4",
        "EtherType::Ipv6",
        "to_ipv4",
        "ev1",
        "ev2",
        "sigv4",
    }

    search_files_in_parallel(args.target, pattern, ignored_dirs, ignored_matches, args.output, args.verbose, args.debug)
