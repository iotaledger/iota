import subprocess
import os
import argparse
import re
import pathlib
import shutil

def is_commit_hash(ref):
    # A commit hash is a 40-character hexadecimal string
    return bool(re.match(r'^[0-9a-f]{40}$', ref))

# Clone a repository (either from a URL or a local folder)
def clone_repo(repo_url, repo_tag, target_folder):
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
            # Copy the local folder to the target folder
            shutil.copytree(
                repo_url_exp,
                target_folder,
                symlinks=True
            )
    else:
        # Clone the repository, the tag can be used as the branch name directly to checkout a specific tag in one step
        subprocess.run(["git", "clone", "--single-branch", "--branch", repo_tag, repo_url, target_folder], check=True)

    # Change working directory to the cloned repo
    os.chdir(target_folder)

def convert_iota_to_sui_folders(folders):
    premap = {
        'crates/iota-common': 'crates/mysten-common',
        'crates/iota-metrics': 'crates/mysten-metrics',
        'crates/iota-network-stack': 'crates/mysten-network',
        'crates/iota-util-mem': 'crates/mysten-util-mem',
        'crates/iota-util-mem-derive': 'crates/mysten-util-mem-derive',
    }

    mapped_folders = []
    for folder in folders:
        if folder in premap:
            folder = premap[folder]
        mapped_folders.append(folder.replace('iota', 'sui'))

    return mapped_folders

# Parse the CODEOWNERS file and return the folders of the code owner
def get_folders_for_code_owner(file_path, code_owner):
    folder_to_owners = {}
    matched_folders = []
    with open(file_path, 'r') as file:
        for line in file:
            line = line.strip()
            if line and not line.startswith('#'):
                folder, *owners = line.split()
                folder_to_owners[folder] = owners

    for folder, owners in folder_to_owners.items():
        if folder == "*":
            # skip the fallback here, we want to check all other patterns first
            continue

        # Check if the pattern matches the relative path of the code owner
        regex_pattern = '.*' + re.escape(code_owner) + '.*'
        for owner in owners:
            if re.match(regex_pattern, owner):
                matched_folders.append(folder.strip('/'))
                break
    
    return convert_iota_to_sui_folders(matched_folders)

# Get the commits of a folder in the specified range
def get_folder_commits(folder, start_ref, end_ref):
    # Check if start_ref and end_ref are commit hashes or tags
    if not is_commit_hash(start_ref):
        subprocess.run(["git", "fetch", "origin", "tag", start_ref], check=True)
    if not is_commit_hash(end_ref):
        subprocess.run(["git", "fetch", "origin", "tag", end_ref], check=True)
    
    # Define the git log command
    git_log_command = ["git", "log", f"{start_ref}..{end_ref}", "--format=format:https://github.com/MystenLabs/sui/commit/%H", "--", folder]
    # Execute the git log command and collect the output
    result = subprocess.run(git_log_command, capture_output=True, text=True)
    git_log_output = result.stdout.strip().split('\n') if result.stdout.strip() else []

    return git_log_output

def analyze_folder_commits(start_ref, end_ref, folders):
    print(f"SINCE: {start_ref}")
    print(f"UNTIL: {end_ref}")
    print(f"FOLDERS: {', '.join(folders)}")
    # Only insert non-empty lists into crates_commits
    folders_commits = {}
    for folder in folders:
        commits = get_folder_commits(folder, start_ref, end_ref)
        if commits: 
            folders_commits[folder] = commits

    # Find duplicate commits
    non_empty_folders = list(folders_commits.keys())
    duplicate_commits = set(
        commit
        for i, folder1 in enumerate(non_empty_folders)
        for folder2 in non_empty_folders[i + 1:]
        for commit in set(folders_commits[folder1]).intersection(folders_commits[folder2])
    )

    # Remove duplicate commits from each folder
    for folder in folders_commits:
        folders_commits[folder] = [commit for commit in folders_commits[folder] if commit not in duplicate_commits]

    # Print the name of the folder, number of commits contained, and the commits
    for folder, commits in folders_commits.items():
        if commits:
            print(f"\n\n## {folder} ({len(commits)})")
            for commit in reversed(commits):
                print(f"- {commit}")

    # Print the duplicate commits
    print(f"\n\n## Cross-folder commits ({len(duplicate_commits)})")
    for commit in reversed(list(duplicate_commits)):
        print(f"- {commit}")


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description='Track upstream commits for specified folders.')
    parser.add_argument('--since', required=True, help='Start commit hash or the tag for git log (e.g., "bb778828e36d53a7d91a27e55109f2f45621badc", "mainnet-v1.32.2"), it is EXCLUDED from the results.')
    parser.add_argument('--until', required=True, help='End commit hash or the tag for git log (e.g., "3ada97c109cc7ae1b451cb384a1f2cfae49c8d3e", "mainnet-v1.36.2"), it is INCLUDED in the results.')
    parser.add_argument('--folders', nargs='+', help='List of folders relative to the project root to track (e.g., "crates/iota-core crates/iota-node").')
    parser.add_argument('--codeowner', help='code owner of the folders (e.g., "node")')
    parser.add_argument('--repo-url', default="git@github.com:MystenLabs/sui.git", help="The URL to the repository. Can also be a local folder.")
    parser.add_argument('--repo-tag', default=None, help="The tag to checkout in the repository.")
    parser.add_argument('--target-folder', default="result", help="The path to the target folder.")
    parser.add_argument('--clone-source', action='store_true', help="Clone the upstream repository.")

    args = parser.parse_args()
    target_folder = args.target_folder
    target_folder = os.path.abspath(os.path.expanduser(target_folder))
    
    # Check if clone_source is true and repo_tag is not specified
    if args.clone_source and not args.repo_tag:
        parser.error("--repo-tag must be specified if --clone-source is true")

    # Check if folders or code owner is specified
    if not args.folders and not args.codeowner:
        print("No crates or code owner specified.")
        exit(1)

    folders = []
    if args.folders:
        folders = convert_iota_to_sui_folders(args.folders)

    if args.codeowner:
        print("Parsing the CODEOWNERS file...")
        # Get crates of the code owner
        base_path = pathlib.Path("../../").absolute().resolve()
        folders.extend(get_folders_for_code_owner(os.path.join(base_path, '.github', 'CODEOWNERS'), args.codeowner))

    if args.clone_source:
        # remove the target folder if it exists
        if os.path.exists(target_folder):
            shutil.rmtree(target_folder)

        # Clone the repository
        clone_repo(
            args.repo_url,
            args.repo_tag,
            target_folder,
        )
    else:
        # Change working directory to the target folder
        os.chdir(target_folder)

    # Analyze the commits of the folders
    analyze_folder_commits(args.since, args.until, folders)