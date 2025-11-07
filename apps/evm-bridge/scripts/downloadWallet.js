import { execSync } from 'child_process';
import { join } from 'path';
import { fileURLToPath } from 'url';
import { dirname } from 'path';
import pkg from '../package.json';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

const defaultVersion = '12.20.0';
const metamaskVersion = pkg?.config?.metamaskVersion || defaultVersion;

try {
    const scriptName = 'download_wallet_artifact_L2.sh';
    const scriptPath = join(__dirname, scriptName);

    execSync(`bash ${scriptPath}`, {
        stdio: 'inherit',
        env: {
            METAMASK_VERSION: metamaskVersion,
        },
    });
} catch (error) {
    console.error('Failed to download wallet artifact:', error);
    process.exit(1);
}
