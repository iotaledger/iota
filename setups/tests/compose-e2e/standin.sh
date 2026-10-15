#!/bin/sh
# Copyright (c) 2026 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

# A stand-in for the JSON-RPC port of the node, for checks that need a slow
# answer. socat runs it for each connection. It reads the request, waits the
# seconds of the request header "X-Delay", and answers with the values of all
# x-forwarded-for headers it got, joined by ", ". It writes the number of
# requests in progress to /tmp/in-progress.log. A request stops to count
# before its answer, so that the count is never above the number that HAProxy
# lets through.

mkdir -p /tmp/conns
mkdir "/tmp/conns/$$"
find /tmp/conns -mindepth 1 -maxdepth 1 | wc -l >>/tmp/in-progress.log
xff=""
length=0
delay=0
while IFS= read -r line; do
    line=$(printf '%s' "$line" | tr -d '\r')
    [ -z "$line" ] && break
    case "$(printf '%s' "$line" | tr '[:upper:]' '[:lower:]')" in
    x-forwarded-for:*) xff=${xff:+$xff, }${line#*: } ;;
    content-length:*) length=${line#*: } ;;
    x-delay:*) delay=${line#*: } ;;
    esac
done
head -c "$length" >/dev/null
sleep "$delay"
rmdir "/tmp/conns/$$"
body="{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":\"$xff\"}"
printf 'HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: %s\r\nConnection: close\r\n\r\n%s' "${#body}" "$body"
