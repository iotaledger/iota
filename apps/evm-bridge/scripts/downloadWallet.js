import { execSync } from 'child_process';
import { join } from 'path';
import { fileURLToPath } from 'url';
import { dirname } from 'path';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

const isLayer1 = process.env.L1 === 'true';
const token = process.env.GITHUB_TOKEN;

if (isLayer1 && !token) {
    console.error('Error: GITHUB_TOKEN not found in environment variables');
    process.exit(1);
}

try {
    const scriptName = isLayer1
        ? 'download_wallet_artifact_L1.sh'
        : 'download_wallet_artifact_L2.sh';
    const scriptPath = join(__dirname, scriptName);

    execSync(`bash ${scriptPath}`, {
        env: isLayer1 ? { ...process.env, GITHUB_TOKEN: token } : null,
        stdio: 'inherit',
    });
} catch (error) {
    console.error('Failed to download wallet artifact:', error);
    process.exit(1);
}
