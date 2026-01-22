#!/usr/bin/env python3
# Copyright (c) 2025 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

import os
import sys
import time
import subprocess
import argparse
from pathlib import Path

# ---------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------
DEFAULT_NUM_VALIDATORS = 10
PAUSE_BETWEEN_PROTOCOLS = 60  # seconds

# ---------------------------------------------------------------------
# Paths
# ---------------------------------------------------------------------
SCRIPT_DIR = Path(__file__).resolve().parent
PRIVNET_DIR = SCRIPT_DIR.parents[3]
REPO_ROOT = PRIVNET_DIR.parents[1]
DOCKER_ROOT = REPO_ROOT / "docker"
FUZZER_DIR = PRIVNET_DIR / "fuzzer"

def run_command(cmd, cwd=None, check=True, shell=False):
    """Runs a shell command."""
    print(f"Running: {' '.join(cmd) if isinstance(cmd, list) else cmd}")
    try:
        subprocess.run(cmd, cwd=cwd, check=check, shell=shell)
    except subprocess.CalledProcessError as e:
        print(f"Error running command: {e}")
        sys.exit(1)

def build_images():
    """Builds the necessary Docker images."""
    print(">>> Building Docker images...")
    
    images = [
        ("iota-node", DOCKER_ROOT / "iota-node"),
        ("iota-tools", DOCKER_ROOT / "iota-tools"),
        ("iota-indexer", DOCKER_ROOT / "iota-indexer"),
    ]

    for name, path in images:
        print(f"Building {name}...")
        # Using sudo because the build scripts often require it or docker requires it
        run_command(["sudo", "./build.sh", "-t", name], cwd=path)


def discover_validator_count(default: int = DEFAULT_NUM_VALIDATORS) -> int:
    """Return the number of running validator containers or fall back to default."""
    try:
        result = subprocess.run(
            ["docker", "ps", "--format", "{{.Names}}"],
            capture_output=True,
            text=True,
            check=True,
        )
    except subprocess.CalledProcessError:
        print("Warning: docker ps failed; using default validator count")
        return default

    names = [line.strip() for line in result.stdout.splitlines() if line.strip().startswith("validator-")]
    if names:
        count = len(names)
        print(f"Detected {count} validator containers")
        return count

    print(f"No validator containers detected; using default count {default}")
    return default

def run_experiment(protocol):
    """Runs the non-triangle stress test for a specific protocol."""
    print(f"\n{'='*60}")
    print(f"=== Starting Non-Triangle Stress Test for {protocol.upper()} ===")
    print(f"{'='*60}\n")

    # 1. Cleanup existing network
    print(">>> Cleaning up existing network...")
    run_command(["sudo", "./cleanup.sh"], cwd=PRIVNET_DIR)

    num_validators = discover_validator_count()

    # 2. Bootstrap network
    print(f">>> Bootstrapping network for {protocol}...")
    # bootstrap.sh generates the configuration
    run_command(["sudo", "./bootstrap.sh", "-n", str(num_validators)], cwd=PRIVNET_DIR)

    # 3. Start network
    print(f">>> Starting network with {protocol}...")
    # run.sh starts the containers and sets the protocol
    run_command(["sudo", "./run.sh", "-n", str(num_validators), "-p", protocol], cwd=PRIVNET_DIR)

    # Wait for network to stabilize
    print(">>> Waiting for network to stabilize (20s)...")
    time.sleep(20)

    # 4. Run Non-Triangle Stress Test
    print(">>> Running Non-Triangle Stress Test (Python)...")
    
    # Ensure venv exists (simple check)
    venv_python = PRIVNET_DIR / ".venv" / "bin" / "python"
    pip_path = PRIVNET_DIR / ".venv" / "bin" / "pip"
    if not venv_python.exists():
        print("Creating Python venv...")
        run_command([sys.executable, "-m", "venv", str(PRIVNET_DIR / ".venv")])
        run_command([str(pip_path), "install", "--upgrade", "pip"])

    run_command([str(pip_path), "install", "-e", "."], cwd=FUZZER_DIR)

    # Run the fuzzer script
    # We use sudo because the fuzzer needs to manipulate docker/iptables
    # We execute the module net_fuzz.experiments.non_triangle_stress
    env = os.environ.copy()
    env["PYTHONPATH"] = str(FUZZER_DIR / "src")
    
    try:
        run_command(
            ["sudo", str(venv_python), "-m", "net_fuzz.experiments.non_triangle_stress"],
            cwd=FUZZER_DIR,
            check=True
        )
    except SystemExit:
        print(f"Test failed for {protocol}")
        # We don't exit here, we might want to continue or cleanup
        pass
    except Exception as e:
        print(f"An error occurred during the test: {e}")

    print(f"=== Finished Non-Triangle Stress Test for {protocol} ===")

    # 5. Cleanup
    print(">>> Cleaning up...")
    run_command(["sudo", "./cleanup.sh"], cwd=PRIVNET_DIR)

def main():
    parser = argparse.ArgumentParser(description="Run Non-Triangle Stress Test on Mysticeti and Starfish")
    parser.add_argument("--skip-build", action="store_true", help="Skip building docker images")
    args = parser.parse_args()

    if not args.skip_build:
        build_images()

    # Run Mysticeti
    run_experiment("mysticeti")

    print(f"Sleeping {PAUSE_BETWEEN_PROTOCOLS}s before next run...")
    time.sleep(PAUSE_BETWEEN_PROTOCOLS)

    # Run Starfish 
    run_experiment("starfish")

    print("\nAll non-triangle stress runs completed.")

if __name__ == "__main__":
    main()
