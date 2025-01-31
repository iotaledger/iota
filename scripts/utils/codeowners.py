# Copyright (c) 2024 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0
import os, fnmatch, argparse

# Parses the CODEOWNERS file and holds all possible codeowners
class CodeOwners(object):
    def __init__(self, file_path, path_mapping_func=None, pattern_mapping_func=None):
        self.path_mapping_func = path_mapping_func

        self.ignored_dirs_patterns=[
            '/.cargo/',
            '/.config/',
            '/.git/',
            '/.pnpm_store/',
            '/.vscode/',
            '/chocolatey/',
            '/target/',
        ]

        # helper class
        class PatternWithOwners(object):
            def __init__(self, pattern, owners):
                self.pattern = pattern
                self.owners = owners

        self.default_owners = None
        self.patterns        = []
        with open(file_path, 'r') as file:
            for line in file:
                line = line.strip()

                # Skip empty lines and comments
                if not line or line.startswith('#'):
                    continue

                glob_pattern, *owners = line.split()

                # If the pattern is a wildcard, set the default owners
                if glob_pattern == "*":
                    self.default_owners = owners
                    continue

                if pattern_mapping_func:
                    glob_pattern = pattern_mapping_func(glob_pattern)

                # Insert the glob_pattern at the beginning of the list to prioritize it
                # Last match wins in the CODEOWNERS file
                self.patterns.insert(0, PatternWithOwners(glob_pattern, owners))

    # matches a path against a pattern
    # if the pattern starts with `/`, match it relative to root_path.
    # otherwise, match it anywhere in the repository hierarchy.
    def match_pattern(self, path, pattern):
        if pattern.startswith("/"):
            # this a path relative to the root folder
            pattern = pattern.lstrip("/")
        else:
            # this is a pattern that can match anywhere in the hierarchy, so we "unroot" it
            pattern = f"**/{pattern}"

        # remove trailing slashes
        path = path.rstrip("/")
        pattern = pattern.rstrip("/")
 
        return fnmatch.fnmatch(os.path.normpath(path), pattern)

    # returns the owners for a specific path or the default owners if 
    # no match is found and match_default is True, otherwise None
    def match_owners_for_path(self, path, match_default=True):
        if self.path_mapping_func:
            path = self.path_mapping_func(path)

        for pattern_with_owners in self.patterns:
            if self.match_pattern(path, pattern_with_owners.pattern):
                return pattern_with_owners.owners
        
        if match_default and self.default_owners:
            return self.default_owners
    
        return None
    
    # scans the folder structure for folders matching specific codeowners
    # and returns the paths of the matched folders.
    # Subfolders are not scanned further if a match is found. (except wildcard)
    def match_paths_for_owners(self, root_folder, owners):
        def get_relative_path_to_root_folder(path):
            relative = os.path.relpath(path, root_folder)
            if self.path_mapping_func:
                relative = self.path_mapping_func(relative)
            
            return relative

        def is_ignored_dir(path):
            return any(self.match_pattern(path, ignored_dir) for ignored_dir in self.ignored_dirs_patterns)

        matched_relative_paths_filtered = {}    # the results for the specified owners
        matched_relative_paths_all = {}         # needed to filter unmatched directories in the second pass
        unmatched_directories = []              # track directories that might need default owners

        # first Pass: traverse and match folders with specified owners
        for root, dirs, _ in os.walk(root_folder):
            relative_root = get_relative_path_to_root_folder(root)
            if is_ignored_dir(relative_root):
                dirs.clear()    # don't walk into the directory if it should be ignored
                continue

            for subfolder in list(dirs):
                relative_subfolder = get_relative_path_to_root_folder(os.path.join(root, subfolder))
                if is_ignored_dir(relative_subfolder):
                    dirs.remove(subfolder)  # don't walk into the directory if it should be ignored
                    continue
                
                # check if this subfolder matches any owner
                matched_owners = self.match_owners_for_path(relative_subfolder, match_default=False)
                if matched_owners:
                    # check if any of the matched owners is in the specified owners
                    if any(matched_owner in owners for matched_owner in matched_owners):
                        if relative_subfolder not in matched_relative_paths_all:
                            matched_relative_paths_all[relative_subfolder] = None
                        
                        if relative_subfolder not in matched_relative_paths_filtered:
                            matched_relative_paths_filtered[relative_subfolder] = []
                        matched_relative_paths_filtered[relative_subfolder].extend(matched_owners)
                        
                        dirs.remove(subfolder)  # stop traversing this directory, there was a direct match
                    else:
                        if relative_subfolder not in matched_relative_paths_all:
                            matched_relative_paths_all[relative_subfolder] = None

                        # not owned by the specified owners, don't walk into the directory
                        dirs.remove(subfolder)
                else:
                    # add to unmatched directories for second pass
                    unmatched_directories.append((root, subfolder))

        # second Pass: handle directories that should get default owners
        for root, subfolder in unmatched_directories:
            relative_root = get_relative_path_to_root_folder(root)
            if is_ignored_dir(relative_root):
                continue

            relative_subfolder = get_relative_path_to_root_folder(os.path.join(root, subfolder))

            # we need to check if any of the subdirectories have a match with a defined owner
            has_matching_subdirectory = any(
                matched_relative_path.startswith(relative_subfolder) and matched_relative_path != relative_subfolder for matched_relative_path in list(matched_relative_paths_all)
            )

            # we also need to check if the parent directory was already matched in the second pass
            has_matching_parent = any(
                relative_subfolder.startswith(matched_relative_path) and matched_relative_path != relative_subfolder for matched_relative_path in list(matched_relative_paths_all)
            )

            if has_matching_subdirectory or has_matching_parent:
                continue

            # assign default owners if specified
            default_owners = self.match_owners_for_path(relative_subfolder, match_default=True)
            
            # check if any of the matched owners is in the specified owners
            if default_owners and any(owner in owners for owner in default_owners):
                if relative_subfolder not in matched_relative_paths_all:
                    matched_relative_paths_all[relative_subfolder] = None
                
                if relative_subfolder not in matched_relative_paths_filtered:
                    matched_relative_paths_filtered[relative_subfolder] = []
                matched_relative_paths_filtered[relative_subfolder].extend(default_owners)

        # remove the leading slash in the keys
        return matched_relative_paths_filtered

################################################################################
if __name__ == "__main__":
    # Argument parser setup
    parser = argparse.ArgumentParser(description="Tool to search for folders in the repository matching specific code owners.")
    parser.add_argument('--target-folder', default="../../", help="The path to the target folder.")
    parser.add_argument('--codeowners', default="@iotaledger/node @iotaledger/core-protocol", nargs='+', help='Code owners of the folders (e.g., "@iotaledger/node @iotaledger/consensus").')
    args = parser.parse_args()

    code_owners = CodeOwners(os.path.join(args.target_folder, ".github/CODEOWNERS"))
    paths_with_owners = code_owners.match_paths_for_owners(args.target_folder, args.codeowners)

    print(f"Found {len(paths_with_owners)} paths for code owners {args.codeowners}:")
    for path, owners in paths_with_owners.items():
        print(f"   {path}: {owners}")
