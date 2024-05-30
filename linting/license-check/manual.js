// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

const readline = require('readline');
const fs = require('node:fs')

const IOTA_COPYRIGHT_HEADER = '// Copyright (c) 2024 IOTA Stiftung';
const OLD_COPYRIGHT_HEADER = '// Copyright (c) Mysten Labs, Inc.';
const MODIFICATION_COPYRIGHT_HEADER = '// Modifications Copyright (c) 2024 IOTA Stiftung';
const LICENSE_IDENTIFIER = '// SPDX-License-Identifier: Apache-2.0';

const IOTA_LICENSE_HEADER = `${IOTA_COPYRIGHT_HEADER}\n${LICENSE_IDENTIFIER}`;
const MODIFICATION_HEADER = `${MODIFICATION_COPYRIGHT_HEADER}\n${LICENSE_IDENTIFIER}`;

function applyHeader(path) {
    let sourceCode = fs.readFileSync(path).toString();

    const hasIotaCopyrightHeader = sourceCode.includes(IOTA_COPYRIGHT_HEADER) || sourceCode.includes(MODIFICATION_COPYRIGHT_HEADER);
    const hasOldCopyrightHeader = sourceCode.includes(OLD_COPYRIGHT_HEADER);

    // No need to add or modify any header
    if (hasIotaCopyrightHeader) {
        return;
    }

    // Put the header between the old header and the  source
    if (hasOldCopyrightHeader) {
        sourceCode = sourceCode.replace(
            `${OLD_COPYRIGHT_HEADER}\n${LICENSE_IDENTIFIER}`,
             `${OLD_COPYRIGHT_HEADER}\n${LICENSE_IDENTIFIER}\n\n${MODIFICATION_HEADER}`
        );
        fs.writeFileSync(path, sourceCode)
    } 
    // Simply put the header in the top
    else {
        fs.writeFileSync(path, `${IOTA_LICENSE_HEADER}\n${sourceCode}`)
    }
}

const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout,
    terminal: false
});

rl.on('line', (file) => {
    console.log("patching...", file)
    try {
        applyHeader(file);
    } catch (err) {
        console.log("error...", file, err)
    }
});

rl.once('close', () => {
    console.log("Finished");
});