// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { z } from 'zod';
import {
    STARDUST_EXPIRATION_UNLOCK_CONDITION_TYPE,
    STARDUST_STORAGE_DEPOSIT_RETURN_UC_TYPE,
    STARDUST_TIMELOCK_TYPE,
} from '../../constants';
import { normalizeStructTag } from '@iota/iota-sdk/utils';

const ExpirationUnlockConditionSchema = z.object({
    type: z
        .string()
        .refine((val) => normalizeStructTag(val) === STARDUST_EXPIRATION_UNLOCK_CONDITION_TYPE),
    fields: z.object({
        owner: z.string(),
        return_address: z.string(),
        unix_time: z.number(),
    }),
});

const StorageDepositReturnUnlockConditionSchema = z.object({
    type: z
        .string()
        .refine((val) => normalizeStructTag(val) === STARDUST_STORAGE_DEPOSIT_RETURN_UC_TYPE),
    fields: z.object({
        return_address: z.string(),
        return_amount: z.string(),
    }),
});

const TimelockUnlockConditionSchema = z.object({
    type: z.string().refine((val) => normalizeStructTag(val) === STARDUST_TIMELOCK_TYPE),
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

const StardustIndexerOutputSchema = z.object({
    id: z.string(),
    balance: z.object({
        value: z.number(),
    }),
    native_tokens: z.object({
        id: z.string(),
        size: z.number(),
    }),
    storage_deposit_return: z
        .object({
            return_address: z.string(),
            return_amount: z.number(),
        })
        .nullable(),
    timelock: z
        .object({
            unix_time: z.number(),
        })
        .nullable(),
    expiration: z
        .object({
            owner: z.string(),
            return_address: z.string(),
            unix_time: z.number(),
        })
        .nullable(),
    metadata: z.array(z.number()).nullable(),
    tag: z.string().nullable(),
    sender: z.string().nullable(),
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
export type StardustIndexerOutput = z.infer<typeof StardustIndexerOutputSchema>;
