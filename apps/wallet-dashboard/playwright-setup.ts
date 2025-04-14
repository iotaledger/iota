// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { execSync } from 'child_process';

async function globalSetup() {
    try {
        console.log('Building wallet...');
        !process.env.CI && execSync('pnpm -w wallet build', { stdio: 'inherit' });
        console.log('Building wallet-dashboard!');
        !process.env.CI && execSync('pnpm -w wallet-dashboard build', { stdio: 'inherit' });
        console.log('Setup complete!');
    } catch (error) {
        console.error('Setup failed:', error);
        process.exit(1);
    }
}

export default globalSetup;
