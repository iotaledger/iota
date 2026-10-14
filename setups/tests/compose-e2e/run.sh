#!/usr/bin/env bash
# Copyright (c) 2026 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

# End-to-end test of the compose setups in setups/fullnode/docker and
# setups/validator. See README.md. Output: one line "CHECK <id> PASS|FAIL|SKIP
# <reason>" per check, then a summary. The exit status is 1 if a check failed.

set -euo pipefail

HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
REPO=$(cd "$HERE/../../.." && pwd)
export NODE_IMAGE=${NODE_IMAGE:-iotaledger/iota-tools:devnet}
NODE_GRPC_MAX_CONNECTIONS=${NODE_GRPC_MAX_CONNECTIONS:-0}
PREFIX=${E2E_PREFIX:-compose-e2e}
NETNS=${E2E_NETNS:-$PREFIX-client}
VETH=${E2E_VETH:-e2e-veth}
KERNEL_LOG=${E2E_KERNEL_LOG:-1}
CHECKS=${CHECKS:-all}

# The order matters: F8-stopped stops the node, F2-control and U1-control
# recreate it, and V7 recreates the validator network.
FULLNODE_CHECKS=(U1 F1 F2 F3 F4 F5 F7 F9 F11 F12 F12-count F13 F14 F15 F17 F3-header F8 F10 F16 F8-stopped F6 F2-control U1-control)
VALIDATOR_CHECKS=(V1 V2 V3 V4 V5 V9 V10 V6 V8 V7)

if [ "$(id -u)" = 0 ]; then SUDO=(); else SUDO=(sudo); fi
mkdir -p "${E2E_WORKDIR:-${TMPDIR:-/tmp}}"
# Only this new directory is removed at the end.
W=$(mktemp -d "${E2E_WORKDIR:-${TMPDIR:-/tmp}}/compose-e2e.XXXXXX")
FN="$W/fullnode"
VN="$W/validator"
HOST4=192.0.2.1
HOST6=2001:db8::1
TABLE=iota_node_limits
RPC='{"jsonrpc":"2.0","id":1,"method":"iota_getChainIdentifier","params":[]}'
MAIN_HAPROXY=$PREFIX-fullnode-haproxy-1
TEST_HAPROXY=$PREFIX-test-haproxy
RESULTS=$W/results

log() { echo "--- $*" >&2; }

check() {
    echo "CHECK $*"
    echo "$2" >>"$RESULTS"
}

# report STATUS ID PASS_TEXT FAIL_TEXT: PASS if STATUS is 0. Give "$?" as
# STATUS: bash expands it before the texts, and a command in the texts
# changes "$?".
report() {
    if [ "$1" = 0 ]; then check "$2" PASS "$3"; else check "$2" FAIL "$4"; fi
}

wanted() {
    case "$CHECKS" in
    all) return 0 ;;
    fullnode) [[ $1 == [FU]* ]] ;;
    validator) [[ $1 == V* ]] ;;
    *) [[ " $CHECKS " == *" $1 "* ]] ;;
    esac
}

# run_check ID: runs check_ID (dashes become underscores). A step that fails
# does not stop the run; a check that ends with no result gets a FAIL line.
run_check() {
    wanted "$1" || return 0
    local before
    before=$(wc -l <"$RESULTS")
    (
        set +e
        "check_${1//-/_}"
    ) || true
    [ "$(wc -l <"$RESULTS")" -gt "$before" ] || check "$1" FAIL "the check ended with no result"
}

inns() { "${SUDO[@]}" ip netns exec "$NETNS" "$@"; }
client() { inns python3 -I "$HERE/client.py" "$@"; }
count_of() { jq -r --arg k "$1" '.counts[$k] // 0'; }

fn_compose() {
    docker compose -p "$PREFIX-fullnode" --project-directory "$FN" \
        -f "$FN/docker-compose.yaml" -f "$HERE/compose.e2e.fullnode.yml" "$@"
}
vn_compose() {
    docker compose -p "$PREFIX-validator" --project-directory "$VN" \
        -f "$VN/docker-compose.yml" -f "$HERE/compose.e2e.validator.yml" "$@"
}
haproxy_image() { docker compose -f "$REPO/setups/fullnode/docker/docker-compose.yaml" config --images | grep haproxy | sort -u; }

# admin CONTAINER COMMAND: one command on the HAProxy admin socket.
admin() { docker exec "$1" sh -c "echo '$2' | socat stdio unix-connect:/tmp/admin.sock"; }
# conn_cur ADDRESS: open connections of an IPv4 address in the main HAProxy.
conn_cur() {
    admin "$MAIN_HAPROXY" "show table st_conn_9000" |
        sed -n "s/.* key=::ffff:$1 .*conn_cur=\([0-9]*\).*/\1/p" | grep . || echo 0
}
released() { [ "$(conn_cur "$1")" = 0 ]; }

# metric URL SERIES: the value of a series; fails if the series is missing.
metric() { curl -s "$1" | awk -v s="$2" '$1 == s { print $2; found = 1 } END { exit !found }'; }
haproxy_metric() { metric http://127.0.0.1:8404/metrics "$1"; }
server_state() { haproxy_metric "haproxy_server_status{proxy=\"node_jsonrpc\",server=\"node\",state=\"$1\"}"; }
server_down() { [ "$(server_state DOWN)" = 1 ]; }

table_present() { "${SUDO[@]}" nft list table inet "$TABLE" >/dev/null 2>&1; }
table_absent() { ! table_present; }

# wait_for SECONDS COMMAND...: runs COMMAND every second until it succeeds;
# returns 1 after SECONDS.
wait_for() {
    local end=$((SECONDS + $1))
    shift
    until "$@" >/dev/null 2>&1; do
        [ "$SECONDS" -lt "$end" ] || return 1
        sleep 1
    done
}

rpc_ok() { curl -s -m 3 -H 'content-type: application/json' -d "$RPC" "$1" | grep -q '"result"'; }
tls_ok() { python3 -I "$HERE/client.py" conns --host "$1" --port 8080 --n 1 --tls | grep -q '"tls": 1'; }
tls_conns() { client conns --host "$1" --port 8080 --src "$2" --n "$3" --tls --hold "${4:-0}"; }

cleanup() {
    set +e
    log "cleanup"
    docker rm -f "$TEST_HAPROXY" "$PREFIX-standin" "$PREFIX-listener" >/dev/null 2>&1
    if [ -f "$FN/docker-compose.yaml" ]; then
        fn_compose -f "$HERE/compose.e2e.node-port.yml" logs --no-color >>"$W/fullnode.log" 2>&1
        fn_compose down -v --remove-orphans >/dev/null 2>&1
    fi
    if [ -f "$VN/docker-compose.yml" ]; then
        vn_compose logs --no-color >"$W/validator.log" 2>&1
        vn_compose down -v --remove-orphans --rmi local >/dev/null 2>&1
    fi
    if table_present; then "${SUDO[@]}" nft delete table inet "$TABLE"; fi
    "${SUDO[@]}" ip netns del "$NETNS" 2>/dev/null
    "${SUDO[@]}" ip link del "$VETH" 2>/dev/null
    if [ -n "${E2E_LOGDIR:-}" ]; then mkdir -p "$E2E_LOGDIR" && cp "$W"/*.log "$E2E_LOGDIR/"; fi
    if [ "${E2E_KEEP_WORKDIR:-0}" = 1 ]; then log "work directory: $W"; else "${SUDO[@]}" rm -rf "$W"; fi
}
trap cleanup EXIT

# ---------------------------------------------------------------- setup

setup_netns() {
    log "network namespace $NETNS"
    "${SUDO[@]}" ip netns add "$NETNS"
    "${SUDO[@]}" ip link add "$VETH" type veth peer name eth0 netns "$NETNS"
    "${SUDO[@]}" ip addr add "$HOST4/24" dev "$VETH"
    "${SUDO[@]}" ip addr add 198.51.100.1/24 dev "$VETH"
    "${SUDO[@]}" ip -6 addr add "$HOST6/48" dev "$VETH" nodad
    "${SUDO[@]}" ip link set "$VETH" up
    inns ip link set lo up
    inns ip link set eth0 up
    local a
    for a in 10 11 12 13 15 16 20 21; do inns ip addr add "192.0.2.$a/24" dev eth0; done
    for a in 10 11 12; do inns ip addr add "198.51.100.$a/24" dev eth0; done
    for a in 2001:db8::10 2001:db8::11 2001:db8:0:1::10; do inns ip -6 addr add "$a/48" dev eth0 nodad; done
    inns ip route add default via "$HOST4"
    inns ip -6 route add default via "$HOST6"
    inns ping -c 1 -W 2 "$HOST4" >/dev/null
    inns ping -6 -c 1 -W 2 "$HOST6" >/dev/null
}

setup_genesis() {
    log "genesis with $NODE_IMAGE"
    mkdir -p "$W/genesis"
    docker run --rm -v "$W/genesis:/w" "$NODE_IMAGE" \
        iota-localnet genesis --committee-size 1 --working-dir /w -f >"$W/genesis.log" 2>&1
    docker run --rm -v "$W/genesis:/w" "$NODE_IMAGE" \
        iota-localnet start --network.config /w --write-config /w/run >>"$W/genesis.log" 2>&1
    "${SUDO[@]}" chmod -R a+rX "$W/genesis"
}

# The upgrade an operator does: a fullnode of the previous compose file, with
# a policy-config block that uses socket-addr, gets the new files and the
# documented change of client-id-source.
setup_fullnode() {
    log "fullnode before the upgrade"
    mkdir -p "$FN/data/config"
    cp "$HERE/fullnode-before.yaml" "$FN/docker-compose.yaml"
    cp "$W/genesis/genesis.blob" "$FN/data/config/"
    python3 -I "$HERE/node_config.py" fullnode "$W/genesis/run/fullnode.yaml" \
        --policy-from "$REPO/setups/fullnode/fullnode-devnet.yaml" --set jsonrpc-server-type=both |
        sed -e '/^  client-id-source:$/{N;s/.*/  client-id-source: socket-addr/}' >"$FN/data/config/fullnode.yaml"
    fn_compose up -d --quiet-pull
    wait_for 120 rpc_ok "http://$HOST4:9000/"
    log "fullnode after the upgrade"
    cp "$REPO/setups/fullnode/docker/docker-compose.yaml" "$FN/"
    cp -r "$REPO/setups/fullnode/docker/haproxy" "$FN/"
    cp "$HERE/limits_9000.e2e.map" "$FN/haproxy/limits_9000.map"
    # 192.0.2.13 is a load balancer that sends the PROXY protocol header.
    echo 192.0.2.13/32 >>"$FN/haproxy/proxy_protocol.lst"
    sed -i 's/^  client-id-source: socket-addr$/  client-id-source:\n    x-forwarded-for: 1/' "$FN/data/config/fullnode.yaml"
    fn_compose up -d --quiet-pull
    wait_for 120 rpc_ok "http://$HOST4:9000/"
}

setup_validator() {
    log "validator"
    mkdir -p "$VN/key-pairs" "$VN/iotadb"
    cp "$REPO/setups/validator/docker-compose-testnet.yml" "$VN/docker-compose.yml"
    cp -r "$REPO/setups/validator/node-limits" "$VN/"
    cp "$HERE/node-limits.e2e.conf" "$VN/node-limits/node-limits.conf"
    cp "$W/genesis/genesis.blob" "$VN/"
    : >"$VN/migration.blob"
    validator_config
    vn_compose up -d --quiet-pull --build
    validator_ready
}

validator_config() {
    python3 -I "$HERE/node_config.py" validator "$W/genesis/run/validator-0.yaml" "$@" >"$VN/validator.yaml"
}
validator_ready() { wait_for 120 tls_ok 127.0.0.1 && wait_for 30 table_present; }

# start_test_haproxy DIR [SERVER]: a second HAProxy with the files in DIR, in
# front of SERVER (default: a new standin.sh, which answers after the seconds
# in the request header "X-Delay"). It listens on port 19002 of the host.
start_test_haproxy() {
    docker rm -f "$TEST_HAPROXY" "$PREFIX-standin" >/dev/null 2>&1 || true
    if [ -z "${2:-}" ]; then
        docker run -d --name "$PREFIX-standin" --network "$PREFIX-fullnode_default" \
            -v "$HERE/standin.sh:/standin.sh:ro" --entrypoint socat "$(haproxy_image)" \
            TCP-LISTEN:9000,fork,reuseaddr 'SYSTEM:sh /standin.sh' >/dev/null
    fi
    docker run -d --name "$TEST_HAPROXY" --network "$PREFIX-fullnode_default" -p 19002:9000 \
        -e IOTA_JSONRPC_SERVER="${2:-$PREFIX-standin:9000}" -v "$1:/usr/local/etc/haproxy:ro" "$(haproxy_image)" >/dev/null
    wait_for 10 sh -c "docker logs $TEST_HAPROXY 2>&1 | grep -q 'Loading success'"
}

# haproxy_copy NAME SED: the HAProxy files of the setup, with the sed script
# applied to haproxy.cfg. Fails if the script changed nothing.
haproxy_copy() {
    mkdir -p "$W/$1"
    cp "$FN/haproxy/"* "$W/$1/"
    sed -i "$2" "$W/$1/haproxy.cfg"
    ! cmp -s "$W/$1/haproxy.cfg" "$FN/haproxy/haproxy.cfg" && echo "$W/$1"
}

# post_test DELAY: a JSON-RPC request with id 42 to the test HAProxy, answered
# by the stand-in after DELAY s. Prints the status line, then the body.
post_test() {
    curl -s -m 40 -D "$W/headers" -o "$W/body" -H 'content-type: application/json' -H "x-delay: $1" \
        -d '{"jsonrpc":"2.0","id":42,"method":"iota_getChainIdentifier","params":[]}' http://127.0.0.1:19002/
    head -n 1 "$W/headers" | tr -d '\r'
    cat "$W/body"
}

# curl_in_ns SRC DELAY: one JSON-RPC request from SRC to the test HAProxy.
curl_in_ns() {
    inns curl -s -m 60 --interface "$1" -H 'content-type: application/json' -H "x-delay: $2" -d "$RPC" "http://$HOST4:19002/"
    echo
}

# ------------------------------------------------------- fullnode checks

node_port_published() { docker port "$(fn_compose ps -q fullnode)" | grep -q '^9000/'; }

check_U1() {
    [ "$(grep -c '^policy-config:' "$FN/data/config/fullnode.yaml")" = 1 ] && rpc_ok "http://$HOST4:9000/" && ! node_port_published
    report "$?" U1 "upgrade of a fullnode with its own policy-config block, as documented: the node answers through HAProxy" \
        "policy-config blocks: $(grep -c '^policy-config:' "$FN/data/config/fullnode.yaml")"
}

# The previous upgrade text: a second policy-config block. The node does not start.
check_U1_control() {
    local block
    block=$(sed -n '/^policy-config:/,/^  dry-run/p' "$FN/data/config/fullnode.yaml")
    printf '%s\n' "$block" >>"$FN/data/config/fullnode.yaml"
    fn_compose up -d --force-recreate fullnode >/dev/null 2>&1
    wait_for 30 sh -c "docker logs $PREFIX-fullnode-fullnode-1 2>&1 | grep -q 'duplicate field'"
    report "$?" U1-control "with a second policy-config block the node stops: duplicate field" "the node did not report a duplicate field"
}

check_F1() {
    local url version ok=1 same other runtime
    for url in "http://$HOST4:9000/" "http://[$HOST6]:9000/"; do
        for version in --http1.1 --http2-prior-knowledge; do
            inns curl -s -m 5 "$version" -H 'content-type: application/json' -d "$RPC" "$url" | grep -q '"result"' || ok=0
        done
    done
    # Two addresses in one IPv6 /64 share the limit of 4.
    client conns --host "$HOST6" --port 9000 --src 2001:db8::10 --n 2 --probe 0.3 --hold 15 >/dev/null &
    sleep 2
    same=$(client conns --host "$HOST6" --port 9000 --src 2001:db8::11 --n 3 --probe 0.3)
    other=$(client conns --host "$HOST6" --port 9000 --src 2001:db8:0:1::10 --n 4 --probe 0.3)
    # Control: limit 6 for the /64 on the admin socket; the open connections stay counted.
    admin "$MAIN_HAPROXY" "add map /usr/local/etc/haproxy/limits_9000.map 2001:db8::/64 6"
    runtime=$(client conns --host "$HOST6" --port 9000 --src 2001:db8::11 --n 5 --probe 0.3)
    admin "$MAIN_HAPROXY" "del map /usr/local/etc/haproxy/limits_9000.map 2001:db8::/64"
    wait
    [ "$ok" = 1 ] && [ "$(count_of open <<<"$same")" = 2 ] && [ "$(count_of reset <<<"$same")" = 1 ] &&
        [ "$(count_of open <<<"$other")" = 4 ]
    report "$?" F1 "JSON-RPC over IPv4 and IPv6, HTTP/1.1 and h2c; 2001:db8::10 and ::11 share 4; another /64 gets its own 4" \
        "requests ok: $ok; same /64: $same; other /64: $other"
    [ "$(count_of open <<<"$runtime")" = 4 ] && [ "$(count_of reset <<<"$runtime")" = 1 ]
    report "$?" F1-control "limit 6 set on the admin socket: ::11 opens 4 next to the 2 of ::10, the 5th is reset" "$runtime"
}

check_F2() {
    local answer
    answer=$(inns curl -s -m 5 -w ' %{http_code}' -H 'content-type: application/json' \
        --data-binary "@$W/big-body.json" "http://$HOST4:9000/")
    ! node_port_published && [[ "$answer" == *'"code":-32600'*' 200' ]]
    report "$?" F2 "the fullnode publishes no 9000; on 9000 HAProxy answers a body over 1 MiB with its JSON-RPC error" \
        "docker port: $(docker port "$(fn_compose ps -q fullnode)" | tr '\n' ' '); answer: ${answer:0:200}"
}

check_F2_control() {
    fn_compose -f "$HERE/compose.e2e.node-port.yml" up -d fullnode >/dev/null 2>&1
    wait_for 120 rpc_ok "http://$HOST4:19000/" && node_port_published
    report "$?" F2-control "with the node port published, docker port lists 9000 and the node answers on it" \
        "docker port: $(docker port "$(fn_compose ps -q fullnode)" | tr '\n' ' ')"
}

blocked_clients() {
    fn_compose -f "$HERE/compose.e2e.node-port.yml" logs --no-color fullnode 2>/dev/null |
        sed -n 's/.*Adding client \(.*\) to blocklist.*/\1/p' | sort -u | tr '\n' ' '
}
node_blocked() {
    local blocked client
    blocked=" $(blocked_clients) "
    for client in "$@"; do [[ "$blocked" == *" $client "* ]] || return 1; done
}

# send_errors HOST SRC XFF [CLIENT_OPTIONS...]: 800 requests that the node
# answers with an error, on 4 connections at once, with x-forwarded-for: XFF.
send_errors() {
    local host=$1 src=$2 xff=$3
    shift 3
    for _ in 1 2 3 4; do
        client "$@" rpc --host "$host" --port 9000 --src "$src" --xff "$xff" --n 200 \
            --method iota_getObject --params '["zz"]' >/dev/null &
    done
    wait
}

check_F3() {
    send_errors "$HOST4" 192.0.2.10 203.0.113.77
    send_errors "$HOST6" 2001:db8::10 203.0.113.77
    wait_for 30 node_blocked 192.0.2.10 2001:db8::10 && [[ "$(blocked_clients)" != *203.0.113.77* ]]
    report "$?" F3 "through HAProxy the node charges 192.0.2.10 and 2001:db8::10, not the sent x-forwarded-for 203.0.113.77" \
        "node blocklist: $(blocked_clients)"
    # Control: a peer on the compose network sends the header to the node itself.
    docker run --rm --network "$PREFIX-fullnode_default" --entrypoint curl "$NODE_IMAGE" -s \
        -H 'content-type: application/json' -H 'x-forwarded-for: 203.0.113.88' \
        -d '{"jsonrpc":"2.0","id":1,"method":"iota_getObject","params":["zz"]}' 'http://fullnode:9000/?n=[1-300]' >/dev/null
    wait_for 30 node_blocked 203.0.113.88
    report "$?" F3-control1 "a peer on the compose network sets the client address with x-forwarded-for" "node blocklist: $(blocked_clients)"
}

check_F3_header() {
    local out kept
    start_test_haproxy "$FN/haproxy"
    out=$(client rpc --host "$HOST4" --port 19002 --src 192.0.2.10 --xff 203.0.113.5)
    # Control: from a network in trusted_proxies.lst, HAProxy keeps the header.
    admin "$TEST_HAPROXY" "add acl /usr/local/etc/haproxy/trusted_proxies.lst 192.0.2.0/24"
    kept=$(client rpc --host "$HOST4" --port 19002 --src 192.0.2.10 --xff 203.0.113.5)
    [ "$(jq -r .last.result <<<"$out")" = 192.0.2.10 ]
    report "$?" F3-header "the node gets one x-forwarded-for value, the client address, not the sent 203.0.113.5" "$out"
    [ "$(jq -r .last.result <<<"$kept")" = "203.0.113.5, 192.0.2.10" ]
    report "$?" F3-header-control "from a network in trusted_proxies.lst, the sent value stays and the client address follows" "$kept"
}

# haproxy_lines ADDRESS ORIGIN: lines of the main HAProxy log for ADDRESS that
# HAProxy wrote at ORIGIN (txn_close, tcp-req-conn, http-after-res, ...).
haproxy_lines() { fn_compose logs --no-color haproxy 2>/dev/null | grep " $1:.* $2 error="; }
refusal_lines() { haproxy_lines "$1" tcp-req-conn | grep -c "PR--"; }

check_F4() {
    local before a b c lines
    before=$(refusal_lines 192.0.2.10)
    client conns --host "$HOST4" --port 9000 --src 192.0.2.10 --n 9 --probe 0.3 --hold 8 >"$W/f4.json" &
    sleep 4
    # While 192.0.2.10 holds its 4 connections.
    b=$(client conns --host "$HOST4" --port 9000 --src 192.0.2.11 --n 4 --probe 0.3)
    c=$(client conns --host "$HOST4" --port 9000 --src 192.0.2.12 --n 10 --probe 0.3)
    wait
    a=$(cat "$W/f4.json")
    lines=$(($(refusal_lines 192.0.2.10) - before))
    [ "$(count_of open <<<"$a")" = 4 ] && [ "$(count_of reset <<<"$a")" = 5 ] && [ "$(count_of open <<<"$b")" = 4 ] && [ "$lines" = 1 ]
    report "$?" F4 "192.0.2.10: 4 open, 5 reset, 1 log line; at the same time 192.0.2.11 opens its own 4" \
        "192.0.2.10: $a; log lines: $lines; 192.0.2.11: $b"
    [ "$(count_of open <<<"$c")" = 10 ]
    report "$?" F4-control "192.0.2.12 with map value 0 (no limit): 10 open" "192.0.2.12: $c"
}

check_F5() {
    local silent late
    client late-request --host "$HOST4" --port 9000 --src 192.0.2.11 --wait 8 >"$W/f5.json" &
    silent=$(client silent --host "$HOST4" --port 9000 --src 192.0.2.10 --max-secs 20)
    wait
    late=$(cat "$W/f5.json")
    jq -e '.how == "eof" and .secs >= 9.5 and .secs <= 11.5' <<<"$silent" >/dev/null
    report "$?" F5 "a connection that sends nothing is closed after $(jq -r .secs <<<"$silent") s ($(jq -r .first_line <<<"$silent"))" "$silent"
    [ "$(jq -r .status <<<"$late")" = 200 ]
    report "$?" F5-control "a connection that sends its first request after 8 s gets 200" "$late"
}

check_F6() {
    local late early
    client stalled-reader --host "$HOST4" --port 9000 --src 192.0.2.11 --wait 30 >"$W/f6.json" &
    late=$(client stalled-reader --host "$HOST4" --port 9000 --src 192.0.2.10 --wait 70)
    wait
    early=$(cat "$W/f6.json")
    [[ "$(jq -r .how <<<"$late")" =~ ^(closed|reset)$ ]]
    report "$?" F6 "a client that does not read a large response is disconnected before 70 s ($(jq -c . <<<"$late"))" "$late"
    [ "$(jq -r .how <<<"$early")" = open ]
    report "$?" F6-control "the same client is still connected at 30 s" "$early"
}

check_F7() {
    local remote port local_ok=1 other
    remote=$(for port in 9184 8404; do inns curl -s -m 3 -o /dev/null -w '%{http_code} ' "http://$HOST4:$port/metrics"; done)
    for port in 9184 8404; do curl -s -m 3 -o /dev/null "http://127.0.0.1:$port/metrics" || local_ok=0; done
    other=$(curl -s -m 3 -o /dev/null -w '%{http_code}' http://127.0.0.1:8404/other)
    [ "$remote" = "000 000 " ] && [ "$local_ok" = 1 ] && [ "$other" = 404 ]
    report "$?" F7 "9184 and 8404 answer on 127.0.0.1 only; other paths on 8404 get 404" \
        "from the client namespace: $remote; on 127.0.0.1: $local_ok; /other on 8404: $other"
    inns curl -s -m 3 -o /dev/null "http://$HOST4:9000/"
    report "$?" F7-control "the client namespace reaches the published port 9000" "the client namespace does not reach 9000"
}

# HTTP/2 clients: 192.0.2.20 and .21 have the shipped default of 64, .12 has no limit.
check_F9() {
    local obey burst stall one nolimit idle='{"answers": {}, "closed": null}'
    obey=$(client h2c --host "$HOST4" --port 9000 --src 192.0.2.20 --requests 100 --obey-settings)
    burst=$(client h2c --host "$HOST4" --port 9000 --src 192.0.2.21 --requests 100)
    stall=$(client h2stall --host "$HOST4" --port 9000 --src 192.0.2.20 --conns 2 --streams 64 --secs 3)
    one=$(client h2stall --host "$HOST4" --port 9000 --src 192.0.2.21 --conns 1 --streams 64 --secs 3)
    nolimit=$(client h2stall --host "$HOST4" --port 9000 --src 192.0.2.12 --conns 2 --streams 64 --secs 3)
    jq -e '.server_max_streams == 64 and .statuses["200"] == 100' <<<"$obey" >/dev/null
    report "$?" F9 "one h2c connection, 100 requests, client keeps to 64 streams: 100 answers" "$obey"
    jq -e '.answered == 100 and .statuses["rst:7"] > 0' <<<"$burst" >/dev/null
    report "$?" F9-control "100 streams at once on one connection: the streams over 64 are refused ($(jq -c .statuses <<<"$burst"))" "$burst"
    jq -e --argjson idle "$idle" '.per_connection == [$idle, {"answers": {"200:-32009": 64}, "closed": null}]' <<<"$stall" >/dev/null
    report "$?" F9-stall "two h2c connections from one address, 64 waiting bodies each: the second gets 64 x -32009 at once" "$stall"
    jq -e --argjson idle "$idle" '.per_connection == [$idle]' <<<"$one" >/dev/null
    report "$?" F9-stall-control "one connection with 64 waiting bodies: no answer, connection open" "$one"
    jq -e --argjson idle "$idle" '.per_connection == [$idle, $idle]' <<<"$nolimit" >/dev/null
    report "$?" F9-map "map value 0: two connections with 64 waiting bodies each get no -32009" "$nolimit"
}

check_F11() {
    local a b direct
    client --proxy-client 198.51.100.77 conns --host "$HOST4" --port 9000 --src 192.0.2.13 --n 5 --probe 0.3 --hold 6 >"$W/f11.json" &
    sleep 3
    b=$(client --proxy-client 198.51.100.78 conns --host "$HOST4" --port 9000 --src 192.0.2.13 --n 4 --probe 0.3)
    wait
    a=$(cat "$W/f11.json")
    send_errors "$HOST4" 192.0.2.13 203.0.113.91 --proxy-client 198.51.100.79
    [ "$(count_of open <<<"$a")" = 4 ] && [ "$(count_of open <<<"$b")" = 4 ] && wait_for 30 node_blocked 198.51.100.79 &&
        [[ "$(blocked_clients)" != *192.0.2.13* ]] && [[ "$(blocked_clients)" != *203.0.113.91* ]]
    report "$?" F11 "PROXY header from a listed load balancer: 198.51.100.77 gets 4 connections, the 5th closed; 198.51.100.78 gets its own 4; the node charges 198.51.100.79" \
        "198.51.100.77: $a; 198.51.100.78: $b; node blocklist: $(blocked_clients)"
    direct=$(client --proxy-client 198.51.100.80 rpc --host "$HOST4" --port 9000 --src 192.0.2.11 --timeout 10)
    [ "$(jq -r '.statuses["400"] // 0' <<<"$direct")" = 1 ]
    report "$?" F11-control "the same header from 192.0.2.11, which is not in proxy_protocol.lst, gets HTTP 400" "$direct"
}

check_F12() {
    local held more requests again
    admin "$MAIN_HAPROXY" "set maxconn server node_websocket/node 2"
    client ws --host "$HOST4" --port 9000 --src 192.0.2.10 --n 2 --hold 12 >"$W/f12.json" &
    sleep 3
    requests=$(client rpc --host "$HOST4" --port 9000 --src 192.0.2.12 --n 5)
    more=$(client ws --host "$HOST4" --port 9000 --src 192.0.2.11 --n 1)
    # Control: with the WebSocket limit back at 200, one more WebSocket opens.
    admin "$MAIN_HAPROXY" "set maxconn server node_websocket/node 200"
    again=$(client ws --host "$HOST4" --port 9000 --src 192.0.2.11 --n 1)
    wait
    held=$(cat "$W/f12.json")
    [ "$(jq -c '[.outcomes[].status]' <<<"$held")" = '[101,101]' ] && jq -e '.results == 5 and .slowest_ms < 1000' <<<"$requests" >/dev/null &&
        jq -e '.outcomes[0].status == 503 and (.outcomes[0].body | fromjson | .error.code == -32009)' <<<"$more" >/dev/null
    report "$?" F12 "WebSocket limit 2 in use: 5 requests answered (slowest $(jq .slowest_ms <<<"$requests") ms); one more WebSocket gets HTTP 503 with JSON-RPC -32009" \
        "held: $held; requests: $requests; one more WebSocket: $more"
    [ "$(jq -r '.outcomes[0].status' <<<"$again")" = 101 ]
    report "$?" F12-control "with the WebSocket limit at 200, the same WebSocket opens" "$again"
}

# 192.0.2.15 has the map value 2: an open WebSocket and 2 waiting requests fit.
check_F12_count() {
    local ws two three
    client ws --host "$HOST4" --port 9000 --src 192.0.2.15 --n 1 --hold 12 >"$W/f12-count.json" &
    sleep 2
    two=$(client h2stall --host "$HOST4" --port 9000 --src 192.0.2.15 --conns 1 --streams 2 --secs 3)
    three=$(client h2stall --host "$HOST4" --port 9000 --src 192.0.2.15 --conns 1 --streams 3 --secs 3)
    wait
    ws=$(cat "$W/f12-count.json")
    [ "$(jq -c '[.outcomes[].status]' <<<"$ws")" = '[101]' ] && jq -e '.per_connection[0].answers == {}' <<<"$two" >/dev/null
    report "$?" F12-count "limit 2 with an open WebSocket: 2 waiting requests are not refused" "WebSocket: $ws; 2 requests: $two"
    jq -e '.per_connection[0].answers == {"200:-32009": 1}' <<<"$three" >/dev/null
    report "$?" F12-count-control "a third waiting request is refused with -32009" "$three"
}

check_F13() {
    local v4 v6
    v4=$(client udp --host "$HOST4" --port 8084 --src 192.0.2.10 | jq -r .how)
    v6=$(client udp --host "$HOST6" --port 8084 --src 2001:db8::10 | jq -r .how)
    [ "$v4 $v6" = "no_reply no_reply" ]
    report "$?" F13 "UDP to the P2P port 8084 reaches the node over IPv4 and IPv6 (no ICMP error)" "IPv4: $v4; IPv6: $v6"
    [ "$(client udp --host "$HOST6" --port 8099 --src 2001:db8::10 | jq -r .how)" = refused ]
    report "$?" F13-control "UDP to a closed port gets an ICMP error" "no ICMP error from a closed port"
}

check_F14() {
    local dir="$W/f14" project=(-p "$PREFIX-f14" --project-directory "$W/f14" -f "$W/f14/docker-compose.yaml" -f "$HERE/compose.e2e.fullnode.yml")
    mkdir -p "$dir"
    cp "$FN/docker-compose.yaml" "$dir/"
    ! docker compose "${project[@]}" create haproxy >"$W/f14.log" 2>&1 && [ ! -e "$dir/haproxy" ]
    report "$?" F14 "without haproxy/, compose stops and creates no directory: $(grep -o 'bind source path does not exist' "$W/f14.log" | head -n 1)" \
        "created a container or $dir/haproxy: $(tail -n 2 "$W/f14.log")"
    docker compose "${project[@]}" down >/dev/null 2>&1
}

# A connection that ends in the middle of an HTTP/2 frame is released.
check_F15() {
    local held
    client conns --host "$HOST4" --port 9000 --src 192.0.2.16 --n 1 --probe 0.3 --hold 5 >/dev/null &
    sleep 2
    held=$(conn_cur 192.0.2.16)
    wait
    client h2partial --host "$HOST4" --port 9000 --src 192.0.2.16 --n 2 >/dev/null
    wait_for 15 released 192.0.2.16
    report "$?" F15 "2 connections that end in the middle of an HTTP/2 frame are released: count 0" "count after 15 s: $(conn_cur 192.0.2.16)"
    [ "$held" = 1 ]
    report "$?" F15-control "the admin socket shows an open connection of the same address as 1" "count of an open connection: $held"
}

# HAProxy's own errors reach the client as HTTP 200, but its metrics and its
# log count them. 1 in 100 other responses are logged.
check_F17() {
    local denied denied_after before own sampled bad
    denied=$(haproxy_metric 'haproxy_frontend_requests_denied_total{proxy="jsonrpc"}')
    before=$(haproxy_lines 192.0.2.11 txn_close | grep -c ' 200 .* error=413 ')
    for _ in 1 2 3; do
        inns curl -s -m 5 -o /dev/null --interface 192.0.2.11 -H 'content-type: application/json' \
            --data-binary "@$W/big-body.json" "http://$HOST4:9000/"
    done
    sleep 1
    denied_after=$(haproxy_metric 'haproxy_frontend_requests_denied_total{proxy="jsonrpc"}')
    own=$(($(haproxy_lines 192.0.2.11 txn_close | grep -c ' 200 .* error=413 ') - before))
    [ $((denied_after - denied)) = 3 ] && [ "$own" = 3 ]
    report "$?" F17 "3 bodies over 1 MiB, HTTP 200 to the client: requests_denied_total +3 and 3 log lines '... 200 ... txn_close error=413'" \
        "requests_denied_total $denied -> $denied_after; log lines: $own"
    # 1000 requests: the number of sampled lines is binomial (mean 10).
    before=$(haproxy_lines 192.0.2.12 http-after-res | wc -l)
    client rpc --host "$HOST4" --port 9000 --src 192.0.2.12 --n 1000 >/dev/null
    sleep 1
    sampled=$(($(haproxy_lines 192.0.2.12 http-after-res | wc -l) - before))
    bad=$(haproxy_lines 192.0.2.12 http-after-res | tail -n +$((before + 1)) |
        grep -cEv ' 200 .* http-after-res error=- conn=[0-9]+ req=[0-9]+$')
    [ "$sampled" -ge 1 ] && [ "$sampled" -le 30 ] && [ "$bad" = 0 ]
    report "$?" F17-sample "1000 requests give $sampled sampled log lines (1 in 100), each with the open connections and requests in progress of the address" \
        "sampled lines: $sampled; lines in another form: $bad"
}

check_F8() {
    local out
    start_test_haproxy "$(haproxy_copy f8 's/^\(    timeout server\) .*/\1 3s/')"
    out=$(post_test 30)
    [[ "$out" == "HTTP/1.1 200"* ]] && jq -e '.id == 42 and .error.code == -32050 and
        (.error.message | startswith("The node did not answer in time"))' <<<"${out#*$'\n'}" >/dev/null
    report "$?" F8 "node timeout: HTTP 200, JSON-RPC -32050 'did not answer in time', id 42" "$out"
    # Control: without the http-error lines, the reply is the HTML 504 of HAProxy.
    start_test_haproxy "$(haproxy_copy f8-html 's/^\(    timeout server\) .*/\1 3s/; /^    http-error /d')"
    out=$(post_test 30)
    [[ "$out" == "HTTP/1.1 504"*"<html>"* ]]
    report "$?" F8-control "without the http-error lines: HTML with 504" "$out"
}

check_F10() {
    local maxconn src peak results queued start waited
    start_test_haproxy "$FN/haproxy"
    for maxconn in 4 0; do
        admin "$TEST_HAPROXY" "set maxconn server node_jsonrpc/node $maxconn"
        docker exec "$PREFIX-standin" sh -c ': >/tmp/in-progress.log'
        for src in 192.0.2.10 192.0.2.11 192.0.2.12; do
            for _ in 1 2 3 4; do curl_in_ns "$src" 2 >>"$W/f10-$maxconn.log" & done
        done
        wait
        peak=$(docker exec "$PREFIX-standin" sort -n /tmp/in-progress.log | tail -n 1)
        results=$(grep -o '"result"' "$W/f10-$maxconn.log" | wc -l)
        if [ "$maxconn" = 4 ]; then
            [ "$peak" -le 4 ] && [ "$results" = 12 ]
            report "$?" F10 "server maxconn 4: at most $peak of 12 requests at the node at once; all 12 answered" \
                "highest at the node: $peak; answered: $results of 12"
        else
            [ "$peak" -gt 4 ]
            report "$?" F10-control "server maxconn 0 (no limit): $peak requests at the node at once" "without maxconn only $peak at once"
        fi
    done
    # A request that waits longer than "timeout queue" (30 s) gets the 503
    # error, with a code that the client does not treat as transient.
    admin "$TEST_HAPROXY" "set maxconn server node_jsonrpc/node 1"
    curl_in_ns 192.0.2.10 45 >/dev/null &
    sleep 1
    start=$SECONDS
    queued=$(curl_in_ns 192.0.2.11 0)
    waited=$((SECONDS - start))
    jq -e '.error.code == -32009 and .error.message == "The node is not available"' <<<"$queued" >/dev/null &&
        [ "$waited" -ge 29 ] && [ "$waited" -le 35 ]
    report "$?" F10-queue "a request queued for ${waited} s gets JSON-RPC -32009 'The node is not available'" "after ${waited} s: $queued"
    docker rm -f "$TEST_HAPROXY" "$PREFIX-standin" >/dev/null
    wait
}

# The HTTP/1 connection age (over at once in this copy) adds "Connection:
# close" to a response, but not to a WebSocket upgrade.
check_F16() {
    local ws post
    start_test_haproxy "$(haproxy_copy f16 's/rand(360),add(1620)/int(-1)/')" fullnode:9000
    ws=$(client ws --host "$HOST4" --port 19002 --src 192.0.2.10 --n 1)
    post=$(curl -s -m 5 -D - -o /dev/null -H 'content-type: application/json' -d "$RPC" http://127.0.0.1:19002/ |
        tr -d '\r' | grep -i '^connection:')
    docker rm -f "$TEST_HAPROXY" >/dev/null
    jq -e '.outcomes[0].status == 101 and (.outcomes[0].connection | ascii_downcase) == "upgrade"' <<<"$ws" >/dev/null
    report "$?" F16 "a WebSocket upgrade past the connection age keeps 'Connection: Upgrade'" "$ws"
    [ "$(tr '[:upper:]' '[:lower:]' <<<"$post")" = "connection: close" ]
    report "$?" F16-control "a request past the connection age gets 'Connection: close'" "header: '$post'"
}

# With the node stopped, the health check marks the server DOWN, and every
# POST gets the 503 error of HAProxy at once.
check_F8_stopped() {
    local up post time ids get origin start down down_state
    up=$(server_state UP)
    fn_compose stop fullnode >/dev/null 2>&1
    start=$SECONDS
    # Checks every 2 s, at most 2 s each, DOWN after 2 failures.
    wait_for 15 server_down
    down=$((SECONDS - start))
    down_state=$(server_state DOWN)
    post=$(curl -s -m 30 -w '\n%{http_code} %{time_total}' -H 'content-type: application/json' -d "$RPC" http://127.0.0.1:9000/)
    time=${post##* }
    post=${post% *}
    ids=$(client ids --host "$HOST4" --port 9000 --src 192.0.2.12)
    # A health check must see the stopped node as down.
    get=$(curl -s -m 30 -o /dev/null -w '%{http_code}' http://127.0.0.1:9000/health)
    origin=$(curl -s -m 30 -o /dev/null -D - -H 'origin: https://example.com' -H 'content-type: application/json' \
        -d "$RPC" http://127.0.0.1:9000/ | tr -d '\r' | grep -i '^access-control-allow-origin:')
    fn_compose start fullnode >/dev/null 2>&1
    [ "$up" = 1 ] && [ "$down_state" = 1 ] && [ "$down" -le 10 ] && awk -v t="$time" 'BEGIN { exit !(t < 1) }'
    report "$?" F8-down "node stopped: the server is DOWN after $down s (UP before), then a POST is answered in $time s" \
        "UP before: $up; DOWN after $down s: $down_state; POST time: $time s"
    [ "${post##*$'\n'}" = 200 ] && jq -e '.error.code == -32009' <<<"${post%$'\n'*}" >/dev/null
    report "$?" F8-stopped "node stopped, POST: HTTP 200, JSON-RPC -32009" "$post"
    jq -e '.bad == []' <<<"$ids" >/dev/null
    report "$?" F8-ids "every error reply parses as JSON and has the sent id ($(jq -r .ids <<<"$ids") ids, among them \"007\", \"00\" and a 15000-character string)" "$ids"
    [ "$get" = 503 ]
    report "$?" F8-get "node stopped, GET /health: HTTP 503" "node stopped, GET /health: HTTP $get"
    [ "$origin" = "access-control-allow-origin: *" ]
    report "$?" F8-cors "an error of HAProxy for a request with an Origin header has 'access-control-allow-origin: *'" "header: '$origin'"
    wait_for 120 rpc_ok "http://$HOST4:9000/"
}

# ------------------------------------------------------ validator checks

load_values() { vn_compose exec -T node-limits sh /etc/iota-node-limits/node-limits.sh load /etc/iota-node-limits/node-limits.conf; }

check_V1() {
    local more
    vn_compose restart node-limits >/dev/null 2>&1
    wait_for 30 table_present
    tls_conns "$HOST4" 192.0.2.11 3 15 >/dev/null &
    sleep 3
    load_values
    more=$(tls_conns "$HOST4" 192.0.2.11 2)
    [ "$(count_of tls <<<"$more")" = 1 ] && [ "$(count_of refused <<<"$more")" = 1 ]
    report "$?" V1 "table loaded and loaded again after a restart; a load of the same values keeps the counts (3 held + 1, then refused)" "after the load: $more"
    # Control: changed values are loaded, and the counts start at zero.
    sed -i 's/^loopback 1024$/loopback 1000/' "$VN/node-limits/node-limits.conf"
    load_values
    more=$(tls_conns "$HOST4" 192.0.2.11 4)
    sed -i 's/^loopback 1000$/loopback 1024/' "$VN/node-limits/node-limits.conf"
    load_values
    [ "$(count_of tls <<<"$more")" = 4 ]
    report "$?" V1-control "after a change of the values the same address opens 4 more" "after a change: $more"
    wait
}

check_V2() {
    local v4 v6a v6b v6c off
    tls_conns "$HOST4" 192.0.2.10 5 12 >"$W/v2-4.json" &
    sleep 4
    tls_conns "$HOST6" 2001:db8::10 2 12 >"$W/v2-6.json" &
    sleep 3
    v6b=$(tls_conns "$HOST6" 2001:db8::11 3)
    v6c=$(tls_conns "$HOST6" 2001:db8:0:1::10 4)
    wait
    v4=$(cat "$W/v2-4.json")
    v6a=$(cat "$W/v2-6.json")
    [ "$(count_of tls <<<"$v4")" = 4 ] && [ "$(count_of refused <<<"$v4")" = 1 ] && [ "$(count_of tls <<<"$v6a")" = 2 ] &&
        [ "$(count_of tls <<<"$v6b")" = 2 ] && [ "$(count_of refused <<<"$v6b")" = 1 ] && [ "$(count_of tls <<<"$v6c")" = 4 ]
    report "$?" V2 "192.0.2.10: 4 TLS, the 5th refused at the SYN; 2001:db8::10 and ::11 share 4; another /64 opens 4" \
        "IPv4: $v4; same /64: $v6a $v6b; other /64: $v6c"
    # Control: IOTA_NODE_LIMITS=off in .env, as documented, stays after the
    # next plain "up"; then 6 connect.
    echo IOTA_NODE_LIMITS=off >"$VN/.env"
    vn_compose up -d >/dev/null 2>&1
    wait_for 30 table_absent && vn_compose up -d >/dev/null 2>&1 && sleep 3 && table_absent
    off=$?
    v4=$(tls_conns "$HOST4" 192.0.2.10 6)
    rm "$VN/.env"
    vn_compose up -d node-limits >/dev/null 2>&1
    wait_for 30 table_present
    [ "$off" = 0 ] && [ "$(count_of tls <<<"$v4")" = 6 ]
    report "$?" V2-control "IOTA_NODE_LIMITS=off in .env: no table, also after a second 'up -d'; 6 TLS connections from 192.0.2.10" \
        "table absent after the second up: $([ "$off" = 0 ] && echo yes || echo no); $v4"
}

check_V3() {
    local o24 o32 o0
    tls_conns "$HOST4" 198.51.100.10 9 10 >"$W/v3-24.json" &
    tls_conns "$HOST4" 198.51.100.11 7 10 >"$W/v3-32.json" &
    o0=$(tls_conns "$HOST4" 198.51.100.12 10)
    wait
    o24=$(cat "$W/v3-24.json")
    o32=$(cat "$W/v3-32.json")
    [ "$(count_of tls <<<"$o24")" = 8 ] && [ "$(count_of refused <<<"$o24")" = 1 ] &&
        [ "$(count_of tls <<<"$o32")" = 6 ] && [ "$(count_of refused <<<"$o32")" = 1 ]
    report "$?" V3 "override /24 = 8: 8 TLS, then refused; /32 = 6 inside it: 6 TLS, then refused" "/24: $o24; /32: $o32"
    [ "$(count_of tls <<<"$o0")" = 10 ]
    report "$?" V3-control "override 0: 10 TLS connections" "override 0: $o0"
}

check_V4() {
    local lo
    lo=$(python3 -I "$HERE/client.py" conns --host 127.0.0.1 --port 8080 --n 10 --tls)
    [ "$(count_of tls <<<"$lo")" = 10 ]
    report "$?" V4 "127.0.0.1: 10 TLS connections (loopback limit)" "$lo"
}

check_V5() {
    local before after
    if [ "$KERNEL_LOG" != 1 ]; then
        check V5 SKIP "E2E_KERNEL_LOG=0: the kernel does not log from this network namespace"
        return
    fi
    before=$("${SUDO[@]}" dmesg | grep -c 'iota-node-limits refused: .*SRC=192.0.2.12 ')
    tls_conns "$HOST4" 192.0.2.12 9 3 >/dev/null
    sleep 1
    after=$("${SUDO[@]}" dmesg | grep -c 'iota-node-limits refused: .*SRC=192.0.2.12 ')
    [ $((after - before)) = 1 ]
    report "$?" V5 "5 refusals from 192.0.2.12 give 1 kernel log line" "kernel log lines for 5 refusals: $((after - before))"
}

check_V6() {
    local stopped=yes killed=no reloaded=no
    vn_compose stop node-limits >/dev/null 2>&1
    table_present && stopped=no
    vn_compose up -d node-limits >/dev/null 2>&1
    wait_for 30 table_present
    vn_compose kill -s KILL node-limits >/dev/null 2>&1
    table_present && killed=yes
    vn_compose up -d node-limits >/dev/null 2>&1
    sleep 3
    "${SUDO[@]}" nft delete table inet "$TABLE"
    wait_for 35 table_present && reloaded=yes
    [ "$stopped$killed$reloaded" = yesyesyes ]
    report "$?" V6 "stop removes the table; SIGKILL leaves it; the helper loads a deleted table again within 35 s" \
        "removed on stop: $stopped; kept after SIGKILL: $killed; loaded again: $reloaded"
}

check_V7() {
    local v6
    v6=$(tls_conns "$HOST6" 2001:db8:0:1::10 1)
    [ "$(count_of tls <<<"$v6")" = 1 ]
    report "$?" V7 "an IPv6 client reaches 8080 on the network of the shipped file" "$v6"
    # Control: with IPv6 on the compose network, Docker sends IPv6 clients to
    # the IPv6 address of the container, where the node does not listen.
    vn_compose down >/dev/null 2>&1
    vn_compose -f "$HERE/compose.e2e.validator-ipv6.yml" up -d >/dev/null 2>&1
    wait_for 120 tls_ok 127.0.0.1
    v6=$(tls_conns "$HOST6" 2001:db8:0:1::10 1)
    vn_compose -f "$HERE/compose.e2e.validator-ipv6.yml" down >/dev/null 2>&1
    vn_compose up -d >/dev/null 2>&1
    validator_ready
    [ "$(count_of refused <<<"$v6")" = 1 ]
    report "$?" V7-control "with enable_ipv6 on the validator network, the IPv6 client is refused" "$v6"
}

restart_validator() {
    validator_config "$@"
    vn_compose up -d --force-recreate iota-node >/dev/null 2>&1
    wait_for 120 tls_ok 127.0.0.1
}

check_V8() {
    local conns refused warns
    if [ "$NODE_GRPC_MAX_CONNECTIONS" != 1 ]; then
        check V8 SKIP "needs a node with grpc-max-connections (NODE_GRPC_MAX_CONNECTIONS=1)"
        return
    fi
    IOTA_NODE_LIMITS=off vn_compose up -d node-limits >/dev/null 2>&1
    restart_validator --set grpc-max-connections=8
    conns=$(tls_conns "$HOST4" 192.0.2.10 9 3)
    refused=$(metric http://127.0.0.1:9184/metrics 'authority_grpc_inbound_connections_refused{reason="max_connections"}') || refused="no series"
    warns=$(vn_compose logs --no-color iota-node 2>/dev/null | grep -c 'WARN.*192.0.2.10')
    [ "$(count_of tls <<<"$conns")" = 8 ] && [ "$(count_of reset <<<"$conns")" = 1 ] && [ "$refused" = 1 ] && [ "$warns" = 1 ]
    report "$?" V8 "grpc-max-connections 8: the 9th connection is reset, refused{max_connections} = 1, one warn line" \
        "connections: $conns; refused metric: $refused; warn lines: $warns"
    restart_validator --set grpc-max-connections=0
    conns=$(tls_conns "$HOST4" 192.0.2.10 12)
    restart_validator
    vn_compose up -d node-limits >/dev/null 2>&1
    validator_ready
    [ "$(count_of tls <<<"$conns")" = 12 ]
    report "$?" V8-control "grpc-max-connections 0 (no limit): 12 TLS connections" "$conns"
}

check_V9() {
    local remote local_ok=no reach
    remote=$(inns curl -s -m 3 -o /dev/null -w '%{http_code}' "http://$HOST4:9184/metrics")
    curl -s -m 3 -o /dev/null "http://127.0.0.1:9184/metrics" && local_ok=yes
    reach=$(tls_conns "$HOST4" 192.0.2.11 1)
    [ "$remote" = 000 ] && [ "$local_ok" = yes ]
    report "$?" V9 "9184 answers on 127.0.0.1 only" "from the client namespace: $remote; on 127.0.0.1: $local_ok"
    [ "$(count_of tls <<<"$reach")" = 1 ]
    report "$?" V9-control "the client namespace reaches 8080" "$reach"
}

# 8081 and 8084/udp have no limit: a listener in the network namespace of the
# node takes 8081, and an address at its limit on 8080 connects to it.
check_V10() {
    local held tcp udp
    docker run -d --name "$PREFIX-listener" --network "container:$(vn_compose ps -q iota-node)" \
        --entrypoint socat "$(haproxy_image)" TCP-LISTEN:8081,fork,reuseaddr 'SYSTEM:sleep 30' >/dev/null
    sleep 1
    tls_conns "$HOST4" 192.0.2.10 5 10 >"$W/v10.json" &
    sleep 3
    tcp=$(client conns --host "$HOST4" --port 8081 --src 192.0.2.10 --n 10 --probe 0.3)
    udp=$(client udp --host "$HOST4" --port 8084 --src 192.0.2.10 | jq -r .how)
    wait
    held=$(cat "$W/v10.json")
    docker rm -f "$PREFIX-listener" >/dev/null
    [ "$(count_of open <<<"$tcp")" = 10 ] && [ "$udp" = no_reply ]
    report "$?" V10 "192.0.2.10 at its 8080 limit: 10 connections to 8081 open, UDP to 8084 gets no ICMP error" "8081: $tcp; 8084/udp: $udp"
    [ "$(count_of refused <<<"$held")" = 1 ]
    report "$?" V10-control "the same address is refused on 8080 at the 5th connection" "$held"
}

# ----------------------------------------------------------------- main

for tool in docker jq python3 curl ip nft; do
    command -v "$tool" >/dev/null || {
        echo "run.sh needs $tool" >&2
        exit 2
    }
done
log "work directory $W, node image $NODE_IMAGE"
: >"$RESULTS"
# A failed step must give FAIL, also when the texts of report run commands.
[[ "$(RESULTS=/dev/null; false; report "$?" guard "$(true)" failed)" == "CHECK guard FAIL failed" ]] || {
    echo "report does not see a failed step" >&2
    exit 2
}
python3 -I -c 'import json; print(json.dumps({"jsonrpc": "2.0", "id": 1, "method": "x", "params": ["x" * 1100000]}))' >"$W/big-body.json"
setup_netns
setup_genesis

# phase SETUP CHECK...: runs SETUP and then the checks, if a check is wanted.
phase() {
    local setup=$1 id any=no status
    shift
    for id in "$@"; do wanted "$id" && any=yes; done
    [ "$any" = yes ] || return 0
    set +e
    (
        set -e
        "$setup"
    )
    status=$?
    set -e
    if [ "$status" != 0 ]; then
        check "$setup" FAIL "the setup failed (exit $status); its checks did not run"
        return 0
    fi
    for id in "$@"; do run_check "$id"; done
}
phase setup_fullnode "${FULLNODE_CHECKS[@]}"
if [ -f "$FN/docker-compose.yaml" ]; then
    fn_compose -f "$HERE/compose.e2e.node-port.yml" logs --no-color >"$W/fullnode.log" 2>&1 || true
    fn_compose down -v >/dev/null 2>&1 || true
fi
phase setup_validator "${VALIDATOR_CHECKS[@]}"

echo "SUMMARY $(grep -c PASS "$RESULTS") PASS, $(grep -c FAIL "$RESULTS") FAIL, $(grep -c SKIP "$RESULTS") SKIP"
! grep -q FAIL "$RESULTS"
