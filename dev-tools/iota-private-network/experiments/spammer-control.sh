#!/bin/bash

# Copyright (c) 2025 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

# Spammer Control Script
# Control the RandomBytes consensus transaction spammer across validators in the private network.
#
# Usage:
#   ./spammer-control.sh start [OPTIONS]  - Start spammer on validators
#   ./spammer-control.sh stop [OPTIONS]   - Stop spammer on validators
#   ./spammer-control.sh status [OPTIONS] - Show spammer status
#
# Options:
#   -v, --validators <range>  Validator selection (e.g., "1-4", "1,3,5", "all"). Default: "all"
#   -t, --tps <number>        Transactions per second (default: 10)
#   -m, --mean-size <bytes>   Mean transaction size in bytes (default: 10240)
#   -s, --std-dev <bytes>     Standard deviation in bytes (default: 1024)
#   -h, --help                Show this help message
#
# Examples:
#   ./spammer-control.sh start
#   ./spammer-control.sh start -v 1-3 -t 50 -m 50000 -s 5000
#   ./spammer-control.sh stop -v 1,3,5
#   ./spammer-control.sh status

set -euo pipefail

# =================== CONSTANTS ===================
ADMIN_PORT=1337
DEFAULT_TPS=10
DEFAULT_MEAN_SIZE=10240  # 10 KiB
DEFAULT_STD_DEV=1024     # 1 KiB
# ==================================================

# =================== COLORS ===================
if [[ -t 1 ]]; then
    RED='\033[0;31m'
    GREEN='\033[0;32m'
    YELLOW='\033[0;33m'
    BLUE='\033[0;34m'
    BOLD='\033[1m'
    NC='\033[0m' # No Color
else
    RED=''
    GREEN=''
    YELLOW=''
    BLUE=''
    BOLD=''
    NC=''
fi
# ==============================================

# =================== FUNCTIONS ===================

usage() {
    cat << EOF
Usage: $0 <command> [OPTIONS]

Commands:
  start   - Start spammer on selected validators
  stop    - Stop spammer on selected validators
  status  - Show spammer status for selected validators

Options:
  -v, --validators <range>  Validator selection (e.g., "1-4", "1,3,5", "all"). Default: "all"
  -t, --tps <number>        Transactions per second (default: $DEFAULT_TPS)
  -m, --mean-size <bytes>   Mean transaction size in bytes (default: $DEFAULT_MEAN_SIZE)
  -s, --std-dev <bytes>     Standard deviation in bytes (default: $DEFAULT_STD_DEV)
  -h, --help                Show this help message

Examples:
  $0 start
  $0 start -v 1-3 -t 50 -m 50000 -s 5000
  $0 stop -v 1,3,5
  $0 status
EOF
}

log() {
    echo -e "${BLUE}[$(date -Iseconds)]${NC} $1"
}

error() {
    echo -e "${RED}[ERROR]${NC} $1" >&2
}

success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

# Auto-detect running validators from Docker
detect_validators() {
    local validators
    validators=$(docker ps --filter "name=^validator-[0-9]" --format '{{.Names}}' 2>/dev/null | sort -V || true)

    if [[ -z "$validators" ]]; then
        error "No running validators detected. Please start the network first."
        exit 1
    fi

    # Extract validator numbers
    echo "$validators" | sed 's/validator-//' | tr '\n' ' '
}

# Parse validator range specification (e.g., "1-4", "1,3,5", "all")
parse_validator_range() {
    local range="$1"
    local available_validators="$2"
    local result=()

    if [[ "$range" == "all" ]]; then
        echo "$available_validators"
        return
    fi

    # Split by comma
    IFS=',' read -ra PARTS <<< "$range"

    for part in "${PARTS[@]}"; do
        if [[ "$part" =~ ^([0-9]+)-([0-9]+)$ ]]; then
            # Range specification (e.g., "1-4")
            local start="${BASH_REMATCH[1]}"
            local end="${BASH_REMATCH[2]}"
            for ((i=start; i<=end; i++)); do
                result+=("$i")
            done
        elif [[ "$part" =~ ^[0-9]+$ ]]; then
            # Single number
            result+=("$part")
        else
            error "Invalid validator specification: $part"
            exit 1
        fi
    done

    # Remove duplicates and sort
    printf '%s\n' "${result[@]}" | sort -n | uniq | tr '\n' ' '
}

# Validate that requested validators are actually running
validate_validators() {
    local requested="$1"
    local available="$2"
    local invalid=()

    for v in $requested; do
        if ! echo "$available" | grep -qw "$v"; then
            invalid+=("$v")
        fi
    done

    if [[ ${#invalid[@]} -gt 0 ]]; then
        warn "Validators not running: ${invalid[*]}"
        warn "Available validators: $available"
    fi

    # Return only valid validators
    for v in $requested; do
        if echo "$available" | grep -qw "$v"; then
            echo -n "$v "
        fi
    done
}

# Execute curl command inside a validator container
validator_curl() {
    local validator_num="$1"
    local method="$2"
    local endpoint="$3"
    local container="validator-$validator_num"

    docker exec "$container" curl -s -X "$method" "http://127.0.0.1:$ADMIN_PORT$endpoint" 2>/dev/null || echo "ERROR"
}

# Start spammer on a validator
start_spammer() {
    local validator_num="$1"
    local tps="$2"
    local mean_size="$3"
    local std_dev="$4"

    local endpoint="/spammer/start?tps=$tps&mean_size=$mean_size&std_dev_size=$std_dev"
    local response
    response=$(validator_curl "$validator_num" "POST" "$endpoint")

    if [[ "$response" == "ERROR" ]]; then
        error "validator-$validator_num: Failed to connect"
        return 1
    elif echo "$response" | grep -q "Spammer started"; then
        success "validator-$validator_num: Spammer started (TPS=$tps, mean_size=$mean_size, std_dev=$std_dev)"
        return 0
    else
        error "validator-$validator_num: $response"
        return 1
    fi
}

# Stop spammer on a validator
stop_spammer() {
    local validator_num="$1"
    local response
    response=$(validator_curl "$validator_num" "POST" "/spammer/stop")

    if [[ "$response" == "ERROR" ]]; then
        error "validator-$validator_num: Failed to connect"
        return 1
    elif echo "$response" | grep -q "Spammer stopped"; then
        success "validator-$validator_num: Spammer stopped"
        return 0
    else
        error "validator-$validator_num: $response"
        return 1
    fi
}

# Get spammer status from a validator
get_status() {
    local validator_num="$1"
    local response
    response=$(validator_curl "$validator_num" "GET" "/spammer/status")

    if [[ "$response" == "ERROR" ]]; then
        echo "ERROR|ERROR|ERROR|ERROR|ERROR|ERROR"
        return 1
    fi

    # Parse JSON response (basic bash parsing, no jq dependency)
    local enabled=$(echo "$response" | grep -oP '"enabled"\s*:\s*\K(true|false)' || echo "unknown")
    local tps=$(echo "$response" | grep -oP '"tps"\s*:\s*\K[0-9]+' || echo "0")
    local mean_size=$(echo "$response" | grep -oP '"mean_size"\s*:\s*\K[0-9]+' || echo "0")
    local std_dev=$(echo "$response" | grep -oP '"std_dev_size"\s*:\s*\K[0-9]+' || echo "0")
    local submitted=$(echo "$response" | grep -oP '"submitted"\s*:\s*\K[0-9]+' || echo "0")
    local errors=$(echo "$response" | grep -oP '"errors"\s*:\s*\K[0-9]+' || echo "0")

    echo "$enabled|$tps|$mean_size|$std_dev|$submitted|$errors"
}

# Display status in a table
display_status_table() {
    local validators="$1"

    echo -e "${BOLD}Spammer Status${NC}"
    printf "%-12s %-10s %-8s %-12s %-10s %-12s %-10s\n" \
        "Validator" "Enabled" "TPS" "Mean Size" "Std Dev" "Submitted" "Errors"
    printf "%-12s %-10s %-8s %-12s %-10s %-12s %-10s\n" \
        "----------" "-------" "---" "----------" "-------" "---------" "------"

    for v in $validators; do
        local status
        status=$(get_status "$v")
        IFS='|' read -r enabled tps mean_size std_dev submitted errors <<< "$status"

        # Color-code the enabled status
        local enabled_colored
        if [[ "$enabled" == "true" ]]; then
            enabled_colored="${GREEN}true${NC}"
        elif [[ "$enabled" == "false" ]]; then
            enabled_colored="${RED}false${NC}"
        else
            enabled_colored="${YELLOW}$enabled${NC}"
        fi

        # Color-code errors
        local errors_colored
        if [[ "$errors" == "ERROR" ]] || [[ "$errors" -gt 0 ]]; then
            errors_colored="${RED}$errors${NC}"
        else
            errors_colored="$errors"
        fi

        # Use printf for plain columns and echo -e for colored ones
        printf "%-12s " "validator-$v"
        echo -en "${enabled_colored}"
        printf "%$((10 - ${#enabled}))s" ""  # Pad to align with "Enabled" column width
        printf "%-8s %-12s %-10s %-12s " "$tps" "$mean_size" "$std_dev" "$submitted"
        echo -e "${errors_colored}"
    done
}

# ==================================================

# =================== MAIN SCRIPT ===================

# Check if Docker is available
if ! command -v docker &> /dev/null; then
    error "Docker is not installed or not in PATH"
    exit 1
fi

# Parse command
if [[ $# -lt 1 ]]; then
    usage
    exit 1
fi

COMMAND="$1"
shift

# Validate command
if [[ ! "$COMMAND" =~ ^(start|stop|status)$ ]]; then
    error "Invalid command: $COMMAND"
    usage
    exit 1
fi

# Default values
VALIDATOR_RANGE="all"
TPS=$DEFAULT_TPS
MEAN_SIZE=$DEFAULT_MEAN_SIZE
STD_DEV=$DEFAULT_STD_DEV

# Parse options
while [[ $# -gt 0 ]]; do
    case "$1" in
        -v|--validators)
            VALIDATOR_RANGE="$2"
            shift 2
            ;;
        -t|--tps)
            TPS="$2"
            shift 2
            ;;
        -m|--mean-size)
            MEAN_SIZE="$2"
            shift 2
            ;;
        -s|--std-dev)
            STD_DEV="$2"
            shift 2
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            error "Unknown option: $1"
            usage
            exit 1
            ;;
    esac
done

# Auto-detect running validators
log "Detecting running validators..."
AVAILABLE_VALIDATORS=$(detect_validators)
log "Available validators: $AVAILABLE_VALIDATORS"

# Parse and validate validator range
REQUESTED_VALIDATORS=$(parse_validator_range "$VALIDATOR_RANGE" "$AVAILABLE_VALIDATORS")
TARGET_VALIDATORS=$(validate_validators "$REQUESTED_VALIDATORS" "$AVAILABLE_VALIDATORS")

if [[ -z "$TARGET_VALIDATORS" ]]; then
    error "No valid validators to target"
    exit 1
fi

log "Target validators: $TARGET_VALIDATORS"
echo

# Execute command
case "$COMMAND" in
    start)
        log "Starting spammer on validators: $TARGET_VALIDATORS"
        for v in $TARGET_VALIDATORS; do
            start_spammer "$v" "$TPS" "$MEAN_SIZE" "$STD_DEV"
        done
        ;;
    stop)
        log "Stopping spammer on validators: $TARGET_VALIDATORS"
        for v in $TARGET_VALIDATORS; do
            stop_spammer "$v"
        done
        ;;
    status)
        display_status_table "$TARGET_VALIDATORS"
        ;;
esac

echo
log "Done"
