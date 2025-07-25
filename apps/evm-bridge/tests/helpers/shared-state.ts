// helpers/shared-state.ts
import fs from 'fs';
import { STATE_FILE } from './paths';
import { TestWalletData } from '../utils/utils';

export type WalletState = {
    tests: {
        sendMaxIotaAmountL1: TestWalletData;
        sendMaxIotaAmountL2: TestWalletData;
        sendMaxNativeTokenAmountL1: TestWalletData;
        sendMaxNativeTokenAmountL2: TestWalletData;
        depositThenWithdraw: TestWalletData;
        depositThenWithdrawNativeToken: TestWalletData;
    };
    // Metadata
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
