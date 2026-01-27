// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { z } from 'zod';
import { BridgeFormInputName } from '../enums';
import { isAddress } from 'viem';
import { IOTA_TYPE_ARG, isValidIotaAddress, parseAmount } from '@iota/iota-sdk/utils';
import BigNumber from 'bignumber.js';
import { MINIMUM_SEND_AMOUNT } from '../constants';
import { CoinBalance, CoinMetadata } from '@iota/iota-sdk/client';

export function createBridgeFormSchema(
    coinBalancesL1: CoinBalance[],
    coinBalancesL2: CoinBalance[],
    coinsMetadataL1: Record<string, CoinMetadata | null>,
    coinsMetadataL2: Record<string, CoinMetadata | null>,
) {
    return z
        .object({
            [BridgeFormInputName.IsFromLayer1]: z.boolean().default(true),
            [BridgeFormInputName.IsDepositAddressManualInput]: z.boolean().default(false),
            [BridgeFormInputName.CoinType]: z.string().default(IOTA_TYPE_ARG),
            [BridgeFormInputName.ReceivingAddress]: z.string().trim(),
            [BridgeFormInputName.DepositAmount]: z
                .string()
                .trim()
                .refine(
                    (value) => {
                        return new BigNumber(value).isGreaterThanOrEqualTo(0);
                    },
                    {
                        message: 'Please enter a valid amount greater than 0',
                    },
                ),
        })
        .required()
        .superRefine((data, ctx) => {
            const value = data[BridgeFormInputName.DepositAmount];
            const isFromLayer1 = data[BridgeFormInputName.IsFromLayer1];
            const selectedCoinType = data[BridgeFormInputName.CoinType];

            const coinMetadata = isFromLayer1
                ? coinsMetadataL1[selectedCoinType]
                : coinsMetadataL2[selectedCoinType];

            const coinBalances = isFromLayer1 ? coinBalancesL1 : coinBalancesL2;
            const availableBalance =
                coinBalances.find((balance) => balance.coinType === selectedCoinType)
                    ?.totalBalance || '0';

            if (!coinMetadata) {
                ctx.addIssue({
                    code: z.ZodIssueCode.custom,
                    message: 'Invalid coin type',
                    path: [BridgeFormInputName.CoinType],
                });
                return;
            }

            if (value) {
                const coinDecimals = coinMetadata.decimals;
                const amount = parseAmount(value, coinDecimals);

                if (!amount || amount > BigInt(availableBalance)) {
                    ctx.addIssue({
                        code: z.ZodIssueCode.custom,
                        message: `Insufficient balance.`,
                        path: [BridgeFormInputName.DepositAmount],
                    });
                }

                // Validate minimum send amount for IOTA
                if (selectedCoinType === IOTA_TYPE_ARG && Number(value) < MINIMUM_SEND_AMOUNT) {
                    ctx.addIssue({
                        code: z.ZodIssueCode.custom,
                        message: `Minimum amount for IOTA is ${MINIMUM_SEND_AMOUNT}`,
                        path: [BridgeFormInputName.DepositAmount],
                    });
                }
            }

            // Validate address based on isFromLayer1
            const address = data[BridgeFormInputName.ReceivingAddress];
            if (address && !(isFromLayer1 ? isAddress(address) : isValidIotaAddress(address))) {
                ctx.addIssue({
                    code: z.ZodIssueCode.custom,
                    message: 'Invalid address',
                    path: [BridgeFormInputName.ReceivingAddress],
                });
            }
        });
}

export type DepositFormData = z.infer<ReturnType<typeof createBridgeFormSchema>>;
