# End-to-end test of the compose setups

`run.sh` tests the files in `setups/fullnode/docker/` and `setups/validator/` as an operator uses them:

1. It creates a network of one validator with `iota-localnet genesis` from `NODE_IMAGE`.
2. It starts a fullnode with the previous compose file (`fullnode-before.yaml`) and a `policy-config` block of its
   own, and upgrades it as the docs say. Then it runs the validator compose file (validator and the nftables helper).
   The overrides in this directory set the node image and the log level, and the test uses small limits
   (`limits_9000.e2e.map`, `node-limits.e2e.conf`).
3. Remote clients use the addresses `192.0.2.10-21`, `198.51.100.10-12`, `2001:db8::10`, `2001:db8::11` and
   `2001:db8:0:1::10` in a network namespace. A veth pair joins it to the host (`192.0.2.1`, `198.51.100.1`,
   `2001:db8::1`), so the clients reach the published ports as remote clients do.

Each check prints one line `CHECK <id> PASS|FAIL|SKIP <reason>`. Most checks have a control: the same measurement
under a changed condition, which must show the other result. A failed step gives a FAIL line for its check, and the
run goes on with the next check. The last line counts the results. The script exits with 1 if a check fails. When it
stops, it removes its containers, networks, network namespace, nftables table and the work directory that it made.

## Requirements

Linux, Docker Engine 28 or later with Compose 2.33.1 or later, `sudo` (or root), `iproute2`, `nftables`, `python3`,
`curl` and `jq`. The script changes the firewall of the host: Docker adds rules for the published ports, and the
nftables helper loads the table `inet iota_node_limits`. Run it on a test machine or a CI runner, or in a privileged
`docker:dind` container.

## Run

```sh
bash setups/tests/compose-e2e/run.sh
CHECKS="F8 F10" bash setups/tests/compose-e2e/run.sh
```

Environment variables:

| Variable                    | Default                        | Meaning                                                                                         |
| --------------------------- | ------------------------------ | ----------------------------------------------------------------------------------------------- |
| `NODE_IMAGE`                | `iotaledger/iota-tools:devnet` | Image with `iota-node` and `iota-localnet`                                                      |
| `NODE_GRPC_MAX_CONNECTIONS` | `0`                            | `1`: the node has `grpc-max-connections`; runs check V8                                         |
| `CHECKS`                    | `all`                          | `fullnode`, `validator`, or a list of check ids such as `F8 V2`                                 |
| `E2E_PREFIX`                | `compose-e2e`                  | Prefix of the compose projects and of the network namespace                                     |
| `E2E_NETNS`, `E2E_VETH`     | `<prefix>-client`, `e2e-veth`  | Names of the network namespace and of the host side of the veth pair                            |
| `E2E_KERNEL_LOG`            | `1`                            | `0` skips check V5, for example in a container, where the kernel does not log nftables messages |
| `E2E_WORKDIR`               | `$TMPDIR` or `/tmp`            | The script makes a new directory in it, and removes only that directory                         |
| `E2E_KEEP_WORKDIR`          | `0`                            | `1` keeps the new work directory                                                                |
| `E2E_LOGDIR`                | none                           | The script copies the container logs to this directory before it removes the work directory     |

## Checks

- U1: The upgrade of a fullnode that has its own `policy-config` block, as the docs say, gives a node that answers
  through HAProxy. Control: with a second `policy-config` block the node stops.
- F1: JSON-RPC through HAProxy over IPv4 and IPv6, HTTP/1.1 and h2c. Two addresses in one IPv6 /64 share the limit of
  4 connections; another /64 does not. Control: a map change on the admin socket.
- F2: The fullnode publishes no 9000; on 9000 HAProxy answers. Control: an override that publishes the node port.
- F3: The node charges errors to the client address, not to an `x-forwarded-for` value that the client sends.
  Control: a peer on the compose network sets the header itself.
- F3-header: The node gets exactly one `x-forwarded-for` value, the client address. Control: from a network in
  `trusted_proxies.lst`, the client's value stays.
- F4: The fifth connection of an address is reset; at the same time another address gets its own limit; 5 refusals
  give 1 log line. Control: map value 0.
- F5: A connection that sends nothing is closed after 10 s. Control: a first request after 8 s is served.
- F6: A client that does not read a large response is disconnected after `timeout client`. Control: it is still
  connected at 30 s.
- F7: 9184 and 8404 answer on 127.0.0.1 only; other paths on 8404 get 404. Control: 9000 answers remote clients.
- F8: A node that does not answer in time: HTTP 200 with JSON-RPC error -32050 and the request id. Control: without
  the `http-error` lines the reply is HTML with 504.
- F8-down: a stopped node is DOWN in the metrics of HAProxy within 10 s, and a POST then gets an answer within 1 s.
  Control: the node was UP before the stop.
- F8-stopped, F8-ids, F8-get, F8-cors: with the node stopped, a POST gets HTTP 200 with JSON-RPC error -32009; each
  reply has the sent id and parses as JSON; `GET /health` gets 503; a request with an `Origin` header gets
  `access-control-allow-origin: *`.
- F9: 100 requests on one h2c connection: a client that keeps to the advertised 64 streams gets 100 answers; two
  connections with 64 waiting bodies each: the second gets HTTP 200 with JSON-RPC error -32009; with map value 0 no
  request is refused. Controls: 100 streams at once; one connection with 64 waiting bodies.
- F10: With `maxconn 4` on the server line, 12 slow requests from 3 addresses reach a stand-in node at most 4 at a
  time, and all get an answer; a request that waits longer than `timeout queue` gets JSON-RPC error -32009. Control:
  without `maxconn`, more than 4 reach it at once.
- F11: A load balancer in `proxy_protocol.lst` sends the PROXY protocol header: the limit per client address and the
  address for the node use the client address in the header. Control: the same header from an address that is not
  in the list gets HTTP 400.
- F12: With a WebSocket limit of 2 in use, JSON-RPC requests are answered at once, and one more WebSocket gets HTTP
  503 with JSON-RPC error -32009. Control: with the limit at 200, it opens.
- F12-count: An open WebSocket does not count as a request in progress. Control: a request over the limit is refused.
- F13: UDP to the P2P port 8084 reaches the fullnode over IPv4 and IPv6. Control: a closed port gets an ICMP error.
- F14: Without the `haproxy/` directory, compose stops and creates no empty directory.
- F15: A connection that ends in the middle of an HTTP/2 frame is released. Control: the admin socket shows an open
  connection.
- F17: 3 bodies over 1 MiB get HTTP 200, but `haproxy_frontend_requests_denied_total` counts them, and the log has
  3 lines with `error=413`. F17-sample: of 1000 requests, 1 to 30 are in the log, each with `conn=` and `req=` of the
  client address.
- F16: A WebSocket upgrade on an HTTP/1 connection past its age keeps `Connection: Upgrade`. Control: a request gets
  `Connection: close`.
- V1: The helper loads its table, and loads it again after a restart; loading an unchanged values file keeps the
  counts. Control: a changed values file.
- V2: Limit 4 per IPv4 address and per IPv6 /64 on 8080, refused at the first packet. Control: `IOTA_NODE_LIMITS=off`
  in `.env` stays after the next `up -d`.
- V3: Overrides: the longest prefix wins; 0 means no limit.
- V4: Loopback clients have their own limit.
- V5: 5 refusals from one address give 1 kernel log line.
- V6: Stopping the helper removes the table; SIGKILL leaves it; a deleted table is loaded again within 35 s.
- V7: IPv6 clients reach 8080. Control: IPv6 on the compose network makes Docker send them to an address where the
  node does not listen.
- V8: `grpc-max-connections`: the connection over the limit is reset and counted. Control: 0 means no limit.
- V9: 9184 answers on 127.0.0.1 only. Control: 8080 answers remote clients.
- V10: An address at its limit on 8080 still connects to 8081 and reaches 8084/udp. Control: the same address is
  refused on 8080.
