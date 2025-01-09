# Cargo Sort

This python script sorts all dependencies in all `Cargo.toml` files in the repository (except `external-crates`) by internal and external dependencies.

## Usage

```bash
usage: run.sh [-h] [--target TARGET] [--skip-dprint] [--debug]

Format the Cargo.toml files and sort internal and external dependencies.

options:
  -h, --help       show this help message and exit
  --target TARGET  Target directory to search in.
  --skip-dprint    Skip running dprint fmt.
  --debug          Show debug prints.
```
