#!/usr/bin/env python3
# Copyright (c) 2026 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0
"""
Mock backend for the DAG visualizer frontend.

Generates fake consensus DAG data on the fly so the frontend can be tested
without a real validator network.

Usage:
    python mock_server.py [OPTIONS]

Options:
    --validators N       Number of validators (default: 10)
    --miss-rate F        Probability [0..1] that a block is missing (default: 0.05)
    --skip-rate F        Probability [0..1] that a leader is skipped (default: 0.15)
    --round-interval-ms  Milliseconds between rounds (default: 55)
    --port PORT          Port to listen on (default: 9186)
    --epoch EPOCH        Current epoch number (default: 1)
    --equivocation-rate  Probability [0..1] of equivocation per round (default: 0.02)
    --stale-rate F       Probability [0..1] per ancestor of being stale (default: 0.08)
    --slow-round-rate F  Probability [0..1] of a round being slow (2-5x) (default: 0.08)
"""

from __future__ import annotations

import argparse
import asyncio
import hashlib
import math
import random
import struct
import time
from typing import Any

from aiohttp import web, WSMsgType

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def short_digest(data: str) -> str:
    """Return a 6-char hex digest, matching the real server truncation."""
    return hashlib.sha256(data.encode()).hexdigest()[:6]


def make_digest(round_: int, author: int, extra: str = "") -> str:
    return short_digest(f"{round_}:{author}:{extra}")


# ---------------------------------------------------------------------------
# Binary encoding helpers
# ---------------------------------------------------------------------------

def encode_str(s: str) -> bytes:
    """1-byte length prefix + UTF-8 bytes."""
    encoded = s.encode("utf-8")
    return struct.pack("<B", len(encoded)) + encoded


def encode_block_ref(ref: dict) -> bytes:
    """Encode a BlockRef: u32 round + u16 author + len-prefixed digest."""
    return struct.pack("<IH", ref["round"], ref["author"]) + encode_str(ref["digest"])


def encode_block(block: dict) -> bytes:
    """Encode a DagBlockJson (without type byte)."""
    buf = struct.pack("<IH", block["round"], block["author"])
    buf += encode_str(block["digest"])
    buf += struct.pack("<d", float(block["timestamp_ms"]))
    ancestors = block.get("ancestors", [])
    buf += struct.pack("<H", len(ancestors))
    for a in ancestors:
        buf += encode_block_ref(a)
    acks = block.get("acknowledgments", [])
    buf += struct.pack("<H", len(acks))
    for a in acks:
        buf += encode_block_ref(a)
    return buf


def encode_leader(leader: dict) -> bytes:
    """Encode a LeaderInfoJson (without type byte)."""
    buf = struct.pack("<II", leader["wave"], leader["leader_round"])
    buf += struct.pack("<H", leader["leader_authority"])
    buf += struct.pack("<B", leader["status"])
    digest = leader.get("block_digest")
    if digest is not None:
        buf += struct.pack("<B", 1)
        buf += encode_str(digest)
    else:
        buf += struct.pack("<B", 0)
    return buf


def encode_event(e: dict) -> bytes:
    """Encode a WebSocket event to binary."""
    t = e["t"]
    if t == 0:  # BlockAccepted
        return struct.pack("<B", 0) + encode_block(e)
    elif t == 1:  # LeaderDecided
        return struct.pack("<B", 1) + encode_leader(e)
    elif t == 2:  # RoundAdvanced
        return struct.pack("<BI", 2, e["round"])
    elif t == 3:  # Lagged
        return struct.pack("<Bd", 3, float(e["missed"]))
    else:
        raise ValueError(f"Unknown event type: {t}")


def encode_committee(data: dict) -> bytes:
    """Encode committee response to binary."""
    buf = struct.pack("<ddd", float(data["epoch"]), float(data["total_stake"]),
                      float(data["quorum_threshold"]))
    validators = data["validators"]
    buf += struct.pack("<H", len(validators))
    for v in validators:
        buf += struct.pack("<B", v["index"])
        buf += struct.pack("<d", float(v["stake"]))
        buf += encode_str(v["hostname"])
    return buf


def encode_status(data: dict) -> bytes:
    """Encode status response to binary (16 bytes fixed)."""
    return struct.pack("<IIII",
                       data["highest_accepted_round"],
                       data["last_commit_index"],
                       data["last_commit_round"],
                       data["num_authorities"])


def encode_epochs(epochs: list[dict]) -> bytes:
    """Encode epochs response to binary."""
    buf = struct.pack("<H", len(epochs))
    for e in epochs:
        buf += struct.pack("<dII", float(e["epoch"]), e["from_round"], e["to_round"])
    return buf


def encode_dag_window(data: dict) -> bytes:
    """Encode DAG window response to binary."""
    buf = struct.pack("<IIII",
                      data["from_round"],
                      data["to_round"],
                      data["highest_accepted_round"],
                      data["last_commit_round"])
    blocks = data["blocks"]
    buf += struct.pack("<I", len(blocks))
    for b in blocks:
        buf += encode_block(b)
    leaders = data["leaders"]
    buf += struct.pack("<I", len(leaders))
    for l in leaders:
        buf += encode_leader(l)
    return buf


# ---------------------------------------------------------------------------
# DAG generator
# ---------------------------------------------------------------------------

class DagGenerator:
    """Produces a stream of deterministic-ish DAG rounds."""

    def __init__(
        self,
        num_validators: int,
        miss_rate: float,
        skip_rate: float,
        round_interval_ms: int,
        epoch: int,
        equivocation_rate: float,
        stale_rate: float,
        max_stored_rounds: int = 100,
    ) -> None:
        self.num_validators = num_validators
        self.miss_rate = miss_rate
        self.skip_rate = skip_rate
        self.round_interval_ms = round_interval_ms
        self.epoch = epoch
        self.equivocation_rate = equivocation_rate
        self.stale_rate = stale_rate
        self.max_stored_rounds = max_stored_rounds

        # Per-validator stake: randomly distributed, sums to 10_000
        raw = [random.randint(50, 200) for _ in range(num_validators)]
        total = sum(raw)
        self.stakes = [int(s / total * 10_000) for s in raw]
        # Fix rounding so it sums exactly
        self.stakes[-1] = 10_000 - sum(self.stakes[:-1])
        self.total_stake = 10_000
        self.quorum_threshold = math.ceil(self.total_stake * 2 / 3)

        # State
        self.current_round = 0
        self.last_commit_round = 0
        self.last_commit_index = 0
        # Storage: round -> list of blocks  (None = missing)
        self.blocks: dict[int, list[dict | None]] = {}
        # Storage: leader_round -> leader info
        self.leaders: dict[int, dict] = {}

        # Generate a seed window so the frontend has data on first fetch
        self._generate_up_to(60)

    # -- Committee ----------------------------------------------------------

    def committee_json(self) -> dict:
        return {
            "epoch": self.epoch,
            "total_stake": self.total_stake,
            "quorum_threshold": self.quorum_threshold,
            "validators": [
                {"index": i, "hostname": f"validator-{i}", "stake": self.stakes[i]}
                for i in range(self.num_validators)
            ],
        }

    # -- Status -------------------------------------------------------------

    def status_json(self) -> dict:
        return {
            "highest_accepted_round": self.current_round,
            "last_commit_index": self.last_commit_index,
            "last_commit_round": self.last_commit_round,
            "num_authorities": self.num_validators,
        }

    # -- DAG window ---------------------------------------------------------

    def dag_window_json(self, from_round: int, to_round: int) -> dict:
        # Only advance the live generator if the requested range is near
        # the current round.  For distant ranges (e.g. "go to round 50000")
        # we generate everything ephemerally — no need to churn through
        # every intermediate round.
        gap = to_round - self.current_round
        if 0 < gap <= self.max_stored_rounds:
            self._generate_up_to(to_round)

        blocks_out: list[dict] = []
        leaders_out: list[dict] = []

        # For rounds already in memory, use them directly.
        # For evicted or future rounds, generate ephemerally (not stored).
        ephemeral: dict[int, list[dict | None]] = {}
        for r in range(max(1, from_round), to_round + 1):
            if r in self.blocks:
                round_blocks = self.blocks[r]
            else:
                round_blocks = self._generate_ephemeral_round(r, ephemeral)
                ephemeral[r] = round_blocks

            for b in round_blocks:
                if b is not None:
                    blocks_out.append(b)

            if r in self.leaders:
                leaders_out.append(self.leaders[r])
            else:
                leader_info = self._generate_ephemeral_leader(r, ephemeral)
                if leader_info:
                    leaders_out.append(leader_info)

        return {
            "from_round": from_round,
            "to_round": to_round,
            "highest_accepted_round": max(self.current_round, to_round),
            "last_commit_round": self.last_commit_round,
            "blocks": blocks_out,
            "leaders": leaders_out,
        }

    # -- Advance one round (returns list of events) -------------------------

    def advance_round(self) -> list[dict]:
        """Generate the next round and return WebSocket events."""
        next_round = self.current_round + 1
        self._generate_up_to(next_round)
        return self._events_for_round(next_round)

    # -- Internal generation ------------------------------------------------

    def _generate_up_to(self, target_round: int) -> None:
        while self.current_round < target_round:
            self.current_round += 1
            self._generate_round(self.current_round)

        # Evict old rounds to cap memory usage
        if self.max_stored_rounds > 0 and len(self.blocks) > self.max_stored_rounds:
            cutoff = self.current_round - self.max_stored_rounds
            for r in list(self.blocks.keys()):
                if r < cutoff:
                    del self.blocks[r]
            for r in list(self.leaders.keys()):
                if r < cutoff:
                    del self.leaders[r]

    def _generate_ephemeral_round(
        self, round_: int, ephemeral: dict[int, list[dict | None]],
    ) -> list[dict | None]:
        """Generate a round's blocks without storing them — for historical REST queries."""
        rng = random.Random(round_ * 1000 + self.num_validators)
        now_ms = int(time.time() * 1000)
        base_ts = now_ms - (self.current_round - round_) * self.round_interval_ms

        round_blocks: list[dict | None] = []
        for author in range(self.num_validators):
            if round_ > 1 and rng.random() < self.miss_rate:
                round_blocks.append(None)
                continue

            # Build ancestors from the previous round (stored or ephemeral)
            ancestors = []
            if round_ > 1:
                prev = self.blocks.get(round_ - 1) or ephemeral.get(round_ - 1, [])
                for a, b in enumerate(prev):
                    if b is not None:
                        ancestors.append({
                            "round": round_ - 1,
                            "author": a,
                            "digest": b["digest"],
                        })

            ts = base_ts + rng.randint(-10, 30)
            round_blocks.append({
                "round": round_,
                "author": author,
                "digest": make_digest(round_, author),
                "timestamp_ms": ts,
                "ancestors": ancestors,
                "acknowledgments": [],
            })

        return round_blocks

    def _generate_ephemeral_leader(
        self, round_: int, ephemeral: dict[int, list[dict | None]],
    ) -> dict | None:
        """Generate leader info for a round without storing it."""
        if round_ < 3 or round_ % 3 != 0:
            return None
        leader_round = round_ - 2
        if leader_round in self.leaders:
            return None  # already have it

        rng = random.Random(leader_round * 1000 + self.num_validators)
        # Consume the same random calls as _generate_ephemeral_round to keep RNG in sync
        for _ in range(self.num_validators):
            rng.random()  # miss check
            rng.randint(-10, 30)  # timestamp jitter
        skipped = rng.random() < self.skip_rate
        status = 1 if skipped else 0

        wave = (leader_round - 1) // 3
        leader_authority = leader_round % self.num_validators

        lr_blocks = self.blocks.get(leader_round) or ephemeral.get(leader_round, [])
        leader_block = lr_blocks[leader_authority] if leader_authority < len(lr_blocks) else None

        leader_info: dict[str, Any] = {
            "wave": wave,
            "leader_round": leader_round,
            "leader_authority": leader_authority,
            "status": status,
            "block_digest": leader_block["digest"] if leader_block and not skipped else None,
        }
        return leader_info

    def _generate_round(self, round_: int) -> None:
        now_ms = int(time.time() * 1000)
        base_ts = now_ms - (self.current_round - round_) * self.round_interval_ms

        round_blocks: list[dict | None] = []
        for author in range(self.num_validators):
            if round_ > 1 and random.random() < self.miss_rate:
                round_blocks.append(None)
                continue

            # Build ancestors: one reference per validator.
            # Normally from round-1, but occasionally stale (older rounds).
            ancestors = []
            if round_ > 1:
                for a in range(self.num_validators):
                    ref_round = round_ - 1

                    # Stale ancestor: reference an older round for this author
                    if round_ > 3 and random.random() < self.stale_rate:
                        if random.random() < 0.25:
                            # Very stale: 5-15 rounds back
                            ref_round = random.randint(
                                max(1, round_ - 15), max(1, round_ - 5)
                            )
                        else:
                            # Moderately stale: 2-4 rounds back
                            ref_round = random.randint(
                                max(1, round_ - 4), round_ - 2
                            )

                    ref_blocks = self.blocks.get(ref_round, [])
                    if a < len(ref_blocks) and ref_blocks[a] is not None:
                        ancestors.append({
                            "round": ref_round,
                            "author": a,
                            "digest": ref_blocks[a]["digest"],
                        })

            # Jitter timestamp per author
            ts = base_ts + random.randint(-10, 30)

            # Build acknowledgments: reference real blocks from round-2,
            # with occasional older ones (round-3, round-4).
            acks: list[dict[str, Any]] = []
            if round_ > 2:
                # Primary: acknowledge blocks from round-2
                target_round = round_ - 2
                target_blocks = self.blocks.get(target_round, [])
                for a in range(len(target_blocks)):
                    if target_blocks[a] is not None and random.random() < 0.6:
                        acks.append({
                            "round": target_round,
                            "author": a,
                            "digest": target_blocks[a]["digest"],
                        })
                # Occasionally also acknowledge from round-3 or round-4
                for older in (round_ - 3, round_ - 4):
                    if older < 1:
                        continue
                    older_blocks = self.blocks.get(older, [])
                    for a in range(len(older_blocks)):
                        if older_blocks[a] is not None and random.random() < 0.15:
                            acks.append({
                                "round": older,
                                "author": a,
                                "digest": older_blocks[a]["digest"],
                            })

            block: dict[str, Any] = {
                "round": round_,
                "author": author,
                "digest": make_digest(round_, author),
                "timestamp_ms": ts,
                "ancestors": ancestors,
                "acknowledgments": acks,
            }
            round_blocks.append(block)

        self.blocks[round_] = round_blocks

        # Equivocation: duplicate block with different digest
        if self.equivocation_rate > 0 and random.random() < self.equivocation_rate:
            candidates = [i for i, b in enumerate(round_blocks) if b is not None]
            if candidates:
                eq_author = random.choice(candidates)
                eq_block = dict(round_blocks[eq_author])  # type: ignore[arg-type]
                eq_block["digest"] = make_digest(round_, eq_author, "equivocation")
                eq_block["timestamp_ms"] += 10
                round_blocks.append(eq_block)

        # Leader decisions: WAVE_LENGTH=3, leader at wave*3+1, decision at wave*3+3
        # Wave 0: leader=1, vote=2, decision=3; Wave 1: leader=4, vote=5, decision=6; ...
        if round_ >= 3 and round_ % 3 == 0:
            leader_round = round_ - 2
            wave = (leader_round - 1) // 3
            leader_authority = leader_round % self.num_validators

            skipped = random.random() < self.skip_rate
            status = 1 if skipped else 0  # 0=committed, 1=skipped

            leader_block = None
            lr_blocks = self.blocks.get(leader_round, [])
            if leader_authority < len(lr_blocks):
                leader_block = lr_blocks[leader_authority]

            leader_info: dict[str, Any] = {
                "wave": wave,
                "leader_round": leader_round,
                "leader_authority": leader_authority,
                "status": status,
                "block_digest": leader_block["digest"] if leader_block and not skipped else None,
            }

            if not skipped:
                self.last_commit_round = leader_round
                self.last_commit_index += 1

            self.leaders[leader_round] = leader_info

    def _events_for_round(self, round_: int) -> list[dict]:
        events: list[dict] = []

        round_blocks = self.blocks.get(round_, [])
        for b in round_blocks:
            if b is None:
                continue
            events.append({
                "t": 0,  # EVENT_BLOCK_ACCEPTED
                "round": b["round"],
                "author": b["author"],
                "digest": b["digest"],
                "timestamp_ms": b["timestamp_ms"],
                "ancestors": b["ancestors"],
                "acknowledgments": b.get("acknowledgments", []),
            })

        # Leader decision events (WAVE_LENGTH=3: decision at wave*3+3)
        if round_ >= 3 and round_ % 3 == 0:
            leader_round = round_ - 2
            info = self.leaders.get(leader_round)
            if info:
                events.append({
                    "t": 1,  # EVENT_LEADER_DECIDED
                    "wave": info["wave"],
                    "leader_round": info["leader_round"],
                    "leader_authority": info["leader_authority"],
                    "status": info["status"],
                    "block_digest": info["block_digest"],
                })

        # Round advanced
        events.append({"t": 2, "round": round_})  # EVENT_ROUND_ADVANCED
        return events


# ---------------------------------------------------------------------------
# HTTP + WebSocket server
# ---------------------------------------------------------------------------

async def create_app(args: argparse.Namespace) -> web.Application:
    dag = DagGenerator(
        num_validators=args.validators,
        miss_rate=args.miss_rate,
        skip_rate=args.skip_rate,
        round_interval_ms=args.round_interval_ms,
        epoch=args.epoch,
        equivocation_rate=args.equivocation_rate,
        stale_rate=args.stale_rate,
        max_stored_rounds=args.max_rounds,
    )

    ws_clients: set[web.WebSocketResponse] = set()
    stop_event = asyncio.Event()

    # -- REST handlers ------------------------------------------------------

    async def handle_committee(_req: web.Request) -> web.Response:
        return web.Response(
            body=encode_committee(dag.committee_json()),
            content_type="application/octet-stream",
        )

    async def handle_status(_req: web.Request) -> web.Response:
        return web.Response(
            body=encode_status(dag.status_json()),
            content_type="application/octet-stream",
        )

    async def handle_dag(req: web.Request) -> web.Response:
        try:
            from_round = int(req.query.get("from_round", 0))
            to_round = int(req.query.get("to_round", dag.current_round))
        except (ValueError, TypeError):
            return web.Response(status=400, text="Invalid query parameters: from_round and to_round must be integers")
        return web.Response(
            body=encode_dag_window(dag.dag_window_json(from_round, to_round)),
            content_type="application/octet-stream",
        )

    async def handle_epochs(_req: web.Request) -> web.Response:
        first_round = min(dag.blocks.keys()) if dag.blocks else 1
        epochs = [{
            "epoch": dag.epoch,
            "from_round": first_round,
            "to_round": dag.current_round,
        }]
        return web.Response(
            body=encode_epochs(epochs),
            content_type="application/octet-stream",
        )

    # -- WebSocket handler --------------------------------------------------

    async def handle_ws(req: web.Request) -> web.WebSocketResponse:
        ws = web.WebSocketResponse(heartbeat=5.0)
        await ws.prepare(req)
        ws_clients.add(ws)
        try:
            async for msg in ws:
                if msg.type in (WSMsgType.CLOSE, WSMsgType.ERROR):
                    break
        finally:
            ws_clients.discard(ws)
        return ws

    # -- Background round ticker --------------------------------------------

    async def round_ticker() -> None:
        await asyncio.sleep(1.0)  # let server start
        base_interval = args.round_interval_ms / 1000.0
        while not stop_event.is_set():
            events = dag.advance_round()
            encoded_list = [encode_event(e) for e in events]
            dead: list[web.WebSocketResponse] = []
            for ws in ws_clients:
                for payload in encoded_list:
                    try:
                        await ws.send_bytes(payload)
                    except (ConnectionResetError, Exception):
                        dead.append(ws)
                        break
            for ws in dead:
                ws_clients.discard(ws)

            # Occasionally slow down a round (2-5x normal duration)
            interval = base_interval
            if random.random() < args.slow_round_rate:
                interval *= random.uniform(2.0, 5.0)
            await asyncio.sleep(interval)

    # -- App assembly -------------------------------------------------------

    app = web.Application()
    app.router.add_get("/api/v1/committee", handle_committee)
    app.router.add_get("/api/v1/status", handle_status)
    app.router.add_get("/api/v1/dag", handle_dag)
    app.router.add_get("/api/v1/epochs", handle_epochs)
    app.router.add_get("/api/v1/ws", handle_ws)

    async def on_startup(_app: web.Application) -> None:
        _app["ticker_task"] = asyncio.create_task(round_ticker())

    async def on_cleanup(_app: web.Application) -> None:
        stop_event.set()
        task = _app.get("ticker_task")
        if task:
            task.cancel()
            try:
                await task
            except asyncio.CancelledError:
                pass
        for ws in list(ws_clients):
            await ws.close()

    app.on_startup.append(on_startup)
    app.on_cleanup.append(on_cleanup)

    return app


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(description="Mock DAG visualizer backend")
    p.add_argument("--validators", type=int, default=10,
                   help="Number of validators (default: 10)")
    p.add_argument("--miss-rate", type=float, default=0.05,
                   help="Block miss probability 0..1 (default: 0.05)")
    p.add_argument("--skip-rate", type=float, default=0.15,
                   help="Leader skip probability 0..1 (default: 0.15)")
    p.add_argument("--round-interval-ms", type=int, default=55,
                   help="Milliseconds between rounds (default: 55)")
    p.add_argument("--port", type=int, default=9186,
                   help="Port to listen on (default: 9186)")
    p.add_argument("--epoch", type=int, default=1,
                   help="Current epoch number (default: 1)")
    p.add_argument("--equivocation-rate", type=float, default=0.02,
                   help="Equivocation probability per round 0..1 (default: 0.02)")
    p.add_argument("--stale-rate", type=float, default=0.08,
                   help="Per-ancestor probability of being stale 0..1 (default: 0.08)")
    p.add_argument("--slow-round-rate", type=float, default=0.08,
                   help="Probability of a round being slow (2-5x) 0..1 (default: 0.08)")
    p.add_argument("--max-rounds", type=int, default=100,
                   help="Max rounds stored in memory for WS streaming; older rounds are evicted (default: 100)")
    return p.parse_args()


def main() -> None:
    args = parse_args()
    print(f"Starting mock DAG server on :{args.port}")
    print(f"  validators={args.validators}  miss_rate={args.miss_rate}  "
          f"skip_rate={args.skip_rate}  round_interval={args.round_interval_ms}ms  "
          f"equivocation_rate={args.equivocation_rate}  stale_rate={args.stale_rate}  "
          f"slow_round_rate={args.slow_round_rate}  max_rounds={args.max_rounds}")
    web.run_app(create_app(args), host="0.0.0.0", port=args.port, print=lambda msg: print(msg))


if __name__ == "__main__":
    main()
