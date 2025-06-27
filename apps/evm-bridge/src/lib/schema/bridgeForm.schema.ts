import { z } from 'zod';
import { BridgeFormInputName } from '../enums';
import { parseAmount } from '../utils';
import { isAddress, parseEther } from 'viem';
import { IOTA_TYPE_ARG, isValidIotaAddress } from '@iota/iota-sdk/utils';
import BigNumber from 'bignumber.js';
import { MINIMUM_SEND_AMOUNT } from '../constants';

export function createBridgeFormSchema(totalAccountBalance: bigint, coinDecimals: number) {
    return z
        .object({
            [BridgeFormInputName.IsFromLayer1]: z.boolean().default(true),
            [BridgeFormInputName.CoinType]: z.string().default(IOTA_TYPE_ARG),
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
                ),
            // .refine(
            //     (value) => {
            //         const amount = isFromLayer1
            //             ? parseAmount(value, coinDecimals)
            //             : parseEther(value);
            //         return amount ? amount <= totalAccountBalance : false;
            //     },
            //     {
            //         message: 'Insufficient balance',
            //     },
            // ),
            [BridgeFormInputName.ReceivingAddress]: z.string().trim(),
            // .refine(
            //     (address) => (isFromLayer1 ? isAddress(address) : isValidIotaAddress(address)),
            //     {
            //         message: 'Invalid address',
            //     },
            // ),
        })
        .required()
        .superRefine((data, ctx) => {
            // Access isFromLayer1 from the form data
            const isFromLayer1 = data[BridgeFormInputName.IsFromLayer1];

            // Validate deposit amount using the form's isFromLayer1 value
            const value = data[BridgeFormInputName.DepositAmount];
            if (value) {
                const amount = isFromLayer1 ? parseAmount(value, coinDecimals) : parseEther(value);

                if (!amount || amount > totalAccountBalance) {
                    ctx.addIssue({
                        code: z.ZodIssueCode.custom,
                        message: 'Insufficient balance',
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
