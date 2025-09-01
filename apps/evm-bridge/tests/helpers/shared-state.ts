import fs from 'fs';
import { STATE_FILE } from './paths';
import { TestWalletData } from '../utils/utils';

export type WalletState = {
    global: {
        addressL1: string;
        mnemonicL1: string;
    };
    tests: {
        sendMaxIotaAmountL1: TestWalletData;
        sendMaxIotaAmountL2: TestWalletData;
        sendMaxNativeTokenAmountL1: TestWalletData;
        sendMaxNativeTokenAmountL2: TestWalletData;
        depositThenWithdrawIota: TestWalletData;
        depositThenWithdrawNativeToken: TestWalletData;
    };
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

export function getTestData<T extends keyof WalletState['tests']>(
    testId: T,
): WalletState['tests'][T] {
    const state = getSharedState();
    const testData = state.tests[testId];

    if (!testData) {
        throw new Error(`No test data found for ID: ${testId}`);
    }

    return testData;
}
