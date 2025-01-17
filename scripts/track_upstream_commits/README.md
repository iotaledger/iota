# Track Upstream Commits

This script identifies all commits for the folders managed by the specified code owner and the provided folders within the provided commit hashes.

## Usage

```bash
usage: track_upstream_commits.py [-h] --since SINCE --until UNTIL [--folders FOLDERS [FOLDERS ...]]
                                 [--codeowner CODEOWNER] [--repo-url REPO_URL] [--repo-tag REPO_TAG]
                                 [--target-folder TARGET_FOLDER] [--clone-source]

Track upstream commits for specified folders.

options:
  -h, --help            show this help message and exit
  --since SINCE         Start commit hash or the tag for git log (e.g., "bb778828e36d53a7d91a27e55109f2f45621badc", "mainnet-v1.32.2"), it is EXCLUDED from the
                        results.
  --until UNTIL         End commit hash or the tag for git log (e.g., "3ada97c109cc7ae1b451cb384a1f2cfae49c8d3e", "mainnet-v1.36.2"), it is INCLUDED in the results.
  --folders FOLDERS [FOLDERS ...]
                        List of folders relative to the project root to track (e.g., "crates/iota-core crates/iota-node").
  --codeowner CODEOWNER
                        code owner of the folders (e.g., "node")
  --repo-url REPO_URL   The URL to the repository. Can also be a local folder.
  --repo-tag REPO_TAG   The tag to checkout in the repository.
  --target-folder TARGET_FOLDER
                        The path to the target folder.
  --clone-source        Clone the upstream repository.
```

## Example

Either codeowner or folders must be provided. If both are provided, the script will aggregate the results from both.

input:

```bash
./run.sh --since bb778828e36d53a7d91a27e55109f2f45621badc --until 3ada97c109cc7ae1b451cb384a1f2cfae49c8d3e --crates crates/iota-bridge --co
deowner node
```

output:

The results include the `crates/iota-bridge` and all the folders that are managed by the `node` team.

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

Or use it with tags:

```
./run.sh --since mainnet-v1.32.2 --until mainnet-v1.36.2 --codeowner node
```
