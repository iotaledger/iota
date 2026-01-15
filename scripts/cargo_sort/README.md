# Cargo Sort

Cargo.toml dependency management tool with two modes:

1. **Sort Mode** (default): Sort dependencies into internal/external groups with comments
2. **Consolidate Mode**: Analyze and consolidate/distribute workspace dependencies

## Sort Mode (Default)

Scans all `Cargo.toml` files and:

- Separates internal (workspace) and external dependencies with comments
- Sorts dependencies alphabetically within each group

### Usage

```bash
./run_sort.sh [options]

# Or directly:
python cargo_sort.py [options]
```

### Options

```
--target TARGET    Target directory to search in (relative or absolute). Default: ../../
--ignore FOLDER    Folder patterns to ignore (can be specified multiple times)
--skip-dprint      Skip running dprint fmt
--skip-sort        Skip sort mode (useful with --consolidate-deps to only consolidate)
--debug            Show debug prints
```

## Consolidate Mode

Analyzes all dependencies across the workspace and:

- If an external dependency is used by **multiple crates**: adds it to root `[workspace.dependencies]` and updates crates to use `package.workspace = true`
- If an external dependency is used by **only one crate**: removes it from root and gives the crate the full version spec

**Note:** Sort mode runs automatically after consolidate mode (use `--skip-sort` to disable).

### Usage

```bash
./run_consolidate.sh [options]

# Or directly:
python cargo_sort.py --consolidate-deps [options]
```

### Options

```
--consolidate-deps     Enable consolidate mode
--target TARGET        Target directory to search in (relative or absolute). Default: ../../
--ignore FOLDER        Folder patterns to ignore (can be specified multiple times)
--min-usage N          Minimum usages to consolidate (default: 2)
--skip-dprint          Skip running dprint fmt
--skip-sort            Skip sort mode after consolidating
```

### Features

- **Version conflict resolution**: Picks the highest version when crates use different versions (with SemVer-compatible specs)
- **Feature merging**: Combines features from all usages into workspace definition
- **Version constraint validation**: Detects and errors on incompatible version constraints:

  | Constraint Type | Example               | Behavior                                         |
  | --------------- | --------------------- | ------------------------------------------------ |
  | Exact pins      | `=1.0.0`              | **ERROR** if higher version exists elsewhere     |
  | Upper bounds    | `<2.0.0`, `<=1.5.0`   | **ERROR** if higher version exists elsewhere     |
  | Bounded ranges  | `>=1.0, <2.0`         | **ERROR** if range conflicts with higher version |
  | Exclusions      | `!=1.5.0`             | **ERROR** if highest version is excluded         |
  | Wildcards       | `*`, `1.*`            | Skipped (very permissive)                        |
  | SemVer specs    | `1.0`, `^1.0`, `~1.0` | **WARN** and use highest version                 |

- **Conflict handling**:
  - RED error + **panic** for exact pins or upper bounds conflicting with higher versions
  - RED error + **panic** for mixed version/git specifications
  - RED error + **panic** for conflicting git revisions
  - YELLOW warning for multiple SemVer versions (uses highest)
  - YELLOW warning for conflicting `default-features` settings
- **Special section handling**: `build-dependencies` and `target.'cfg(...)'.dependencies` don't drive consolidation (but can still use workspace refs)

### Examples

```bash
# Run consolidate mode (will also sort after)
./run_consolidate.sh

# Consolidate with custom minimum usage threshold
./run_consolidate.sh --min-usage 3

# Only consolidate, don't sort
python cargo_sort.py --consolidate-deps --skip-sort

# Consolidate external-crates workspace separately
python cargo_sort.py --consolidate-deps \
  --target ../../external-crates/move
```
