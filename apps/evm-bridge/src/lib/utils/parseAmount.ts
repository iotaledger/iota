import { BigNumber } from 'bignumber.js';

export function parseAmount(amount: string, coinDecimals: number) {
    try {
        return BigInt(new BigNumber(amount).shiftedBy(coinDecimals).integerValue().toString());
    } catch {
        return null;
    }
}
