# Copyright (c) 2024 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0
import os, re, argparse, subprocess

COMMENT_DEPENDENCIES_START_EXTERNAL = "# external dependencies"
COMMENT_DEPENDENCIES_START_INTERNAL = "# internal dependencies"

def get_package_name_from_cargo_toml(file_path):
    # search for the [package] section in the Cargo.toml file
    section_regex = re.compile(r'^\[([a-zA-Z0-9_-]+)\]$')
    package_section_regex = re.compile(r'^\[package\]$')
    package_name_regex = re.compile(r'^name\s*=\s*"(.*)"$')

    with open(file_path, 'r') as file:
        lines = file.readlines()
    
    in_package_section = False
    for line in lines:
        stripped_line = line.strip()

        if not in_package_section and package_section_regex.match(stripped_line):
            in_package_section = True
            continue

        if in_package_section:
            package_name_match = package_name_regex.match(stripped_line)
            if package_name_match:
                return package_name_match.group(1)
            
            if section_regex.match(stripped_line):
                # we are done with the package section
                return None
    
    # no package section found
    return None

def get_package_names_from_cargo_tomls(directory):
    print("Getting \"internal\" crates from 'Cargo.toml' files...")

    package_names = {}

    # find all Cargo.toml files in the target folder
    for root, _, files in os.walk(directory):
        for file in files:
            if file == 'Cargo.toml':
                file_path = os.path.join(root, file)
                package_name = get_package_name_from_cargo_toml(file_path)
                if package_name:
                    package_names[package_name] = None
    
    return package_names

def process_cargo_toml(file_path, internal_crates_dict, debug):
    with open(file_path, 'r') as file:
        lines = file.readlines()

    array_start_regex = re.compile(r'^([a-zA-Z0-9_-]+)\s*=\s*\[$')
    crates_line_regex = re.compile(r'^([a-zA-Z0-9_-]+)(?:\.workspace)?\s*=\s*(?:{[^}]*\bpackage\s*=\s*"(.*?)"[^}]*}|.*)$')

    class Section(object):
        def __init__(self, line):
            self.line = line
            self.unknown_lines_start = []
            self.external_crates = {}
            self.internal_crates = {}
            self.unknown_lines_end = []
        
        def add_node(self, node):
            # we only want to add "internal crates" if we are in a "dependencies" section,
            # otherwise we treat everything as external crates, so we don't have different
            # groups for external and internal crates, but we still sort them.
            if ('dependencies' not in self.line) or (not node.name in internal_crates_dict):
                self.external_crates[node.alias] = node
            else:
                self.internal_crates[node.alias] = node
        
        def add_unknown_line(self, line):
            if not self.external_crates and not self.internal_crates:
                self.unknown_lines_start.append(line)
            else:
                self.unknown_lines_end.append(line)

        def get_processed_lines(self):
            # check if the nodes in the section should be sorted
            sort_nodes = any(word in self.line for word in ['dependencies', 'profile'])
            
            processed_lines = []

            # add potential unprocessed lines (comments at the start of the section)
            if self.unknown_lines_start:
                processed_lines.extend(self.unknown_lines_start)
            
            # add the section header
            processed_lines.append(self.line)
                        
            both_dependency_groups_exist = self.external_crates and self.internal_crates
            if both_dependency_groups_exist:
                processed_lines.append(COMMENT_DEPENDENCIES_START_EXTERNAL)

            # add the external crates
            external_crates = self.external_crates
            if sort_nodes:
                # sort the external crates by alias
                external_crates = {key: self.external_crates[key] for key in sorted(self.external_crates)}
            for crate_alias in external_crates:
                processed_lines.extend(external_crates[crate_alias].get_processed_lines())
            
            if both_dependency_groups_exist:
                # add a newline between external and internal crates
                processed_lines.append('')
            
            if both_dependency_groups_exist:
                processed_lines.append(COMMENT_DEPENDENCIES_START_INTERNAL)
            
            # add the internal crates
            internal_crates = self.internal_crates
            if sort_nodes:
                # sort the internal crates by alias
                internal_crates = {key: self.internal_crates[key] for key in sorted(self.internal_crates)}
            for crate_alias in internal_crates:
                processed_lines.extend(internal_crates[crate_alias].get_processed_lines())
            
            # add potential unprocessed lines (comments at the end of the section)
            if self.unknown_lines_end:
                processed_lines.extend(self.unknown_lines_end)
            return processed_lines

    class Node(object):
        def __init__(self, name, alias, start, is_multiline, comments):
            self.name           = name
            self.alias          = alias
            self.lines          = [start]
            self.is_multiline   = is_multiline
            self.comments       = comments
        
        def add_line(self, line):
            if not self.is_multiline:
                raise Exception(f"Node {self.name} is not multiline")
            self.lines.append(line)
        
        def get_processed_lines(self):
            if self.is_multiline and len(self.lines) > 2:
                # sort all the lines except the first and the last one
                self.lines = [self.lines[0]] + sorted(self.lines[1:-1]) + [self.lines[-1]]

            processed_lines = []
            for comment in self.comments:
                if not comment.strip():
                    # skip empty lines
                    continue
                processed_lines.append(comment)
            for line in self.lines:
                processed_lines.append(line)
            return processed_lines

    processed_lines   = []
    current_section   = None
    current_node      = None    
    unprocessed_lines = []

    def print_debug_info(msg):
        if debug:
            print(msg)

    def finish_node():
        nonlocal current_node
        if current_node:
            # if we have a current node, finish it
            current_node = None

    def finish_section():
        nonlocal current_node, current_section, processed_lines, unprocessed_lines

        finish_node()
        
        if current_section:
            # if we have a current section, finish it
            # We need to check were the unprocessed lines belong to by scanning in reverse.
            # If there is a newline between the next section and the remaining unprocessed lines,
            # the unprocessed lines belong to the current section.
            if unprocessed_lines:
                unprocessed_lines_current_section = []
                unprocessed_lines_next_section = []

                newline_found = False
                for line in reversed(unprocessed_lines):
                    if not line.strip():
                        # found a newline, the unprocessed lines belong to the current section
                        newline_found = True
                        # skip the newline
                        continue

                    if newline_found:
                        unprocessed_lines_current_section.insert(0, line)
                    else:
                        unprocessed_lines_next_section.insert(0, line)

                for unprocessed_line in unprocessed_lines_current_section:
                    current_section.add_unknown_line(unprocessed_line)

                # set the unprocessed lines to contain the comments for the next section
                # this will be picked up while creating the next section
                unprocessed_lines = unprocessed_lines_next_section
            
            processed_lines.extend(current_section.get_processed_lines())
            current_section = None
            
            # add a newline between sections
            processed_lines.append('')

    for line in lines:
        # strip the line of leading/trailing whitespace
        stripped_line = line.strip()

        if stripped_line in [COMMENT_DEPENDENCIES_START_EXTERNAL, COMMENT_DEPENDENCIES_START_INTERNAL]:
            # skip the line if it is the start of the external or internal crates
            continue

        print_debug_info(f"Processing line: '{stripped_line}'")

        # check if the line is a section header
        is_section_header = stripped_line.startswith('[') and stripped_line.endswith(']')
        if is_section_header:
            print_debug_info(f"   -> Section header: {stripped_line}")

            finish_section()
            
            # create a new section
            current_section = Section(stripped_line)
            for unprocessed_line in unprocessed_lines:
                if not unprocessed_line.strip():
                    # skip empty lines
                    continue
                current_section.add_unknown_line(unprocessed_line)
            unprocessed_lines = []
            continue

        # check if the line is an array start
        array_start_regex_search = array_start_regex.search(stripped_line)
        if array_start_regex_search:
            print_debug_info(f"   -> Array start: {array_start_regex_search.group(1)}")
            
            finish_node()

            array_name = array_start_regex_search.group(1)

            # create a new node
            if not current_section:
                raise Exception(f"Node {array_name.name} without section")
            current_node = Node(name=array_name, alias=array_name, start=stripped_line, is_multiline=True, comments=unprocessed_lines)
            current_section.add_node(current_node)
            unprocessed_lines = []
            
            continue
        
        # check if the line is an array end
        is_array_end = "[" not in stripped_line and stripped_line.endswith(']')
        if is_array_end:
            print_debug_info(f"   -> Array end: {stripped_line}")
            
            if not current_node:
                raise Exception(f"Array end {stripped_line} without start")
            
            # add the unprocessed lines to the current node
            for unprocessed_line in unprocessed_lines:
                if not unprocessed_line.strip():
                    # skip empty lines
                    continue
                current_node.add_line(unprocessed_line)
            unprocessed_lines = []

            # add the end line to the current node
            current_node.add_line(line.rstrip())

            finish_node()
            continue

        # check if the line is a crate line
        crate_regex_search  = crates_line_regex.search(stripped_line)
        if crate_regex_search:
            print_debug_info(f"   -> Crate: {crate_regex_search.group(1)}")
            
            crate_alias = crate_regex_search.group(1)
            crate_name = crate_regex_search.group(1)
            if crate_regex_search.group(2):
                crate_name = crate_regex_search.group(2)
            
            # create a new node
            if not current_section:
                raise Exception(f"Node {crate_name} without section")
            
            current_section.add_node(Node(name=crate_name, alias=crate_alias, start=stripped_line, is_multiline=False, comments=unprocessed_lines))
            unprocessed_lines = []
            continue
        
        # unknown line type, add it to the unprocessed lines
        print_debug_info(f"   -> Unknown line: {stripped_line}")
        unprocessed_lines.append(line.rstrip())

    finish_section()

    # Rewrite the file with the processed lines
    with open(file_path, 'w') as file:
        # write a newline for every entry in the processed lines list except the last one,
        # it is a newline anyway (added by finish_section)
        for line in processed_lines[:-1]:
            file.write(f"{line}\n")

def find_and_process_toml_files(directory, internal_crates_dict, ignored_folders, debug):
    print("Processing Cargo.toml files...")

    # Compile the regex patterns for ignored folders
    ignored_folders = [re.compile(pattern) for pattern in ignored_folders]

    for root, dirs, files in os.walk(directory):
        # Skip the entire directory if the root matches any ignored folder pattern
        if any(pattern.search(root) for pattern in ignored_folders):
            print(f"   Skipping directory (regex): {root}")
            dirs.clear()    # Don't walk into the directory if it should be ignored
            continue

        for file in files:
            if file == 'Cargo.toml':
                file_path = os.path.join(root, file)
                print(f'Processing {file_path}')
                process_cargo_toml(file_path, internal_crates_dict, debug)

# Run dprint fmt
def run_dprint_fmt(directory):
    cwd = os.getcwd()

    print("Running dprint fmt...")
    try:
        os.chdir(directory)
        subprocess.run(["dprint", "fmt"], check=True)
    finally:
        os.chdir(cwd)

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description='Format the Cargo.toml files and sort internal and external dependencies.')
    parser.add_argument('--target', default="../../", help='Target directory to search in.')
    parser.add_argument('--skip-dprint', action='store_true', help='Skip running dprint fmt.')
    parser.add_argument('--debug', action='store_true', help='Show debug prints.')
    
    args = parser.parse_args()

    internal_crates_dict = get_package_names_from_cargo_tomls(args.target)
    
    # add special cases
    internal_crates_dict["iota-rust-sdk"] = None

    # ignored folders
    ignored_folders = [
        "external-crates",
    ]
    
    find_and_process_toml_files(args.target, internal_crates_dict, ignored_folders, args.debug)

    if not args.skip_dprint:
        run_dprint_fmt(args.target)
