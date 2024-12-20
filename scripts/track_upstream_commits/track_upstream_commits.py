import subprocess
import os
import argparse
import re
import pathlib
import shutil

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

# Parse the CODEOWNERS file and return the crates of the code owner
def get_crates_for_code_owner(file_path, code_owner):
    pattern_to_owners = {}
    matched_crates = []
    with open(file_path, 'r') as file:
        for line in file:
            line = line.strip()
            if line and not line.startswith('#'):
                pattern, *owners = line.split()
                pattern_to_owners[pattern] = owners

    for pattern, owners in pattern_to_owners.items():
        if pattern == "*":
            # skip the fallback here, we want to check all other patterns first
            continue

        # Check if the pattern matches the relative path of the code owner
        regex_pattern = '.*' + re.escape(code_owner) + '.*'
        for owner in owners:
            if re.match(regex_pattern, owner):
                matched_crates.append(pattern.replace('iota', 'sui').strip('/'))
                break
    
    return matched_crates
    
# Get the first and last commit of the specified range
def get_commit_range(since, until, crates):
    # Define the git log command
    git_log_command = ["git", "log", f"--since={since}", f"--until={until}", "--format=format:%H", "--"]
    git_log_command.extend(crates)

    # Execute the git log command and collect the output
    result = subprocess.run(git_log_command, capture_output=True, text=True)
    git_log_output = result.stdout.strip().split('\n')

    print(f"First commit: {git_log_output[-1]}")
    print(f"Last commit: {git_log_output[0]}")

# Get the commits of a crate in the specified range
def get_crate_commits(crate, since, until):
    # Define the git log command
    git_log_command = ["git", "log", f"--since={since}", f"--until={until}", "--format=format:https://github.com/MystenLabs/sui/commit/%H", "--", crate]
    # Execute the git log command and collect the output
    result = subprocess.run(git_log_command, capture_output=True, text=True)
    git_log_output = result.stdout.strip().split('\n') if result.stdout.strip() else []

    return git_log_output

def analyze_crate_commits(since, until, crates):
    print(f"SINCE: {since}")
    print(f"UNTIL: {until}")
    print(f"CRATES: {', '.join(crates)}")
    # Only insert non-empty lists into crates_commits
    crates_commits = {}
    for crate in crates:
        commits = get_crate_commits(crate, since, until)
        if commits: 
            crates_commits[crate] = commits

    # Find duplicate commits
    non_empty_crates = list(crates_commits.keys())
    duplicate_commits = set(
        commit
        for i, crate1 in enumerate(non_empty_crates)
        for crate2 in non_empty_crates[i + 1:]
        for commit in set(crates_commits[crate1]).intersection(crates_commits[crate2])
    )

    # Remove duplicate commits from each crate
    for crate in crates_commits:
        crates_commits[crate] = [commit for commit in crates_commits[crate] if commit not in duplicate_commits]

    # Print the name of the crate, number of commits contained, and the commits
    for crate, commits in crates_commits.items():
        if commits:
            print(f"\n\n## {crate} ({len(commits)})")
            for commit in commits:
                print(f"- {commit}")

    # Print the duplicate commits
    print(f"\n\n## Cross-crate commits ({len(duplicate_commits)})")
    for commit in duplicate_commits:
        print(f"- {commit}")


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description='Track upstream commits for specified crates.')
    parser.add_argument('--since', required=True, help='Start date for git log (e.g., "2024-09-05").')
    parser.add_argument('--until', required=True, help='End date for git log (e.g., "2024-10-26").')
    parser.add_argument('--codeowner', required=True, help='code owner of the crates (e.g., "core-node)')
    parser.add_argument('--repo-url', default="git@github.com:MystenLabs/sui.git", help="The URL to the repository. Can also be a local folder.")
    parser.add_argument('--repo-tag', default=None, help="The tag to checkout in the repository.")
    parser.add_argument('--version', default=None, help="The semantic version to filter overwrites/patches if not found in the repo-tag.")
    parser.add_argument('--target-folder', default="result", help="The path to the target folder.")
    parser.add_argument('--clone-source', action='store_true', help="Clone the upstream repository.")
    parser.add_argument('--compare-source-folder', help="The path to the source folder for comparison.")

    args = parser.parse_args()
    target_folder = args.target_folder
    target_folder = os.path.abspath(os.path.expanduser(target_folder))
    
    # Check if clone_source is true and repo_tag is not specified
    if args.clone_source and not args.repo_tag:
        parser.error("--repo-tag must be specified if --clone-source is true")

    # get current root folder
    source_folder = os.path.abspath(os.path.join(os.getcwd(), "..", ".."))
    if args.compare_source_folder:
        source_folder = os.path.abspath(args.compare_source_folder)

    # Get crates of the code owner
    base_path = pathlib.Path("../../").absolute().resolve()
    print("Parsing the CODEOWNERS file...")
    crates = get_crates_for_code_owner(os.path.join(base_path, '.github', 'CODEOWNERS'), args.codeowner)

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

    # Get the commit range
    get_commit_range(args.since, args.until, crates)

    # Analyze the commits of the crates
    analyze_crate_commits(args.since, args.until, crates)