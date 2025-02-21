# Copyright (c) 2024 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0
import re, argparse
from collections import defaultdict

# import all funcs from codesearch.py
from codesearch import *

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description='Search for occurrences of a regex in the rust tests.')
    parser.add_argument('--target', default="../../", help='Target directory to search in.')
    parser.add_argument('--regex', default='', help='Regex pattern to search for.')
    parser.add_argument('--output', default="output.txt", help='Output file to save the results.')
    parser.add_argument('--verbose', action='store_true', help='Display detailed file path and line number for each occurrence.')
    parser.add_argument('--debug', action='store_true', help='Display the line where the occurrence was found.')
    
    args = parser.parse_args()

    if not args.regex:
        print("Please provide a regex pattern to search for.")
        exit(1)

    # Compile the test decorators pattern
    decorators_patterns = [re.compile(decorator) for decorator in [r'#\[test(\([^)]*\))?\]', r'#\[tokio::test(\([^)]*\))?\]', r'#\[sim_test(\([^)]*\))?\]']]
    
    # Compile the function name pattern
    function_name_regex = re.compile(r'fn\s+([\w_]+)\s*(<[^>]*>)?\s*\(')
    
    # Compile the content regex
    content_regex = re.compile(args.regex, flags=re.IGNORECASE)
    
    # Function to initialize the result dictionary
    def init_result_func():
        return {"func_name": "", "file_path": "", "line_num": "", "line": ""}

    # Function to search for occurrences of the regex pattern in a single file
    def search_in_file_func(file_path):
        with open(file_path, 'r') as file:
            lines = file.readlines()

        # Keep track of whether we are inside a function and have found a decorator
        inside_function = False
        current_decorator = None
        function_start = None
        function_body = []
        function_name = None

        results = defaultdict(init_result_func)
        for i, line in enumerate(lines):
            stripped_line = line.strip()

            # Look for decorators
            if not inside_function:
                for decorator in decorators_patterns:
                    if re.match(decorator, stripped_line):
                        current_decorator = decorator
                        break
            
            # Look for function definition
            if current_decorator and not inside_function:
                match = function_name_regex.search(stripped_line)
                if match:
                    function_name = match.group(1)
                    function_start = i + 1  # Line number in editor (1-based)
                    inside_function = True
                    function_body = []
            
            # If inside a function, collect its body
            if inside_function:
                function_body.append(line)

            # End of function when encountering an unindented line or after finding a closing brace
            if inside_function and stripped_line.endswith('}'):
                # Check if function name matches the content regex
                if content_regex.search(function_name):
                    results[function_name] = {
                        "func_name": function_name,
                        "file_path": file_path,
                        "line_num": function_start,
                        "line": function_name
                    }

                # Check if the function body matches the content regex
                for body_line in function_body:
                    if content_regex.search(body_line):
                        # If a match is found in the body, add the index to the function_start to get the exact line number
                        results[function_name] = {
                            "func_name": function_name,
                            "file_path": file_path,
                            "line_num": function_start,
                            "line": body_line.strip()
                        }
                        break  # Stop after finding the first match in the function body

                # Reset state for next function
                inside_function = False
                current_decorator = None
                function_name = None
                function_body = []

        return results

    # Function to merge results from different parallel processes
    def merge_results_func(results, result):
        for match_name, data in result.items():
            if match_name in results:
                raise ValueError(f"Duplicate function name found: {match_name}")
            results[match_name] = data
    
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
            'scripts',
            'target',
            'node_modules',
        ],
        file_pattern=re.compile(r'\.rs$', flags=re.IGNORECASE),
        search_in_file_func=search_in_file_func,
        init_results_func=lambda: defaultdict(init_result_func),
        merge_results_func=merge_results_func,
        post_process_results_func=None
    )

    # Open the output file to save the results
    with open(args.output, 'w', encoding='utf-8') as f:
        for match_name, data in results.items():
            # write function name to file
            f.write(f"{data['func_name']}\n")
            # write file path and line number to file
            f.write(f"    {data['file_path']}:{data['line_num']}\n")

            # print the function name in red color
            print(f"\033[91m{data['func_name']}\033[0m")
            if args.verbose:
                # print the file and the line number in blue color
                print(f"    \033[94m{data['file_path']}:{data['line_num']}\033[0m")
                if args.debug:
                    # print the line in green color
                    print(f"        => \033[92m{data['line']}\033[0m")
