// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { execSync } from 'child_process';

async function globalSetup() {
    console.log('Running pre-test setup...');

    // Run your commands here
    try {
        // Build dependencies first
        console.log('Building wallet...');
        execSync('pnpm -w wallet build', { stdio: 'inherit' });

        // Any other commands you need to run
        console.log('Setup complete!');
    } catch (error) {
        console.error('Setup failed:', error);
        process.exit(1);
    }
}

export default globalSetup;
