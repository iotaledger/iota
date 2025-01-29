// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { readFile, writeFile } from 'fs/promises';

const LICENSE =
    '// Copyright (c) Mysten Labs, Inc.\n// Modifications Copyright (c) 2024 IOTA Stiftung\n// SPDX-License-Identifier: Apache-2.0\n\n';

async function prependLicense(filename) {
    const content = await readFile(filename, 'utf8');
    writeFile(filename, LICENSE + content);
}

prependLicense('src/lib/utils/analytics/ampli/index.ts');
