#!/usr/bin/env bash
# Tune Linux network sysctls for high-rate localhost stress testing.
# Saves the pre-tune values to a backup so you can `restore` them later
# (no reboot needed). Settings would also revert on reboot anyway.
#
# Usage:
#   sudo ./tune-sysctl.sh           # apply tuned values (default)
#   sudo ./tune-sysctl.sh apply
#   sudo ./tune-sysctl.sh restore   # roll back to saved originals
#   sudo ./tune-sysctl.sh show      # print current vs tuned vs backup

set -euo pipefail

if [ "$EUID" -ne 0 ]; then
  echo "Run as root: sudo $0 [apply|restore|show]" >&2
  exit 1
fi

# /var/run is wiped on reboot, which is fine — settings revert too.
BACKUP=/var/run/tune-sysctl.original

# Pairs: "sysctl.key tuned_value"
KEYS=(
  "net.ipv4.ip_local_port_range|1024 65535"
  "net.ipv4.tcp_tw_reuse|1"
  "net.ipv4.tcp_fin_timeout|5"
  "net.ipv4.tcp_max_tw_buckets|1000000"
  "net.core.somaxconn|16384"
  "net.ipv4.tcp_max_syn_backlog|16384"
  "net.core.netdev_max_backlog|16384"
  "fs.file-max|2000000"
)

save_backup() {
  # Only save if the backup doesn't already exist — otherwise re-running
  # `apply` would overwrite originals with already-tuned values, and
  # `restore` would no longer roll back to true originals.
  if [ -f "$BACKUP" ]; then
    echo "  (backup already exists at $BACKUP — not overwriting)"
    return
  fi
  : > "$BACKUP"
  for pair in "${KEYS[@]}"; do
    local key="${pair%%|*}"
    local cur
    cur=$(sysctl -n "$key" 2>/dev/null || echo "")
    # Store as KEY|VALUE so multi-word values (port range) round-trip.
    echo "$key|$cur" >> "$BACKUP"
  done
  echo "  backup saved → $BACKUP"
}

apply_tuned() {
  echo "=> Saving originals before tuning..."
  save_backup
  echo
  echo "=> Applying tuned values:"
  for pair in "${KEYS[@]}"; do
    local key="${pair%%|*}"
    local val="${pair#*|}"
    local cur
    cur=$(sysctl -n "$key" 2>/dev/null || echo "?")
    sysctl -w "$key=$val" >/dev/null
    printf "  %-40s  %-20s →  %s\n" "$key" "$cur" "$val"
  done
  echo
  echo "Tip: also raise the per-process FD limit before launching stress:"
  echo "   ulimit -n 65536"
  echo "(stress-load-shedding.sh already does this.)"
}

restore_original() {
  if [ ! -f "$BACKUP" ]; then
    echo "No backup at $BACKUP — nothing to restore." >&2
    echo "Either you've rebooted (which restored defaults already) or you" >&2
    echo "never ran 'apply' since boot." >&2
    exit 1
  fi
  echo "=> Restoring original values from $BACKUP:"
  while IFS='|' read -r key val; do
    [ -z "$key" ] && continue
    local cur
    cur=$(sysctl -n "$key" 2>/dev/null || echo "?")
    sysctl -w "$key=$val" >/dev/null
    printf "  %-40s  %-20s →  %s\n" "$key" "$cur" "$val"
  done < "$BACKUP"
  rm -f "$BACKUP"
  echo
  echo "Backup file removed."
}

show_status() {
  echo "=> Current sysctl values:"
  for pair in "${KEYS[@]}"; do
    local key="${pair%%|*}"
    local tuned="${pair#*|}"
    local cur
    cur=$(sysctl -n "$key" 2>/dev/null || echo "?")
    printf "  %-40s  current=%-20s  tuned=%s\n" "$key" "$cur" "$tuned"
  done
  echo
  if [ -f "$BACKUP" ]; then
    echo "=> Backup at $BACKUP (originals from before 'apply'):"
    sed 's/|/  =  /' "$BACKUP" | column -t | sed 's/^/  /'
  else
    echo "=> No backup at $BACKUP — no 'apply' has run since boot."
  fi
}

case "${1:-apply}" in
  apply)    apply_tuned ;;
  restore)  restore_original ;;
  show)     show_status ;;
  *)        echo "Usage: $0 [apply|restore|show]" >&2; exit 1 ;;
esac
