# Copyright (c) 2024 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0
import os, re, subprocess, shutil

# Check if a reference is a commit hash
def is_commit_hash(ref):
    # A commit hash is a 40-character hexadecimal string
    return bool(re.match(r'^[0-9a-f]{40}$', ref))

# Run the git command to get the short commit message (title)
def get_commit_short_message(commit_hash):
    short_message = subprocess.check_output(
        ['git', 'show', '-s', '--format=%s', commit_hash],
        stderr=subprocess.STDOUT,
        text=True
    ).strip()
    return short_message

# Clone a repository (either from a URL or a local folder)
def clone_repo(repo_url, repo_tag, clone_history, target_folder, ignored_folders=[], ignored_files=[], ignored_file_types=[]):
    print(f"Cloning '{repo_url}' with tag '{repo_tag}' to '{target_folder}'...")
    repo_url_exp = os.path.expanduser(repo_url)

    # Check if the repository is a git repository or a local folder
    if os.path.exists(repo_url_exp):
        # Fetch the latest changes
        subprocess.run(["git", "fetch", "--all"], cwd=repo_url_exp, check=True)

        # Checkout the tag in the source folder
        subprocess.run(["git", "checkout", repo_tag], cwd=repo_url_exp, check=True)

        # helper function to check if the current reference is following a branch
        def is_following_branch():
            # Run the git command to check the current reference
            result = subprocess.run(
                ['git', 'rev-parse', '--abbrev-ref', 'HEAD'],
                cwd=repo_url_exp,
                capture_output=True,
                text=True,
                check=True
            )
            # If the result is 'HEAD', we're not following a branch (detached state or similar)
            return result.stdout.strip() != "HEAD"

        # Pull the latest changes
        if is_following_branch():
            print("   Pulling latest changes...")
            subprocess.run(["git", "pull"], cwd=repo_url_exp, check=True)

        # Check if the local folder equals the target folder
        if os.path.abspath(repo_url_exp) != os.path.abspath(target_folder):

            # Compile the regex patterns for ignored folders, files, and file types
            ignored_folders = [re.compile(pattern) for pattern in ignored_folders]
            ignored_files = [re.compile(pattern) for pattern in ignored_files]
            ignored_file_types = [re.compile(pattern) for pattern in ignored_file_types]

            # helper function to filter ignored items
            def filter_ignored_items(dir, contents):
                dir = os.path.relpath(dir, os.path.abspath(repo_url_exp))

                if any(pattern.search(dir) for pattern in ignored_folders):
                    print(f"   Skipping directory (regex): {dir}")
                    return contents
                
                ignored = []

                # Check if the item is a folder and matches any ignored folder pattern
                for item_name in contents:
                    item_path = os.path.join(dir, item_name)

                    if os.path.isdir(item_path):
                        if any(pattern.search(item_path) for pattern in ignored_folders):
                            print(f"   Skipping directory (regex): {item_path}")
                            ignored.append(item_name)
                            continue
                    else:
                        # Ignore specified file types using regex patterns
                        if any(pattern.search(os.path.splitext(item_name)[1]) for pattern in ignored_file_types):
                            print(f"   Skipping file (type regex): {item_path}")
                            ignored.append(item_name)
                            continue

                        # Ignore specified files using regex patterns
                        if any(pattern.search(item_name) for pattern in ignored_files):
                            print(f"   Skipping file (regex): {item_path}")
                            ignored.append(item_name)
                            continue

                return ignored

            # Copy the local folder to the target folder
            shutil.copytree(
                repo_url_exp,
                target_folder,
                ignore=filter_ignored_items,
                symlinks=True
            )
    else:
        # Clone the repository, the tag can be used as the branch name directly to checkout a specific tag in one step
        cmd = ["git", "clone"]
        
        # If the repo_tag is a commit hash, we need to checkout the commit hash after cloning
        if not is_commit_hash(repo_tag):
            if not clone_history:
                cmd += ["--depth", "1"]

            cmd += ["--single-branch", "--branch", repo_tag, repo_url, target_folder]

            subprocess.run(cmd, check=True)

            # Change working directory to the cloned repo
            os.chdir(target_folder)
        else:
            # Clone the repository without checking out the tag
            subprocess.run(cmd + [repo_url, target_folder], check=True)

            # Change the working directory to the target folder
            os.chdir(target_folder)

            # Checkout the tag in the target folder
            subprocess.run(["git", "checkout", repo_tag], check=True)
