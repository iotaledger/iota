// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Browser example: build a real staking transaction with the IOTA TypeScript
// SDK against testnet, then simulate it in the local Move VM (wasm). The wasm
// Store resolves every object the VM touches on demand by calling `fetchObject`
// below — so only what this transaction actually reads is fetched, exactly when
// the VM reads it. Nothing executes on a node.

import { getFullnodeUrl, IotaClient } from "@iota/iota-sdk/client";
import { Transaction } from "@iota/iota-sdk/transactions";

const RPC_URL = getFullnodeUrl("testnet");
const GRAPHQL_URL = "https://graphql.testnet.iota.cafe";
// A testnet address that holds gas (mirrors iota-rust-sdk's stake.rs example).
// Simulation is unsigned (dev-inspect), so no private key is needed — the
// address only has to own a coin the SDK can pick for gas.
const SENDER =
  "0xda1820edf693ee32b5729907b9b2ec8e64980ee8c008c17e89cfb4e5ecd72151";
const SYSTEM_STATE =
  "0x0000000000000000000000000000000000000000000000000000000000000005";
const STAKE_NANOS = 1_000_000_000n; // 1 IOTA

const client = new IotaClient({ url: RPC_URL });

// Wall-clock spent inside fetchObject across one simulation, and the number of
// fetches. Reset before each run; the fetches happen synchronously inside the
// simulate() call, so VM time = simulate wall-clock − this.
let fetchMs = 0;
let fetchCount = 0;

const els = {
  run: document.getElementById("run") as HTMLButtonElement,
  runCustom: document.getElementById("run-custom") as HTMLButtonElement,
  txBytes: document.getElementById("tx-bytes") as HTMLTextAreaElement,
  sigs: document.getElementById("sigs") as HTMLTextAreaElement,
  hint: document.getElementById("hint")!,
  status: document.getElementById("status")!,
  timing: document.getElementById("timing")!,
  output: document.getElementById("output")!,
  txPanel: document.getElementById("tx-panel")!,
  txDesc: document.getElementById("tx-desc")!,
  tx: document.getElementById("tx")!,
  resultPanel: document.getElementById("result-panel")!,
  logPanel: document.getElementById("log-panel")!,
  log: document.getElementById("log")!,
};

// --- The on-demand object resolver handed to the wasm Store -----------------

// Synchronously fetch an object's full BCS from testnet GraphQL, at the exact
// version when given, else latest. The Move VM is synchronous, so this blocks
// (sync XMLHttpRequest) until the response — that's the price of resolving
// objects mid-execution without a node. Returns the base-64 `Object` BCS, or
// null if the object doesn't exist.
function fetchObject(idHex: string, version: number | null): string | null {
  const at = version === null ? "" : `, version: ${version}`;
  const query = `{ object(address: "${idHex}"${at}) { bcs } }`;
  const xhr = new XMLHttpRequest();
  xhr.open("POST", GRAPHQL_URL, /* async */ false);
  xhr.setRequestHeader("content-type", "application/json");
  fetchCount++;
  const started = performance.now();
  try {
    xhr.send(JSON.stringify({ query }));
  } catch (e) {
    fetchMs += performance.now() - started;
    logLine("miss", `fetch error ${idHex}: ${e}`);
    return null;
  }
  fetchMs += performance.now() - started;
  if (xhr.status !== 200) {
    logLine("miss", `fetch ${idHex} → HTTP ${xhr.status}`);
    return null;
  }
  const bcs = JSON.parse(xhr.responseText)?.data?.object?.bcs ?? null;
  logLine(bcs ? "obj" : "miss", `${bcs ? "fetched  " : "not found "} ${idHex}`);
  return bcs;
}

// --- Flow -------------------------------------------------------------------

async function buildStakeTx(): Promise<{ bytes: Uint8Array; validator: string }> {
  const sys = await client.getLatestIotaSystemState();
  const validator = sys.activeValidators[0].iotaAddress;

  const tx = new Transaction();
  const [coin] = tx.splitCoins(tx.gas, [tx.pure.u64(STAKE_NANOS)]);
  tx.moveCall({
    target: "0x3::iota_system::request_add_stake",
    arguments: [tx.object(SYSTEM_STATE), coin, tx.pure.address(validator)],
  });
  tx.setSender(SENDER);
  const bytes = await tx.build({ client });
  return { bytes, validator };
}

async function chainParams() {
  const [sys, gasPrice] = await Promise.all([
    client.getLatestIotaSystemState(),
    client.getReferenceGasPrice(),
  ]);
  return {
    chain: "testnet",
    protocol_version: Number(sys.protocolVersion),
    reference_gas_price: Number(gasPrice),
    epoch_id: Number(sys.epoch),
    epoch_timestamp_ms: Number(sys.epochStartTimestampMs),
  };
}

// Staking flow: the SDK builds a request_add_stake tx on testnet, then we
// dev-inspect it locally (unsigned).
async function runStaking(wasm: any) {
  setRunning(true);
  resetPanels();
  setStatus("busy", "Building transaction on testnet…");
  try {
    const { bytes, validator } = await buildStakeTx();
    await doSimulate(
      wasm,
      bytesToB64(bytes),
      [],
      `Stake ${
        Number(STAKE_NANOS) / 1e9
      } IOTA with validator ${short(validator)} via 0x3::iota_system::request_add_stake`,
      Transaction.from(bytes).getData(),
    );
  } catch (e) {
    showError(e);
  } finally {
    setRunning(false);
  }
}

// Custom flow: dev-inspect user-supplied transaction bytes, with optional
// signatures (one per line — e.g. a sender and a sponsor signature).
async function runCustom(wasm: any) {
  const txB64 = els.txBytes.value.trim();
  const signatures = els.sigs.value
    .split(/\s+/)
    .map((s) => s.trim())
    .filter(Boolean);
  setRunning(true);
  resetPanels();
  setStatus("busy", "Simulating custom transaction…");
  try {
    if (!txB64) throw new Error("Provide transaction bytes (base64).");
    const data = Transaction.from(b64ToBytes(txB64)).getData();
    await doSimulate(
      wasm,
      txB64,
      signatures,
      `Custom transaction · dev-inspect${
        signatures.length ? ` · ${signatures.length} signature(s)` : ""
      }`,
      data,
    );
  } catch (e) {
    showError(e);
  } finally {
    setRunning(false);
  }
}

// Shared: fetch chain params, show the decoded tx (collapsed), run the VM
// resolving objects on demand, and render the result. Signatures, when present,
// are verified before execution.
async function doSimulate(
  wasm: any,
  txB64: string,
  signatures: string[],
  desc: string,
  txData: unknown,
) {
  const params = await chainParams();
  els.txDesc.textContent = desc;
  els.tx.textContent = "";
  els.tx.append(jsonNode(txData, null, false));
  els.txPanel.hidden = false;

  setStatus("busy", "Simulating in the local Move VM…");
  els.resultPanel.hidden = false;
  // `objects: []` — the Store fetches everything on demand via fetchObject.
  // The fetches run synchronously inside simulate(), so the wall-clock of this
  // call is fetch time + VM time.
  fetchMs = 0;
  fetchCount = 0;
  const wallStart = performance.now();
  const result = wasm.simulate(
    { tx_b64: txB64, ...params, objects: [], strict: false, signatures },
    fetchObject,
  );
  const wall = performance.now() - wallStart;
  renderTiming(fetchMs, fetchCount, Math.max(0, wall - fetchMs), wall);

  const sig = signatures.length
    ? ` · signature ${
      result.signature_verified
        ? "verified"
        : `rejected${result.signature_error ? ` (${result.signature_error})` : ""}`
    }`
    : "";
  setStatus(
    result.success ? "ok" : "bad",
    `Execution ${result.success ? "succeeded" : "failed"}${sig}`,
  );
  els.output.append(jsonNode(result, null));
}

// --- Bootstrap --------------------------------------------------------------

(async () => {
  try {
    // Loaded at runtime (not bundled) so the wasm-bindgen glue resolves the
    // .wasm via import.meta.url. Build it first — see README.md.
    const wasm = await import("./pkg/iota_vm_sdk.js");
    await wasm.default();
    els.hint.textContent = `sender ${short(SENDER)} · testnet`;
    els.run.disabled = false;
    els.runCustom.disabled = false;
    els.run.addEventListener("click", () => runStaking(wasm));
    els.runCustom.addEventListener("click", () => runCustom(wasm));
  } catch (e) {
    setStatus("bad", "Failed to load wasm");
    els.resultPanel.hidden = false;
    els.output.textContent =
      String(e) + "\n\nDid you build the wasm package? See README.md.";
  }
})();

// --- Helpers ----------------------------------------------------------------

function setStatus(kind: "ok" | "bad" | "busy", text: string) {
  els.status.className = kind;
  els.status.textContent = text;
}

function setRunning(busy: boolean) {
  els.run.disabled = busy;
  els.runCustom.disabled = busy;
}

function resetPanels() {
  els.log.textContent = "";
  els.output.textContent = "";
  els.timing.textContent = "";
  els.logPanel.hidden = false;
}

// Show the timing breakdown: object-fetch time (and count), VM execution time,
// and the combined total (the simulate() wall-clock).
function renderTiming(
  fetch: number,
  count: number,
  vm: number,
  total: number,
) {
  els.timing.textContent = "";
  const metric = (label: string, ms: number, extra = "") => {
    const el = document.createElement("span");
    el.append(`${label} `, Object.assign(document.createElement("b"), {
      textContent: `${Math.round(ms)} ms`,
    }));
    if (extra) el.append(` ${extra}`);
    return el;
  };
  els.timing.append(
    metric("fetch objects", fetch, `(${count} req)`),
    metric("VM execution", vm),
    metric("total", total),
  );
}

function showError(e: any) {
  setStatus("bad", "Error");
  els.resultPanel.hidden = false;
  els.output.textContent = String(e?.message ?? e);
}

function b64ToBytes(b64: string): Uint8Array {
  const bin = atob(b64);
  const bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
  return bytes;
}

function logLine(kind: "obj" | "miss" | "info", text: string) {
  const div = document.createElement("div");
  div.className = kind;
  div.textContent = text;
  els.log.append(div);
  els.log.scrollTop = els.log.scrollHeight;
}

function short(id: string): string {
  return id.length > 14 ? `${id.slice(0, 8)}…${id.slice(-4)}` : id;
}

function bytesToB64(bytes: Uint8Array): string {
  let s = "";
  for (const b of bytes) s += String.fromCharCode(b);
  return btoa(s);
}

const span = (cls: string, text: string) => {
  const el = document.createElement("span");
  el.className = cls;
  el.textContent = text;
  return el;
};

// Build a collapsible DOM tree for a JSON value using native <details>.
// `key` labels this node (object key or array index), or null at the root.
// `open` controls whether this node starts expanded; nested nodes always start
// open, so expanding a collapsed root reveals the whole subtree.
function jsonNode(value: any, key: string | number | null, open = true): Node {
  const label = key === null ? null : span("jkey", JSON.stringify(key));
  if (value === null || typeof value !== "object") {
    const cls = value === null
      ? "jnull"
      : typeof value === "string"
      ? "jstr"
      : typeof value === "number"
      ? "jnum"
      : "jbool";
    const row = document.createElement("div");
    row.className = "jrow";
    if (label) row.append(label, span("jpunct", ": "));
    const text = typeof value === "string" ? JSON.stringify(value) : String(value);
    row.append(span(cls, text));
    return row;
  }

  const arr = Array.isArray(value);
  const entries: [string | number, any][] = arr
    ? value.map((v: any, i: number) => [i, v])
    : Object.entries(value);
  const close = arr ? "]" : "}";

  const details = document.createElement("details");
  details.className = "jnode";
  details.open = open;

  const summary = document.createElement("summary");
  if (label) summary.append(label, span("jpunct", ": "));
  summary.append(span("jpunct", arr ? "[" : "{"));
  summary.append(span("jcount", ` ${entries.length} ${arr ? "items" : "keys"} `));
  summary.append(span("jclosed jpunct", close));

  const kids = document.createElement("div");
  kids.className = "jkids";
  for (const [k, v] of entries) kids.append(jsonNode(v, k));

  details.append(summary, kids, span("jbrace jpunct", close));
  return details;
}
