---
name: Report an Infrastructure Bug
about: Report bugs related to Indexer, JSON-RPC, GraphQL, or other infrastructure components
title: ""
labels: ["infrastructure", "c-bug"]
---

## Bug description

Briefly describe the bug.

## Context

**Components** (select all that apply):

- [ ] `iota-indexer`
- [ ] `iota-json-rpc`
- [ ] `iota-graphql-rpc`
- [ ] Other: __________

**Issue type** (select one):

- [ ] JSON-RPC API bug (failing request, unexpected response, etc.)
- [ ] Crate bug (logic bug, runtime error, performance issue, etc.)

## Version & Environment

Specify exactly which version, commit, or branch you're using, and where you observed the issue:

- **Version (tag), commit hash, or branch**:
- **Environment** (Mainnet, Testnet, Devnet, Local, other, etc.):

## Steps to reproduce the bug

Provide clear, copy-&-paste instructions to reproduce the issue.

- For JSON-RPC or GraphQL API bugs, include your request and full response.
- If relevant, add commands or scripts to set up test data locally (e.g., PTB commands).

Example JSON-RPC request:

```sh
curl 'http://localhost:9124/' \
-H 'Content-Type: application/json' \
--data-raw '{"jsonrpc":"2.0","id":3,"method":"iota_getTransactionBlock","params":["HZ6wGXArNs6gagzaVZruprPX7QaaxfPo5ZFS76wUpK4f",{"showInput":true,"showEffects":true,"showEvents":true,"showBalanceChanges":true,"showObjectChanges":true}]}' | json_pp
```

## Expected behaviour

Describe in detail what you expect to happen.

## Actual behaviour

Describe clearly what actually happens, including error messages or unexpected behavior.

## Errors & logs

Paste any errors, logs, stack traces, or screenshots here.

## System details (optional, recommended)

Provide additional system details if relevant:

- **OS & Kernel**:
- **RAM**:
- **CPU cores**:
- **Container/VM**: yes ☐ / no ☐, details:
- **Network**: VPN? Proxy? Firewall?
