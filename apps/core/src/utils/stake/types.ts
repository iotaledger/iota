// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { z } from 'zod';
import { TIMELOCK_IOTA_TYPE } from '../../constants/timelock.constants';

const UidSchema = z.object({
    id: z.string(),
});

const BalanceSchema = z.object({
    value: z.bigint(),
});

export const TimelockedObjectSchema = z.object({
    id: UidSchema,
    locked: BalanceSchema,
    expirationTimestampMs: z.number(),
    label: z.string().optional(),
});

export const TimelockedObjectFieldsSchema = z.object({
    id: UidSchema,
    locked: z.string(),
    expiration_timestamp_ms: z.string(),
    label: z.string().optional(),
});

const TimelockedObjectContentSchema = z.object({
    dataType: z.string().optional(),
    type: z.literal(TIMELOCK_IOTA_TYPE).optional(),
    fields: TimelockedObjectFieldsSchema,
});

export const TimelockedIotaObjectSchema = z.object({
    objectId: z.string(),
    version: z.string(),
    digest: z.string(),
    type: z.literal(TIMELOCK_IOTA_TYPE),
    display: z.object({
        data: z.nullable(z.any()),
        error: z.nullable(z.any()),
    }),
    content: TimelockedObjectContentSchema.optional(),
});

const BaseTimelockedStakeSchema = z.object({
    expirationTimestampMs: z.string(),
    label: z.string().nullable().optional(),
    principal: z.string(),
    stakeActiveEpoch: z.string(),
    stakeRequestEpoch: z.string(),
    timelockedStakedIotaId: z.string(),
});

const PendingTimelockedStakeSchema = BaseTimelockedStakeSchema.extend({
    status: z.literal('Pending'),
});

const ActiveTimelockedStakeSchema = BaseTimelockedStakeSchema.extend({
    status: z.literal('Active'),
    estimatedReward: z.string(),
});

const UnstakedTimelockedStakeSchema = BaseTimelockedStakeSchema.extend({
    status: z.literal('Unstaked'),
});

const TimelockedStakeSchema = z.discriminatedUnion('status', [
    PendingTimelockedStakeSchema,
    ActiveTimelockedStakeSchema,
    UnstakedTimelockedStakeSchema,
]);

export const DelegatedTimelockedStakeSchema = z.object({
    validatorAddress: z.string().min(1, { message: 'Validator address cannot be empty' }),
    stakingPool: z.string().min(1, { message: 'Staking pool cannot be empty' }),
    stakes: z.array(TimelockedStakeSchema),
});
