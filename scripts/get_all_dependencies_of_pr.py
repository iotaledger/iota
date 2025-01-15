import subprocess
import re
from datetime import datetime

# Configuration: Set these variables before running the script
REPO_PATH = "$HOME/works/sui"  # Source repository (Sui)
INITIAL_COMMIT = "a5eab1a"  # Initial commit of the last rebase
BASE_COMMIT = "fe8982b"  # Base commit of the PR in the Sui repository
TARGET_COMMIT = "045352d"  # Target commit of the PR in the Sui repository
REPO_URL = "https://github.com/MystenLabs/sui"  # Replace with Sui repository URL
OUTPUT_FILE = "pr_dependencies.txt"  # Output file to save the PR dependencies

def run_command(cmd):
    """Run a shell command and return its output."""
    try:
        result = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, check=True)
        return result.stdout.strip()
    except subprocess.CalledProcessError as e:
        print(f"Command failed: {' '.join(cmd)}\nError: {e.stderr}")
        return ""

def get_changed_files(base_commit, target_commit):
    """List changed files between base and target commits (base excluded, target included)."""
    cmd = ["git", "diff", "--name-only", f"{base_commit}..{target_commit}"]
    return run_command(cmd).splitlines()

def get_commit_history(file_path, initial_commit, target_commit):
    """Get the commit history for a file from initial to target commit."""
    cmd = ["git", "log", "--pretty=format:%H %s", "--follow", f"{initial_commit}..{target_commit}", "--", file_path]
    return run_command(cmd).splitlines()

def extract_pr_numbers(commit_message):
    """Extract PR numbers from commit messages."""
    pr_pattern = r"#(\d+)"  # Assuming PRs are referenced as (#123)
    return re.findall(pr_pattern, commit_message)

def get_pr_details(pr_number, initial_commit, target_commit):
    """Get details for a given PR, including its changed files and commit time."""
    cmd = ["git", "log", "--pretty=format:%H %ct", f"--grep=#{pr_number}", f"{initial_commit}..{target_commit}"]
    commit_info = run_command(cmd).splitlines()
    if not commit_info:
        return None
    latest_commit, commit_time = commit_info[0].split()
    changed_files = get_changed_files_from_pr(pr_number, initial_commit, target_commit)
    return {
        "pr_number": pr_number,
        "url": f"{REPO_URL}/pull/{pr_number}",
        "changed_files": sorted(changed_files) if changed_files else ["No files found"],
        "commit_time": int(commit_time)
    }

def get_changed_files_from_pr(pr_number, initial_commit, target_commit):
    """Get changed files for a given PR, filtered by the commit range."""
    cmd = ["git", "log", "--pretty=format:%H", f"--grep=#{pr_number}", f"{initial_commit}..{target_commit}"]
    pr_commits = run_command(cmd).splitlines()
    changed_files = set()
    for commit in pr_commits:
        cmd = ["git", "show", "--pretty=", "--name-only", commit]
        files = run_command(cmd).splitlines()
        changed_files.update(files)
    return changed_files

def get_all_dependent_prs(initial_commit, base_commit, target_commit):
    """Retrieve all dependent PRs for the changed files."""
    processed_files = set()
    processed_prs = set()
    all_prs = []
    files_to_process = set(get_changed_files(base_commit, target_commit))

    while files_to_process:
        new_files = set()
        for file in files_to_process:
            if file in processed_files:
                continue
            print(f"Processing file: {file}")
            processed_files.add(file)
            commit_history = get_commit_history(file, initial_commit, target_commit)
            for commit in commit_history:
                pr_numbers = extract_pr_numbers(commit)
                for pr_number in pr_numbers:
                    if pr_number not in processed_prs:
                        processed_prs.add(pr_number)
                        pr_details = get_pr_details(pr_number, initial_commit, target_commit)
                        if pr_details:
                            all_prs.append(pr_details)
                            pr_files = get_changed_files_from_pr(pr_number, initial_commit, target_commit)
                            new_files.update(pr_files - processed_files)
        files_to_process = new_files

    # Sort PRs by commit time (ascending)
    sorted_prs = sorted(all_prs, key=lambda x: x["commit_time"])
    return sorted_prs

def save_pr_dependencies_to_file(pr_dependencies, output_file):
    """Save the PR dependencies to a file."""
    with open(output_file, "w") as file:
        file.write("PR Dependencies (sorted by commit time):\n")
        for pr in pr_dependencies:
            commit_time = datetime.utcfromtimestamp(pr["commit_time"]).strftime('%Y-%m-%d %H:%M:%S')
            file.write(f"PR #{pr['pr_number']}: {pr['url']}\n")
            file.write(f"Commit Time: {commit_time}\n")
            file.write("Changed Files:\n")
            for changed_file in pr["changed_files"]:
                file.write(f"  - {changed_file}\n")
            file.write("\n")
    print(f"PR dependencies saved to {output_file}")

if __name__ == "__main__":
    # Navigate to the repository
    print(f"Starting to find dependent PRs...")
    subprocess.run(["cd", REPO_PATH], shell=True, check=False)

    # Retrieve all dependent PRs
    dependent_prs = get_all_dependent_prs(INITIAL_COMMIT, BASE_COMMIT, TARGET_COMMIT)

    # Save to file
    save_pr_dependencies_to_file(dependent_prs, OUTPUT_FILE)
