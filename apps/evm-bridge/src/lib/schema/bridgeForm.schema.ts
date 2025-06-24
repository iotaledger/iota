import { z } from 'zod';
import { BridgeFormInputName } from '../enums';
import { parseAmount } from '../utils';
import { isAddress, parseEther } from 'viem';
import { isValidIotaAddress } from '@iota/iota-sdk/utils';
import BigNumber from 'bignumber.js';
import { MINIMUM_SEND_AMOUNT } from '../constants';

export function createBridgeFormSchema(
    totalAccountBalance: bigint,
    coinDecimals: number,
    isFromLayer1: boolean,
) {
    return z
        .object({
            [BridgeFormInputName.DepositAmount]: z
                .string()
                .trim()
                .refine(
                    (value) => {
                        return new BigNumber(value).isGreaterThanOrEqualTo(MINIMUM_SEND_AMOUNT);
                    },
                    {
                        message: 'Invalid amount',
                    },
                )
                .refine(
                    (value) => {
                        const amount = isFromLayer1
                            ? parseAmount(value, coinDecimals)
                            : parseEther(value);
                        return amount ? amount <= totalAccountBalance : false;
                    },
                    {
                        message: 'Insufficient balance',
                    },
                ),
            [BridgeFormInputName.ReceivingAddress]: z
                .string()
                .trim()
                .refine(
                    (address) => (isFromLayer1 ? isAddress(address) : isValidIotaAddress(address)),
                    {
                        message: 'Invalid address',
                    },
                ),
        })
        .required();
}

export type DepositFormData = z.infer<ReturnType<typeof createBridgeFormSchema>>;
