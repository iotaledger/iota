// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Browser example for iota-vm-sdk's wasm Move VM. Two independent flows, one
// per tab:
//
//   * "Stake" builds a real request_add_stake transaction with the IOTA
//     TypeScript SDK, then simulates it here. It is built gasless — no gas
//     payment — so the VM mints a mock gas coin (the same one a node mints on
//     its dev-inspect / dry-run paths), which is what lets it succeed in both
//     execution modes regardless of the sender's on-chain balance.
//
//   * "Your own transaction" decodes and simulates arbitrary BCS transaction
//     bytes, optionally verifying supplied signatures.
//
// Either flow picks an execution mode: dev-inspect (relaxed checks, meters at
// the protocol max gas) or dry-run (full validation at the transaction's own
// gas budget). In both flows the wasm Store resolves every object the VM reads
// on demand via `fetchObject`; nothing executes on a node.

import { getFullnodeUrl, IotaClient } from "@iota/iota-sdk/client";
import { Transaction } from "@iota/iota-sdk/transactions";
import PRESETS from "./presets.json";

// The networks the example can target. `chain` is what the wasm VM understands
// (mainnet / testnet); devnet and localnet have no dedicated variant, so they
// run with the chain left unset.
type NetName = "testnet" | "mainnet" | "devnet" | "localnet";
interface Network {
  rpc: string;
  graphql: string;
  chain?: string;
}
const NETWORKS: Record<NetName, Network> = {
  testnet: {
    rpc: getFullnodeUrl("testnet"),
    graphql: "https://graphql.testnet.iota.cafe",
    chain: "testnet",
  },
  mainnet: {
    rpc: getFullnodeUrl("mainnet"),
    graphql: "https://graphql.mainnet.iota.cafe",
    chain: "mainnet",
  },
  devnet: {
    rpc: getFullnodeUrl("devnet"),
    graphql: "https://graphql.devnet.iota.cafe",
  },
  localnet: {
    rpc: getFullnodeUrl("localnet"),
    graphql: "http://127.0.0.1:9125/",
  },
};

// The sender of record for the staking transaction. Because that transaction is
// built gasless, this address needs no coins on any network — it is only the
// declared sender.
const SENDER =
  "0xda1820edf693ee32b5729907b9b2ec8e64980ee8c008c17e89cfb4e5ecd72151";
const SYSTEM_STATE =
  "0x0000000000000000000000000000000000000000000000000000000000000005";
const STAKE_NANOS = 1_000_000_000n; // 1 IOTA
// Declared budget for the staking transaction. dev-inspect ignores it (it
// meters at the protocol max off the mock gas coin); dry-run charges real gas
// against it, so it must comfortably cover the transaction's cost.
const STAKE_GAS_BUDGET = 2_000_000_000n; // 2 IOTA

type Mode = "dev-inspect" | "dry-run";

// Chain parameters passed to simulate. Self-contained presets carry the four
// numeric fields; live runs additionally set `chain` for known networks.
interface Params {
  chain?: string;
  protocol_version: number;
  reference_gas_price: number;
  epoch_id: number;
  epoch_timestamp_ms: number;
}

// One "Your own transaction" example. `objects` + `params` are present only for
// self-contained presets, which run offline against those exact objects; live
// presets omit them and use the selected network's objects and chain params.
interface Preset {
  id: string;
  label: string;
  note: string;
  txB64: string;
  signatures: string[];
  objects?: string[];
  params?: Params;
  mode: Mode;
  verify: boolean;
}

const presets = PRESETS as Preset[];
// The preset currently loaded into the inputs, if the transaction box still
// holds its bytes — its `objects` / `params` then drive the simulation.
let loadedPreset: Preset | null = null;

// The currently selected network and a client bound to it; both are updated by
// the network selector.
let netName: NetName = "testnet";
let client = new IotaClient({ url: NETWORKS[netName].rpc });

// The set of result elements for one tab. Both tabs render into their own copy
// so switching tabs never mixes results.
interface Out {
  status: HTMLElement;
  timing: HTMLElement;
  txDesc: HTMLElement;
  tx: HTMLElement;
  output: HTMLElement;
  log: HTMLElement;
  txPanel: HTMLElement;
  resultPanel: HTMLElement;
  logPanel: HTMLElement;
}

function outFor(prefix: string): Out {
  const $ = (id: string) => document.getElementById(`${prefix}-${id}`)!;
  return {
    status: $("status"),
    timing: $("timing"),
    txDesc: $("tx-desc"),
    tx: $("tx"),
    output: $("output"),
    log: $("log"),
    txPanel: $("tx-panel"),
    resultPanel: $("result-panel"),
    logPanel: $("log-panel"),
  };
}

const stakeOut = outFor("stake");
const customOut = outFor("custom");

const els = {
  run: document.getElementById("run") as HTMLButtonElement,
  runCustom: document.getElementById("run-custom") as HTMLButtonElement,
  hint: document.getElementById("hint")!,
  txBytes: document.getElementById("tx-bytes") as HTMLTextAreaElement,
  sigs: document.getElementById("sigs") as HTMLTextAreaElement,
};

// Wall-clock spent inside fetchObject across one simulation, and the number of
// fetches. Reset before each run; the fetches happen synchronously inside the
// simulate() call, so VM time = simulate wall-clock − this. `currentLog` is the
// log element of the tab currently running, so fetchObject logs into it.
let fetchMs = 0;
let fetchCount = 0;
let currentLog: HTMLElement | null = null;

// --- The on-demand object resolver handed to the wasm Store -----------------

// Synchronously fetch an object's full BCS from the selected network's GraphQL,
// at the exact version when given, else latest. The Move VM is synchronous, so
// this blocks (sync XMLHttpRequest) until the response — that's the price of
// resolving objects mid-execution without a node. Returns the base-64 `Object`
// BCS, or null if the object doesn't exist.
function fetchObject(idHex: string, version: number | null): string | null {
  const at = version === null ? "" : `, version: ${version}`;
  const query = `{ object(address: "${idHex}"${at}) { bcs } }`;
  const xhr = new XMLHttpRequest();
  xhr.open("POST", NETWORKS[netName].graphql, /* async */ false);
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

// --- Flows ------------------------------------------------------------------

// Build a gasless request_add_stake transaction: split 1 IOTA off the gas coin
// and stake it. With no gas payment the wasm mints a mock gas coin, so the
// split has funds to draw from in both execution modes.
async function buildStakeTx(): Promise<{ bytes: Uint8Array; validator: string }> {
  const [sys, gasPrice] = await Promise.all([
    client.getLatestIotaSystemState(),
    client.getReferenceGasPrice(),
  ]);
  const validator = sys.activeValidators[0].iotaAddress;

  const tx = new Transaction();
  const [coin] = tx.splitCoins(tx.gas, [tx.pure.u64(STAKE_NANOS)]);
  tx.moveCall({
    target: "0x3::iota_system::request_add_stake",
    arguments: [tx.object(SYSTEM_STATE), coin, tx.pure.address(validator)],
  });
  tx.setSender(SENDER);
  tx.setGasOwner(SENDER);
  tx.setGasPayment([]); // gasless: the VM mints a mock gas coin
  tx.setGasPrice(Number(gasPrice));
  tx.setGasBudget(Number(STAKE_GAS_BUDGET));
  const bytes = await tx.build({ client });
  return { bytes, validator };
}

// Staking flow: the SDK builds a request_add_stake tx for the selected network,
// then we simulate it locally in the selected mode.
async function runStaking(wasm: any) {
  const mode = currentMode("stake-mode");
  setRunning(true);
  currentLog = stakeOut.log;
  resetPanels(stakeOut);
  setStatus(stakeOut, "busy", "Building transaction…");
  try {
    const { bytes, validator } = await buildStakeTx();
    await doSimulate(wasm, {
      out: stakeOut,
      txB64: bytesToB64(bytes),
      signatures: [],
      strict: mode === "dry-run",
      objects: [],
      params: await chainParams(),
      desc: `Stake ${
        Number(STAKE_NANOS) / 1e9
      } IOTA with validator ${short(validator)} · ${mode} · gasless`,
      txData: Transaction.from(bytes).getData(),
    });
  } catch (e) {
    showError(stakeOut, e);
  } finally {
    setRunning(false);
  }
}

// Custom flow: simulate user-supplied transaction bytes in the selected mode,
// verifying the supplied signatures (one per line) when "Verify signatures" is
// checked.
async function runCustom(wasm: any) {
  const mode = currentMode("custom-mode");
  const verify = currentSig() === "verify";
  const txB64 = els.txBytes.value.trim();
  const signatures = verify
    ? els.sigs.value.split(/\s+/).map((s) => s.trim()).filter(Boolean)
    : [];
  setRunning(true);
  currentLog = customOut.log;
  resetPanels(customOut);
  setStatus(customOut, "busy", "Simulating custom transaction…");
  try {
    if (!txB64) throw new Error("Provide transaction bytes (base64).");
    const data = Transaction.from(b64ToBytes(txB64)).getData();
    // A self-contained preset (still loaded in the box) brings its own objects
    // and chain params, so it runs offline; anything else uses the selected
    // network's live params and fetches objects on demand.
    const preset = loadedPreset?.txB64 === txB64 ? loadedPreset : null;
    const objects = preset?.objects ?? [];
    const params = preset?.params ?? await chainParams();
    const source = preset?.objects
      ? ` · offline, ${preset.objects.length} bundled objects`
      : ` · ${netName}`;
    await doSimulate(wasm, {
      out: customOut,
      txB64,
      signatures,
      strict: mode === "dry-run",
      objects,
      params,
      desc: `Custom transaction · ${mode}${
        verify
          ? ` · verifying ${signatures.length} signature(s)`
          : " · signatures skipped"
      }${source}`,
      txData: data,
    });
  } catch (e) {
    showError(customOut, e);
  } finally {
    setRunning(false);
  }
}

// Shared: fetch chain params, show the decoded tx (collapsed), run the VM
// resolving objects on demand, and render the result. Signatures, when present,
// are verified before execution.
async function doSimulate(
  wasm: any,
  opts: {
    out: Out;
    txB64: string;
    signatures: string[];
    strict: boolean;
    objects: string[];
    params: Params;
    desc: string;
    txData: unknown;
  },
) {
  const { out } = opts;
  out.txDesc.textContent = opts.desc;
  out.tx.textContent = "";
  out.tx.append(jsonNode(opts.txData, null, false));
  out.txPanel.hidden = false;

  setStatus(out, "busy", "Simulating in the local Move VM…");
  out.resultPanel.hidden = false;
  // Any object not pre-seeded in `objects` is fetched on demand via
  // fetchObject. The fetches run synchronously inside simulate(), so the
  // wall-clock of this call is fetch time + VM time. `strict: true` selects
  // dry-run, else dev-inspect.
  fetchMs = 0;
  fetchCount = 0;
  const wallStart = performance.now();
  let result: any;
  try {
    result = wasm.simulate(
      {
        tx_b64: opts.txB64,
        ...opts.params,
        objects: opts.objects.map((bcs_b64) => ({ bcs_b64 })),
        strict: opts.strict,
        signatures: opts.signatures,
      },
      fetchObject,
    );
  } catch (e) {
    renderTiming(out, fetchMs, fetchCount, 0, performance.now() - wallStart);
    // A standard (non-MoveAuthenticator) signature is verified before the
    // transaction runs, so a bad one is a hard error rather than a result: the
    // body never executes. Show that as a rejection instead of a raw error.
    const msg = String((e as any)?.message ?? e);
    if (msg.startsWith("SignatureVerification")) {
      out.status.textContent = "";
      out.status.append(
        statRow("muted", "Tx body", "not committed (unauthorized)"),
        statRow("bad", "Signatures", "rejected"),
      );
      out.output.textContent = msg;
      return;
    }
    throw e;
  }
  const wall = performance.now() - wallStart;
  renderTiming(out, fetchMs, fetchCount, Math.max(0, wall - fetchMs), wall);

  // Two verdicts, coloured separately. A rejected signature dominates: whether
  // it is a standard signature (a precondition) or a MoveAuthenticator (Move
  // code that aborts mid-execution), the transaction is unauthorized and its
  // body does not take effect — so the body's own success is moot and shown
  // muted rather than as a failure of its logic.
  const sigRejected = opts.signatures.length > 0 && !result.signature_verified;
  const bodyRow = sigRejected
    ? statRow("muted", "Tx body", "not committed (unauthorized)")
    : statRow(
      result.success ? "ok" : "bad",
      "Tx body",
      result.success ? "executed successfully" : "execution failed",
    );
  let sigRow: HTMLElement;
  if (!opts.signatures.length) {
    sigRow = statRow("muted", "Signatures", "not checked (unsigned)");
  } else if (result.signature_verified) {
    sigRow = statRow(
      "ok",
      "Signatures",
      `verified (${opts.signatures.length})`,
    );
  } else {
    sigRow = statRow("bad", "Signatures", "rejected");
  }
  out.status.textContent = "";
  out.status.append(bodyRow, sigRow);
  out.output.append(jsonNode(result, null));
}

async function chainParams() {
  const [sys, gasPrice] = await Promise.all([
    client.getLatestIotaSystemState(),
    client.getReferenceGasPrice(),
  ]);
  const params = {
    protocol_version: Number(sys.protocolVersion),
    reference_gas_price: Number(gasPrice),
    epoch_id: Number(sys.epoch),
    epoch_timestamp_ms: Number(sys.epochStartTimestampMs),
  };
  // Only mainnet / testnet map to a wasm `Chain`; devnet runs with it unset.
  const chain = NETWORKS[netName].chain;
  return chain ? { chain, ...params } : params;
}

// --- Bootstrap --------------------------------------------------------------

(async () => {
  wireTabs();
  wireSegments();
  wireNetwork();
  wireSignatureToggle();
  wirePresets();
  try {
    // Loaded at runtime (not bundled) so the wasm-bindgen glue resolves the
    // .wasm via import.meta.url. Build it first — see README.md.
    const wasm = await import("./pkg/iota_vm_sdk.js");
    await wasm.default();
    updateHint();
    els.run.disabled = false;
    els.runCustom.disabled = false;
    els.run.addEventListener("click", () => runStaking(wasm));
    els.runCustom.addEventListener("click", () => runCustom(wasm));
  } catch (e) {
    setStatus(stakeOut, "bad", "Failed to load wasm");
    stakeOut.resultPanel.hidden = false;
    stakeOut.output.textContent =
      String(e) + "\n\nDid you build the wasm package? See README.md.";
  }
})();

// --- UI wiring --------------------------------------------------------------

function wireTabs() {
  const tabs: ("stake" | "custom")[] = ["stake", "custom"];
  for (const name of tabs) {
    document.getElementById(`tab-${name}`)!.addEventListener("click", () => {
      for (const other of tabs) {
        const selected = other === name;
        document
          .getElementById(`tab-${other}`)!
          .setAttribute("aria-selected", String(selected));
        document.getElementById(`panel-${other}`)!.hidden = !selected;
      }
    });
  }
}

// A segmented control: clicking a button makes it the only `.active` one among
// its siblings.
function wireSegments() {
  for (const seg of document.querySelectorAll<HTMLElement>(".seg")) {
    seg.addEventListener("click", (e) => {
      const btn = (e.target as HTMLElement).closest("button");
      if (!btn || !seg.contains(btn)) return;
      for (const b of seg.querySelectorAll("button")) {
        b.classList.toggle("active", b === btn);
      }
    });
  }
}

// The network selector rebuilds the client and repoints on-demand fetches at
// the chosen network's GraphQL.
function wireNetwork() {
  document.getElementById("network")!.addEventListener("click", (e) => {
    const btn = (e.target as HTMLElement).closest<HTMLElement>("button");
    if (!btn?.dataset.net) return;
    netName = btn.dataset.net as NetName;
    client = new IotaClient({ url: NETWORKS[netName].rpc });
    updateHint();
  });
}

// Grey out the signatures box when "Skip" is selected — the input is ignored.
function wireSignatureToggle() {
  const sync = () => (els.sigs.disabled = currentSig() !== "verify");
  document.getElementById("custom-sig")!.addEventListener("click", sync);
  sync();
}

// Render one chip per example preset; loading one fills the inputs and sets the
// recommended mode / signature toggles. The first preset is loaded on startup.
function wirePresets() {
  const row = document.getElementById("presets")!;
  const chips = presets.map((p) => {
    const chip = document.createElement("button");
    chip.type = "button";
    chip.className = "chip";
    chip.textContent = p.label;
    chip.addEventListener("click", () => loadPreset(p, chip, chips));
    row.append(chip);
    return chip;
  });
  if (chips.length) loadPreset(presets[0], chips[0], chips);
}

function loadPreset(p: Preset, chip: HTMLElement, chips: HTMLElement[]) {
  els.txBytes.value = p.txB64;
  els.sigs.value = p.signatures.join("\n");
  setSeg("custom-mode", "mode", p.mode);
  setSeg("custom-sig", "sig", p.verify ? "verify" : "skip");
  els.sigs.disabled = !p.verify;
  loadedPreset = p;
  document.getElementById("preset-note")!.textContent = p.note;
  for (const c of chips) c.classList.toggle("active", c === chip);
}

// Select the button carrying `data-<attr>="value"` in a segmented control.
function setSeg(segId: string, attr: string, value: string) {
  for (const b of document.querySelectorAll<HTMLElement>(`#${segId} button`)) {
    b.classList.toggle("active", b.dataset[attr] === value);
  }
}

function updateHint() {
  els.hint.textContent = `sender ${short(SENDER)} · ${netName}`;
}

function currentMode(segId: string): Mode {
  const active = document.querySelector<HTMLElement>(`#${segId} .active`);
  return (active?.dataset.mode as Mode) ?? "dev-inspect";
}

function currentSig(): "verify" | "skip" {
  const active = document.querySelector<HTMLElement>(`#custom-sig .active`);
  return (active?.dataset.sig as "verify" | "skip") ?? "verify";
}

// --- Helpers ----------------------------------------------------------------

// A single status line: a coloured dot, an optional label, and a value.
function statRow(
  kind: "ok" | "bad" | "busy" | "muted",
  label: string,
  value: string,
): HTMLElement {
  const row = document.createElement("div");
  row.className = `stat ${kind}`;
  const dot = document.createElement("span");
  dot.className = "dot";
  row.append(dot);
  if (label) {
    const l = document.createElement("span");
    l.className = "stat-label";
    l.textContent = label;
    row.append(l);
  }
  const v = document.createElement("span");
  v.className = "stat-value";
  v.textContent = value;
  row.append(v);
  return row;
}

// A single-line status (busy / error), replacing any previous rows.
function setStatus(out: Out, kind: "ok" | "bad" | "busy", text: string) {
  out.status.textContent = "";
  out.status.append(statRow(kind, "", text));
}

function setRunning(busy: boolean) {
  els.run.disabled = busy;
  els.runCustom.disabled = busy;
}

function resetPanels(out: Out) {
  out.log.textContent = "";
  out.output.textContent = "";
  out.timing.textContent = "";
  out.logPanel.hidden = false;
}

// Show the timing breakdown: object-fetch time (and count), VM execution time,
// and the combined total (the simulate() wall-clock).
function renderTiming(
  out: Out,
  fetch: number,
  count: number,
  vm: number,
  total: number,
) {
  out.timing.textContent = "";
  const metric = (label: string, ms: number, extra = "") => {
    const el = document.createElement("span");
    el.append(`${label} `, Object.assign(document.createElement("b"), {
      textContent: `${Math.round(ms)} ms`,
    }));
    if (extra) el.append(` ${extra}`);
    return el;
  };
  out.timing.append(
    metric("fetch objects", fetch, `(${count} req)`),
    metric("VM execution", vm),
    metric("total", total),
  );
}

function showError(out: Out, e: any) {
  setStatus(out, "bad", "Error");
  out.resultPanel.hidden = false;
  out.output.textContent = String(e?.message ?? e);
}

function b64ToBytes(b64: string): Uint8Array {
  const bin = atob(b64);
  const bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
  return bytes;
}

function logLine(kind: "obj" | "miss" | "info", text: string) {
  if (!currentLog) return;
  const div = document.createElement("div");
  div.className = kind;
  div.textContent = text;
  currentLog.append(div);
  currentLog.scrollTop = currentLog.scrollHeight;
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
