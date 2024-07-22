# Copyright (c) 2024 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

# This script creates dependency graphs between the internal crates.
# It outputs SVG files and an index.html.

import re, pathlib, os, subprocess
from bs4 import BeautifulSoup

OUTPUT_FOLDER = "dependency_graphs"

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

def parse_cargo_tree(file_path, base_path):
    with open(file_path, 'r') as file:
        lines = file.readlines()

    dependencies = {}

    # get all internal crates first
    for line in lines:
        depth = calculate_graph_depth(line)
        if depth != 0:
            continue

        crate_name = get_internal_crate_name(base_path, line)
        if crate_name == None:
            continue
        
        dependencies[crate_name] = []
    
    # loop again to determine the dependencies
    stack = []
    for line in lines:
        crate_name = get_internal_crate_name(base_path, line)
        if crate_name == None:
            continue

        depth = calculate_graph_depth(line)
        if depth == 0:
            stack = [crate_name]
        else:
            parent = stack[depth - 1]
            dependencies[parent].append(crate_name)
            if depth < len(stack):
                stack[depth] = crate_name
            else:
                stack.append(crate_name)
        
    return dependencies

# create one file to rule them all
def generate_dot_all(dependencies):
    with open('%s/all.dot' % (OUTPUT_FOLDER), 'w') as file:
        file.write("digraph dependencies {\n")
        for parent, children in dependencies.items():
            for child in children:
                file.write(f'    "{parent}" -> "{child}";\n')
        file.write("}\n")

# but there are too many of them, so let's go into detail
def generate_dot_per_crate(dependencies):
    # root level
    for parent, children in dependencies.items():
        if len(children) == 0:
            continue
        
        with open('%s/%s.dot' % (OUTPUT_FOLDER, parent), 'w') as file:
            file.write("digraph dependencies {\n")
            for child in children:
                file.write(f'    "{parent}" -> "{child}";\n')
            file.write("}\n")

# the dot command line tool didn't convert URL or href to hyperlinks, so we have to do it ourselves
def add_hyperlinks_to_svg(svg_file, dependencies):
    with open(svg_file, 'r') as file:
        svg_content = file.read()

    soup = BeautifulSoup(svg_content, 'xml')
    nodes = soup.find_all('g', {'class': 'node'})

    for i, node in enumerate(nodes):
        title = node.find('title')
        if title and title.string in dependencies and len(dependencies[title.string]) > 0:
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
        else:
            # Set the title color to red
            for text_element in node.find_all('text'):
                text_element['style'] = 'fill:red'

    with open(svg_file, 'w') as file:
        file.write(str(soup))

# create an overview index.html
def create_index_html(folder):
    index_file_path = pathlib.Path(folder) / "index.html"
    with open(index_file_path, 'w') as file:
        file.write("<html><body>\n")
        file.write("<h1>IOTA-Rebased Dependency Graphs</h1>\n")
        file.write("<ul>\n")
        for svg_file in pathlib.Path(folder).glob('*.svg'):
            file.write(f'<li><a href="{svg_file.name}">{svg_file.stem}</a></li>\n')
        file.write("</ul>\n")
        file.write("</body></html>\n")

if __name__ == '__main__':
    # Create the output folder if it doesn't exist
    pathlib.Path(OUTPUT_FOLDER).mkdir(parents=True, exist_ok=True)

    # Run the cargo tree command and save the output to a file
    tree_file_path = f"{OUTPUT_FOLDER}/tree.txt"
    with open(tree_file_path, 'w') as tree_file:
        result = subprocess.run(['cargo', 'tree'], stdout=tree_file)
        if result.returncode:
            raise "cargo tree process exited with return code %d" % (result.returncode)
    
    # Parse the cargo tree and generate the DOT file
    dependencies = parse_cargo_tree('%s/tree.txt' % (OUTPUT_FOLDER), pathlib.Path("../").absolute().resolve())
    
    generate_dot_all(dependencies)
    generate_dot_per_crate(dependencies)

    # Convert DOT files to SVG
    for dot_file in pathlib.Path(OUTPUT_FOLDER).glob('*.dot'):
        svg_file = dot_file.with_suffix('.svg')
        result = subprocess.run(['dot', '-Tsvg', str(dot_file), '-o', str(svg_file)])
        if result.returncode:
            raise "dot process exited with return code %d" % (result.returncode)
        os.remove(dot_file)

    # Add hyperlinks to all SVG files for easier navigation
    for svg_file in pathlib.Path(OUTPUT_FOLDER).glob('*.svg'):
        add_hyperlinks_to_svg(svg_file, dependencies)

    # Create index.html for better overview
    create_index_html(OUTPUT_FOLDER)