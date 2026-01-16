# Copyright (c) 2024 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0
import os, re, argparse, subprocess
from collections import defaultdict
from dataclasses import dataclass, field
from typing import Optional, Tuple

"""
Cargo.toml dependency management tool.

This script has two main modes:
1. Sort mode (default): Sort dependencies into internal/external groups with comments
2. Consolidate mode (--consolidate-deps): Analyze and consolidate/distribute workspace dependencies

Sort Mode:
- Scans all Cargo.toml files
- Separates internal (workspace) and external dependencies with comments
- Sorts dependencies alphabetically within each group

Consolidate Mode:
- If an external dependency is used by multiple crates:
  - Adds it to the root Cargo.toml [workspace.dependencies]
  - Updates individual crates to use `package.workspace = true`
- If an external dependency is only used by one crate:
  - Removes it from root Cargo.toml (if present)
  - Ensures the crate has the full version spec
"""

# ANSI color codes
RED = '\033[91m'
YELLOW = '\033[93m'
RESET = '\033[0m'

COMMENT_DEPENDENCIES_START_EXTERNAL = "# external dependencies"
COMMENT_DEPENDENCIES_START_INTERNAL = "# internal dependencies"

def get_package_name_from_cargo_toml(file_path: str) -> Optional[str]:
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

def get_package_names_from_cargo_tomls(directory: str, ignored_patterns: list = None) -> dict:
    # get all internal crate names from the workspace.
    print("Getting \"internal\" crates from 'Cargo.toml' files...")

    package_names = {}

    def extract_package_name(file_path: str, names_dict: dict):
        package_name = get_package_name_from_cargo_toml(file_path)
        if package_name:
            names_dict[package_name] = None

    find_and_process_toml_files(directory, ignored_patterns or [], extract_package_name, names_dict=package_names)
    
    return package_names

def find_all_cargo_tomls(directory: str, ignored_patterns: list = None) -> list:
    # find all Cargo.toml files in the directory.
    cargo_tomls = []
    ignored_patterns = ignored_patterns or []
    ignored_regexes = [re.compile(p) for p in ignored_patterns]

    for root, dirs, files in os.walk(directory):
        if 'target' in root.split(os.sep):
            dirs.clear()    # Don't walk into the directory if we're skipping it
            continue

        if any(r.search(root) for r in ignored_regexes):
            dirs.clear()    # Don't walk into the directory if we're skipping it
            continue

        for file in files:
            if file == 'Cargo.toml':
                cargo_tomls.append(os.path.join(root, file))

    return cargo_tomls

def find_and_process_toml_files(directory: str, ignored_patterns: list, process_func, **kwargs):
    # find all Cargo.toml files and process them with the given function.
    #   args:
    #       directory: Root directory to search
    #       ignored_patterns: Patterns to ignore during traversal
    #       process_func: Function to call on each Cargo.toml file
    #       **kwargs: Additional arguments to pass to process_func
    cargo_tomls = find_all_cargo_tomls(directory, ignored_patterns)
    for file_path in cargo_tomls:
        process_func(file_path, **kwargs)

def run_dprint_fmt(directory: str):
    # run dprint fmt to format the files.
    cwd = os.getcwd()
    print("Running dprint fmt...")
    try:
        os.chdir(directory)
        subprocess.run(["dprint", "fmt"], check=True)
    finally:
        os.chdir(cwd)

# ==============================================================================
# Sort Mode - Sort dependencies into internal/external groups
# ==============================================================================

def process_cargo_toml_sort(file_path: str, internal_crates_dict: dict, debug: bool):
    # process a single Cargo.toml file - sort dependencies into groups.
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

# ==============================================================================
# Consolidate Mode - Analyze and consolidate/distribute workspace dependencies
# ==============================================================================

def is_special_dep_section(section: str) -> bool:
    """
    Check if a dependency section is "special" and shouldn't drive consolidation.
    
    Special sections are:
    - build-dependencies: Only used during build (build.rs)
    - target.'cfg(...)'.dependencies: Platform-specific dependencies
    """
    if section.endswith('build-dependencies'):
        return True
    if section.startswith('target.') and '.dependencies' in section:
        return True
    return False

def merge_features(features_list: list) -> Optional[list]:
    # merge multiple feature lists into one, removing duplicates.
    all_features = set()
    has_any = False
    
    for features in features_list:
        if features:
            has_any = True
            all_features.update(features)
    
    if not has_any:
        return None
    
    return sorted(all_features)

def is_version_locked_or_bounded(version_str: str) -> Tuple[bool, bool, bool, str]:
    """
    Check if a version string is locked (exact), upper-bounded, or a wildcard.
    
    Returns:
        (is_exact, is_upper_bounded, is_wildcard, reason)
        - is_exact: True if version uses '=' prefix (exact pin)
        - is_upper_bounded: True if version uses '<' or '<=' prefix, or has '!=' exclusions
        - is_wildcard: True if version uses '*' (very permissive, non-conflicting)
        - reason: Human-readable description of the constraint
    """
    if not version_str:
        return (False, False, False, "")
    
    v = version_str.strip()
    
    # Check for wildcard: "*", "1.*", "1.2.*"
    if v == '*' or v.endswith('.*'):
        return (False, False, True, f"wildcard '{v}'")
    
    # Check for exact pin: "=1.0.0" (not ">=" or "<=")
    if v.startswith('=') and not v.startswith('==') and len(v) > 1 and v[1] != '>':
        return (True, False, False, f"exact pin '{v}'")
    
    # Check for upper bound: "<1.0.0" or "<=1.0.0"
    if v.startswith('<'):
        return (False, True, False, f"upper bound '{v}'")
    
    # Check for version ranges with upper bounds: ">=1.0, <2.0"
    if ',' in v and '<' in v:
        return (False, True, False, f"bounded range '{v}'")
    
    # Check for exclusions: "!=1.5.0" or ranges with exclusions: ">=1.0, !=1.5"
    if '!=' in v:
        return (False, True, False, f"exclusion '{v}'")
    
    return (False, False, False, "")

def parse_version(version_str: str) -> Tuple:
    # parse a version string into a tuple for comparison.
    if not version_str:
        return (0,)
    
    # Note: We strip version prefixes (^, ~, >=, etc.) because for consolidation purposes,
    # we only care about picking the "highest" base version. The prefix semantics (compatible,
    # minimum, etc.) are preserved when we write back the original version string - we don't
    # reconstruct it from this tuple. This is just for comparison/sorting.
    clean = version_str.lstrip('^~>=<')
    clean = clean.split(',')[0].strip()
    parts = []
    for part in clean.split('.'):
        part = part.split('-')[0].split('+')[0]
        try:
            parts.append(int(part))
        except ValueError:
            parts.append(0)

    return tuple(parts) if parts else (0,)

@dataclass
class Dependency:
    # represents a dependency with its specification.
    name: str
    alias: str
    version: Optional[str] = None
    git: Optional[str] = None
    rev: Optional[str] = None
    branch: Optional[str] = None
    path: Optional[str] = None
    features: Optional[list] = None
    default_features: Optional[bool] = None
    optional: Optional[bool] = None
    workspace: bool = False
    raw_spec: str = ""
    is_internal: bool = False

@dataclass
class CargoToml:
    # represents a parsed Cargo.toml file.
    file_path: str
    package_name: Optional[str] = None
    is_root: bool = False
    dependencies: dict = field(default_factory=dict)
    raw_lines: list = field(default_factory=list)

def parse_inline_table(spec_str: str) -> dict:
    # parse an inline table like { version = "1.0", features = ["foo"] }.
    result = {}
    inner = spec_str.strip()[1:-1].strip()
    if not inner:
        return result

    features_match = re.search(r'features\s*=\s*\[([^\]]*)\]', inner)
    if features_match:
        features_str = features_match.group(1)
        features = [f.strip().strip('"\'') for f in features_str.split(',') if f.strip()]
        result['features'] = features
        inner = re.sub(r',?\s*features\s*=\s*\[[^\]]*\]\s*,?', ',', inner)

    # Split the remaining string by commas, but respect nested braces.
    # We track brace depth to avoid splitting on commas inside nested structures.
    # Example: "version = \"1.0\", package = \"foo\"" -> ["version = \"1.0\"", "package = \"foo\""]
    parts = []
    current = ""
    depth = 0
    for char in inner:
        if char == '{':
            depth += 1
        elif char == '}':
            depth -= 1
        elif char == ',' and depth == 0:
            # Only split on commas at the top level (depth == 0)
            if current.strip():
                parts.append(current.strip())
            current = ""
            continue
        current += char
    if current.strip():
        parts.append(current.strip())

    for part in parts:
        if '=' not in part:
            raise ValueError(f"Invalid inline table entry: {part}")
        key, value = part.split('=', 1)
        key = key.strip()
        value = value.strip().strip('"\'')
        
        if value.lower() == 'true':
            result[key] = True
        elif value.lower() == 'false':
            result[key] = False
        else:
            result[key] = value

    return result

def parse_dependency_spec(alias: str, spec: str) -> Dependency:
    # parse a dependency specification string.
    spec = spec.strip()
    dep = Dependency(name=alias, alias=alias, raw_spec=spec)

    if spec.startswith('"') and spec.endswith('"') and '{' not in spec:
        dep.version = spec.strip('"')
        return dep

    if spec.startswith('{') and spec.endswith('}'):
        parsed = parse_inline_table(spec)
        if 'package' in parsed:
            dep.name = parsed['package']
        if 'version' in parsed:
            dep.version = parsed['version']
        if 'git' in parsed:
            dep.git = parsed['git']
        if 'rev' in parsed:
            dep.rev = parsed['rev']
        if 'branch' in parsed:
            dep.branch = parsed['branch']
        if 'path' in parsed:
            dep.path = parsed['path']
            dep.is_internal = True
        if 'features' in parsed:
            dep.features = parsed['features']
        if 'default-features' in parsed:
            dep.default_features = parsed['default-features']
        if 'optional' in parsed:
            dep.optional = parsed['optional']
        if parsed.get('workspace') == True:
            dep.workspace = True
        return dep

    raise ValueError(f"Unable to parse dependency spec: {spec}")

def parse_cargo_toml_consolidate(file_path: str, internal_crates: set) -> CargoToml:
    # parse a Cargo.toml file and extract dependencies for consolidation.
    with open(file_path, 'r') as f:
        lines = f.readlines()

    cargo_toml = CargoToml(file_path=file_path, raw_lines=lines)
    cargo_toml.package_name = get_package_name_from_cargo_toml(file_path)
    cargo_toml.is_root = '[workspace]' in ''.join(lines)

    section_regex = re.compile(r'^\[([a-zA-Z0-9_.\'-]+(?:\.[a-zA-Z0-9_.\'-]+)*)\]$')
    array_section_regex = re.compile(r'^\[\[([a-zA-Z0-9_-]+)\]\]$')
    dep_line_regex = re.compile(r'^([a-zA-Z0-9_-]+)(?:\.workspace)?\s*=\s*(.+)$')

    current_section = None
    in_array_section = False

    for i, line in enumerate(lines):
        stripped = line.strip()

        if not stripped or stripped.startswith('#'):
            # Skip empty lines and comments - we're only interested in extracting
            # dependency information for analysis. The original file structure
            # (including comments) is preserved when we update files later.
            continue

        if array_section_regex.match(stripped):
            current_section = None
            in_array_section = True
            continue

        section_match = section_regex.match(stripped)
        if section_match:
            current_section = section_match.group(1)
            in_array_section = False
            if current_section not in cargo_toml.dependencies:
                cargo_toml.dependencies[current_section] = {}
            continue

        if in_array_section:
            # Skip content inside array sections like [[bin]], [[test]], [[example]].
            # These contain binary/test definitions, not dependencies we need to analyze.
            # The original file content is preserved when we update dependencies later.
            continue

        is_dep_section = (
            current_section and
            current_section.endswith('dependencies')
        )

        if is_dep_section:
            dep_match = dep_line_regex.match(stripped)
            if dep_match:
                alias = dep_match.group(1)
                spec = dep_match.group(2)

                if '.workspace' in line and '= true' in line:
                    dep = Dependency(name=alias, alias=alias, workspace=True, raw_spec="{ workspace = true }")
                else:
                    dep = parse_dependency_spec(alias, spec)

                if alias in internal_crates or dep.name in internal_crates:
                    dep.is_internal = True
                if dep.path:
                    dep.is_internal = True

                cargo_toml.dependencies[current_section][alias] = dep

        # Lines that don't match dependency patterns (e.g., other config keys in sections)
        # are ignored during analysis. We only care about extracting dependency info here.
        # When updating files, we do line-by-line replacement preserving all other content.

    return cargo_toml

def analyze_dependencies(cargo_tomls: list, internal_crates: set) -> dict:
    # Analyze all dependencies across all Cargo.toml files.
    deps_analysis = defaultdict(lambda: {'usages': [], 'root_spec': None})

    for toml_path in cargo_tomls:
        cargo_toml = parse_cargo_toml_consolidate(toml_path, internal_crates)

        for section, deps in cargo_toml.dependencies.items():
            for alias, dep in deps.items():
                if dep.is_internal:
                    continue

                if cargo_toml.is_root and section == 'workspace.dependencies':
                    deps_analysis[alias]['root_spec'] = dep
                else:
                    deps_analysis[alias]['usages'].append((toml_path, section, dep))

    return deps_analysis

def build_workspace_dep_spec(dep: Dependency) -> str:
    # build a workspace.dependencies specification string from a Dependency.
    parts = []

    if dep.git:
        parts.append(f'git = "{dep.git}"')
        if dep.rev:
            parts.append(f'rev = "{dep.rev}"')
        if dep.branch:
            parts.append(f'branch = "{dep.branch}"')
    elif dep.version:
        parts.append(f'version = "{dep.version}"')

    if dep.name != dep.alias:
        parts.append(f'package = "{dep.name}"')

    if dep.features:
        features_str = ', '.join(f'"{f}"' for f in dep.features)
        parts.append(f'features = [{features_str}]')

    # Note: We only write "default-features = false" explicitly. If default_features is True
    # or None, we omit it since true is the Cargo default. Crates that need to override
    # with default-features=false will specify it via build_crate_workspace_ref().
    if dep.default_features is False:
        parts.append('default-features = false')

    if len(parts) == 1 and dep.version and not dep.git:
        return f'"{dep.version}"'

    if parts:
        return '{ ' + ', '.join(parts) + ' }'
    return '{ }'

def build_crate_workspace_ref(dep: Dependency) -> Optional[str]:
    # build a crate-level workspace reference, preserving local overrides.
    extra_parts = []

    if dep.features:
        features_str = ', '.join(f'"{f}"' for f in dep.features)
        extra_parts.append(f'features = [{features_str}]')

    # Include both default-features = false AND default-features = true
    # This is necessary when workspace has default-features=false but this crate needs them
    if dep.default_features is not None:
        extra_parts.append(f'default-features = {str(dep.default_features).lower()}')

    if dep.optional:
        extra_parts.append('optional = true')

    if not extra_parts:
        return None

    parts = ['workspace = true'] + extra_parts
    return '{ ' + ', '.join(parts) + ' }'

def build_full_dep_spec(dep: Dependency) -> str:
    # Build a full dependency specification for standalone use (not workspace ref).
    # Difference from build_workspace_dep_spec: this includes 'optional' field,
    # which is a crate-level concern, not a workspace-level one.
    parts = []

    if dep.git:
        parts.append(f'git = "{dep.git}"')
        if dep.rev:
            parts.append(f'rev = "{dep.rev}"')
        if dep.branch:
            parts.append(f'branch = "{dep.branch}"')
    elif dep.version:
        parts.append(f'version = "{dep.version}"')

    if dep.name != dep.alias:
        parts.append(f'package = "{dep.name}"')

    if dep.features:
        features_str = ', '.join(f'"{f}"' for f in dep.features)
        parts.append(f'features = [{features_str}]')

    if dep.default_features is False:
        parts.append('default-features = false')

    if dep.optional:
        parts.append('optional = true')

    if len(parts) == 1 and parts[0].startswith('version'):
        return f'"{dep.version}"'

    return '{ ' + ', '.join(parts) + ' }'

def update_root_cargo_toml(root_path: str, deps_to_add: dict, deps_to_remove: set):
    # update the root Cargo.toml to add/remove workspace dependencies.
    with open(root_path, 'r') as f:
        lines = f.readlines()

    new_lines = []
    in_workspace_deps = False
    section_regex = re.compile(r'^\[([a-zA-Z0-9_.-]+(?:\.[a-zA-Z0-9_.-]+)*)\]$')
    dep_line_regex = re.compile(r'^([a-zA-Z0-9_-]+)\s*=')

    added_deps = set()
    workspace_deps_end = -1
    i = 0

    # First pass: copy lines, removing deps marked for removal, and track where
    # [workspace.dependencies] section ends so we can insert new deps there.
    while i < len(lines):
        line = lines[i]
        stripped = line.strip()

        section_match = section_regex.match(stripped)
        if section_match:
            if in_workspace_deps:
                workspace_deps_end = len(new_lines)
            in_workspace_deps = section_match.group(1) == 'workspace.dependencies'

        if in_workspace_deps:
            dep_match = dep_line_regex.match(stripped)
            if dep_match:
                alias = dep_match.group(1)
                if alias in deps_to_remove:
                    i += 1
                    continue

        new_lines.append(line)
        i += 1

    if in_workspace_deps:
        workspace_deps_end = len(new_lines)

    if deps_to_add and workspace_deps_end > 0:
        insert_lines = []
        for alias, dep in sorted(deps_to_add.items()):
            if alias not in added_deps:
                spec = build_workspace_dep_spec(dep)
                insert_lines.append(f'{alias} = {spec}\n')
                added_deps.add(alias)

        new_lines = new_lines[:workspace_deps_end] + insert_lines + new_lines[workspace_deps_end:]

    with open(root_path, 'w') as f:
        f.writelines(new_lines)
    print(f"Updated {root_path}")

def update_crate_cargo_toml(toml_path: str, updates: dict):
    # update a crate's Cargo.toml to use workspace refs or full specs.
    with open(toml_path, 'r') as f:
        lines = f.readlines()

    new_lines = []
    current_section = None
    in_array_section = False

    section_regex = re.compile(r'^\[([a-zA-Z0-9_.\'-]+(?:\.[a-zA-Z0-9_.\'-]+)*)\]$')
    array_section_regex = re.compile(r'^\[\[([a-zA-Z0-9_-]+)\]\]$')
    dep_line_regex = re.compile(r'^([a-zA-Z0-9_-]+)(?:\.workspace)?\s*=\s*(.+)$')

    # Process each line, replacing dependency lines that need updating.
    # We track current section and whether we're in an array section to
    # only modify lines in actual dependency sections.
    for line in lines:
        stripped = line.strip()

        if array_section_regex.match(stripped):
            current_section = None
            in_array_section = True
            new_lines.append(line)
            continue

        section_match = section_regex.match(stripped)
        if section_match:
            current_section = section_match.group(1)
            in_array_section = False
            new_lines.append(line)
            continue

        is_dep_section = (
            current_section and
            current_section.endswith('dependencies') and
            not in_array_section
        )

        if is_dep_section:
            dep_match = dep_line_regex.match(stripped)
            if dep_match:
                alias = dep_match.group(1)
                
                # Check for both regular updates and section-specific updates
                update_info = None
                if alias in updates:
                    update_info = updates[alias]
                else:
                    # Check for section-specific update (for clean_features)
                    section_key = f"{alias}:{current_section}"
                    if section_key in updates:
                        update_info = updates[section_key]
                
                if update_info:
                    # Handle different update info formats
                    if len(update_info) == 3:
                        action, dep, expected_section = update_info
                        # Only apply if we're in the expected section
                        if current_section != expected_section:
                            new_lines.append(line)
                            continue
                    else:
                        action, dep = update_info
                    
                    indent = len(line) - len(line.lstrip())
                    indent_str = line[:indent]

                    if action == 'to_workspace':
                        new_spec = build_crate_workspace_ref(dep)
                        if new_spec is None:
                            new_lines.append(f'{indent_str}{alias}.workspace = true\n')
                        else:
                            new_lines.append(f'{indent_str}{alias} = {new_spec}\n')
                    elif action == 'fix_default_features':
                        # Fix invalid default-features=false in workspace dependency
                        if dep.workspace:
                            new_spec = build_crate_workspace_ref(dep)
                            if new_spec is None:
                                new_lines.append(f'{indent_str}{alias}.workspace = true\n')
                            else:
                                new_lines.append(f'{indent_str}{alias} = {new_spec}\n')
                        else:
                            # For non-workspace deps, use full spec
                            new_spec = build_full_dep_spec(dep)
                            new_lines.append(f'{indent_str}{alias} = {new_spec}\n')
                    elif action == 'clean_features':
                        # Clean up redundant features from workspace dependency
                        new_spec = build_crate_workspace_ref(dep)
                        if new_spec is None:
                            new_lines.append(f'{indent_str}{alias}.workspace = true\n')
                        else:
                            new_lines.append(f'{indent_str}{alias} = {new_spec}\n')
                    else:  # 'to_full'
                        new_spec = build_full_dep_spec(dep)
                        new_lines.append(f'{indent_str}{alias} = {new_spec}\n')
                    continue

        new_lines.append(line)

    with open(toml_path, 'w') as f:
        f.writelines(new_lines)
    print(f"Updated {toml_path}")

def get_crate_path(toml_path: str, target_dir: str) -> str:
    # Convert absolute toml path to relative crate path from workspace root.
    rel_path = os.path.relpath(toml_path, target_dir)
    return rel_path.replace('/Cargo.toml', '') if rel_path.endswith('/Cargo.toml') else rel_path

def should_ignore_dependency(args, dep_name: str, crate_path: str = None) -> bool:
    """Check if a dependency should be ignored based on strict-ignore rules."""
    if not args.strict_ignore:
        return False
    
    # Parse ignore rules
    ignored_deps = set()
    ignored_dep_crate_combos = {}
    ignored_crates = set()
    
    for ignore_rule in args.strict_ignore:
        if ':' in ignore_rule:
            dep_part, crate_part = ignore_rule.split(':', 1)
            if dep_part == '*':
                ignored_crates.add(crate_part)
            else:
                if dep_part not in ignored_dep_crate_combos:
                    ignored_dep_crate_combos[dep_part] = set()
                ignored_dep_crate_combos[dep_part].add(crate_part)
        else:
            ignored_deps.add(ignore_rule)
    
    # Check if dependency is completely ignored
    if dep_name in ignored_deps:
        return True
    
    # Check if specific dep:crate combo is ignored
    if crate_path and dep_name in ignored_dep_crate_combos:
        if crate_path in ignored_dep_crate_combos[dep_name]:
            return True
    
    # Check if crate is globally ignored
    if crate_path and crate_path in ignored_crates:
        return True
    
    return False

def run_consolidate_mode(args, target_dir: str, root_cargo_toml: str, ignore_patterns: list, iteration: int = 1):
    # run the consolidate dependencies mode.
    internal_crates = set(get_package_names_from_cargo_tomls(target_dir, ignore_patterns).keys())
    print(f"  Found {len(internal_crates)} internal crates")

    print("Finding Cargo.toml files...")
    cargo_tomls = find_all_cargo_tomls(target_dir, ignore_patterns)
    print(f"  Found {len(cargo_tomls)} Cargo.toml files")

    if not os.path.exists(root_cargo_toml):
        raise Exception(f"Error: Root Cargo.toml not found at {root_cargo_toml}")

    if root_cargo_toml not in cargo_tomls:
        cargo_tomls.append(root_cargo_toml)

    print("Analyzing dependencies...")
    deps_analysis = analyze_dependencies(cargo_tomls, internal_crates)

    deps_to_consolidate = {}
    deps_to_distribute = {}
    version_conflicts = {}

    for alias, info in deps_analysis.items():
        usages = info['usages']
        root_spec = info['root_spec']
        
        # Check for version conflicts for this package
        pkg_name = alias
        if root_spec:
            pkg_name = root_spec.name
        elif usages:
            pkg_name = usages[0][2].name
        
        # Track version conflicts with detailed crate information
        if pkg_name not in version_conflicts:
            workspace_ver = root_spec.version if root_spec and root_spec.version else None
            crate_versions = {}
            
            for toml_path, section, dep in usages:
                if not dep.workspace and dep.version:
                    if dep.version not in crate_versions:
                        crate_versions[dep.version] = []
                    crate_path = get_crate_path(toml_path, target_dir)
                    crate_versions[dep.version].append(crate_path)
            
            # Check for conflicts
            if workspace_ver and crate_versions:
                all_versions = set(crate_versions.keys())
                all_versions.add(workspace_ver)
                
                if len(all_versions) > 1:
                    version_conflicts[pkg_name] = {
                        'workspace_version': workspace_ver,
                        'crate_versions': crate_versions
                    }
            elif len(crate_versions) > 1:
                version_conflicts[pkg_name] = {
                    'workspace_version': None,
                    'crate_versions': crate_versions
                }

        # Use relative paths from workspace root for unique crate identification
        unique_crates = set(get_crate_path(usage[0], target_dir) for usage in usages)
        usage_count = len(unique_crates)

        regular_usages = [u for u in usages if not is_special_dep_section(u[1])]
        regular_usage_count = len(set(get_crate_path(u[0], target_dir) for u in regular_usages))

        if regular_usage_count >= args.min_usage:
            if not root_spec:
                version_deps = []
                git_deps = []

                for toml_path, section, dep in usages:
                    if is_special_dep_section(section):
                        continue
                    if dep.version:
                        version_deps.append((toml_path, dep))
                    elif dep.git:
                        git_deps.append((toml_path, dep))

                if version_deps and git_deps:
                    print(f"{RED}ERROR: Dependency '{alias}' has MIXED version and git specifications!{RESET}")
                    print(f"  Version specs:")
                    for path, dep in version_deps:
                        print(f"    - {os.path.relpath(path)}: {dep.version}")
                    print(f"  Git specs:")
                    for path, dep in git_deps:
                        rev_info = dep.rev or dep.branch or "no rev"
                        print(f"    - {os.path.relpath(path)}: {dep.git} ({rev_info})")
                    print(f"  {RED}Please resolve manually!{RESET}")
                    raise Exception("Mixed version and git specifications detected.")

                best_dep = None

                if version_deps:
                    unique_versions = set(dep.version for _, dep in version_deps)
                    if len(unique_versions) > 1:
                        # Check for locked or upper-bounded versions that conflict with higher versions
                        locked_deps = []
                        bounded_deps = []
                        exclusion_conflicts = []
                        
                        # Filter out wildcards for determining highest version (they're too permissive)
                        non_wildcard_deps = [(p, d) for p, d in version_deps 
                                             if not is_version_locked_or_bounded(d.version)[2]]
                        
                        if not non_wildcard_deps:
                            # All versions are wildcards, just use the first one
                            highest_version = version_deps[0]
                        else:
                            highest_version = max(non_wildcard_deps, key=lambda x: parse_version(x[1].version))
                        highest_version_tuple = parse_version(highest_version[1].version)
                        highest_version_str = highest_version[1].version
                        
                        for path, dep in version_deps:
                            is_exact, is_bounded, is_wildcard, reason = is_version_locked_or_bounded(dep.version)
                            dep_version_tuple = parse_version(dep.version)
                            
                            # Skip wildcards - they're permissive and don't conflict
                            if is_wildcard:
                                continue
                            
                            if is_exact and dep_version_tuple < highest_version_tuple:
                                locked_deps.append((path, dep, reason))
                            elif is_bounded:
                                # For exclusions, check if the highest version is excluded
                                if '!=' in dep.version:
                                    # Extract excluded versions and check if highest is among them
                                    excluded = [v.strip().lstrip('!=') for v in dep.version.split(',') if '!=' in v]
                                    for excl_ver in excluded:
                                        if parse_version(excl_ver) == highest_version_tuple:
                                            exclusion_conflicts.append((path, dep, f"excludes '{excl_ver}'"))
                                            break
                                elif dep_version_tuple < highest_version_tuple:
                                    bounded_deps.append((path, dep, reason))
                        
                        if locked_deps or bounded_deps or exclusion_conflicts:
                            print(f"{RED}ERROR: Dependency '{alias}' has CONFLICTING version constraints!{RESET}")
                            print(f"  Highest version found: {highest_version_str} in {os.path.relpath(highest_version[0])}")
                            if locked_deps:
                                print(f"  Exact pins that conflict:")
                                for path, dep, reason in locked_deps:
                                    print(f"    - {os.path.relpath(path)}: {reason}")
                            if bounded_deps:
                                print(f"  Upper-bounded versions that conflict:")
                                for path, dep, reason in bounded_deps:
                                    print(f"    - {os.path.relpath(path)}: {reason}")
                            if exclusion_conflicts:
                                print(f"  Exclusions that conflict with highest version:")
                                for path, dep, reason in exclusion_conflicts:
                                    print(f"    - {os.path.relpath(path)}: {reason}")
                            print(f"  {RED}Please resolve manually!{RESET}")
                            raise Exception("Conflicting version constraints detected.")
                        
                        # No locked/bounded conflicts, just different SemVer specs - warn and use highest
                        print(f"{YELLOW}NOTE: Dependency '{alias}' has multiple versions: {unique_versions}{RESET}")
                        print(f"  Using highest version.")

                    # Handle default-features logic:
                    # If ANY crate uses default-features=false, set workspace to false
                    # and ALL other crates (that didn't explicitly disable defaults) need default-features=true
                    print(f"[DEBUG] Considering {alias} for consolidation (version_deps):")
                    for path, dep in version_deps:
                        print(f"[DEBUG]   {os.path.relpath(path)}: default_features={dep.default_features}")
                    
                    # Check if any crate explicitly disables default features
                    has_explicit_false = any(dep.default_features is False for _, dep in version_deps)
                    workspace_default_features = None
                    
                    if has_explicit_false:
                        workspace_default_features = False
                        print(f"{YELLOW}NOTE: Dependency '{alias}' has at least one crate with default-features=false.{RESET}")
                        print(f"  Setting default-features=false in workspace.")
                        print(f"  Other crates will explicitly set default-features=true to maintain behavior.")

                    highest_ver_dep = max(version_deps, key=lambda x: parse_version(x[1].version))[1]
                    all_features = merge_features([dep.features for _, dep in version_deps])

                    best_dep = Dependency(
                        name=highest_ver_dep.name,
                        alias=highest_ver_dep.alias,
                        version=highest_ver_dep.version,
                        git=highest_ver_dep.git,
                        rev=highest_ver_dep.rev,
                        branch=highest_ver_dep.branch,
                        features=all_features,
                        default_features=workspace_default_features,
                    )

                elif git_deps:
                    unique_revs = set((dep.git, dep.rev, dep.branch) for _, dep in git_deps)
                    if len(unique_revs) > 1:
                        print(f"{RED}ERROR: Dependency '{alias}' has CONFLICTING git specifications!{RESET}")
                        for path, dep in git_deps:
                            rev_info = dep.rev or dep.branch or "no rev"
                            print(f"    - {os.path.relpath(path)}: {dep.git} ({rev_info})")
                        print(f"  {RED}Please resolve manually!{RESET}")
                        raise Exception("Conflicting git specifications detected.")

                    # Handle default-features logic:
                    # If ANY crate uses default-features=false, set workspace to false
                    # and ALL other crates (that didn't explicitly disable defaults) need default-features=true
                    print(f"[DEBUG] Considering {alias} for consolidation (git_deps):")
                    for path, dep in git_deps:
                        print(f"[DEBUG]   {os.path.relpath(path)}: default_features={dep.default_features}")
                    
                    # Check if any crate explicitly disables default features
                    has_explicit_false = any(dep.default_features is False for _, dep in git_deps)
                    workspace_default_features = None
                    
                    if has_explicit_false:
                        workspace_default_features = False
                        print(f"{YELLOW}NOTE: Dependency '{alias}' has at least one crate with default-features=false.{RESET}")
                        print(f"  Setting default-features=false in workspace.")
                        print(f"  Other crates will explicitly set default-features=true to maintain behavior.")
                    
                    first_dep = git_deps[0][1]
                    all_features = merge_features([dep.features for _, dep in git_deps])

                    best_dep = Dependency(
                        name=first_dep.name,
                        alias=first_dep.alias,
                        git=first_dep.git,
                        rev=first_dep.rev,
                        branch=first_dep.branch,
                        features=all_features,
                        default_features=workspace_default_features,
                    )

                if best_dep:
                    deps_to_consolidate[alias] = {
                        'dep': best_dep,
                        'usages': usages,
                    }
        elif usage_count == 1:
            usage = usages[0] if usages else None
            is_special = usage and is_special_dep_section(usage[1])
            # Check if dependency should be kept in workspace despite single usage
            keep_in_workspace = alias in args.keep_in_workspace
            if root_spec and not is_special and not keep_in_workspace:
                deps_to_distribute[alias] = {
                    'root_spec': root_spec,
                    'usage': usage,
                }

    print(f"\n=== Analysis Results ===")
    print(f"Dependencies to consolidate (add to workspace): {len(deps_to_consolidate)}")
    print(f"Dependencies to distribute (remove from workspace): {len(deps_to_distribute)}")
    
    # Show dependencies kept in workspace due to --keep-in-workspace
    if args.keep_in_workspace:
        kept_deps = []
        for alias, info in deps_analysis.items():
            if alias in args.keep_in_workspace and info['root_spec'] and len(info['usages']) == 1:
                kept_deps.append(alias)
        if kept_deps:
            print(f"Dependencies kept in workspace (--keep-in-workspace): {len(kept_deps)}")
            for dep in sorted(kept_deps):
                print(f"  {dep}")

    # Show version conflicts with detailed crate information
    if version_conflicts:
        print(f"\n{YELLOW} Version Conflicts Found:{RESET}")
        
        for pkg_name, conflict in version_conflicts.items():
            workspace_ver = conflict['workspace_version']
            crate_versions = conflict['crate_versions']
            
            # Check if this dependency is completely ignored (no crate-specific rules)
            dep_completely_ignored = args.strict and should_ignore_dependency(args, pkg_name, None)
            
            # Helper function to format crates with ignore annotations
            def format_crates_with_ignore(crates):
                displayed_crates = []
                for crate in crates:
                    crate_ignored = args.strict and should_ignore_dependency(args, pkg_name, crate)
                    
                    if crate_ignored:
                        displayed_crates.append(f"{RED}{crate} (ignored){RESET}")
                    else:
                        displayed_crates.append(crate)
                
                return ', '.join(displayed_crates)
            
            # Helper function to display version conflicts
            def display_version_conflict(pkg_name, conflict, dep_completely_ignored, format_crates_fn):
                workspace_ver = conflict['workspace_version']
                crate_versions = conflict['crate_versions']
                
                # Print header based on conflict type
                if workspace_ver:
                    print(f"\n  Package '{pkg_name}' has conflicting versions:")
                    if dep_completely_ignored:
                        print(f"    {RED}(IGNORED in strict mode){RESET}")
                    print(f"    WORKSPACE: {workspace_ver}")
                else:
                    versions_list = sorted(crate_versions.keys())
                    print(f"\n  Package '{pkg_name}' has multiple versions in crates: {versions_list}")
                    if dep_completely_ignored:
                        print(f"    {RED}(IGNORED in strict mode){RESET}")
                
                # Print crate versions
                for version, crates in sorted(crate_versions.items()):
                    crate_list = format_crates_fn(crates)
                    print(f"    {crate_list}: ({version})")
                
                # Print suggestion
                suggestion = "using consistent versions across workspace and crates" if workspace_ver else "consolidating to workspace dependency"
                print(f"    {YELLOW}Consider {suggestion}.{RESET}")
            
            display_version_conflict(pkg_name, conflict, dep_completely_ignored, format_crates_with_ignore)
        
        if args.strict:
            # Check if any conflicts should cause strict mode failure
            strict_failures = {}
            for pkg_name, conflict in version_conflicts.items():
                if should_ignore_dependency(args, pkg_name, None):
                    continue  # Ignore this dependency completely
                
                workspace_ver = conflict['workspace_version']
                crate_versions = conflict['crate_versions']
                filtered_crate_versions = {}
                
                for version, crates in crate_versions.items():
                    filtered_crates = []
                    for crate in crates:
                        # Skip if this specific dep:crate combo is ignored
                        if should_ignore_dependency(args, pkg_name, crate):
                            continue
                        filtered_crates.append(crate)
                    
                    if filtered_crates:
                        filtered_crate_versions[version] = filtered_crates
                
                # Check if there are still conflicts after filtering
                if workspace_ver and filtered_crate_versions:
                    all_versions = set(filtered_crate_versions.keys())
                    all_versions.add(workspace_ver)
                    if len(all_versions) > 1:
                        strict_failures[pkg_name] = {
                            'workspace_version': workspace_ver,
                            'crate_versions': filtered_crate_versions
                        }
                elif len(filtered_crate_versions) > 1:
                    strict_failures[pkg_name] = {
                        'workspace_version': None,
                        'crate_versions': filtered_crate_versions
                    }
            
            if strict_failures:
                raise Exception(f"\n{RED}❌ STRICT MODE: Version conflicts detected (after applying ignore rules)! Exiting with error.{RESET}")

    if deps_to_consolidate:
        print("\nTo consolidate:")
        for alias, info in sorted(deps_to_consolidate.items()):
            crates = set(get_crate_path(u[0], target_dir) for u in info['usages'])
            print(f"  {alias}: used by {len(crates)} crates - {crates}")

    if deps_to_distribute:
        print("\nTo distribute:")
        for alias, info in sorted(deps_to_distribute.items()):
            if info['usage']:
                crate = get_crate_path(info['usage'][0], target_dir)
                print(f"  {alias}: only used by {crate}")
            else:
                print(f"  {alias}: defined in workspace but not used")

    # Clean up invalid default-features=false in workspace dependencies
    invalid_default_features = {}
    for alias, info in deps_analysis.items():
        if alias not in deps_to_consolidate and alias not in deps_to_distribute:
            # Only fix the truly invalid case: workspace = true + default-features = false
            for toml_path, section, dep in info['usages']:
                if dep.default_features is False and dep.workspace is True:
                    # Invalid: workspace dependency with default-features=false (has no effect)
                    if toml_path not in invalid_default_features:
                        invalid_default_features[toml_path] = {}
                    
                    clean_dep = Dependency(
                        name=dep.name,
                        alias=dep.alias,
                        features=dep.features,  # Keep original features unchanged
                        default_features=None,  # Remove the invalid false setting
                        optional=dep.optional,
                        workspace=True
                    )
                    invalid_default_features[toml_path][alias] = ('fix_default_features', clean_dep)

    if invalid_default_features:
        print(f"\nFound {sum(len(updates) for updates in invalid_default_features.values())} invalid default-features=false in workspace dependencies:")
        for toml_path, updates in invalid_default_features.items():
            crate_path = get_crate_path(toml_path, target_dir)
            deps_list = list(updates.keys())
            print(f"  {crate_path}: {', '.join(deps_list)}")

    # Clean up redundant features in existing workspace dependencies
    redundant_features_cleanup = {}
    for alias, info in deps_analysis.items():
        if alias not in deps_to_consolidate and alias not in deps_to_distribute and info['root_spec']:
            workspace_dep = info['root_spec']
            # Check existing workspace dependencies for redundant features
            for toml_path, section, dep in info['usages']:
                if dep.workspace and dep.features and workspace_dep.features:
                    # Filter out features that are already in the workspace dependency
                    workspace_features_set = set(workspace_dep.features)
                    crate_features_set = set(dep.features)
                    additional_features = crate_features_set - workspace_features_set
                    
                    # Only update if we can remove some features
                    if len(additional_features) < len(crate_features_set):
                        if toml_path not in redundant_features_cleanup:
                            redundant_features_cleanup[toml_path] = {}
                        
                        cleaned_features = sorted(list(additional_features)) if additional_features else None
                        clean_dep = Dependency(
                            name=dep.name,
                            alias=dep.alias,
                            features=cleaned_features,
                            default_features=dep.default_features,
                            optional=dep.optional,
                            workspace=True
                        )
                        # Use section-specific key to handle dependencies in multiple sections
                        section_key = f"{alias}:{section}"
                        redundant_features_cleanup[toml_path][section_key] = ('clean_features', clean_dep, section)

    if redundant_features_cleanup:
        print(f"\nFound {sum(len(updates) for updates in redundant_features_cleanup.values())} workspace dependencies with redundant features:")
        for toml_path, updates in redundant_features_cleanup.items():
            crate_path = get_crate_path(toml_path, target_dir)
            # Extract just the dependency names from section-specific keys
            deps_list = [key.split(':')[0] for key in updates.keys()]
            print(f"  {crate_path}: {', '.join(sorted(set(deps_list)))}")  # Use set to dedupe

    if not deps_to_consolidate and not deps_to_distribute and not invalid_default_features and not redundant_features_cleanup:
        return False  # No changes made

    root_additions = {alias: info['dep'] for alias, info in deps_to_consolidate.items()}
    root_removals = set(deps_to_distribute.keys())

    if root_additions or root_removals:
        update_root_cargo_toml(root_cargo_toml, root_additions, root_removals)

    crate_updates = defaultdict(dict)

    # Handle dependencies being newly consolidated to workspace
    for alias, info in deps_to_consolidate.items():
        workspace_dep = info['dep']
        for toml_path, section, dep in info['usages']:
            if not dep.workspace:
                # Check if this specific dep:crate combination should be ignored
                crate_path = get_crate_path(toml_path, target_dir)
                if should_ignore_dependency(args, alias, crate_path):
                    continue  # Skip this conversion
                
                # When converting to workspace ref, handle default-features correctly:
                # If workspace has default-features=false but this crate didn't explicitly disable them,
                # it needs default-features=true to maintain original behavior
                explicit_default_features = None
                if workspace_dep.default_features is False and dep.default_features is not False:
                    # Workspace is false, but this crate wants defaults (either None or True)
                    explicit_default_features = True
                elif dep.default_features is not None and dep.default_features != workspace_dep.default_features:
                    # Crate has explicit setting that differs from workspace
                    explicit_default_features = dep.default_features
                
                # Only specify features that are not already defined in workspace features
                explicit_features = None
                if dep.features and workspace_dep.features:
                    # Filter out features that are already in the workspace dependency
                    workspace_features_set = set(workspace_dep.features)
                    crate_features_set = set(dep.features)
                    additional_features = crate_features_set - workspace_features_set
                    if additional_features:
                        explicit_features = sorted(list(additional_features))
                elif dep.features and not workspace_dep.features:
                    # Workspace has no features, but crate does - keep all crate features
                    explicit_features = dep.features
                elif dep.features != workspace_dep.features:
                    # Handle other cases where features differ (e.g., None vs empty list)
                    explicit_features = dep.features
                
                crate_dep = Dependency(
                    name=dep.name,
                    alias=dep.alias,
                    features=explicit_features,
                    default_features=explicit_default_features,
                    optional=dep.optional,
                    workspace=True
                )
                crate_updates[toml_path][alias] = ('to_workspace', crate_dep)

    # Handle existing workspace dependencies that have non-workspace crate usages
    for alias, info in deps_analysis.items():
        if alias not in deps_to_consolidate and alias not in deps_to_distribute and info['root_spec']:
            workspace_dep = info['root_spec']
            for toml_path, section, dep in info['usages']:
                if not dep.workspace and not is_special_dep_section(section):
                    # Check if this specific dep:crate combination should be ignored
                    crate_path = get_crate_path(toml_path, target_dir)
                    if should_ignore_dependency(args, alias, crate_path):
                        continue  # Skip this conversion
                    
                    # Convert non-workspace usage to workspace reference
                    explicit_default_features = None
                    if workspace_dep.default_features is False and dep.default_features is not False:
                        explicit_default_features = True
                    elif dep.default_features is not None and dep.default_features != workspace_dep.default_features:
                        explicit_default_features = dep.default_features
                    
                    # Only specify features that are not already defined in workspace features
                    explicit_features = None
                    if dep.features and workspace_dep.features:
                        workspace_features_set = set(workspace_dep.features)
                        crate_features_set = set(dep.features)
                        additional_features = crate_features_set - workspace_features_set
                        if additional_features:
                            explicit_features = sorted(list(additional_features))
                    elif dep.features and not workspace_dep.features:
                        explicit_features = dep.features
                    elif dep.features != workspace_dep.features:
                        explicit_features = dep.features
                    
                    crate_dep = Dependency(
                        name=dep.name,
                        alias=dep.alias,
                        features=explicit_features,
                        default_features=explicit_default_features,
                        optional=dep.optional,
                        workspace=True
                    )
                    crate_updates[toml_path][alias] = ('to_workspace', crate_dep)

    for alias, info in deps_to_distribute.items():
        if info['usage']:
            toml_path, section, dep = info['usage']
            if dep.workspace:
                full_dep = Dependency(
                    name=info['root_spec'].name,
                    alias=alias,
                    version=info['root_spec'].version,
                    git=info['root_spec'].git,
                    rev=info['root_spec'].rev,
                    branch=info['root_spec'].branch,
                    features=dep.features or info['root_spec'].features,
                    default_features=dep.default_features if dep.default_features is not None else info['root_spec'].default_features,
                    optional=dep.optional,
                )
                crate_updates[toml_path][alias] = ('to_full', full_dep)

    # Merge invalid default-features fixes into crate updates
    for toml_path, updates in invalid_default_features.items():
        for alias, update in updates.items():
            crate_updates[toml_path][alias] = update

    # Merge redundant features cleanup into crate updates
    for toml_path, updates in redundant_features_cleanup.items():
        for alias, update in updates.items():
            crate_updates[toml_path][alias] = update

    for toml_path, updates in crate_updates.items():
        if updates:
            update_crate_cargo_toml(toml_path, updates)

    # Return whether any changes were made
    changes_made = (
        bool(deps_to_consolidate) or 
        bool(deps_to_distribute) or 
        bool(invalid_default_features) or 
        bool(redundant_features_cleanup)
    )
    
    return changes_made

def run_consolidate_mode_with_loop(args, target_dir: str, root_cargo_toml: str, ignore_patterns: list):
    # Run consolidate mode in a loop until no more changes are detected
    max_iterations = 10  # Safety limit to prevent infinite loops
    iteration = 1
    
    print(f"=== Consolidate Mode (Loop) ===")
    
    while iteration <= max_iterations:
        print(f"\n--- Consolidation Pass {iteration} ---")
        
        try:
            changes_made = run_consolidate_mode(args, target_dir, root_cargo_toml, ignore_patterns, iteration)
        except Exception as e:
            print(f"{RED}ERROR: Consolidation failed with error: {e}{RESET}")
            print("Stopping consolidation due to unresolvable conflicts.")
            return 1
        
        if not changes_made:  # No changes made
            if iteration == 1:
                print("No changes needed!")
            else:
                print(f"Consolidation completed after {iteration - 1} passes.")
            return 0
        
        # changes were made, continue to next iteration
        iteration += 1
    
    raise Exception(f"{RED}ERROR: Reached maximum iteration limit ({max_iterations}). Some dependencies may still need consolidation.{RESET}")

# ==============================================================================
# Main Entry Point
# ==============================================================================

if __name__ == '__main__':
    parser = argparse.ArgumentParser(
        description='Cargo.toml dependency management tool.',
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Modes:
  Sort (default):
    Sorts dependencies into internal/external groups with comment markers.
    
  Consolidate (--consolidate-deps):
    Analyzes and consolidates/distributes workspace dependencies.
    - If an external dep is used by multiple crates: add to workspace
    - If an external dep is used by one crate: remove from workspace
    - Use --keep-in-workspace to prevent specific deps from being removed

Examples:
  # Sort dependencies (default mode)
  python cargo_sort.py

  # Consolidate dependencies, ignoring external-crates
  python cargo_sort.py --consolidate-deps --ignore external-crates
  
  # Consolidate but keep fastcrypto-vdf in workspace even if used by one crate
  python cargo_sort.py --consolidate-deps --keep-in-workspace fastcrypto-vdf
"""
    )
    parser.add_argument(
        '--target',
        default='../../',
        help='Target directory to search in. Default: ../../'
    )
    parser.add_argument(
        '--ignore',
        action='append',
        default=[],
        help='Folder patterns to ignore (can be specified multiple times).'
    )
    parser.add_argument(
        '--skip-dprint',
        action='store_true',
        help='Skip running dprint fmt.'
    )
    parser.add_argument(
        '--skip-sort',
        action='store_true',
        help='Skip sort mode after consolidating dependencies.'
    )
    parser.add_argument(
        '--debug',
        action='store_true',
        help='Show debug prints (for sort mode).',
    )
    
    # Consolidate mode options
    parser.add_argument(
        '--consolidate-deps',
        action='store_true',
        help='Run consolidate mode: analyze and consolidate/distribute workspace dependencies.'
    )
    parser.add_argument(
        '--min-usage',
        type=int,
        default=2,
        help='Minimum usages to consolidate a dependency (consolidate mode). Default: 2'
    )
    parser.add_argument(
        '--strict',
        action='store_true',
        help='Strict mode: exit with error if version conflicts are found (consolidate mode).'
    )
    parser.add_argument(
        '--strict-ignore',
        action='append',
        default=[],
        help='Ignore dependencies/crates in strict mode. Format: "dep_name" or "dep_name:crate/path" or "*:crate/path". Can be specified multiple times.'
    )
    parser.add_argument(
        '--keep-in-workspace',
        action='append',
        default=[],
        help='Dependencies to keep in workspace even if used by only one crate (consolidate mode). Can be specified multiple times.'
    )

    args = parser.parse_args()

    # Resolve target to absolute path
    # If an absolute path is provided, use it directly; otherwise resolve relative to script dir
    if os.path.isabs(args.target):
        target_dir = os.path.normpath(args.target)
    else:
        script_dir = os.path.dirname(os.path.abspath(__file__))
        target_dir = os.path.normpath(os.path.join(script_dir, args.target))

    # Build ignore patterns
    ignore_patterns = [rf'[/\\]{re.escape(p)}([/\\]|$)' for p in args.ignore]

    if args.consolidate_deps:
        # Consolidate mode
        root_cargo_toml = os.path.join(target_dir, 'Cargo.toml')

        print(f"=== Consolidate Mode ===")
        print(f"Analyzing workspace at: {target_dir}")
        print(f"Root Cargo.toml: {root_cargo_toml}")
        if ignore_patterns:
            print(f"Ignoring folders: {args.ignore}")
        
        result = run_consolidate_mode_with_loop(args, target_dir, root_cargo_toml, ignore_patterns)
        if result != 0:
            print(f"Consolidate mode failed with code {result}")
            exit(result)

        print("\nDone!")

    if args.skip_sort:
        print("Skipping sort mode as per --skip-sort flag.")
        exit(0)

    # Sort mode (default)
    print(f"=== Sort Mode ===")
    if ignore_patterns:
        print(f"Ignoring folders: {args.ignore}")
    
    internal_crates_dict = get_package_names_from_cargo_tomls(target_dir, None)

    # add special cases
    internal_crates_dict["iota-sdk-types"] = None
    internal_crates_dict["iota-sdk-transaction-builder"] = None
    internal_crates_dict["iota-flamegraph-svg"] = None

    print("Processing Cargo.toml files...")
    find_and_process_toml_files(
        target_dir,
        ignore_patterns,
        process_cargo_toml_sort,
        internal_crates_dict=internal_crates_dict,
        debug=args.debug
    )

    if not args.skip_dprint:
        run_dprint_fmt(target_dir)

    print("\nDone!")
