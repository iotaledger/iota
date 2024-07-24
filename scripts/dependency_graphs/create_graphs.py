# Copyright (c) 2024 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

# This script creates dependency graphs between the internal crates.
# It outputs SVG files and an index.html.

import re, pathlib, os, subprocess
from bs4 import BeautifulSoup

# runs the cargo tree command and returns the result
def run_cargo_tree(skip_dev_dependencies, save_to_file=False):
    args = ['--no-dedupe', '--depth=1']
    if skip_dev_dependencies:
        args.append('--edges=no-dev')

    # Run the cargo tree command and store the output in a string variable
    result = subprocess.run(['cargo', 'tree'] + args, stdout=subprocess.PIPE, text=True)
    if result.returncode:
        raise Exception("cargo tree process exited with return code %d" % result.returncode)
    
    if save_to_file:
        with open('debug_tree.txt', 'w') as f:
            f.write(result.stdout)

    return result.stdout

# calculates the depth of the crate in the graph
def calculate_graph_depth(line):
    markers = ['│   ', '├── ', '└── ', '    ']
    depth = 0
    for marker in markers:
        depth += line.count(marker)
    return depth

# returns None if not an internal crate
def get_internal_crate_name(base_path, line):
    package_info = re.sub(r'^[├─└─│ ]+\s*', '', line.strip())
    crate_name = None
    if f'({base_path}' in package_info and not 'external-crates/' in package_info:
        crate_name = re.sub(r' v[0-9]+\.[0-9]+\.[0-9]+.*', '', package_info).strip()
    return crate_name

# holds infos about a dependency
class Dependency(object):
    def __init__(self, name, is_dev_dependency):
        self.name               = name
        self.is_dev_dependency  = is_dev_dependency

# parse the cargo tree from a file into a map of dependencies
def parse_cargo_tree(cargo_tree_output, base_path, skip_dev_dependencies):
    dependencies_dict = {}

    lines = cargo_tree_output.split('\n')

    # get all internal crates first
    for line in lines:
        depth = calculate_graph_depth(line)
        if depth != 0:
            continue

        crate_name = get_internal_crate_name(base_path, line)
        if crate_name == None:
            continue
        
        dependencies_dict[crate_name] = []
    
    # loop again to determine the dependencies
    stack = []
    skip_depth = None
    for line in lines:
        depth = calculate_graph_depth(line)

        if skip_dev_dependencies and '[dev-dependencies]' in line:
            if (skip_depth != None) and (depth > skip_depth):
                # we found a nested dev-dependency, don't overwrite where we are coming from!
                continue

            # we found dev-dependencies which we don't want to track
            skip_depth = depth
            continue

        elif (skip_depth != None) and (depth <= skip_depth):
            # we left the dev-dependencies branch
            skip_depth = None

        if skip_depth != None:
            # we are still in the dev-dependencies branch
            continue

        crate_name = get_internal_crate_name(base_path, line)
        if crate_name == None:
            continue

        if depth == 0:
            stack = [crate_name]
        else:
            parent = stack[depth - 1]
            dependencies_dict[parent].append(Dependency(crate_name, is_dev_dependency=False))
            if depth < len(stack):
                stack[depth] = crate_name
            else:
                stack.append(crate_name)
        
    return dependencies_dict

# create one file to rule them all
def generate_dot_all(output_folder, dependencies_dict):
    with open('%s/all.dot' % (output_folder), 'w') as file:
        file.write("digraph dependencies {\n")
        for crate_name, dependencies in dependencies_dict.items():
            for dependency in dependencies:
                file.write(f'    "{crate_name}" -> "{dependency.name}";\n')
        file.write("}\n")

# but there are too many of them, so let's go into detail
def generate_dot_per_crate(output_folder, dependencies_dict):
    # Create a reverse dependency map
    reverse_dependencies = {}
    for crate_name, dependencies in dependencies_dict.items():
        for dependency in dependencies:
            if dependency.name not in reverse_dependencies:
                reverse_dependencies[dependency.name] = []
            reverse_dependencies[dependency.name].append(crate_name)
    
    # root level
    for crate_name, dependencies in dependencies_dict.items():
        if len(dependencies) == 0:
            continue
        
        with open(f'{output_folder}/{crate_name}.dot', 'w') as file:
            file.write("digraph dependencies {\n")
            
            # Set the text color of the current crate to green
            file.write(f'    "{crate_name}" [fontcolor=green];\n')
            
            # Add direct dependencies
            for dependency in dependencies:
                color = 'red' if len(dependencies_dict[dependency.name]) == 0 else 'black'
                file.write(f'    "{dependency.name}" [fontcolor={color}];\n')
                file.write(f'    "{crate_name}" -> "{dependency.name}";\n')
            
            # Add reverse dependencies
            if crate_name in reverse_dependencies:
                for reverse_parent in reverse_dependencies[crate_name]:
                    file.write(f'    "{reverse_parent}" [fontcolor=blue];\n')
                    file.write(f'    "{reverse_parent}" -> "{crate_name}";\n')
            
            file.write("}\n")

# the dot command line tool didn't convert URL or href to hyperlinks, so we have to do it ourselves
def add_hyperlinks_to_svg(svg_file, dependencies_dict):
    with open(svg_file, 'r') as file:
        svg_content = file.read()

    soup = BeautifulSoup(svg_content, 'xml')
    nodes = soup.find_all('g', {'class': 'node'})

    for i, node in enumerate(nodes):
        title = node.find('title')
        if title and title.string in dependencies_dict and len(dependencies_dict[title.string]) > 0:
            link = None
            if i == 0:
                # add a way to go back on root level
                link = soup.new_tag('a', href=f"javascript:history.back()")
            else:
                # add the hyperlink to other svg files
                link = soup.new_tag('a', href=f"{title.string}.svg")
            for element in node.find_all():
                link.append(element.extract())
            node.append(link)
    
    with open(svg_file, 'w') as file:
        file.write(str(soup))

# create an overview index.html
def create_index_html(folder):
    index_file_path = pathlib.Path(folder) / "index.html"
    with open(index_file_path, 'w') as file:
        file.write("<html><body>\n")
        file.write("<h1>IOTA-Rebased Dependency Graphs</h1>\n")
        file.write("<ul>\n")
        svg_files = sorted(pathlib.Path(folder).glob('*.svg'), key=lambda f: f.name)
        for svg_file in svg_files:
            file.write(f'<li><a href="{svg_file.name}">{svg_file.stem}</a></li>\n')
        file.write("</ul>\n")
        file.write("</body></html>\n")

# converts the dot files to another format and deletes the source files
def convert_dot_files(output_folder, file_type):
    for dot_file in pathlib.Path(output_folder).glob('*.dot'):
        result_file = dot_file.with_suffix('.%s' % (file_type))
        result = subprocess.run(['dot', '-T%s' % (file_type), str(dot_file), '-o', str(result_file)])
        if result.returncode:
            raise "dot process exited with return code %d" % (result.returncode)
        os.remove(dot_file)

################################################################################
if __name__ == '__main__':  
    output_folder = "output"
    skip_dev_dependencies = True            # whether or not to include the `dev-dependencies`

    # Create the output folder if it doesn't exist
    pathlib.Path(output_folder).mkdir(parents=True, exist_ok=True)

    cargo_tree_output = run_cargo_tree(skip_dev_dependencies)
    
    # Parse the cargo tree and generate the DOT file
    dependencies_dict = parse_cargo_tree(cargo_tree_output, pathlib.Path("../../").absolute().resolve(), skip_dev_dependencies)
    
    generate_dot_all(output_folder, dependencies_dict)
    generate_dot_per_crate(output_folder, dependencies_dict)

    # Convert DOT files to SVG
    convert_dot_files(output_folder, "svg")

    # Add hyperlinks to all SVG files for easier navigation
    for svg_file in pathlib.Path(output_folder).glob('*.svg'):
        add_hyperlinks_to_svg(svg_file, dependencies_dict)

    # Create index.html for better overview
    create_index_html(output_folder)