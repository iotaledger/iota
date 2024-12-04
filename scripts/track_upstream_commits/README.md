# Track Upstream Commits

This script identifies all commits for the crates managed by the specified code owner within the provided time range.

## Usage

```bash
usage: track_upstream.py [-h] --since SINCE --until UNTIL --codeowner CODEOWNER [--repo-url REPO_URL] [--repo-tag REPO_TAG] [--version VERSION]
                         [--target-folder TARGET_FOLDER] [--clone-source] [--compare-source-folder COMPARE_SOURCE_FOLDER]

Track upstream commits for specified crates.

options:
  -h, --help            show this help message and exit
  --since SINCE         Start date for git log (e.g., "2024-09-05").
  --until UNTIL         End date for git log (e.g., "2024-10-26").
  --codeowner CODEOWNER
                        code owner of the crates (e.g., "core-node)
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

input:
```bash
./run.sh --since "2024-09-05" --until "2024-10-26" --codeowner "node"
```

output:
```
Parsing the CODEOWNERS file...
First commit: bb778828e36d53a7d91a27e55109f2f45621badc
Last commit: 29ff3e3f53fc7aa590da97d1033265645be1ea1a
SINCE: 2024-09-05
UNTIL: 2024-10-25
CRATES: docker, crates/sui-archival, crates/sui-authority-aggregation, crates/sui-config, crates/sui-core, crates/sui-network, crates/sui-network-stack, crates/sui-node, crates/sui-types, crates/sui-protocol-config, crates/sui-protocol-config-macros, crates/sui-rest-api, crates/sui-snapshot, crates/sui-storage


## docker (4)
- https://github.com/MystenLabs/sui/commit/6b231597e707bae887ca038d670ba3aa02775d37
- https://github.com/MystenLabs/sui/commit/037f13e3e413dced1ea6d6ac6b52d7ac27642ba8
- https://github.com/MystenLabs/sui/commit/e67a2f40db3c68d879ec723c7a73d3ba27f4099b
- https://github.com/MystenLabs/sui/commit/88c7c85453bff0f41492cf591a52c2781d681b68


## crates/sui-archival (1)
- https://github.com/MystenLabs/sui/commit/72603de6260795d5c9ed60f885a4ebe717a9430e


## crates/sui-authority-aggregation (1)
- https://github.com/MystenLabs/sui/commit/a9cd80942cf6c216be89dacdc790ac44f1213521

.....
```