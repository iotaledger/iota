// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { DelegatedTimelockedStake, IotaObjectData } from '@iota/iota-sdk/client';
import {
    SUPPLY_INCREASE_INVESTOR_VESTING_DURATION,
    SUPPLY_INCREASE_STAKER_VESTING_DURATION,
    SUPPLY_INCREASE_STARTING_VESTING_YEAR,
    SUPPLY_INCREASE_VESTING_LABEL,
    SUPPLY_INCREASE_VESTING_PAYOUTS_IN_1_YEAR,
    SUPPLY_INCREASE_VESTING_PAYOUT_SCHEDULE_MILLISECONDS,
} from '../../constants';
import {
    SupplyIncreaseUserType,
    SupplyIncreaseVestingPayout,
    SupplyIncreaseVestingPortfolio,
    TimelockedIotaResponse,
    TimelockedObject,
    TimelockedStakedIota,
    VestingOverview,
} from '../../interfaces';
import { isTimelockedObject, isTimelockedStakedIota } from '../timelock';

export function getLastSupplyIncreaseVestingPayout(
    objects: (TimelockedObject | TimelockedStakedIota)[],
): SupplyIncreaseVestingPayout | undefined {
    const vestingObjects = objects.filter(isSupplyIncreaseVestingObject);

    if (vestingObjects.length === 0) {
        return undefined;
    }

    const vestingPayoutMap = supplyIncreaseVestingObjectsToPayoutMap(vestingObjects);

    const payouts: SupplyIncreaseVestingPayout[] = Array.from(vestingPayoutMap.values());

    return payouts.sort((a, b) => b.expirationTimestampMs - a.expirationTimestampMs)[0];
}

function supplyIncreaseVestingObjectsToPayoutMap(
    vestingObjects: (TimelockedObject | TimelockedStakedIota)[],
): Map<number, SupplyIncreaseVestingPayout> {
    const expirationToVestingPayout = new Map<number, SupplyIncreaseVestingPayout>();

    for (const vestingObject of vestingObjects) {
        let objectValue = 0;
        if (isTimelockedObject(vestingObject)) {
            objectValue = (vestingObject as TimelockedObject).locked.value;
        } else if (isTimelockedStakedIota(vestingObject)) {
            objectValue = (vestingObject as TimelockedStakedIota).stakedIota.principal.value;
        }

        if (!expirationToVestingPayout.has(vestingObject.expirationTimestampMs)) {
            expirationToVestingPayout.set(vestingObject.expirationTimestampMs, {
                amount: objectValue,
                expirationTimestampMs: vestingObject.expirationTimestampMs,
            });
        } else {
            const vestingPayout = expirationToVestingPayout.get(
                vestingObject.expirationTimestampMs,
            );

            if (!vestingPayout) {
                continue;
            }

            vestingPayout.amount += objectValue;

            expirationToVestingPayout.set(vestingObject.expirationTimestampMs, vestingPayout);
        }
    }

    return expirationToVestingPayout;
}

export function getSupplyIncreaseVestingUserType(
    vestingUserPayouts: SupplyIncreaseVestingPayout[],
): SupplyIncreaseUserType | undefined {
    const payoutTimelocks = vestingUserPayouts.map((payout) => payout.expirationTimestampMs);
    const latestPayout = payoutTimelocks.sort((a, b) => b - a)[0];

    if (!latestPayout) {
        return;
    } else {
        const isEntity =
            new Date(latestPayout).getFullYear() >
            SUPPLY_INCREASE_STARTING_VESTING_YEAR + SUPPLY_INCREASE_STAKER_VESTING_DURATION;
        return isEntity ? SupplyIncreaseUserType.Entity : SupplyIncreaseUserType.Staker;
    }
}

export function buildSupplyIncreaseVestingSchedule(
    referencePayout: SupplyIncreaseVestingPayout,
    currentEpochTimestamp: number,
): SupplyIncreaseVestingPortfolio {
    const userType = getSupplyIncreaseVestingUserType([referencePayout]);

    if (!userType || currentEpochTimestamp >= referencePayout.expirationTimestampMs) {
        // if the latest payout has already been unlocked, we cant build a vesting schedule
        return [];
    }

    const payoutsCount = getSupplyIncreaseVestingPayoutsCount(userType);

    return Array.from({ length: payoutsCount }).map((_, i) => ({
        amount: referencePayout.amount,
        expirationTimestampMs:
            referencePayout.expirationTimestampMs -
            SUPPLY_INCREASE_VESTING_PAYOUT_SCHEDULE_MILLISECONDS * i,
    }));
}

export function getVestingOverview(
    objects: (TimelockedObject | TimelockedStakedIota)[],
    currentEpochTimestamp: number,
): VestingOverview {
    const vestingObjects = objects.filter(isSupplyIncreaseVestingObject);
    const latestPayout = getLastSupplyIncreaseVestingPayout(vestingObjects);

    if (vestingObjects.length === 0 || !latestPayout) {
        return {
            totalVested: 0,
            totalUnlocked: 0,
            totalLocked: 0,
            totalStaked: 0,
            availableClaiming: 0,
            availableStaking: 0,
        };
    }

    const userType = getSupplyIncreaseVestingUserType([latestPayout]);
    const vestingPayoutsCount = getSupplyIncreaseVestingPayoutsCount(userType!);
    const totalVestedAmount = vestingPayoutsCount * latestPayout.amount;
    const vestingPortfolio = buildSupplyIncreaseVestingSchedule(
        latestPayout,
        currentEpochTimestamp,
    );
    const totalLockedAmount = vestingPortfolio.reduce(
        (acc, current) =>
            current.expirationTimestampMs > currentEpochTimestamp ? acc + current.amount : acc,
        0,
    );
    const totalUnlockedVestedAmount = totalVestedAmount - totalLockedAmount;

    const timelockedStakedObjects = vestingObjects.filter(isTimelockedStakedIota);
    const totalStaked = timelockedStakedObjects.reduce(
        (acc, current) => acc + current.stakedIota.principal.value,
        0,
    );

    const timelockedObjects = vestingObjects.filter(isTimelockedObject);

    const totalAvailableClaimingAmount = timelockedObjects.reduce(
        (acc, current) =>
            current.expirationTimestampMs <= currentEpochTimestamp
                ? acc + current.locked.value
                : acc,
        0,
    );
    const totalAvailableStakingAmount = timelockedObjects.reduce(
        (acc, current) =>
            current.expirationTimestampMs > currentEpochTimestamp
                ? acc + current.locked.value
                : acc,
        0,
    );

    return {
        totalVested: totalVestedAmount,
        totalUnlocked: totalUnlockedVestedAmount,
        totalLocked: totalLockedAmount,
        totalStaked: totalStaked,
        availableClaiming: totalAvailableClaimingAmount,
        availableStaking: totalAvailableStakingAmount,
    };
}

// Get number of payouts to construct vesting schedule
export function getSupplyIncreaseVestingPayoutsCount(userType: SupplyIncreaseUserType): number {
    const vestingDuration =
        userType === SupplyIncreaseUserType.Staker
            ? SUPPLY_INCREASE_STAKER_VESTING_DURATION
            : SUPPLY_INCREASE_INVESTOR_VESTING_DURATION;

    return SUPPLY_INCREASE_VESTING_PAYOUTS_IN_1_YEAR * vestingDuration;
}

export function mapTimelockObjects(iotaObjects: IotaObjectData[]): TimelockedObject[] {
    return iotaObjects.map((iotaObject) => {
        if (!iotaObject?.content?.dataType || iotaObject.content.dataType !== 'moveObject') {
            return {
                id: { id: '' },
                locked: { value: 0 },
                expirationTimestampMs: 0,
            };
        }
        const fields = iotaObject.content.fields as unknown as TimelockedIotaResponse;
        return {
            id: fields.id,
            locked: { value: Number(fields.locked) },
            expirationTimestampMs: Number(fields.expiration_timestamp_ms),
            label: fields.label,
        };
    });
}

export function timelockObjectsFromIotaObjects(
    stakedTimelockedObjects: DelegatedTimelockedStake[],
): TimelockedStakedIota[] {
    const result: TimelockedStakedIota[] = [];
    stakedTimelockedObjects.map((stakedTimelockedObject) => {
        const stakeMapped: TimelockedStakedIota[] = stakedTimelockedObject.stakes.map((stake) => {
            return {
                id: { id: stake.timelockedStakedIotaId },
                expirationTimestampMs: Number(stake.expirationTimestampMs),
                stakedIota: {
                    id: { id: stake.timelockedStakedIotaId },
                    poolId: stakedTimelockedObject.stakingPool,
                    stakeActivationEpoch: Number(stake.stakeRequestEpoch),
                    principal: { value: Number(stake.principal) },
                },
                label: stake.label,
            };
        });
        result.push(...stakeMapped);
    });
    return result;
}

export function isSupplyIncreaseVestingObject(
    obj: TimelockedObject | TimelockedStakedIota,
): boolean {
    return obj.label === SUPPLY_INCREASE_VESTING_LABEL;
}
