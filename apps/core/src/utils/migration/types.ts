// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { z } from 'zod';
import { STARDUST_PACKAGE_ID } from '../../constants';

const ExpirationUnlockConditionSchema = z.object({
    type: z.literal(
        `${STARDUST_PACKAGE_ID}::expiration_unlock_condition::ExpirationUnlockCondition`,
    ),
    fields: z.object({
        owner: z.string(),
        return_address: z.string(),
        unix_time: z.number(),
    }),
});

const StorageDepositReturnUnlockConditionSchema = z.object({
    type: z.literal(
        `${STARDUST_PACKAGE_ID}::storage_deposit_return_unlock_condition::StorageDepositReturnUnlockCondition`,
    ),
    fields: z.object({
        return_address: z.string(),
        return_amount: z.string(),
    }),
});

const TimelockUnlockConditionSchema = z.object({
    type: z.literal(`${STARDUST_PACKAGE_ID}::timelock_unlock_condition::TimelockUnlockCondition`),
    fields: z.object({
        unix_time: z.number(),
    }),
});

const CommonOutputObjectSchema = z.object({
    id: z.object({
        id: z.string(),
    }),
    balance: z.string(),
    native_tokens: z.object({
        type: z.literal('0x2::bag::Bag'),
        fields: z.object({
            id: z.object({
                id: z.string(),
            }),
            size: z.string(),
        }),
    }),
});

const CommonOutputObjectWithUcSchema = CommonOutputObjectSchema.extend({
    expiration_uc: ExpirationUnlockConditionSchema.nullable().optional(),
    storage_deposit_return_uc: StorageDepositReturnUnlockConditionSchema.nullable().optional(),
    timelock_uc: TimelockUnlockConditionSchema.nullable().optional(),
});

export const BasicOutputObjectSchema = CommonOutputObjectWithUcSchema.extend({
    metadata: z.array(z.number()).nullable().optional(),
    tag: z.array(z.number()).nullable().optional(),
    sender: z.string().nullable().optional(),
});

export const NftOutputObjectSchema = CommonOutputObjectWithUcSchema;

export type ExpirationUnlockCondition = z.infer<typeof ExpirationUnlockConditionSchema>;
export type StorageDepositReturnUnlockCondition = z.infer<
    typeof StorageDepositReturnUnlockConditionSchema
>;
export type TimelockUnlockCondition = z.infer<typeof TimelockUnlockConditionSchema>;
export type CommonOutputObject = z.infer<typeof CommonOutputObjectSchema>;
export type CommonOutputObjectWithUc = z.infer<typeof CommonOutputObjectWithUcSchema>;
export type BasicOutputObject = z.infer<typeof BasicOutputObjectSchema>;
export type NftOutputObject = z.infer<typeof NftOutputObjectSchema>;
