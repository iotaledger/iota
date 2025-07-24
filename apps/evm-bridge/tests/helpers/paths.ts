// Create a new file: tests/utils/paths.ts
import path from 'path';

// Use tests directory as base for .state folder
export const TEST_DIR = path.join(__dirname, '..');
export const STATE_DIR = path.join(TEST_DIR, '.state');
export const STATE_FILE = path.join(STATE_DIR, 'wallet-state.json');
export const USER_DATA_DIR = path.join(STATE_DIR, 'user-data');
export const USER_DATA_DIR_L1 = path.join(USER_DATA_DIR, 'l1');
export const USER_DATA_DIR_L2 = path.join(USER_DATA_DIR, 'l2');
