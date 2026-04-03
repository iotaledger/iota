// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { MIN_NUMBER_IOTA_TO_STAKE } from '../../constants';
import { CoinFormat, formatBalance } from '@iota/iota-sdk/utils';
import BigNumber from 'bignumber.js';
import { mixed, object } from 'yup';

export function createUnstakeValidationSchema(
    principalAmount: bigint,
    coinSymbol: string,
    decimals: number,
    minimumStake: bigint,
) {
    return object({
        amount: mixed<BigNumber>()
            .transform((_, original) => {
                return new BigNumber(original);
            })
            .test('required', `\${path} is a required field`, (value) => {
                return !!value;
            })
            .test('valid', 'The value provided is not valid.', (value) => {
                if (!value || value.isNaN() || !value.isFinite()) {
                    return false;
                }
                return true;
            })
            .test(
                'min',
                `Unstake amount must be at least ${MIN_NUMBER_IOTA_TO_STAKE} ${coinSymbol}`,
                (amount) =>
                    amount ? amount.shiftedBy(decimals).gte(minimumStake.toString()) : false,
            )
            .test('max', (amount, ctx) => {
                const amountNanos = amount ? BigInt(amount.shiftedBy(decimals).toFixed(0)) : 0n;

                if (amountNanos > principalAmount) {
                    return ctx.createError({
                        message: `\${path} must be less than ${formatBalance(
                            principalAmount,
                            decimals,
                            CoinFormat.Full,
                        )} ${coinSymbol}`,
                    });
                }
                return true;
            })
            .test('remaining', (amount, ctx) => {
                const amountNanos = amount ? BigInt(amount.shiftedBy(decimals).toFixed(0)) : 0n;
                const remaining = principalAmount - amountNanos;

                if (remaining > 0n && remaining < minimumStake) {
                    return ctx.createError({
                        message: `Remaining stake must be at least ${MIN_NUMBER_IOTA_TO_STAKE} ${coinSymbol}`,
                    });
                }
                return true;
            })
            .test(
                'max-decimals',
                `The value exceeds the maximum decimals (${decimals}).`,
                (amount) => {
                    return amount ? amount.shiftedBy(decimals).isInteger() : false;
                },
            )
            .label('Amount'),
    });
}
