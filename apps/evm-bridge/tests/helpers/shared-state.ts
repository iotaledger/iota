// helpers/shared-state.ts
import fs from 'fs';
import path from 'path';
import { STATE_DIR, STATE_FILE } from './paths';

export type WalletState = {
    extensionIdL1: string;
    extensionIdL2: string;
    addressL1: string;
    addressL2: string;
    mnemonicL1: string;
    mnemonicL2: string;
    createdAt: string;
};

export function getSharedState(): WalletState {
    try {
        if (!fs.existsSync(STATE_FILE)) {
            throw new Error('Wallet state file not found. Did global setup run?');
        }
        const data = fs.readFileSync(STATE_FILE, 'utf8');
        return JSON.parse(data) as WalletState;
    } catch (e) {
        console.error('Failed to read wallet state:', e);
        throw new Error('Failed to read wallet state. Make sure global setup has run.');
    }
}

export function getUserDataPaths() {
    return {
        userDataDirL1: path.join(STATE_DIR, 'user-data', 'l1'),
        userDataDirL2: path.join(STATE_DIR, 'user-data', 'l2'),
    };
}
