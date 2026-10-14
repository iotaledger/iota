#!/bin/sh
# Copyright (c) 2026 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

# Connection limits per client address for the TCP ports of a validator, with
# nftables. The rules are in their own table, inet iota_node_limits. They count
# the connections of each IPv4 address and of each IPv6 /64, and reset a new
# connection over the limit at its first packet.
#
# Usage:
#   node-limits.sh generate <values-file>  Write the nftables rules to stdout.
#   node-limits.sh load <values-file>      Load the rules. Skip if the loaded
#                                          table has the same rules.
#   node-limits.sh unload                  Remove the table.
#   node-limits.sh run <values-file>       Load the rules, then load them again
#                                          every 30 s, or at once on SIGHUP, if
#                                          the table or the values file changes.
#                                          Remove the table on SIGTERM or SIGINT.
#
# With IOTA_NODE_LIMITS=off, "run" removes the table and does not load it. If
# the values file has an error, "run" keeps the loaded table and logs the error
# every 30 s. If it starts with such a file, it loads no limits and exits 1.
# See node-limits.conf for the values file.

set -eu

TABLE=iota_node_limits
RULES_FILE=${IOTA_NODE_LIMITS_RULES_FILE:-/run/iota-node-limits.nft}
INTERVAL=30

log() {
    echo "node-limits: $*" >&2
}

# Writes the rules for the values file $1 to stdout. Stops with an error that
# names the line of a bad value.
generate() {
    awk -v file="$1" -v table="$TABLE" '
function fail(msg) {
    if (NR > 0)
        msg = file ":" NR ": " msg
    else
        msg = file ": " msg
    print "node-limits: " msg > "/dev/stderr"
    failed = 1
    exit 1
}
function is_count(s) {
    return s ~ /^[0-9]+$/ && length(s) <= 9
}
function check_port(s) {
    if (s !~ /^[0-9]+$/ || length(s) > 5 || s + 0 < 1 || s + 0 > 65535)
        fail("port must be a number from 1 to 65535: " s)
    return s + 0
}
function check_count(s, what) {
    if (!is_count(s))
        fail(what " must be a whole number of 0 or more: " s)
    return s + 0
}
# The network of a key from network_key, written as nft writes it: IPv4 in
# decimal, IPv6 in lower-case hex with the longest run of zero groups as "::".
function canonical(nk, f, n, i, out, best, bestlen, run, start) {
    split(nk, f, "[ /]")
    if (f[1] == "4")
        return f[2] "." f[3] "." f[4] "." f[5] "/" f[6]
    best = 0
    bestlen = 1
    run = 0
    for (i = 2; i <= 9; i++) {
        if (f[i] == 0) {
            if (run == 0)
                start = i
            run++
            if (run > bestlen) {
                best = start
                bestlen = run
            }
        } else {
            run = 0
        }
    }
    out = ""
    for (i = 2; i <= 9; i++) {
        if (best && i == best) {
            out = out "::"
            i += bestlen - 1
            continue
        }
        if (out != "" && out !~ /::$/)
            out = out ":"
        out = out sprintf("%x", f[i])
    }
    return out "/" f[10]
}
function hex4(s, i, c, v) {
    v = 0
    for (i = 1; i <= length(s); i++) {
        c = index("0123456789abcdef", substr(s, i, 1))
        v = v * 16 + c - 1
    }
    return v
}
# Checks an IPv4 or IPv6 network and returns it in a canonical form:
# "4 <32-bit groups>/<len>" or "6 <eight 16-bit groups>/<len>". A bare address
# is a network of one address.
function network_key(s, parts, n, len, i, a, v, out, halves, groups, head, tail, g, bits, keep) {
    s = tolower(s)
    len = -1
    if (split(s, parts, "/") > 2)
        fail("not a network: " s)
    if (index(s, "/")) {
        if (parts[2] !~ /^[0-9]+$/ || length(parts[2]) > 3)
            fail("not a network: " s)
        len = parts[2] + 0
    }
    a = parts[1]
    if (index(a, ":") == 0) {
        if (len < 0)
            len = 32
        if (len > 32 || a !~ /^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+$/)
            fail("not an IPv4 network: " s)
        split(a, g, ".")
        out = "4"
        for (i = 1; i <= 4; i++) {
            if (length(g[i]) > 3 || g[i] + 0 > 255)
                fail("not an IPv4 network: " s)
            # nft reads a number with a leading zero as octal.
            if (g[i] ~ /^0[0-9]/)
                fail("an IPv4 number with a leading zero is ambiguous: " s)
            bits = len - (i - 1) * 8
            keep = bits >= 8 ? 256 : (bits <= 0 ? 1 : 2 ^ bits)
            v = g[i] + 0
            if (bits < 8 && v % (256 / keep) != 0)
                fail("the address has bits set after the prefix length: " s)
            out = out " " v
        }
        return out "/" len
    }
    if (len < 0)
        len = 128
    if (len > 128 || a !~ /^[0-9a-f:]+$/ || a ~ /:::/)
        fail("not an IPv6 network: " s)
    n = split(a, halves, "::")
    if (n > 2)
        fail("not an IPv6 network: " s)
    split("", g)
    head = (halves[1] == "") ? 0 : split(halves[1], h1, ":")
    tail = (n == 2 && halves[2] != "") ? split(halves[2], h2, ":") : 0
    if ((n == 1 && head != 8) || (n == 2 && head + tail > 7))
        fail("not an IPv6 network: " s)
    groups = 0
    for (i = 1; i <= head; i++)
        g[++groups] = h1[i]
    for (i = head + tail; i < 8; i++)
        g[++groups] = "0"
    for (i = 1; i <= tail; i++)
        g[++groups] = h2[i]
    out = "6"
    for (i = 1; i <= 8; i++) {
        if (g[i] == "" || length(g[i]) > 4)
            fail("not an IPv6 network: " s)
        v = hex4(g[i])
        bits = len - (i - 1) * 16
        keep = bits >= 16 ? 65536 : (bits <= 0 ? 1 : 2 ^ bits)
        if (bits < 16 && v % (65536 / keep) != 0)
            fail("the address has bits set after the prefix length: " s)
        out = out " " v
    }
    return out "/" len
}
/\r$/ {
    fail("the file has Windows line ends (CRLF); save it with Unix line ends (LF)")
}
{
    sub(/#.*/, "")
    if (NF == 0)
        next
    key = $1
    if (key == "limit") {
        if (NF != 3)
            fail("expected: limit <port> <connections>")
        port = check_port($2)
        if (port in limit)
            fail("port " port " has a limit already")
        limit[port] = check_count($3, "connections")
        ports[++nports] = port
    } else if (key == "override") {
        if (NF != 4)
            fail("expected: override <port> <network> <connections>")
        port = check_port($2)
        if (!(port in limit))
            fail("override for port " port ", which has no limit line before it")
        if (limit[port] == 0)
            fail("override for port " port ", which has no limit (0)")
        nk = network_key($3)
        if ((port, nk) in seen)
            fail("network " $3 " has an override for port " port " already")
        seen[port, nk] = 1
        n = ++noverrides[port]
        # The rules get the network that was checked, not the text of the file.
        o_net[port, n] = canonical(nk)
        o_family[port, n] = substr(nk, 1, 1)
        split(nk, tmp, "/")
        o_len[port, n] = tmp[2] + 0
        o_count[port, n] = check_count($4, "connections")
    } else if (key == "loopback") {
        if (NF != 2)
            fail("expected: loopback <connections>")
        if (loopback != "")
            fail("loopback is set already")
        loopback = check_count($2, "connections")
    } else if (key == "handshake-timeout") {
        if (NF != 2)
            fail("expected: handshake-timeout <seconds>")
        if (timeout != "")
            fail("handshake-timeout is set already")
        timeout = check_count($2, "handshake-timeout")
        if (timeout < 1)
            fail("handshake-timeout must be 1 or more")
        if (timeout > 3600)
            fail("handshake-timeout must be 3600 or less")
    } else {
        fail("unknown setting: " key)
    }
}
END {
    if (failed)
        exit 1
    NR = 0
    if (loopback == "")
        fail("loopback is not set")
    if (timeout == "")
        fail("handshake-timeout is not set")
    for (i = 1; i <= nports; i++)
        if (limit[ports[i]] > 0)
            limited[++nlimited] = ports[i]
    if (nlimited == 0)
        fail("no port has a limit")

    # Overrides of a port in the order of the rules: IPv4 first, then the
    # longest prefix first, so that a smaller network wins over a larger one.
    for (p = 1; p <= nlimited; p++) {
        port = limited[p]
        for (i = 1; i <= noverrides[port]; i++)
            order[port, i] = i
        for (i = 2; i <= noverrides[port]; i++) {
            j = i
            while (j > 1) {
                a = order[port, j - 1]
                b = order[port, j]
                if (o_family[port, a] < o_family[port, b] ||
                    (o_family[port, a] == o_family[port, b] && o_len[port, a] >= o_len[port, b]))
                    break
                order[port, j - 1] = b
                order[port, j] = a
                j--
            }
        }
    }

    m6 = "ffff:ffff:ffff:ffff::"
    out("# Generated by node-limits.sh. Change the values file, not this file.")
    out("table inet " table " {}")
    out("delete table inet " table)
    out("table inet " table " {")
    out("  ct timeout handshake_v4 { protocol tcp; l3proto ip; policy = { syn_sent: " timeout ", syn_recv: " timeout " }; }")
    out("  ct timeout handshake_v6 { protocol tcp; l3proto ip6; policy = { syn_sent: " timeout ", syn_recv: " timeout " }; }")
    out("  set logged_v4 { type ipv4_addr; size 65535; flags dynamic,timeout; timeout 2m; }")
    out("  set logged_v6 { type ipv6_addr; size 65535; flags dynamic,timeout; timeout 2m; }")
    for (p = 1; p <= nlimited; p++) {
        port = limited[p]
        sets(port, "", "default, " limit[port])
        if (loopback > 0)
            sets(port, "_lo", "loopback, " loopback)
        for (i = 1; i <= noverrides[port]; i++) {
            k = order[port, i]
            if (o_count[port, k] > 0)
                sets(port, "_o" (i - 1), "override " o_net[port, k] ", " o_count[port, k])
        }
        out("  counter p" port "_accepted {}")
        out("  counter p" port "_refused {}")
        out("  counter p" port "_untracked {}")
    }
    list = ""
    vmap = ""
    for (p = 1; p <= nlimited; p++) {
        list = list (p > 1 ? ", " : "") limited[p]
        vmap = vmap (p > 1 ? ", " : "") limited[p] " : jump p" limited[p]
    }
    out("  chain prerouting {")
    out("    type filter hook prerouting priority mangle; policy accept;")
    out("    fib daddr type != local return")
    out("    tcp flags & (fin|syn|rst|ack) != syn return")
    out("    ct state != new return")
    out("    tcp dport != { " list " } return")
    out("    meta nfproto ipv4 ct timeout set \"handshake_v4\"")
    out("    meta nfproto ipv6 ct timeout set \"handshake_v6\"")
    out("    tcp dport vmap { " vmap " }")
    out("  }")
    for (p = 1; p <= nlimited; p++) {
        port = limited[p]
        out("  chain p" port " {")
        for (i = 1; i <= noverrides[port]; i++) {
            k = order[port, i]
            fam = o_family[port, k] == "4" ? "ip" : "ip6"
            if (o_count[port, k] == 0)
                out("    " fam " saddr " o_net[port, k] " counter name p" port "_accepted accept")
            else
                out("    " fam " saddr " o_net[port, k] " goto p" port "_o" (i - 1))
        }
        if (loopback > 0) {
            out("    ip saddr 127.0.0.0/8 goto p" port "_lo")
            out("    ip6 saddr ::1 goto p" port "_lo")
        } else {
            out("    ip saddr 127.0.0.0/8 counter name p" port "_accepted accept")
            out("    ip6 saddr ::1 counter name p" port "_accepted accept")
        }
        count_rules(port, "", limit[port])
        out("  }")
        if (loopback > 0) {
            out("  chain p" port "_lo {  # loopback")
            count_rules(port, "_lo", loopback)
            out("  }")
        }
        for (i = 1; i <= noverrides[port]; i++) {
            k = order[port, i]
            if (o_count[port, k] == 0)
                continue
            out("  chain p" port "_o" (i - 1) " {  # override " o_net[port, k])
            count_rules(port, "_o" (i - 1), o_count[port, k])
            out("  }")
        }
    }
    out("  chain refuse {")
    out("    add @logged_v4 { ip saddr limit rate 1/minute burst 1 packets } limit rate 20/second log prefix \"iota-node-limits refused: \" level warn goto send_rst")
    out("    add @logged_v6 { ip6 saddr & " m6 " limit rate 1/minute burst 1 packets } limit rate 20/second log prefix \"iota-node-limits refused: \" level warn goto send_rst")
    out("    goto send_rst")
    out("  }")
    out("  chain send_rst {")
    out("    reject with tcp reset")
    out("  }")
    out("}")
}
function out(line) {
    print line
}
function sets(port, suffix, comment) {
    out("  set p" port suffix "_v4 { type ipv4_addr; size 65535; flags dynamic; }  # " comment)
    out("  set p" port suffix "_v6 { type ipv6_addr; size 65535; flags dynamic; }")
}
function count_rules(port, suffix, n,   s) {
    s = "@p" port suffix
    out("    add " s "_v4 { ip saddr ct count over " n " } counter name p" port "_refused goto refuse")
    out("    add " s "_v6 { ip6 saddr & " m6 " ct count over " n " } counter name p" port "_refused goto refuse")
    out("    meta nfproto ipv4 ip saddr != " s "_v4 counter name p" port "_untracked")
    out("    meta nfproto ipv6 ip6 saddr & " m6 " != " s "_v6 counter name p" port "_untracked")
    out("    counter name p" port "_accepted")
}
' "$1"
}

# Writes the rules with a checksum of them in the table comment.
generate_with_checksum() {
    rules=$(generate "$1") || return 1
    sum=$(printf '%s\n' "$rules" | cksum | awk '{ print $1 "-" $2 }')
    printf '%s\n' "$rules" | awk -v sum="$sum" -v table="$TABLE" '
        { print }
        $0 == "table inet " table " {" { print "  comment \"node-limits " sum "\"" }'
}

loaded_checksum() {
    nft list table inet "$TABLE" 2>/dev/null |
        sed -n 's/^[[:space:]]*comment "node-limits \(.*\)"$/\1/p' | head -n 1
}

load() {
    new=$(generate_with_checksum "$1") || return 1
    sum=$(printf '%s\n' "$new" | sed -n 's/^  comment "node-limits \(.*\)"$/\1/p')
    if [ "$(loaded_checksum)" = "$sum" ]; then
        return 0
    fi
    printf '%s\n' "$new" >"$RULES_FILE"
    nft -c -f "$RULES_FILE" || return 1
    nft -f "$RULES_FILE" || return 1
    log "loaded table inet $TABLE from $1 ($RULES_FILE)"
}

unload() {
    if nft list table inet "$TABLE" >/dev/null 2>&1; then
        nft delete table inet "$TABLE"
        log "removed table inet $TABLE"
    fi
}

run() {
    trap 'unload; exit 0' TERM INT
    trap 'reload=1' HUP
    if [ "${IOTA_NODE_LIMITS:-on}" = off ]; then
        unload
        log "IOTA_NODE_LIMITS=off: the table is not loaded"
        trap 'exit 0' TERM INT
        while :; do
            sleep "$INTERVAL" &
            wait $! || true
        done
    fi
    if ! load "$1"; then
        log "no limits are loaded: fix $1, then start again"
        exit 1
    fi
    while :; do
        reload=0
        sleep "$INTERVAL" &
        wait $! || true
        if [ "$reload" = 1 ]; then kill $! 2>/dev/null || true; fi
        load "$1" || log "$1 has an error; the loaded limits stay until the next start"
    done
}

usage() {
    sed -n '/^# Usage:/,/^# See /p' "$0" | sed 's/^# \{0,1\}//' >&2
    exit 2
}

case "${1:-}" in
generate)
    [ $# -eq 2 ] || usage
    generate_with_checksum "$2"
    ;;
load)
    [ $# -eq 2 ] || usage
    load "$2"
    ;;
unload)
    [ $# -eq 1 ] || usage
    unload
    ;;
run)
    [ $# -eq 2 ] || usage
    run "$2"
    ;;
*)
    usage
    ;;
esac
