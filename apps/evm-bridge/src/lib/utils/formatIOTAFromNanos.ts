import { IOTA_DECIMALS } from '@iota/iota-sdk/utils';
import BigNumber from 'bignumber.js';

export function formatIOTAFromNanos(amount: bigint): string {
    return new BigNumber(amount.toString()).dividedBy(10 ** IOTA_DECIMALS).toString();
}
