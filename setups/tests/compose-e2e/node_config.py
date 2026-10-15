#!/usr/bin/env python3
# Copyright (c) 2026 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

"""Turns a node config written by `iota-localnet genesis` into a config for
the compose files: paths under /opt/iota, listeners on the ports of the compose
files, no seed peers. With --policy-from, it appends the `policy-config` block
of a shipped fullnode config.

Usage: node_config.py fullnode|validator <generated.yaml> [--policy-from <file>]
       [--set <key>=<value>]...
"""

import argparse
import re
import sys

PATHS = {
    'fullnode': {'db-path': '/opt/iota/db/full_node_db'},
    'validator': {'db-path': '/opt/iota/db/authorities_db'},
}


def top_level_block(lines, key):
    """The lines of the top-level `key` block, from its line to the next top-level key."""
    out = None
    for line in lines:
        if out is None:
            if line.startswith(key + ':'):
                out = [line]
        elif line and not line[0].isspace():
            break
        else:
            out.append(line)
    return out


def main():
    p = argparse.ArgumentParser()
    p.add_argument('kind', choices=['fullnode', 'validator'])
    p.add_argument('generated')
    p.add_argument('--policy-from')
    p.add_argument('--set', action='append', default=[], help='top-level key=value to add or replace')
    a = p.parse_args()

    out, section, skip_indent = [], None, None
    for line in open(a.generated).read().splitlines():
        stripped = line.lstrip()
        indent = len(line) - len(stripped)
        if skip_indent is not None:
            if stripped and (indent < skip_indent or (indent == skip_indent and not stripped.startswith('-'))):
                skip_indent = None
            else:
                continue
        if indent == 0 and stripped:
            section = stripped.split(':', 1)[0]
        key = stripped.split(':', 1)[0]
        if indent == 0 and key == 'db-path':
            line = f'db-path: {PATHS[a.kind]["db-path"]}'
        elif indent == 0 and key == 'network-address':
            line = 'network-address: /ip4/0.0.0.0/tcp/8080/http'
        elif indent == 0 and key == 'json-rpc-address':
            line = 'json-rpc-address: "0.0.0.0:9000"'
        elif indent == 0 and key == 'metrics-address':
            line = 'metrics-address: "0.0.0.0:9184"'
        elif indent == 0 and key == 'migration-tx-data-path':
            continue
        elif section == 'consensus-config' and key == 'db-path':
            line = ' ' * indent + 'db-path: /opt/iota/db/consensus_db'
        elif section == 'genesis' and key == 'genesis-file-location':
            line = ' ' * indent + 'genesis-file-location: /opt/iota/config/genesis.blob'
        elif section == 'p2p-config' and indent == 2 and key == 'listen-address':
            line = '  listen-address: "0.0.0.0:8084"'
        elif section == 'p2p-config' and indent == 2 and key == 'external-address':
            line = '  external-address: /ip4/127.0.0.1/udp/8084'
        elif section == 'p2p-config' and indent == 2 and key == 'seed-peers':
            skip_indent = indent + 2
            continue
        out.append(line)

    for item in a.set:
        key, value = item.split('=', 1)
        out = [ln for ln in out if not re.match(re.escape(key) + r':', ln)]
        out.append(f'{key}: {value}')

    if a.policy_from:
        block = top_level_block(open(a.policy_from).read().splitlines(), 'policy-config')
        if not block:
            sys.exit(f'{a.policy_from} has no policy-config block')
        if any(ln.startswith('policy-config:') for ln in out):
            sys.exit(f'{a.generated} has a policy-config block already')
        out.extend(block)

    print('\n'.join(out))


if __name__ == '__main__':
    main()
