// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
//
// Regenerates `presets.json`, the "Your own transaction" example transactions.
// Run it after changing an example or refreshing the MoveAuthenticator
// fixtures: `node gen-presets.mjs`.
//
// Two kinds of preset:
//
//   * self-contained — carry their own `objects` (base-64 BCS) and chain
//     `params`, so they run offline against those exact objects regardless of
//     the selected network. The two MoveAuthenticator cases come straight from
//     the crate's committed test fixtures (tests/fixtures/*.json), which pin a
//     protocol version and bundle every object the run reads.
//
//   * live — no objects, built here with the TS SDK as gasless transactions
//     (the VM mints a mock gas coin). They run against whatever network is
//     selected, using its live chain params. Signatures are produced from fixed
//     keypairs so the file is reproducible.

import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { getFullnodeUrl, IotaClient } from "@iota/iota-sdk/client";
import { Transaction } from "@iota/iota-sdk/transactions";
import { Ed25519Keypair } from "@iota/iota-sdk/keypairs/ed25519";

const here = dirname(fileURLToPath(import.meta.url));
const fixturesDir = join(here, "..", "..", "tests", "fixtures");
const client = new IotaClient({ url: getFullnodeUrl("testnet") });

const b64 = (u8) => Buffer.from(u8).toString("base64");

// A deterministic keypair from a fixed one-byte pattern, so reruns are stable.
function keypair(tag) {
  const secret = new Uint8Array(32);
  for (let i = 0; i < 32; i++) secret[i] = (i * 7 + tag) & 0xff;
  return Ed25519Keypair.fromSecretKey(secret);
}

// Turn a committed MoveAuthenticator fixture into a self-contained preset.
function fromFixture(file, over) {
  const d = JSON.parse(readFileSync(join(fixturesDir, file), "utf8"));
  return {
    txB64: d.tx_b64,
    signatures: d.signatures,
    objects: d.objects.map((o) => o.bcs_b64),
    params: {
      protocol_version: d.protocol_version,
      reference_gas_price: d.reference_gas_price,
      epoch_id: d.epoch_id,
      epoch_timestamp_ms: d.epoch_timestamp_ms,
    },
    mode: "dev-inspect",
    verify: true,
    ...over,
  };
}

// A gasless transaction signed by `sender`, optionally sponsored by a different
// gas owner (who also signs). `shape(tx)` adds the commands.
async function liveTx(sender, sponsor, shape) {
  const tx = new Transaction();
  shape(tx);
  tx.setSender(sender.getPublicKey().toIotaAddress());
  tx.setGasOwner((sponsor ?? sender).getPublicKey().toIotaAddress());
  tx.setGasPayment([]); // gasless: the VM mints a mock gas coin
  tx.setGasPrice(1000); // == reference gas price on every IOTA network
  tx.setGasBudget(5_000_000);
  const bytes = await tx.build({ client });
  const signatures = [(await sender.signTransaction(bytes)).signature];
  if (sponsor) {
    signatures.push((await sponsor.signTransaction(bytes)).signature);
  }
  return { txB64: b64(bytes), signatures };
}

const sender = keypair(3);
const sponsor = keypair(101);

// A signed no-op: one signature, empty PTB, succeeds.
const noop = await liveTx(sender, null, () => {});

// A sponsored no-op: sender + sponsor signatures, both verify, succeeds.
const sponsored = await liveTx(sender, sponsor, () => {});

// The same no-op, but with a corrupted Ed25519 signature: flip a byte in the
// 64-byte signature body while leaving the public key intact, so it is the
// right signer with a signature that does not verify.
const badSignature = (() => {
  const sig = Buffer.from(noop.signatures[0], "base64"); // [flag][64 sig][32 pk]
  sig[1] ^= 0xff;
  return { txB64: noop.txB64, signatures: [sig.toString("base64")] };
})();

// Splits more than the mock gas coin holds, then transfers the result — the
// split aborts with InsufficientCoinBalance, so the body fails while the
// signature still verifies.
const abort = await liveTx(sender, null, (tx) => {
  const [coin] = tx.splitCoins(tx.gas, [
    tx.pure.u64(2_000_000_000_000_000_000n), // > the 1B-IOTA mock coin
  ]);
  tx.transferObjects([coin], tx.pure.address(
    sender.getPublicKey().toIotaAddress(),
  ));
});

const presets = [
  {
    id: "noop",
    label: "Signed no-op",
    note: "One signature over an empty transaction — verifies and succeeds.",
    ...noop,
    mode: "dev-inspect",
    verify: true,
  },
  {
    id: "sponsor",
    label: "Sender + sponsor",
    note:
      "Sponsored transaction: two signatures (sender, then sponsor gas owner) — both verify and it succeeds.",
    ...sponsored,
    mode: "dev-inspect",
    verify: true,
  },
  {
    id: "move-auth-valid",
    label: "MoveAuthenticator ✓",
    note:
      "Authenticates via an on-chain Move function (authenticate_ed25519) that accepts — signature verified, succeeds. Runs offline from bundled objects.",
    ...fromFixture("move_auth_ed25519_valid.json"),
  },
  {
    id: "bad-signature",
    label: "Invalid signature",
    note:
      "A valid transaction carrying a corrupted Ed25519 signature — the signer is right but the signature does not verify, so it is rejected before execution.",
    ...badSignature,
    mode: "dev-inspect",
    verify: true,
  },
  {
    id: "move-auth-invalid",
    label: "MoveAuthenticator ✗",
    note:
      "The Move authenticator function rejects a bogus signature — signature fails and execution does not succeed. Runs offline from bundled objects.",
    ...fromFixture("move_auth_ed25519_invalid.json"),
  },
  {
    id: "abort",
    label: "Aborts in execution",
    note:
      "Signature verifies, but the transaction splits more than the gas coin holds — execution aborts with InsufficientCoinBalance.",
    ...abort,
    mode: "dev-inspect",
    verify: true,
  },
];

writeFileSync(join(here, "presets.json"), JSON.stringify(presets, null, 2) + "\n");
console.log(`Wrote presets.json with ${presets.length} presets:`);
for (const p of presets) {
  console.log(
    `  ${p.id.padEnd(18)} ${p.signatures.length} sig(s)` +
      (p.objects ? `, ${p.objects.length} objects (offline)` : ", live"),
  );
}
