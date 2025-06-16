# Copyright (c) 2024 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0
import os, re, argparse
from collections import defaultdict

# import all funcs from codesearch.py
from codesearch import *

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description='Search for occurrences of function or variable names ending with V(\\d+) or v(\\d+) in Rust (*.rs) files.')
    parser.add_argument('--target', default="../../", help='Target directory to search in.')
    parser.add_argument('--output', default="output.txt", help='Output file to save the results.')
    parser.add_argument('--verbose', action='store_true', help='Display detailed file path and line number for each occurrence.')
    parser.add_argument('--debug', action='store_true', help='Display the line where the occurrence was found.')
    
    args = parser.parse_args()

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

    # Compile the version pattern
    version_pattern = re.compile(r'(?:\w|::)*(?:[Vv])\d+\b')

    # Function to initialize the result dictionary
    def init_result_func():
        return {"count": 0, "locations": [], "crates": {}}

    # Function to search for occurrences of the regex pattern in a single file
    def search_in_file_func(file_path):
        occurrences = defaultdict(init_result_func)

        with open(file_path, 'r', encoding='utf-8') as f:
            for line_num, line in enumerate(f, 1):
                matches = version_pattern.findall(line)
                if matches:
                    for match in matches:
                        if match in ignored_matches:
                            continue
                        occurrences[match]["count"] += 1
                        occurrences[match]["locations"].append((file_path, line_num, line.strip()))
                        occurrences[match]["crates"][get_crate_name(os.path.dirname(file_path))] = None
        
        return occurrences

    # Function to merge results from different parallel processes
    def merge_results_func(results, result):
        for match_name, data in result.items():
            results[match_name]["count"] += data["count"]
            results[match_name]["locations"].extend(data["locations"])
            results[match_name]["crates"].update(data["crates"])

    results = search_files_in_parallel(
        target_dir=args.target,
        ignored_dirs=[
            '.cargo',
            '.changeset',
            '.config',
            '.git',
            '.github',
            '.husky',
            '.pnpm_store',
            '.vscode',
            'chocolatey',
            'linting',
            'move-compiler',
            'move-vm-types',
            'move-bytecode-verifier',
            'move-binary-format',
            'move-core-types',
            'move-ir-types',
            'move-model',
            'move-vm-integration-tests',
            'node_modules',
            'scripts',
            'target',
            'unit_tests',
        ],
        file_pattern=re.compile(r'\.rs$', flags=re.IGNORECASE),
        search_in_file_func=search_in_file_func,
        init_results_func=lambda: defaultdict(init_result_func),
        merge_results_func=merge_results_func,
        post_process_results_func=lambda results: dict(sorted(results.items(), key=lambda x: x[0])) # sort the results by name
    )

    # Open the output file to save the results
    with open(args.output, 'w', encoding='utf-8') as f:
        for match_name, data in results.items():
            # print the match name and the number of occurrences in red color
            print(f"\033[91m{match_name:90}\033[0m {data['count']}: occurrence(s)")
            # print the crates where the match was found in light blue color
            print(f"  - Crates: \033[94m{', '.join(data['crates'].keys())}\033[0m")
            f.write(f"{match_name:90}: {data['count']} occurrence(s)\n")
            for file_path, line_num, line in data["locations"]:
                f.write(f"  - {file_path}, line {line_num}\n")
                if args.verbose:
                    # print the file path and line number in white color
                    print(f"    - \033[97m{file_path}, line {line_num}\033[0m")
                    if args.debug:
                        # print the line in green color
                        print(f"        => \033[92m{line}\033[0m")
