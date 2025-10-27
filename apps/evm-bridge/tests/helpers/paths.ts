import path from 'path';

export const TEST_DIR = path.join(__dirname, '..');
export const STATE_DIR = path.join(TEST_DIR, '.state');
export const STATE_FILE = path.join(STATE_DIR, 'wallet-state.json');
