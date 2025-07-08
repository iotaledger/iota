import { execSync } from 'child_process';
import { join } from 'path';
import { fileURLToPath } from 'url';
import { dirname } from 'path';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

try {
    const scriptName = 'download_wallet_artifact_L2.sh';
    const scriptPath = join(__dirname, scriptName);

    execSync(`bash ${scriptPath}`, {
        stdio: 'inherit',
    });
} catch (error) {
    console.error('Failed to download wallet artifact:', error);
    process.exit(1);
}
