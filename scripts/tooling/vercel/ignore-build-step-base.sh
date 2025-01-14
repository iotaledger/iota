#!/bin/bash

cd "$(dirname "$0")"

# Check for excluded branches and exit if necessary
bash check-excluded-branches.sh
