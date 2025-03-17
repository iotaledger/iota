#!/usr/bin/env python3

import sys
import re

# Check if a filename is provided
if len(sys.argv) != 2:
    print(f"Usage: {sys.argv[0]} <filename>")
    sys.exit(1)

filename = sys.argv[1]

# Regular expression to match the entire pattern in one go
pattern = re.compile(
    r"diesel::allow_tables_to_appear_in_same_query!\(\n"
    r"(.*)"  # Captures table names
    r"\);\n",
    re.DOTALL
)

# Function to format the replacement text
def replace_match(match):
    tables_block = match.group(1).rstrip(",\n")  # Remove last comma
    return (
        "#[macro_export]\n"
        "macro_rules! for_all_tables {\n"
        "    ($action:path) => {\n"
        "        $action!(\n"
        f"{tables_block}\n"
        "        );\n"
        "    };\n"
        "}\n"
        "pub use for_all_tables;\n"
        "for_all_tables!(diesel::allow_tables_to_appear_in_same_query);\n"
    )

# Read the file
with open(filename, "r") as file:
    content = file.read()

# Perform the replacement
new_content = pattern.sub(replace_match, content)

# Write the modified content back to the file
with open(filename, "w") as file:
    file.write(new_content)

print(f"Pattern replaced successfully in '{filename}'.")
