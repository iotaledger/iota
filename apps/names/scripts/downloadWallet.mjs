// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { execSync } from 'child_process';
import { dirname, join } from 'path';
import { fileURLToPath } from 'url';
import dotenv from 'dotenv';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

dotenv.config({ path: join(__dirname, '.env') });
dotenv.config({ path: join(__dirname, '.env.local'), override: true });

const token = process.env.GITHUB_TOKEN;

if (!token) {
    console.error('Error: GITHUB_TOKEN not found in environment variables');
    process.exit(1);
}

try {
    const scriptPath = join(__dirname, 'download_wallet_artifact.sh');

    execSync(`bash ${scriptPath}`, {
        env: { ...process.env, GITHUB_TOKEN: token },
        stdio: 'inherit',
    });
} catch (error) {
    console.error('Failed to download wallet artifact:', error);
    process.exit(1);
}
