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

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

const branches = ["mainnet", "testnet", "devnet"];

const fetchSpec = async (branch) => {
    const url = `https://raw.githubusercontent.com/iotaledger/iota/${branch}/crates/iota-open-rpc/spec/openrpc.json`;
    const maxAttempts = 4;
    let lastError;
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
            lastError = err;
            console.log(`Attempt ${attempt}/${maxAttempts} to download ${branch} openrpc spec failed: ${err.message}`);
            if (attempt < maxAttempts) {
                await sleep(2000 * attempt);
            }
        }
    }
    throw new Error(`Could not download ${branch} openrpc spec after ${maxAttempts} attempts: ${lastError.message}`);
};

const downloadFile = async (branch) => {
    const branchdir = path.join(topdir, branch);
    if (!fs.existsSync(branchdir)){
        fs.mkdirSync(branchdir);
    }
    const data = await fetchSpec(branch);
    fs.writeFileSync(path.join(branchdir, "openrpc.json"), data, 'utf8');
    console.log(`Downloaded ${branch} openrpc spec.`);
};

Promise.all(branches.map(downloadFile)).catch((err) => {
    console.error(err);
    process.exit(1);
});
