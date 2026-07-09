// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

const axios = require('axios');
const fs = require('fs');
const path = require('path');

// Create directory

const topdir = path.join(__dirname, "../open-spec");

if (!fs.existsSync(topdir)){
    fs.mkdirSync(topdir);
}

// Spec bundled in this repository, used as a fallback when the remote download
// fails so the build never ends up with a missing `openrpc.json` import.
const localSpec = path.join(__dirname, "../../../../crates/iota-open-rpc/spec/openrpc.json");

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

const branches = ["mainnet", "testnet", "devnet"];

const fetchSpec = async (branch) => {
    const url = `https://raw.githubusercontent.com/iotaledger/iota/${branch}/crates/iota-open-rpc/spec/openrpc.json`;
    const maxAttempts = 4;
    for (let attempt = 1; attempt <= maxAttempts; attempt++) {
        try {
            const res = await axios({
                method: "get",
                url,
                responseType: "text",
                timeout: 30000,
            });
            return res.data;
        } catch (err) {
            console.log(`Attempt ${attempt}/${maxAttempts} to download ${branch} openrpc spec failed: ${err.message}`);
            if (attempt < maxAttempts) {
                await sleep(2000 * attempt);
            }
        }
    }
    return null;
};

const downloadFile = async (branch) => {
    const branchdir = path.join(topdir, branch);
    if (!fs.existsSync(branchdir)){
        fs.mkdirSync(branchdir);
    }
    const specPath = path.join(branchdir, "openrpc.json");
    const backupPath = path.join(branchdir, "openrpc_backup.json");

    const data = await fetchSpec(branch);

    if (data !== null) {
        if (fs.existsSync(specPath)) {
            fs.renameSync(specPath, backupPath);
        }
        fs.writeFileSync(specPath, data, 'utf8');
        console.log(`Downloaded ${branch} openrpc spec.`);
        return;
    }

    // Download failed after retries. Keep whatever we already have; otherwise
    // fall back to the spec bundled in this repository so the build can proceed.
    if (fs.existsSync(specPath)) {
        console.log(`Using existing ${branch} openrpc spec after failed download.`);
        return;
    }
    if (fs.existsSync(localSpec)) {
        fs.copyFileSync(localSpec, specPath);
        console.log(`Using bundled repository openrpc spec for ${branch} after failed download.`);
        return;
    }

    throw new Error(`Could not download ${branch} openrpc spec and no fallback is available.`);
};

Promise.all(branches.map(downloadFile)).catch((err) => {
    console.error(err);
    process.exit(1);
});
