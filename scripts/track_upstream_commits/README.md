# Track Upstream Commits

This script identifies all commits for the crates managed by the specified code owner and the provided crates within the provided commit hashes.

## Usage

```bash
usage: track_upstream_commits.py [-h] --since SINCE --until UNTIL [--crates CRATES [CRATES ...]]
                                 [--codeowner CODEOWNER] [--repo-url REPO_URL] [--repo-tag REPO_TAG]
                                 [--version VERSION] [--target-folder TARGET_FOLDER] [--clone-source]
                                 [--compare-source-folder COMPARE_SOURCE_FOLDER]

Track upstream commits for specified crates.

options:
  -h, --help            show this help message and exit
  --since SINCE         Start commit hash for git log (e.g., "bb778828e36d53a7d91a27e55109f2f45621badc").
  --until UNTIL         End commit hash for git log (e.g., "3ada97c109cc7ae1b451cb384a1f2cfae49c8d3e"),
                        it is included in the results.
  --crates CRATES [CRATES ...]
                        List of crates to track (e.g., "iota-core iota-node").
  --codeowner CODEOWNER
                        code owner of the crates (e.g., "node)
  --repo-url REPO_URL   The URL to the repository. Can also be a local folder.
  --repo-tag REPO_TAG   The tag to checkout in the repository.
  --version VERSION     The semantic version to filter overwrites/patches if not found in the repo-tag.
  --target-folder TARGET_FOLDER
                        The path to the target folder.
  --clone-source        Clone the upstream repository.
  --compare-source-folder COMPARE_SOURCE_FOLDER
                        The path to the source folder for comparison.
```

## Example

Either codeowner or crates must be provided. If both are provided, the script will aggregate the results from both.

input:

```bash
./run.sh --since bb778828e36d53a7d91a27e55109f2f45621badc --until 3ada97c109cc7ae1b451cb384a1f2cfae49c8d3e --crates iota-bridge --co
deowner node
```

output:

The results include the `iota-bridge` and all the crates that are managed by the `node` team.

```
Not in a virtual environment. Activating...
Parsing the CODEOWNERS file...
SINCE: bb778828e36d53a7d91a27e55109f2f45621badc
UNTIL: 3ada97c109cc7ae1b451cb384a1f2cfae49c8d3e
CRATES: crates/sui-bridge, docker, crates/sui-archival, crates/sui-authority-aggregation, crates/sui-config, crates/sui-core, crates/sui-network, crates/sui-network-stack, crates/sui-node, crates/sui-types, crates/sui-protocol-config, crates/sui-protocol-config-macros, crates/sui-rest-api, crates/sui-snapshot, crates/sui-storage


## crates/sui-bridge (21)
- https://github.com/MystenLabs/sui/commit/31b15dde1758a6ba7d7029ecbd74804180f4800c
- https://github.com/MystenLabs/sui/commit/2c1b6e24d25b219aa3272e0d9bed89e06b9bc629
- https://github.com/MystenLabs/sui/commit/d6adff2b8c8f1a14291122c0a510ebb1abb7300c
- https://github.com/MystenLabs/sui/commit/df41d44893038acd21c791df1329c7f3a588a32b
...


## docker (4)
- https://github.com/MystenLabs/sui/commit/6b231597e707bae887ca038d670ba3aa02775d37
- https://github.com/MystenLabs/sui/commit/037f13e3e413dced1ea6d6ac6b52d7ac27642ba8
...


## crates/sui-archival (1)
- https://github.com/MystenLabs/sui/commit/72603de6260795d5c9ed60f885a4ebe717a9430e


## crates/sui-config (15)
- https://github.com/MystenLabs/sui/commit/c3562a362bc04802e7ae074ab9947fa9697e4488
- https://github.com/MystenLabs/sui/commit/e920c3e0cfc8673e0858c69a94d8bbc261b0fa27

...
```
