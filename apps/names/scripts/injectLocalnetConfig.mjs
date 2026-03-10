// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { readFileSync, writeFileSync } from 'fs';
import { dirname, join } from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

const LOCALNET_JSON_PATH = join(__dirname, '../../scripts/package-info/localnet.json');
const CONSTANTS_TS_PATH = join(__dirname, '../../sdk/src/constants.ts');
const PLACEHOLDER = 'REPLACE PLACEHOLDER';

try {
    const localnetConfig = JSON.parse(readFileSync(LOCALNET_JSON_PATH, 'utf-8'));
    const localnetConfigStr = JSON.stringify(localnetConfig);

    const constantsContent = readFileSync(CONSTANTS_TS_PATH, 'utf-8');

    const lines = constantsContent.split('\n');
    const placeholderIndex = lines.findIndex((line) => line.includes(PLACEHOLDER));

    if (placeholderIndex === -1) {
        throw new Error(`Could not find "${PLACEHOLDER}" in constants.ts`);
    }

    lines.splice(placeholderIndex, 0, `    localnet: ${localnetConfigStr},`);

    writeFileSync(CONSTANTS_TS_PATH, lines.join('\n'), 'utf-8');

    console.log('✅ Injected localnet config into constants.ts');
} catch (error) {
    console.error('❌ Error injecting localnet config:', error.message);
    process.exit(1);
}
