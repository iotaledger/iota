#!/bin/bash

# Script to execute commands on some or all testbed Node machines
# Usage:
#   ./run_command.sh [--restart] <command> [args...]
#   ./run_command.sh --limit N [--restart] <command> [args...]
#
# Examples:
#   ./run_command.sh apt update -y
#
#   ./run_command.sh --limit 3 \
#     'ADDR=$(grep -m 1 admin node.log | sed -E "s/.*address=([^ ]+).*/\1/"); \
#      curl -X POST "http://$ADDR/spammer/start?tps=20&mean_size=30000&std_dev_size=3000"'
#
#   ./run_command.sh --restart "echo hello && sleep 5"
#
# The --limit/-n flag restricts execution to the first N Node machines.
# The --restart flag wraps the command in a tmux-restart sequence
# on the remote host.

set -e

LIMIT=""
WITH_TMUX_RESTART=0

print_usage() {
    echo "Usage: $0 [--limit N] [--restart] <command> [args...]"
    echo ""
    echo "Examples:"
    echo "  $0 apt update -y"
    echo "  $0 --limit 5 'echo hello from limited nodes'"
    echo "  $0 --restart \"echo hello && sleep 5\""
}

# --- Parse optional flags -----------------------------------------------------

while [[ $# -gt 0 ]]; do
    case "$1" in
        -n|--limit)
            if [[ -z "${2:-}" ]]; then
                echo "Error: --limit/-n requires a numeric argument"
                print_usage
                exit 1
            fi
            LIMIT="$2"
            shift 2
            ;;
        --restart)
            WITH_TMUX_RESTART=1
            shift
            ;;
        --help|-h)
            print_usage
            exit 0
            ;;
        --) # end of flags
            shift
            break
            ;;
        *)  # first non-flag argument = start of command
            break
            ;;
    esac
done

# Check that there is at least one argument left for the command
if [ $# -eq 0 ]; then
    print_usage
    exit 1
fi

# Get the command to execute (all remaining script arguments)
REMOTE_COMMAND="$@"

# Wrap command if we are doing the tmux restart dance
if [[ "$WITH_TMUX_RESTART" -eq 1 ]]; then
    # This builds a single remote shell command that will:
    #  1) Grab pane PID of node:0.0
    #  2) Extract its command
    #  3) Kill tmux session "node"
    #  4) Run the user command
    #  5) Start a new tmux session "node" with the previous command
    REMOTE_EFFECTIVE_COMMAND='set -x;PID=$(tmux display-message -p -t node:0.0 "#{pane_pid}") && CMD=$(ps -p "$PID" -o cmd= | cut -d " " -f6-) && tmux send-keys -t node:0.0 C-c  && sleep 30 && '"$REMOTE_COMMAND"' && tmux new -d -s node bash -lc "cd iota && $CMD"'
else
    REMOTE_EFFECTIVE_COMMAND="$REMOTE_COMMAND"
fi

echo "Getting testbed status..."
# Run the orchestrator and capture output
ORCHESTRATOR_OUTPUT=$(cargo run --bin iota-aws-orchestrator -- testbed status 2>&1)

# Extract SSH commands ONLY for [Node   ] lines
# Then add SSH options to skip host key checking and known_hosts updates
SSH_COMMANDS=$(
    echo "$ORCHESTRATOR_OUTPUT" \
    | grep "\[Node" \
    | grep -o "ssh -i [^ ]* [^ ]*@[0-9\.]*" \
    | sed 's/^ssh /ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=\/dev\/null /'
)

# Total machines before limiting
TOTAL_COUNT=$(echo "$SSH_COMMANDS" | sed '/^\s*$/d' | wc -l)

if [ "$TOTAL_COUNT" -eq 0 ]; then
    echo "Error: No Node SSH commands found in orchestrator output"
    exit 1
fi

# Apply limit if requested
if [[ -n "$LIMIT" ]]; then
    if ! [[ "$LIMIT" =~ ^[0-9]+$ ]]; then
        echo "Error: --limit must be an integer"
        exit 1
    fi

    if [ "$LIMIT" -le 0 ]; then
        echo "Error: --limit must be > 0"
        exit 1
    fi

    if [ "$LIMIT" -gt "$TOTAL_COUNT" ]; then
        echo "Warning: --limit ($LIMIT) is greater than available nodes ($TOTAL_COUNT), using all $TOTAL_COUNT nodes."
    else
        SSH_COMMANDS=$(echo "$SSH_COMMANDS" | sed '/^\s*$/d' | head -n "$LIMIT")
        echo "Limiting execution to $LIMIT node(s) out of $TOTAL_COUNT."
    fi
fi

# Count the number of *selected* node machines
MACHINE_COUNT=$(echo "$SSH_COMMANDS" | sed '/^\s*$/d' | wc -l)

echo "Found $TOTAL_COUNT node machines in total"
echo "Running on $MACHINE_COUNT node machine(s)"
echo "Executing command: $REMOTE_EFFECTIVE_COMMAND"
echo "----------------------------------------"

# Create a temporary directory for logs
LOG_DIR=$(mktemp -d)
echo "Logs will be stored in: $LOG_DIR"

# Counter for progress
COUNTER=0

# Execute command on selected node machines in parallel
while IFS= read -r SSH_CMD; do
    # skip empty lines just in case
    [ -z "$SSH_CMD" ] && continue

    COUNTER=$((COUNTER + 1))
    HOST=$(echo "$SSH_CMD" | grep -oP '[^@\s]+@\K[0-9.]+')

    # Execute in background and log output
    (
        echo "[$COUNTER/$MACHINE_COUNT] Executing on $HOST..."
        if $SSH_CMD "$REMOTE_EFFECTIVE_COMMAND" > "$LOG_DIR/$HOST.log" 2>&1; then
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
