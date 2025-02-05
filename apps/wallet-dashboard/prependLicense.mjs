// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { readFile, writeFile } from 'fs/promises';

const LICENSE = '// Copyright (c) 2024 IOTA Stiftung\n// SPDX-License-Identifier: Apache-2.0\n\n';

async function prependLicense(filename) {
    const content = await readFile(filename, 'utf8');
    writeFile(filename, LICENSE + content);
}

prependLicense('lib/utils/analytics/ampli/index.ts');
