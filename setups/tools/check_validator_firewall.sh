#!/bin/bash
set -euo pipefail

# Default to mainnet
API_URL="https://api.mainnet.iota.cafe"
NETWORK="iota-6364aad5"
readonly SCRIPT_NAME="$(basename "$0")"

# Functions
usage() {
    echo "Usage: $SCRIPT_NAME [--testnet] <address>"
    echo "Options:"
    echo "  --testnet        Use testnet instead of mainnet"
    echo "Arguments:"
    echo "  <address>        Extract values for this iotaAddress"
    echo "Example:"
    echo "  $SCRIPT_NAME 0x864c651958094732a1227134cf7cab7587f05a399398804552553fbc01dba4e7"
    echo "  $SCRIPT_NAME --testnet 0xaaa9f7320f4663147fbbbc0325c6fb80e22c0833af14f4e9590e2620208b98a2"
    exit 1
}

extract_host_port() {
    local addr="$1"
    # Extract from formats like "/dns/host/tcp/port/http" or "/dns/host/udp/port"
    if [[ $addr =~ /dns/([^/]+)/(tcp|udp)/([0-9]+) ]]; then
        local host="${BASH_REMATCH[1]}"
        local port="${BASH_REMATCH[3]}"
        echo "$host $port"
    else
        echo "❌ Could not parse address: $addr" >&2
        return 1
    fi
}

fetch_validator_data() {
    local target_address="$1"
    local payload=$(cat <<EOF
{
"jsonrpc": "2.0",
"method": "iotax_getLatestIotaSystemStateV2",
"params": [],
"id": 1
}
EOF
)

    curl -s -X POST "$API_URL" \
        -H "Content-Type: application/json" \
        -d "$payload" \
    | jq --arg addr "$target_address" -r '
        .result.V2.activeValidators[] | select(.iotaAddress == $addr) | 
        "\(.name)|\(.netAddress)|\(.p2pAddress)|\(.primaryAddress)|\(.networkPubkeyBytes)"'
}

display_validator_info() {
    local validator_name="$1"
    local target_address="$2"
    local net_address="$3"
    local p2p_address="$4"
    local primary_address="$5"

    echo "=== Validator Firewall Test ==="

    if [[ "$NETWORK" == "iota-2304aa97" ]]; then
        echo "📡 Network: $NETWORK (Testnet)"
    else
        echo "📡 Network: $NETWORK (Mainnet)"
    fi
    echo

    echo "Name: $validator_name"
    echo "Address: $target_address"
    echo ""
    echo "Extracted endpoints:"
    echo "  primaryAddress: $primary_address"
    echo "      netAddress: $net_address"
    echo "      p2pAddress: $p2p_address"
    echo
}

test_tls_endpoint() {
    local endpoint_name="$1"
    local address="$2"
    local pub_key_bytes="$3"
    
    # Convert base64 to hex with 0x prefix
    local pub_key_hex="0x$(echo "$pub_key_bytes" | base64 -d | xxd -p -c 9999 | tr -d '\n')"

    echo "Testing $endpoint_name endpoint and checking TLS certificate..."
    local host_port
    host_port=$(extract_host_port "$address")
    if [[ $? -ne 0 ]]; then
        echo "❌ Failed to parse $endpoint_name"
        return 1
    fi

    local host port
    read -r host port <<< "$host_port"

    local tls_public_key
    set +e  # Temporarily disable exit on error
    # Extract public key and convert to raw bytes (skip DER ASN.1 structure)
    # Use a temporary file and background job with timeout
    local tmpfile=$(mktemp)
    (
        echo | openssl s_client -connect "$host:$port" -servername "$NETWORK" 2>/dev/null | \
            openssl x509 -pubkey -noout 2>/dev/null | \
            openssl pkey -pubin -text -noout 2>/dev/null | \
            grep -A 10 "pub:" 2>/dev/null | \
            tail -n +2 | \
            tr -d ' :\n' | \
            sed 's/^/0x/' 2>/dev/null > "$tmpfile"
    ) &
    local bg_pid=$!
    local timeout=3
    local count=0
    while kill -0 $bg_pid 2>/dev/null && [ $count -lt $((timeout * 10)) ]; do
        sleep 0.1
        count=$((count + 1))
    done
    local openssl_exit_code=0
    if kill -0 $bg_pid 2>/dev/null; then
        kill -9 $bg_pid 2>/dev/null
        wait $bg_pid 2>/dev/null || openssl_exit_code=$?
    else
        wait $bg_pid 2>/dev/null || openssl_exit_code=$?
    fi
    tls_public_key=$(cat "$tmpfile" 2>/dev/null || echo "")
    rm -f "$tmpfile"
    set -e  # Re-enable exit on error
    
    if [[ $openssl_exit_code -ne 0 || -z "$tls_public_key" ]]; then
        echo "❌ Failed to extract TLS public key from $endpoint_name"
        echo "   💡 Hint: Check if TCP port $port is open in your firewall for host $host"
    elif [[ "$tls_public_key" == "$pub_key_hex" ]]; then
        echo "✅ $endpoint_name is reachable and TLS certificate matches expected public key"
    else
        echo "❌ TLS certificate public key does NOT match expected public key"
        echo "   💡 Hint: Check if there is another service running on TCP port $port on host $host"
    fi
    echo
}

test_p2p_endpoint() {
    local p2p_address="$1"
    
    echo "Testing p2pAddress..."
    if ! command -v iota-tool >/dev/null 2>&1; then
        echo "❌ iota-tool not found - cannot test p2pAddress"
        return 1
    fi

    local host_port
    host_port=$(extract_host_port "$p2p_address")
    if [[ $? -ne 0 ]]; then
        echo "❌ Failed to parse p2pAddress"
        return 1
    fi

    local host port
    read -r host port <<< "$host_port"

    local ping_output
    ping_output=$(iota-tool anemo ping --server-name "$NETWORK" "$host:$port" 2>&1)

    if echo "$ping_output" | grep -q "closed by peer"; then
        echo "✅ p2pAddress is reachable and not accepting connections (expected for validators)"
    elif echo "$ping_output" | grep -q "deadline has elapsed"; then
        echo "❌ p2pAddress connection timeout - port seems unreachable"
        echo "   💡 Hint: Check if UDP port $port is open in your firewall for host $host"
    else
        echo "❌ p2pAddress connection error: $ping_output"
    fi
    echo
}

main() {
    # Parse arguments
    local use_testnet=false
    local target_address=""
    
    while [[ $# -gt 0 ]]; do
        case $1 in
            --testnet)
                use_testnet=true
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
                if [[ -z "$target_address" ]]; then
                    target_address="$1"
                else
                    echo "❌ Too many arguments" >&2
                    usage
                fi
                shift
                ;;
        esac
    done

    if [[ -z "$target_address" ]]; then
        echo "❌ Error: Address argument is required" >&2
        echo ""
        usage
    fi

    # Set network configuration
    if [[ "$use_testnet" == true ]]; then
        API_URL="https://api.testnet.iota.cafe"
        NETWORK="iota-2304aa97"
    fi

    # Fetch validator data
    local validator_data
    validator_data=$(fetch_validator_data "$target_address")

    if [[ -z "$validator_data" ]]; then
        echo "❌ No validator found with address: $target_address"
        exit 1
    fi

    # Parse the addresses
    local validator_name net_address p2p_address primary_address network_pub_key_bytes
    IFS='|' read -r validator_name net_address p2p_address primary_address network_pub_key_bytes <<< "$validator_data"

    # Display validator info
    display_validator_info "$validator_name" "$target_address" "$net_address" "$p2p_address" "$primary_address"

    # Test endpoints
    test_tls_endpoint "primaryAddress" "$primary_address" "$network_pub_key_bytes"
    test_tls_endpoint "netAddress" "$net_address" "$network_pub_key_bytes"
    test_p2p_endpoint "$p2p_address"
}

# Run main function with all arguments
main "$@"