import fs from 'fs';
import { existsSync } from 'fs';
import { STATE_FILE } from '../helpers/paths';

async function globalTeardown() {
    console.log('🧹 Cleaning up wallet data for next run');

    // Clean up state file
    if (existsSync(STATE_FILE)) {
        console.log(`- Removing wallet state file: ${STATE_FILE}`);
        try {
            fs.unlinkSync(STATE_FILE);
        } catch (error) {
            console.error(`Failed to delete state file: ${error}`);
        }
    }

    console.log('✅ Cleanup complete - next test run will create fresh wallets');
}

export default globalTeardown;
