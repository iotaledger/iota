#!/bin/bash
# utils file for build cache server scripts

# Get script directory and source config
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/config.sh"

# Colors for output
export RED='\033[0;31m'
export GREEN='\033[0;32m'
export YELLOW='\033[1;33m'
export BLUE='\033[0;34m'
export NC='\033[0m' # No Color

# Helper function to print colored output
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Helper function to get authentication arguments for curl
get_auth_args() {
    if [ -n "$BUILD_CACHE_USER" ] && [ -n "$BUILD_CACHE_PASSWORD" ]; then
        echo "-u $BUILD_CACHE_USER:$BUILD_CACHE_PASSWORD"
    fi
}

# Print current configuration
print_config() {
    log_info "Build Cache Server Configuration:"
    log_info "  Server: $BUILD_CACHE_SERVER_URI"
    if [ -n "$BUILD_CACHE_USER" ]; then
        log_info "  User: $BUILD_CACHE_USER"
    fi
    log_info "  Commit: $COMMIT"
    log_info "  CPU Target: $CPU_TARGET"
    log_info "  Binaries: $BINARIES"
    log_info
}

# Helper function to build query parameters with optional features and toolchain
build_query_params() {
    local base_params="commit=$COMMIT&cpu_target=$CPU_TARGET"
    if [ -n "$TOOLCHAIN" ]; then
        base_params="${base_params}&toolchain=${TOOLCHAIN}"
    fi
    if [ -n "$FEATURES" ]; then
        base_params="${base_params}&features=${FEATURES}"
    fi
    echo "$base_params"
}

build_query_params_with_binaries() {
    local base_params=$(build_query_params)
    base_params="${base_params}&binaries=$BINARIES"
    echo "$base_params"
}

# Function to check if binaries are available
check_availability() {
    # Make check request
    local query_params=$(build_query_params_with_binaries)
    RESPONSE=$(curl -s -w "\n%{http_code}" $(get_auth_args) \
        "$BUILD_CACHE_SERVER_URI/check?${query_params}")

    # Parse response
    HTTP_CODE=$(echo "$RESPONSE" | tail -n1)
    BODY=$(echo "$RESPONSE" | sed '$d')

    if [ "$HTTP_CODE" = "200" ]; then
        if [ "$JSON_OUTPUT" = true ]; then
            echo "$BODY" | jq .
        else
            # Parse and display availability nicely
            AVAILABLE=$(echo "$BODY" | jq -r '.available')
            AVAILABLE_BINARIES=$(echo "$BODY" | jq -r '.binaries | join(", ")')
            
            if [ "$AVAILABLE" = "true" ]; then
                return 0  # Success
            else
                return 1  # Not available
            fi
        fi
    else
        log_error "Check request failed (HTTP $HTTP_CODE)"
        if [ -n "$BODY" ]; then
            log_error "Response: $BODY"
        fi
        return 1  # Not available
    fi
}

# Function to request a build
build() {
    # Convert comma-separated binaries to JSON array
    IFS=',' read -ra BINARY_ARRAY <<< "$BINARIES"
    BINARY_JSON="["
    for i in "${!BINARY_ARRAY[@]}"; do
        if [ $i -gt 0 ]; then
            BINARY_JSON="$BINARY_JSON,"
        fi
        BINARY_JSON="$BINARY_JSON\"${BINARY_ARRAY[$i]}\""
    done
    BINARY_JSON="$BINARY_JSON]"

    # Add optional toolchain
    TOOLCHAIN_JSON="null"
    if [ -n "$TOOLCHAIN" ]; then
        TOOLCHAIN_JSON="\"$TOOLCHAIN\""
    fi

    # Add optional features array
    FEATURES_JSON="[]"
    if [ -n "$FEATURES" ]; then
        IFS=',' read -ra FEATURES_ARRAY <<< "$FEATURES"
        FEATURES_JSON="["
        for i in "${!FEATURES_ARRAY[@]}"; do
            if [ $i -gt 0 ]; then
                FEATURES_JSON="$FEATURES_JSON,"
            fi
            FEATURES_JSON="$FEATURES_JSON\"${FEATURES_ARRAY[$i]}\""
        done
        FEATURES_JSON="$FEATURES_JSON]"
    fi

    # Create JSON payload
    PAYLOAD=$(cat <<EOF
{
    "commit": "$COMMIT",
    "cpu_target": "$CPU_TARGET",
    "toolchain": $TOOLCHAIN_JSON,
    "features": $FEATURES_JSON,
    "binaries": $BINARY_JSON
}
EOF
    )

    # Send build request
    log_info "Sending build request..."
    RESPONSE=$(curl -s -w "\n%{http_code}" -X POST \
        -H "Content-Type: application/json" \
        -d "$PAYLOAD" \
        $(get_auth_args) \
        "$BUILD_CACHE_SERVER_URI/build")

    # Parse response
    HTTP_CODE=$(echo "$RESPONSE" | tail -n1)
    BODY=$(echo "$RESPONSE" | sed '$d')

    if [ "$HTTP_CODE" = "202" ]; then
        log_success "Build request accepted"
        
        # Parse the resolved commit from the response
        RESOLVED_COMMIT=$(echo "$BODY" | jq -r '.resolved_commit' 2>/dev/null || echo "$COMMIT")
        if [ "$RESOLVED_COMMIT" != "$COMMIT" ]; then
            log_info "Resolved '$COMMIT' to commit '$RESOLVED_COMMIT'"
            # Update the global COMMIT variable to use resolved commit for subsequent operations
            export COMMIT="$RESOLVED_COMMIT"
        fi
        
        MESSAGE=$(echo "$BODY" | jq -r '.message' 2>/dev/null || echo "Build started")
        log_info "$MESSAGE"
    else
        log_error "Build request failed (HTTP $HTTP_CODE)"
        if [ -n "$BODY" ]; then
            log_error "Response: $BODY"
        fi
        exit 1
    fi

}

# Function to get build status
get_build_status() {
    local query_params=$(build_query_params_with_binaries)
    RESPONSE=$(curl -s -w "\n%{http_code}" $(get_auth_args) \
        "$BUILD_CACHE_SERVER_URI/status?${query_params}" 2>/dev/null || echo "\n000")
    
    HTTP_CODE=$(echo "$RESPONSE" | tail -n1)
    BODY=$(echo "$RESPONSE" | sed '$d')
    
    if [ "$HTTP_CODE" = "200" ]; then
        # Parse the JSON status - it could be a simple string or a JSON object
        STATUS_RAW=$(echo "$BODY" | jq -r '.status' 2>/dev/null || echo "Unknown")
        
        # Check if status is a JSON object (like {"Failed": "error message"})
        if echo "$STATUS_RAW" | jq empty 2>/dev/null; then
            # It's valid JSON, extract the key (status type)
            STATUS_TYPE=$(echo "$STATUS_RAW" | jq -r 'keys[0]' 2>/dev/null || echo "Unknown")
            if [ "$STATUS_TYPE" = "Failed" ]; then
                ERROR_MSG=$(echo "$STATUS_RAW" | jq -r '.Failed' 2>/dev/null || echo "Unknown error")
                echo "Failed:$ERROR_MSG"
            else
                echo "$STATUS_TYPE"
            fi
        else
            # It's a simple string status
            echo "$STATUS_RAW"
        fi
    elif [ "$HTTP_CODE" = "404" ]; then
        echo "NotFound"
    else
        echo "Error"
    fi
}

# Function to wait for build completion
wait() {
    # Wait for build completion
    START_TIME=$(date +%s)
    while true; do
        CURRENT_TIME=$(date +%s)
        ELAPSED=$((CURRENT_TIME - START_TIME))
        
        # Check timeout
        if [ $ELAPSED -gt $BUILD_TIMEOUT ]; then
            log_error "Build timeout reached (${BUILD_TIMEOUT}s)"
            exit 1
        fi
        
        # Check if binaries are available
        if check_availability; then
            log_success "All binaries are available!"
            return 0
        fi
        
        # Get and display build status
        STATUS=$(get_build_status)
        REMAINING=$((BUILD_TIMEOUT - ELAPSED))
        
        case $STATUS in
            "Building")
                log_info "Build in progress... (${ELAPSED}s elapsed, ${REMAINING}s remaining)"
                ;;
            "Queued")
                log_info "Build queued... (${ELAPSED}s elapsed, ${REMAINING}s remaining)"
                ;;
            Failed:*)
                ERROR_MSG="${STATUS#Failed:}"  # Remove "Failed:" prefix
                log_error "Build failed: $ERROR_MSG"
                exit 1
                ;;
            "NotFound")
                log_warning "No build found. Starting new build..."
                build  # Use the function directly instead of calling the script
                ;;
            "Success")
                log_warning "Build marked as successful but binaries not available yet... (${ELAPSED}s elapsed)"
                ;;
            *)
                log_warning "Unknown build status: $STATUS (${ELAPSED}s elapsed, ${REMAINING}s remaining)"
                ;;
        esac
        
        sleep $CHECK_INTERVAL
    done
}

# Function to download binaries
download() {
    log_info "Downloading binaries from $BUILD_CACHE_SERVER_URI"

    # Create output directory
    mkdir -p "$OUTPUT_DIR"

    # Convert comma-separated binaries to array
    IFS=',' read -ra BINARY_ARRAY <<< "$BINARIES"

    # Download each binary
    ACTUAL_COMMIT=""
    for BINARY in "${BINARY_ARRAY[@]}"; do
        BINARY=$(echo "$BINARY" | xargs)  # Trim whitespace
        BINARY_PATH="$OUTPUT_DIR/$BINARY"
        
        log_info "Checking $BINARY..."
        
        # Use temporary file for headers
        HEADER_FILE=$(mktemp)
        
        # Build download URL with optional params
        local query_params=$(build_query_params)
        local download_url="$BUILD_CACHE_SERVER_URI/download?$query_params&binary=$BINARY"
        
        # Check if binary already exists and get its checksum for ETag
        if [ -f "$BINARY_PATH" ]; then
            EXISTING_SHA=$(sha256sum "$BINARY_PATH" | cut -d' ' -f1)
            log_info "Binary $BINARY exists (SHA256: $EXISTING_SHA), checking if update needed..."
            
            # Download with ETag support
            RESPONSE=$(curl -s -w "\n%{http_code}" -L -D "$HEADER_FILE" \
                $(get_auth_args) \
                -H "If-None-Match: \"sha256:$EXISTING_SHA\"" \
                "$download_url" \
                -o "$BINARY_PATH.tmp")
        else
            log_info "Downloading $BINARY..."
            
            # Download without ETag (new file)
            RESPONSE=$(curl -s -w "\n%{http_code}" -L -D "$HEADER_FILE" \
                $(get_auth_args) \
                "$download_url" \
                -o "$BINARY_PATH.tmp")
        fi
        
        HTTP_CODE=$(echo "$RESPONSE" | tail -n1)
        
        if [ "$HTTP_CODE" = "200" ]; then
            # Extract the actual commit hash from headers if present
            ACTUAL_COMMIT=$(grep -i "x-iota-build-commit-hash:" "$HEADER_FILE" | cut -d' ' -f2 | tr -d '\r\n' || echo "")
            mv "$BINARY_PATH.tmp" "$BINARY_PATH"
            chmod +x "$BINARY_PATH"
            log_success "Downloaded $BINARY to $BINARY_PATH (commit: ${ACTUAL_COMMIT:-$COMMIT})"
        elif [ "$HTTP_CODE" = "304" ]; then
            # Binary is up to date
            rm -f "$BINARY_PATH.tmp"
            log_success "$BINARY is up to date (SHA256: $EXISTING_SHA)"
        else
            log_error "Failed to download $BINARY (HTTP $HTTP_CODE)"
            rm -f "$BINARY_PATH.tmp"  # Remove partial file
            exit 1
        fi
        
        # Clean up temporary header file
        rm -f "$HEADER_FILE"
    done

    log_success "All binaries processed successfully in $OUTPUT_DIR"
}