# Copyright (c) 2024 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

# This script creates dependency graphs between the internal crates.
# It outputs SVG files and an index.html.
import re, sys, pathlib, os, io, subprocess, argparse
from bs4 import BeautifulSoup
sys.path.append('../utils')
from codeowners import CodeOwners

# holds infos about a crate
class Crate(object):
    def __init__(self, name, owners):
        self.name = name
        self.owners = owners
        self.dependencies = {}

# holds infos about a dependency
class Dependency(object):
    def __init__(self, name, is_dev_dependency):
        self.name = name
        self.is_dev_dependency = is_dev_dependency

# runs the cargo tree command and returns the result
def run_cargo_tree(skip_dev_dependencies, save_to_file=False):
    tree_edges = '--edges=no-build'
    if skip_dev_dependencies:
        tree_edges += ',no-dev'
    
    # Run the cargo tree command and store the output in a string variable
    result = subprocess.run(['cargo', 'tree', '-q', '--all-features', '--no-dedupe', '--depth=1', tree_edges], stdout=subprocess.PIPE, text=True)
    if result.returncode:
        raise Exception("cargo tree process exited with return code %d" % result.returncode)
    
    if save_to_file:
        with open('debug_tree.txt', 'w') as f:
            f.write(result.stdout)

    return result.stdout

# calculates the depth of the crate in the graph.
# requires "--prefix=indent" (default)
def calculate_graph_depth(line):
    markers = ['│   ', '├── ', '└── ', '    ']
    depth = 0
    for marker in markers:
        depth += line.count(marker)
    return depth

# returns None if not an internal crate
def get_internal_package_info(base_path, line):
    package_info = re.sub(r'^[├─└─│ ]+\s*', '', line.strip())
    if f'({base_path}' in package_info and not 'external-crates/' in package_info:
        return package_info
    return None

def get_crate_name_by_package_info(package_info):
    return re.sub(r' v[0-9]+\.[0-9]+\.[0-9]+.*', '', package_info).strip()

def get_code_owners_by_package_info(code_owners, base_path, package_info):
    crate_path = re.search(r'\((.*?)\)', package_info).group(1).replace(str(base_path), '')
    return ", ".join(code_owners.match_owners_for_path(crate_path))

# parse the cargo tree from a file into a map of dependencies
def parse_cargo_tree(cargo_tree_output, base_path, code_owners, skip_dev_dependencies):
    lines = cargo_tree_output.split('\n')

    # get all internal crates first
    crates_dict = {}
    for line in lines:
        depth = calculate_graph_depth(line)
        if depth != 0:
            continue

        package_info = get_internal_package_info(base_path, line)
        if package_info == None:
            continue

        crate_name = get_crate_name_by_package_info(package_info)
        code_owners_line = get_code_owners_by_package_info(code_owners, base_path, package_info)
        
        crates_dict[crate_name] = Crate(crate_name, code_owners_line)
    
    # loop again to determine the dependencies
    crate_names_stack = []
    depth_dev_dependencies = None
    for line in lines:
        depth = calculate_graph_depth(line)

        if '[dev-dependencies]' in line:
            # we found new dev-dependencies
            # only set the depth if we are not nested inside another dev_dependency already
            if (depth_dev_dependencies == None) or (depth < depth_dev_dependencies):
                depth_dev_dependencies = depth
            
            # we can skip that line, no matter what
            continue
        
        elif (depth_dev_dependencies != None) and (depth <= depth_dev_dependencies):
            # we left the dev-dependencies branch
            depth_dev_dependencies = None

        if (depth_dev_dependencies != None) and skip_dev_dependencies:
            # we are inside a dev-dependencies branch, which we want to skip
            continue

        package_info = get_internal_package_info(base_path, line)
        if package_info == None:
            continue

        crate_name = get_crate_name_by_package_info(package_info)
        
        if depth == 0:
            crate_names_stack = [crate_name]
        else:
            parent_name = crate_names_stack[depth - 1]
            
            is_dev_dependency = depth_dev_dependencies!=None

            parent_crate = crates_dict[parent_name]

            if not crate_name in parent_crate.dependencies or not is_dev_dependency and parent_crate.dependencies[crate_name].is_dev_dependency:
                # only set the dependency if it didn't exist, or if it was a dev_dependency before but now it's not
                parent_crate.dependencies[crate_name] = Dependency(crate_name, is_dev_dependency)
            if depth < len(crate_names_stack):
                crate_names_stack[depth] = crate_name
            else:
                crate_names_stack.append(crate_name)
        
    return crates_dict

def get_node_color(crates_dict, dependency):
    if dependency.is_dev_dependency:
        return 'orange'
    if len(crates_dict[dependency.name].dependencies) == 0:
        return 'red'
    return 'black'

def generate_dot_file(file_path, content):
    with open(file_path, 'w') as file:
        file.write("""digraph dependencies {
    // Define graph attributes
    rankdir=LR;
""")
        
        file.write(content)
        
        # add the legend
        file.write("""
    // Legend as a subgraph
    subgraph cluster_legend {
        style = "dashed";
        rankdir = LR;

        // Define the legend items
        legend_parent [label="PARENTS", shape=plaintext, fontcolor=blue, fontsize=10, fontname="Helvetica-Bold", width=0, height=0];
        legend_root [label="ROOT CRATE", shape=plaintext, fontcolor=green, fontsize=10, fontname="Helvetica-Bold", width=0, height=0];
        legend_normal [label="NORMAL DEPENDENCY", shape=plaintext, fontcolor=black, fontsize=10, fontname="Helvetica-Bold", width=0, height=0];
        legend_no_dep [label="CRATE WITHOUT DEPENDENCIES", shape=plaintext, fontcolor=red, fontsize=10, fontname="Helvetica-Bold", width=0, height=0];
        legend_dev [label="DEV-DEPENDENCY", shape=plaintext, fontcolor=orange, fontsize=10, fontname="Helvetica-Bold", width=0, height=0];

        // Arrange legend nodes vertically with smaller spacing
        legend_parent -> legend_root [style=invis, weight=1];
        legend_root -> legend_normal [style=invis, weight=1];
        legend_normal -> legend_no_dep [style=invis, weight=1];
        legend_no_dep -> legend_dev [style=invis, weight=1];
    }
}""")

# create one file to rule them all
def generate_dot_all(output_folder, crates_dict):
    buffer = io.StringIO()

    visited = {}
    for crate in crates_dict.values():
        if crate.name not in visited:
            visited[crate.name] = True
            buffer.write(f'    "{crate.name}" [label="{crate.name}\n({crate.owners})"];\n')

        for dependency in crate.dependencies.values():
            dependency_crate = crates_dict[dependency.name]
            if dependency_crate.name not in visited:
                visited[dependency_crate.name] = True
                buffer.write(f'    "{dependency_crate.name}" [label="{dependency_crate.name}\n({dependency_crate.owners})", fontcolor={get_node_color(crates_dict, dependency)}];\n')
            
            buffer.write(f'    "{crate.name}" -> "{dependency_crate.name}";\n')

    generate_dot_file('%s/all.dot' % (output_folder), buffer.getvalue())

# but there are too many of them, so let's go into detail
def generate_dot_per_crate(output_folder, crates_dict, traverse_all):
    # Create a reverse dependency map
    reverse_dependency_names = {}
    for crate in crates_dict.values():
        for dependency in crate.dependencies.values():
            if dependency.name not in reverse_dependency_names:
                reverse_dependency_names[dependency.name] = []
            reverse_dependency_names[dependency.name].append(crate.name)
    
    # root level
    for crate in crates_dict.values():
        buffer = io.StringIO()        

        # Set the text color of the current crate to green
        buffer.write(f'    "{crate.name}" [label="{crate.name}\n({crate.owners})", fontcolor=green];\n')
        
        # Add reverse dependencies
        if crate.name in reverse_dependency_names:
            for reverse_parent_name in reverse_dependency_names[crate.name]:
                parent_crate = crates_dict[reverse_parent_name]
                buffer.write(f'    "{parent_crate.name}" [label="{parent_crate.name}\n({parent_crate.owners})", fontcolor=blue];\n')
                buffer.write(f'    "{parent_crate.name}" -> "{crate.name}";\n')
        
        def write_crate_dependencies(crate, traversed_dict):
            for dependency in crate.dependencies.values():
                dependency_crate = crates_dict[dependency.name]
                buffer.write(f'    "{dependency_crate.name}" [label="{dependency_crate.name}\n({dependency_crate.owners})", fontcolor={get_node_color(crates_dict, dependency)}];\n')
                buffer.write(f'    "{crate.name}" -> "{dependency_crate.name}";\n')
            
            if traverse_all:
                for dependency in crate.dependencies.values():
                    if dependency.name in traversed_dict:
                        continue
                    
                    traversed_dict[dependency.name] = True                        
                    write_crate_dependencies(crates_dict[dependency.name], traversed_dict)
        
        # Add direct dependencies
        write_crate_dependencies(crate, {})

        file_name = f'{output_folder}/{crate.name}_full.dot' if traverse_all else f'{output_folder}/{crate.name}.dot' 
        generate_dot_file(file_name, buffer.getvalue())

# the dot command line tool didn't convert URL or href to hyperlinks, so we have to do it ourselves
def add_hyperlinks_to_svg(svg_file, crates_dict):
    with open(svg_file, 'r') as file:
        svg_content = file.read()

    soup = BeautifulSoup(svg_content, 'xml')
    nodes = soup.find_all('g', {'class': 'node'})

    for i, node in enumerate(nodes):
        title = node.find('title')
        if not title:
            continue

        crate_name = title.string
        if crate_name in crates_dict:
            link = None
            if i == 0:
                # add a way to go back on root level
                link = soup.new_tag('a', href=f"javascript:history.back()")
            else:
                # add the hyperlink to other svg files
                link = soup.new_tag('a', href=f"{crate_name}_full.svg" if "_full" in str(svg_file) else f"{crate_name}.svg")
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
        
        file.write("<h3>All Dependencies</h3>\n")
        file.write("<ul>\n")
        file.write(f'<li><a href="all.svg">all</a></li>\n')
        file.write("</ul>\n")
        
        file.write("<h3>Direct Dependencies Only</h3>\n")
        file.write("<ul>\n")
        svg_files_direct = sorted(f for f in pathlib.Path(folder).glob('*.svg') if not f.name.endswith('_full.svg') and not f.name == 'all.svg')
        for svg_file in svg_files_direct:
            file.write(f'<li><a href="{svg_file.name}">{svg_file.stem}</a></li>\n')
        file.write(f'<li><a href="all.svg">all</a></li>\n')
        file.write("</ul>\n")
        
        file.write("<h3>Full Dependencies</h3>\n")
        file.write("<ul>\n")
        svg_files_full = sorted(pathlib.Path(folder).glob('*_full.svg'), key=lambda f: f.name)
        for svg_file in svg_files_full:
            file.write(f'<li><a href="{svg_file.name}">{svg_file.stem}</a></li>\n')
        file.write(f'<li><a href="all.svg">all</a></li>\n')
        file.write("</ul>\n")
        
        file.write("</body></html>\n")

# converts the dot files to another format and deletes the source files
def convert_dot_files(output_folder, file_type):
    for dot_file in pathlib.Path(output_folder).glob('*.dot'):
        result_file = dot_file.with_suffix('.%s' % (file_type))
        result = subprocess.run(['dot', '-T%s' % (file_type), str(dot_file), '-o', str(result_file)])
        if result.returncode:
            raise Exception("dot process exited with return code %d, file: %s" % (result.returncode, str(dot_file)))
        os.remove(dot_file)

################################################################################
if __name__ == '__main__':
    # Argument parser setup
    parser = argparse.ArgumentParser(description="Create dependency graphs of internal crates.")
    parser.add_argument('--target-folder', default="output", help="The path to the target folder.")
    parser.add_argument('--skip-dev-dependencies', action='store_true', help="Whether or not to include the `dev-dependencies`.")

    args = parser.parse_args()
    target_folder = args.target_folder
    target_folder = os.path.abspath(os.path.expanduser(target_folder))

    base_path = pathlib.Path("../../").absolute().resolve()

    print("Target folder: %s" % (target_folder))
    if args.skip_dev_dependencies:
        print("The output is generated with skipped dev-dependencies.")
    else:
        print("The output is generated with dev-dependencies.")

    # parse the code owners file
    print("Parsing the CODEOWNERS file...")
    code_owners = CodeOwners(os.path.join(base_path, '.github', 'CODEOWNERS'))
    
    # run the cargo tree binary
    print("Creating dependency tree via 'cargo tree'...")
    cargo_tree_output = run_cargo_tree(args.skip_dev_dependencies)
    
    # parse the cargo tree
    print("Parsing the cargo tree...")
    crates_dict = parse_cargo_tree(cargo_tree_output, base_path, code_owners, args.skip_dev_dependencies)
    
    # create the output folder if it doesn't exist
    pathlib.Path(target_folder).mkdir(parents=True, exist_ok=True)

    # generate the DOT files
    print("Generating DOT files...")
    generate_dot_all(target_folder, crates_dict)
    generate_dot_per_crate(target_folder, crates_dict, False)
    generate_dot_per_crate(target_folder, crates_dict, True)

    # Convert DOT files to SVG
    print("Converting DOT files to SVG...")
    convert_dot_files(target_folder, "svg")

    # Add hyperlinks to all SVG files for easier navigation
    print("Adding hyperlinks to SVG files...")
    for svg_file in pathlib.Path(target_folder).glob('*.svg'):
        add_hyperlinks_to_svg(svg_file, crates_dict)

    # Create index.html for better overview
    print("Creating index.html...")
    create_index_html(target_folder)

    print(f"Generating graphs done. Open {target_folder}/index.html in your browser to view the dependency graphs.")