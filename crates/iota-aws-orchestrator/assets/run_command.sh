#!/bin/bash

# Script to execute commands on all testbed Node machines
# Usage: ./run_command.sh <command> [args...]
# example: run_command.sh \
# 'ADDR=$(grep -m 1 admin node.log | sed -E "s/.*address=([^ ]+).*/\1/"); \
# curl -X POST "http://$ADDR/spammer/start?tps=20&mean_size=30000&std_dev_size=3000"'
# collects the admin address from the node.log and executes the curl admin command

set -e

# Check if at least one argument is provided
if [ $# -eq 0 ]; then
    echo "Usage: $0 <command> [args...]"
    echo "Example: $0 apt update -y"
    exit 1
fi

# Get the command to execute (all script arguments)
REMOTE_COMMAND="$@"

echo "Getting testbed status..."
# Run the orchestrator and capture output
ORCHESTRATOR_OUTPUT=$(cargo run --bin iota-aws-orchestrator -- testbed status 2>&1)

# Extract SSH commands ONLY for [Node   ] lines
# Then add SSH options to skip host key checking and known_hosts updates
SSH_COMMANDS=$(
    echo "$ORCHESTRATOR_OUTPUT" \
    | grep "\[Node" \
    | grep -o "ssh -i [^ ]* [^ ]*@[0-9\.]*" \
    | sed 's/^ssh /ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=\/dev\/null /' \
    | sort -u
)

# Count the number of node machines
MACHINE_COUNT=$(echo "$SSH_COMMANDS" | sed '/^\s*$/d' | wc -l)

if [ "$MACHINE_COUNT" -eq 0 ]; then
    echo "Error: No Node SSH commands found in orchestrator output"
    exit 1
fi

echo "Found $MACHINE_COUNT node machines"
echo "Executing command: $REMOTE_COMMAND"
echo "----------------------------------------"

# Create a temporary directory for logs
LOG_DIR=$(mktemp -d)
echo "Logs will be stored in: $LOG_DIR"

# Counter for progress
COUNTER=0

# Execute command on all node machines in parallel
while IFS= read -r SSH_CMD; do
    # skip empty lines just in case
    [ -z "$SSH_CMD" ] && continue

    COUNTER=$((COUNTER + 1))
    HOST=$(echo "$SSH_CMD" | grep -oP '[^@\s]+@\K[0-9.]+')

    # Execute in background and log output
    (
        echo "[$COUNTER/$MACHINE_COUNT] Executing on $HOST..."
        if $SSH_CMD "$REMOTE_COMMAND" > "$LOG_DIR/$HOST.log" 2>&1; then
            echo "[$COUNTER/$MACHINE_COUNT] ✓ Success on $HOST"
        else
            echo "[$COUNTER/$MACHINE_COUNT] ✗ Failed on $HOST (see $LOG_DIR/$HOST.log)"
        fi
    ) &
done <<< "$SSH_COMMANDS"

# Wait for all background jobs to complete
wait

echo "----------------------------------------"
echo "Execution complete!"
echo "Logs available in: $LOG_DIR"
echo ""
echo "To view logs for a specific host:"
echo "  cat $LOG_DIR/<host-ip>.log"
echo ""
echo "To view all logs:"
echo "  cat $LOG_DIR/*.log"
