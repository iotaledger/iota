#!/usr/bin/env python3
# Copyright (c) 2026 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

"""Test clients for run.sh (Python standard library only).

Each command prints one line of JSON with its result.
"""

import argparse
import base64
import ipaddress
import json
import os
import select
import socket
import ssl
import struct
import sys
import time


# Set by --proxy-client: the client address that a PROXY protocol v2 header
# names at the start of each connection.
PROXY_CLIENT = None


def proxy_v2_header(client, server, port):
    """A PROXY protocol v2 header for a TCP connection from `client` to `server`."""
    src, dst = ipaddress.ip_address(client), ipaddress.ip_address(server)
    family = 0x11 if src.version == 4 else 0x21
    addresses = src.packed + dst.packed + struct.pack('>HH', 40000, port)
    return b'\r\n\r\n\x00\r\nQUIT\n' + bytes([0x21, family]) + struct.pack('>H', len(addresses)) + addresses


def connect(host, port, src=None, timeout=3.0, rcvbuf=None):
    family = socket.AF_INET6 if ':' in host else socket.AF_INET
    s = socket.socket(family, socket.SOCK_STREAM)
    if rcvbuf:
        s.setsockopt(socket.SOL_SOCKET, socket.SO_RCVBUF, rcvbuf)
    if src:
        s.bind((src, 0))
    s.settimeout(timeout)
    s.connect((host, port))
    if PROXY_CLIENT:
        s.sendall(proxy_v2_header(PROXY_CLIENT, host, port))
    return s


def error_name(e):
    if isinstance(e, ConnectionRefusedError):
        return 'refused'
    if isinstance(e, ConnectionResetError):
        return 'reset'
    if isinstance(e, socket.timeout):
        return 'timeout'
    return type(e).__name__


def emit(**kw):
    print(json.dumps(kw), flush=True)


def rpc_body(method='iota_getChainIdentifier', params=None):
    return json.dumps({'jsonrpc': '2.0', 'id': 1, 'method': method, 'params': params or []}).encode()


def http_request(host, body, xff=None):
    lines = ['POST / HTTP/1.1', f'Host: {host}', 'Content-Type: application/json', f'Content-Length: {len(body)}']
    if xff:
        lines.append(f'X-Forwarded-For: {xff}')
    return ('\r\n'.join(lines) + '\r\n\r\n').encode() + body


def read_response(s, timeout, with_headers=False):
    """Reads one HTTP/1.1 response with a Content-Length. Returns (status, body),
    or (status, headers, body)."""
    deadline = time.monotonic() + timeout
    buf = b''
    while b'\r\n\r\n' not in buf:
        s.settimeout(max(0.01, deadline - time.monotonic()))
        data = s.recv(65536)
        if not data:
            raise ConnectionError('closed before the response')
        buf += data
    head, body = buf.split(b'\r\n\r\n', 1)
    lines = head.decode('latin1').split('\r\n')
    headers = {k.strip().lower(): v.strip() for k, _, v in (ln.partition(':') for ln in lines[1:])}
    length = int(headers.get('content-length', '0'))
    while len(body) < length:
        s.settimeout(max(0.01, deadline - time.monotonic()))
        data = s.recv(65536)
        if not data:
            break
        body += data
    if with_headers:
        return int(lines[0].split()[1]), headers, body
    return int(lines[0].split()[1]), body


def cmd_conns(a):
    """Opens --n connections one after the other, then holds them for --hold s.

    Per connection: 'open' (or 'tls' with --tls), 'refused' (reset in answer to
    the SYN), 'reset' (reset after the TCP handshake, before any byte), 'eof' or
    another error."""
    held, outcomes = [], []
    for _ in range(a.n):
        try:
            s = connect(a.host, a.port, a.src)
        except Exception as e:  # noqa: BLE001 (every failure is a result)
            outcomes.append(error_name(e))
            continue
        outcome = 'open'
        try:
            if a.tls:
                ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
                ctx.check_hostname = False
                ctx.verify_mode = ssl.CERT_NONE
                ctx.set_alpn_protocols(['h2'])
                s = ctx.wrap_socket(s, server_hostname='iota')
                outcome = 'tls'
            elif a.probe:
                s.settimeout(a.probe)
                try:
                    outcome = 'eof' if s.recv(1) == b'' else 'data'
                except socket.timeout:
                    outcome = 'open'
        except Exception as e:  # noqa: BLE001
            outcome = error_name(e)
        outcomes.append(outcome)
        held.append(s)
    counts = {}
    for o in outcomes:
        counts[o] = counts.get(o, 0) + 1
    emit(kind='conns', src=a.src, counts=counts, outcomes=outcomes)
    if a.hold:
        time.sleep(a.hold)


def cmd_rpc(a):
    """Sends --n JSON-RPC requests on one HTTP/1.1 keep-alive connection.
    Reports the statuses, the result count, the slowest request and the last
    body."""
    s = connect(a.host, a.port, a.src, timeout=a.timeout)
    statuses, last, results, slowest = {}, None, 0, 0.0
    for _ in range(a.n):
        start = time.monotonic()
        s.sendall(http_request(a.host, rpc_body(a.method, json.loads(a.params)), a.xff))
        status, body = read_response(s, a.timeout)
        slowest = max(slowest, time.monotonic() - start)
        statuses[str(status)] = statuses.get(str(status), 0) + 1
        last = body.decode(errors='replace')
        results += '"result"' in last
    try:
        last = json.loads(last) if last else None
    except ValueError:
        last = {'not_json': last[:80]}
    emit(kind='rpc', src=a.src, statuses=statuses, results=results, slowest_ms=round(slowest * 1000), last=last)


# JSON-RPC ids for cmd_ids: each reply must have the same id.
IDS = [7, -12, 0, 'ab"c', 'x-1', '007', '00', 'a' * 15000]


def cmd_ids(a):
    """Sends one request per id in IDS, and one with a 20000-character id, and
    checks that each reply parses as JSON and has the sent id. The reply to the
    long id must only parse."""
    s = connect(a.host, a.port, a.src, timeout=10)
    bad = []
    for rpc_id in IDS + ['a' * 20000]:
        body = json.dumps({'jsonrpc': '2.0', 'id': rpc_id, 'method': 'x', 'params': []}).encode()
        s.sendall(http_request(a.host, body))
        _, reply = read_response(s, 10)
        try:
            got = json.loads(reply)['id']
        except (ValueError, KeyError, TypeError):
            bad.append({'sent': str(rpc_id)[:20], 'reply': reply[:60].decode(errors='replace')})
            continue
        if rpc_id in IDS and (got != rpc_id or type(got) is not type(rpc_id)):
            bad.append({'sent': str(rpc_id)[:20], 'got': str(got)[:20]})
    emit(kind='ids', ids=len(IDS) + 1, bad=bad)


def cmd_silent(a):
    """Connects, sends nothing and reports when the server answers or closes."""
    t0 = time.monotonic()
    s = connect(a.host, a.port, a.src)
    s.settimeout(a.max_secs)
    data = b''
    how = 'open'
    try:
        while True:
            d = s.recv(65536)
            if not d:
                how = 'eof'
                break
            data += d
    except socket.timeout:
        how = 'still_open'
    except Exception as e:  # noqa: BLE001
        how = error_name(e)
    emit(kind='silent', secs=round(time.monotonic() - t0, 3), how=how, first_line=data.split(b'\r\n')[0].decode('latin1'))


def cmd_late_request(a):
    """Connects, waits --wait s, then sends one request and reads the answer."""
    s = connect(a.host, a.port, a.src, timeout=a.wait + 10)
    time.sleep(a.wait)
    try:
        s.sendall(http_request(a.host, rpc_body()))
        status, _ = read_response(s, 10)
        emit(kind='late_request', wait=a.wait, status=status)
    except Exception as e:  # noqa: BLE001
        emit(kind='late_request', wait=a.wait, error=error_name(e))


def cmd_stalled_reader(a):
    """Sends a request with a large response, never reads, and reports when the
    server closes the connection."""
    params = [['0x2'] * 10, {'showBcs': True, 'showContent': True}]
    s = connect(a.host, a.port, a.src, timeout=5, rcvbuf=4096)
    s.sendall(http_request(a.host, rpc_body('iota_multiGetObjects', params)))
    t0 = time.monotonic()
    time.sleep(a.wait)
    # The server has closed the connection if a read gets EOF or a reset after
    # the bytes that are in the receive buffer.
    s.settimeout(2)
    how, received = 'open', 0
    try:
        while True:
            d = s.recv(1 << 20)
            if not d:
                how = 'closed'
                break
            received += len(d)
    except socket.timeout:
        how = 'open'
    except Exception as e:  # noqa: BLE001
        how = error_name(e)
    emit(kind='stalled_reader', waited=round(time.monotonic() - t0, 1), how=how, received=received)


def cmd_udp(a):
    """Sends one UDP datagram and reports 'refused' (ICMP port unreachable),
    'reply' or 'no_reply' after --wait s."""
    family = socket.AF_INET6 if ':' in a.host else socket.AF_INET
    s = socket.socket(family, socket.SOCK_DGRAM)
    s.bind((a.src, 0))
    s.connect((a.host, a.port))
    s.settimeout(a.wait)
    s.send(b'probe')
    try:
        s.recv(100)
        how = 'reply'
    except socket.timeout:
        how = 'no_reply'
    except ConnectionRefusedError:
        how = 'refused'
    emit(kind='udp', src=a.src, port=a.port, how=how)


def cmd_ws(a):
    """Opens --n WebSocket connections, subscribes on each, and holds them for
    --hold s. Reports the HTTP status, the Connection header and the body of
    each upgrade reply."""
    held, outcomes = [], []
    sub = json.dumps({'jsonrpc': '2.0', 'id': 1, 'method': 'iotax_subscribeEvent', 'params': [{'All': []}]}).encode()
    for _ in range(a.n):
        try:
            s = connect(a.host, a.port, a.src, timeout=10)
            key = base64.b64encode(os.urandom(16)).decode()
            s.sendall((f'GET / HTTP/1.1\r\nHost: {a.host}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n'
                       f'Sec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n').encode())
            status, headers, body = read_response(s, 10, with_headers=True)
        except Exception as e:  # noqa: BLE001
            outcomes.append({'status': error_name(e)})
            continue
        if status == 101:
            mask = os.urandom(4)
            payload = bytes(b ^ mask[i % 4] for i, b in enumerate(sub))
            s.sendall(bytes([0x81, 0x80 | len(payload)]) + mask + payload)
            held.append(s)
        outcomes.append({'status': status, 'connection': headers.get('connection'),
                         'body': body.decode(errors='replace')})
    emit(kind='ws', src=a.src, outcomes=outcomes)
    if a.hold:
        time.sleep(a.hold)


# HTTP/2 without TLS (h2c).

def h2_frame(kind, flags, stream, payload=b''):
    return struct.pack('>I', len(payload))[1:] + bytes([kind, flags]) + struct.pack('>I', stream) + payload


def hpack_literal(name, value):
    name, value = name.encode(), value.encode()
    return b'\x00' + bytes([len(name)]) + name + bytes([len(value)]) + value


def h2_status(block):
    if block and block[0] & 0x80:
        return {8: 200, 9: 204, 13: 404, 14: 500}.get(block[0] & 0x7f)
    if len(block) > 2 and block[0] in (0x08, 0x48) and not block[1] & 0x80:
        return int(block[2:2 + block[1]])
    return None


def classify(status, body):
    """An HTTP status, followed by the JSON-RPC error code if the body has one."""
    try:
        code = json.loads(body).get('error', {}).get('code')
    except (ValueError, AttributeError):
        code = None
    return f'{status}:{code}' if code is not None else str(status)


class H2:
    def __init__(self, a):
        self.s = connect(a.host, a.port, a.src, timeout=5)
        self.s.sendall(b'PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n' + h2_frame(4, 0, 0))
        self.host = a.host
        self.buf = b''
        self.max_streams = None
        self.next_id = 1
        self.closed = None

    def headers(self, length):
        return (hpack_literal(':method', 'POST') + hpack_literal(':scheme', 'http') + hpack_literal(':path', '/')
                + hpack_literal(':authority', self.host) + hpack_literal('content-type', 'application/json')
                + hpack_literal('content-length', str(length)))

    def open_stream(self, body, send_body=True):
        sid = self.next_id
        self.next_id += 2
        if send_body:
            self.s.sendall(h2_frame(1, 0x4, sid, self.headers(len(body))) + h2_frame(0, 0x1, sid, body))
        else:
            self.s.sendall(h2_frame(1, 0x4, sid, self.headers(len(body))))
        return sid

    def frames(self, timeout):
        """Yields (type, flags, stream, payload) until timeout without data."""
        r, _, _ = select.select([self.s], [], [], timeout)
        if not r:
            return
        try:
            d = self.s.recv(1 << 20)
        except Exception as e:  # noqa: BLE001
            self.closed = error_name(e)
            return
        if not d:
            self.closed = 'eof'
            return
        self.buf += d
        while len(self.buf) >= 9:
            n = int.from_bytes(self.buf[:3], 'big')
            if len(self.buf) < 9 + n:
                break
            kind, flags = self.buf[3], self.buf[4]
            sid = struct.unpack('>I', self.buf[5:9])[0] & 0x7fffffff
            payload = self.buf[9:9 + n]
            self.buf = self.buf[9 + n:]
            if kind == 4 and not flags & 1:
                self.s.sendall(h2_frame(4, 1, 0))
                for i in range(0, len(payload), 6):
                    key, value = struct.unpack('>HI', payload[i:i + 6])
                    if key == 3:
                        self.max_streams = value
            elif kind == 0 and n:
                self.s.sendall(h2_frame(8, 0, 0, struct.pack('>I', n)) + h2_frame(8, 0, sid, struct.pack('>I', n)))
            yield kind, flags, sid, payload


def cmd_h2c(a):
    """Sends --requests JSON-RPC requests on one h2c connection.

    With --obey-settings the client keeps at most SETTINGS_MAX_CONCURRENT_STREAMS
    streams open, as HTTP/2 clients do. Without it, it opens all streams at once.
    Reports the status of each stream: an HTTP status or 'rst:<code>'."""
    h2 = H2(a)
    body = rpc_body()
    results, open_streams, sent = {}, set(), 0
    deadline = time.monotonic() + a.max_secs
    if a.obey_settings:
        while h2.max_streams is None and time.monotonic() < deadline and not h2.closed:
            for _ in h2.frames(0.5):
                pass
    limit = h2.max_streams if a.obey_settings else a.requests
    status, data = {}, {}
    while len(results) < a.requests and time.monotonic() < deadline and not h2.closed:
        while sent < a.requests and len(open_streams) < limit:
            open_streams.add(h2.open_stream(body))
            sent += 1
        for kind, flags, sid, payload in h2.frames(0.5):
            if kind == 1:
                status.setdefault(sid, h2_status(payload))
            elif kind == 0:
                data[sid] = data.get(sid, b'') + payload
            elif kind == 3:
                results.setdefault(sid, 'rst:%d' % struct.unpack('>I', payload)[0])
                open_streams.discard(sid)
            if kind in (0, 1) and flags & 1:
                results.setdefault(sid, classify(status.get(sid), data.get(sid, b'')))
                open_streams.discard(sid)
    counts = {}
    for st in results.values():
        counts[str(st)] = counts.get(str(st), 0) + 1
    emit(kind='h2c', src=a.src, requests=a.requests, answered=len(results), statuses=counts,
         server_max_streams=h2.max_streams, closed=h2.closed)


def cmd_h2partial(a):
    """Opens --n h2c connections. Each sends the preface, SETTINGS and the first
    12 bytes of a PING frame, then closes with unread data (a reset)."""
    for _ in range(a.n):
        s = connect(a.host, a.port, a.src)
        s.sendall(b'PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n' + h2_frame(4, 0, 0) + h2_frame(6, 0, 0, b'ABCDEFGH')[:12])
        time.sleep(0.1)
        s.close()
    emit(kind='h2partial', src=a.src, n=a.n)


def cmd_h2stall(a):
    """Opens --conns h2c connections. Each opens --streams requests that announce
    a body and never send it. Reports, per connection, the answers within
    --secs s."""
    conns = [H2(a) for _ in range(a.conns)]
    out = []
    for h2 in conns:
        for _ in range(a.streams):
            h2.open_stream(b'x' * 100, send_body=False)
        answers, status, data = {}, {}, {}
        end = time.monotonic() + a.secs
        while time.monotonic() < end and not h2.closed:
            for kind, flags, sid, payload in h2.frames(0.2):
                if kind == 1:
                    status.setdefault(sid, h2_status(payload))
                elif kind == 0:
                    data[sid] = data.get(sid, b'') + payload
                elif kind == 3:
                    answers.setdefault(sid, 'rst:%d' % struct.unpack('>I', payload)[0])
                if kind in (0, 1) and flags & 1:
                    answers.setdefault(sid, classify(status.get(sid), data.get(sid, b'')))
        counts = {}
        for st in answers.values():
            counts[str(st)] = counts.get(str(st), 0) + 1
        out.append({'answers': counts, 'closed': h2.closed})
    emit(kind='h2stall', src=a.src, per_connection=out)


def main():
    p = argparse.ArgumentParser()
    sub = p.add_subparsers(dest='cmd', required=True)

    def common(sp):
        sp.add_argument('--host', required=True)
        sp.add_argument('--port', type=int, required=True)
        sp.add_argument('--src')

    sp = sub.add_parser('conns')
    common(sp)
    sp.add_argument('--n', type=int, default=1)
    sp.add_argument('--tls', action='store_true')
    sp.add_argument('--probe', type=float, default=0)
    sp.add_argument('--hold', type=float, default=0)
    sp.set_defaults(fn=cmd_conns)

    sp = sub.add_parser('rpc')
    common(sp)
    sp.add_argument('--n', type=int, default=1)
    sp.add_argument('--xff')
    sp.add_argument('--method', default='iota_getChainIdentifier')
    sp.add_argument('--params', default='[]')
    sp.add_argument('--timeout', type=float, default=30)
    sp.set_defaults(fn=cmd_rpc)

    sp = sub.add_parser('ids')
    common(sp)
    sp.set_defaults(fn=cmd_ids)

    sp = sub.add_parser('silent')
    common(sp)
    sp.add_argument('--max-secs', type=float, default=20)
    sp.set_defaults(fn=cmd_silent)

    sp = sub.add_parser('late-request')
    common(sp)
    sp.add_argument('--wait', type=float, required=True)
    sp.set_defaults(fn=cmd_late_request)

    sp = sub.add_parser('stalled-reader')
    common(sp)
    sp.add_argument('--wait', type=float, required=True)
    sp.set_defaults(fn=cmd_stalled_reader)

    sp = sub.add_parser('udp')
    common(sp)
    sp.add_argument('--wait', type=float, default=2)
    sp.set_defaults(fn=cmd_udp)

    sp = sub.add_parser('ws')
    common(sp)
    sp.add_argument('--n', type=int, default=1)
    sp.add_argument('--hold', type=float, default=0)
    sp.set_defaults(fn=cmd_ws)

    sp = sub.add_parser('h2c')
    common(sp)
    sp.add_argument('--requests', type=int, default=100)
    sp.add_argument('--obey-settings', action='store_true')
    sp.add_argument('--max-secs', type=float, default=30)
    sp.set_defaults(fn=cmd_h2c)

    sp = sub.add_parser('h2partial')
    common(sp)
    sp.add_argument('--n', type=int, default=1)
    sp.set_defaults(fn=cmd_h2partial)

    sp = sub.add_parser('h2stall')
    common(sp)
    sp.add_argument('--conns', type=int, default=2)
    sp.add_argument('--streams', type=int, default=64)
    sp.add_argument('--secs', type=float, default=3)
    sp.set_defaults(fn=cmd_h2stall)

    p.add_argument('--proxy-client', help='send a PROXY protocol v2 header that names this client address')
    a = p.parse_args()
    global PROXY_CLIENT
    PROXY_CLIENT = a.proxy_client
    a.fn(a)


if __name__ == '__main__':
    sys.exit(main())
