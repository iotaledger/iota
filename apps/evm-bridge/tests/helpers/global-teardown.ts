// tests/helpers/global-teardown.ts
import fs from 'fs';
import { rmSync, existsSync } from 'fs';
import { STATE_FILE, USER_DATA_DIR, USER_DATA_DIR_L1, USER_DATA_DIR_L2 } from '../helpers/paths';

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

    // Clean up user data directories
    [USER_DATA_DIR_L1, USER_DATA_DIR_L2].forEach((dir) => {
        if (existsSync(dir)) {
            console.log(`- Removing wallet data directory: ${dir}`);
            try {
                rmSync(dir, { recursive: true, force: true });
            } catch (error) {
                console.error(`Failed to delete directory ${dir}: ${error}`);
            }
        }
    });

    // Optionally, clean the entire state directory if it's empty
    try {
        const items = fs.readdirSync(USER_DATA_DIR);
        if (items.length === 0) {
            fs.rmdirSync(USER_DATA_DIR);
        }
    } catch (error) {
        // Ignore errors here
    }

    console.log('✅ Cleanup complete - next test run will create fresh wallets');
}

export default globalTeardown;
