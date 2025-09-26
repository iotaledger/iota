#!/bin/bash
set -euo pipefail

# IOTA Peer Discovery Tool

# Configuration
NETWORK="iota-6364aad5"  # Default to mainnet
readonly SCRIPT_NAME="$(basename "$0")"

# Functions
usage() {
    echo "Usage: $SCRIPT_NAME [--testnet] <hostname:port>"
    echo "Options:"
    echo "  --testnet    Use Testnet network (iota-2304aa97)"
    echo "Examples:"
    echo "  $SCRIPT_NAME node.example.com:8084"
    echo "  $SCRIPT_NAME --testnet node.example.com:8084"
    exit 1
}

log() {
    echo "$1" >&2
}

error() {
    echo "❌ $1" >&2
    exit 1
}

check_dependencies() {
    if ! command -v iota-tool >/dev/null 2>&1; then
        error "iota-tool binary not found. Please ensure it's installed and in your PATH."
    fi
}

test_connectivity() {
    local host="$1"
    echo "🏓 Testing connectivity..."
    local ping_output
    ping_output=$(iota-tool anemo ping --server-name "$NETWORK" "$host" 2>&1)
    if [[ "$ping_output" =~ "connection error" ]] || [[ "$ping_output" =~ "deadline has elapsed" ]] || [[ "$ping_output" =~ "failed" ]]; then
        error "Failed to ping $host: $(echo "$ping_output" | grep -o 'connection error:.*' || echo "$ping_output")"
    fi
    echo "✅ Ping successful"
}

fetch_peer_data() {
    local host="$1"
    echo "👥 Fetching known peers..."
    local raw_data
    raw_data=$(iota-tool anemo call --server-name "$NETWORK" "$host" Discovery GetKnownPeersV2 "()" 2>/dev/null) || error "Failed to fetch peer data"
    echo "$raw_data"
}

parse_peer_entry() {
    local line="$1"
    local peer_id=""
    local address=""
    local timestamp=""
    
    # Extract peer_id
    if [[ "$line" =~ peer_id:\"([^\"]+)\" ]]; then
        peer_id="${BASH_REMATCH[1]}"
    fi
    
    # Extract address
    if [[ "$line" =~ addresses:\[\"([^\"]+)\"\] ]]; then
        address="${BASH_REMATCH[1]}"
        # Clean up address format
        address="${address//\/dns\//}"
        address="${address//\/ip4\//}"
        address="${address//\/udp\//:}"
    fi
    
    # Extract timestamp
    if [[ "$line" =~ timestamp_ms:([0-9]+) ]]; then
        timestamp="${BASH_REMATCH[1]}"
    fi
    
    echo "${peer_id}|${address}|${timestamp}"
}

format_timestamp() {
    local timestamp_ms="$1"
    if command -v date >/dev/null 2>&1; then
        local timestamp_s=$((timestamp_ms / 1000))
        date -r "$timestamp_s" '+%Y-%m-%d %H:%M:%S' 2>/dev/null || echo "$timestamp_ms"
    else
        echo "$timestamp_ms"
    fi
}

print_table_border() {
    local style="$1"  # top, middle, or bottom
    local chars=( "─" "┬" "┼" "┴" )
    case "$style" in
        top)    chars=( "─" "┬" "┬" "┬" ); echo -n "┌" ;;
        middle) chars=( "─" "┼" "┼" "┼" ); echo -n "├" ;;
        bottom) chars=( "─" "┴" "┴" "┴" ); echo -n "└" ;;
    esac
    
    printf "%s%s" "$(printf '%*s' 66 '' | tr ' ' "${chars[0]}")" "${chars[1]}"
    printf "%s%s" "$(printf '%*s' 47 '' | tr ' ' "${chars[0]}")" "${chars[2]}"
    printf "%s" "$(printf '%*s' 21 '' | tr ' ' "${chars[0]}")"
    
    case "$style" in
        top)    echo "┐" ;;
        middle) echo "┤" ;;
        bottom) echo "┘" ;;
    esac
}

print_peer_table() {
    local raw_data="$1"
    
    local self_peer=""
    local self_line
    self_line=$(echo "$raw_data" | grep -o 'own_info:([^)]*)' | head -1)
    if [[ -n "$self_line" ]]; then
        self_peer=$(parse_peer_entry "$self_line")
        
        echo "🏠 Peer Information:"
        IFS='|' read -r self_peer_id self_address self_timestamp <<< "$self_peer"
        local self_short_peer_id="${self_peer_id:0:64}"
        local self_short_address="${self_address:0:45}"
        local self_formatted_time
        self_formatted_time=$(format_timestamp "$self_timestamp")
        self_formatted_time="${self_formatted_time:0:19}"
        
        echo "   • Peer ID: $self_short_peer_id"
        echo "   • Address: $self_short_address"
        echo "   • Timestamp: $self_formatted_time"
        echo
        
        self_peer="${self_peer}|SELF"
    fi
    
    # Extract known peers
    local known_peers
    known_peers=$(echo "$raw_data" | grep -o 'data:([^)]*)' | while read -r line; do
        local peer_entry
        peer_entry=$(parse_peer_entry "$line")
        if [[ -n "$peer_entry" ]]; then
            echo "${peer_entry}|PEER"
        fi
    done)
    
    # Combine known peers only (exclude self from table)
    local unique_peers
    if [[ -n "$self_peer" ]]; then
        IFS='|' read -r self_peer_id self_address self_timestamp <<< "$(echo "$self_peer" | sed 's/|SELF$//')"
        unique_peers=$(echo "$known_peers" | grep -v '^$' | grep -v "^$self_peer_id|" | sort -t'|' -k1,1 -u | sort -t'|' -k3,3nr)
    else
        unique_peers=$(echo "$known_peers" | grep -v '^$' | sort -t'|' -k1,1 -u | sort -t'|' -k3,3nr)
    fi
    
    local total_count
    total_count=$(echo "$unique_peers" | wc -l | tr -d ' ')
    echo "📊 Found $total_count known peer(s)"
    echo
    
    # Print table
    print_table_border "top"
    printf "│ %-64s │ %-45s │ %-19s │\n" "PEER ID" "ADDRESS" "TIMESTAMP"
    print_table_border "middle"
    
    local known_count=0
    local -a unique_hosts_array
    
    while IFS='|' read -r peer_id address timestamp access_type; do
        [[ -n "$peer_id" ]] || continue
        
        # Format fields
        local short_peer_id="${peer_id:0:64}"
        local short_address="${address:0:45}"
        local formatted_time
        formatted_time=$(format_timestamp "$timestamp")
        formatted_time="${formatted_time:0:19}"
        
        ((known_count++))
        local host="${address%%:*}"
        
        local host_exists=false
        for existing_host in "${unique_hosts_array[@]:-}"; do
            if [[ "$existing_host" == "$host" ]]; then
                host_exists=true
                break
            fi
        done
        
        if [[ "$host_exists" == false ]]; then
            unique_hosts_array+=("$host")
        fi
        
        printf "│ %-64s │ %-45s │ %-19s │\n" \
            "$short_peer_id" "$short_address" "$formatted_time"
    done <<< "$unique_peers"
    
    print_table_border "bottom"
    
    # Print summary
    echo
    echo "📈 Summary:"
    echo "   • Known peers: $known_count peer(s)"
    
    if [[ $known_count -gt 0 ]]; then
        echo "   • Unique hosts: ${#unique_hosts_array[@]}"
    fi
}

main() {
    local host=""
    
    # Check dependencies first
    check_dependencies
    
    # Parse arguments
    while [[ $# -gt 0 ]]; do
        case $1 in
            --testnet)
                NETWORK="iota-2304aa97"
                shift
                ;;
            --help|-h)
                usage
                ;;
            -*)
                echo "❌ Unknown option: $1" >&2
                usage
                ;;
            *)
                if [[ -n "$host" ]]; then
                    echo "❌ Multiple hostnames provided" >&2
                    usage
                fi
                host="$1"
                shift
                ;;
        esac
    done
    
    # Check if hostname was provided
    if [[ -z "$host" ]]; then
        echo "❌ No hostname provided" >&2
        usage
    fi
    
    echo "🔗 Connecting to IOTA Node: $host"
    if [[ "$NETWORK" == "iota-2304aa97" ]]; then
        echo "📡 Network: $NETWORK (Testnet)"
    else
        echo "📡 Network: $NETWORK (Mainnet)"
    fi
    echo
    
    test_connectivity "$host"
    echo
    
    local raw_data
    raw_data=$(fetch_peer_data "$host")
    echo
    
    print_peer_table "$raw_data"
}

main "$@"