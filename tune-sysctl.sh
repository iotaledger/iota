#!/usr/bin/env bash
# Maximize Linux network tunables for high-rate localhost stress testing.
# Settings are session-only (reverted on reboot). Run after every reboot:
#   sudo ./tune-sysctl.sh
#
# Reverting: just reboot, or run `sudo sysctl --system` to reload from /etc.

set -euo pipefail

if [ "$EUID" -ne 0 ]; then
  echo "Run as root: sudo $0" >&2
  exit 1
fi

apply() {
  local key=$1 value=$2
  current=$(sysctl -n "$key" 2>/dev/null || echo "?")
  sysctl -w "$key=$value" >/dev/null
  echo "  $key:  $current  →  $value"
}

echo "=> Ephemeral port range (max available)"
apply net.ipv4.ip_local_port_range "1024 65535"

echo "=> TIME_WAIT socket reuse"
apply net.ipv4.tcp_tw_reuse 1

echo "=> Short TIME_WAIT teardown (aggressive — 5s instead of default 60s)"
apply net.ipv4.tcp_fin_timeout 5

echo "=> TIME_WAIT bucket limit (max sockets in TIME_WAIT state)"
apply net.ipv4.tcp_max_tw_buckets 1000000

echo "=> Listen socket backlog"
apply net.core.somaxconn 16384

echo "=> SYN backlog (half-open connection queue)"
apply net.ipv4.tcp_max_syn_backlog 16384

echo "=> Network interface receive queue"
apply net.core.netdev_max_backlog 16384

echo "=> System-wide file descriptor max"
apply fs.file-max 2000000

echo
echo "=> Verifying:"
for k in net.ipv4.ip_local_port_range net.ipv4.tcp_tw_reuse net.ipv4.tcp_fin_timeout \
         net.ipv4.tcp_max_tw_buckets net.core.somaxconn net.ipv4.tcp_max_syn_backlog \
         net.core.netdev_max_backlog fs.file-max; do
  printf "  %-40s = %s\n" "$k" "$(sysctl -n "$k")"
done

echo
echo "Tip: also raise the per-process FD limit before launching stress:"
echo "   ulimit -n 65536"
echo "(stress-load-shedding.sh already does this.)"
