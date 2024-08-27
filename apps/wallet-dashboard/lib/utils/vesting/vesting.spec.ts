// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    MOCKED_SUPPLY_INCREASE_VESTING_TIMELOCKED_OBJECTS,
    MOCKED_VESTING_TIMELOCKED_STAKED_OBJECTS,
    SUPPLY_INCREASE_STAKER_VESTING_DURATION,
    SUPPLY_INCREASE_VESTING_PAYOUTS_IN_1_YEAR,
} from '../../constants';

import { SupplyIncreaseUserType, SupplyIncreaseVestingPayout } from '../../interfaces';
import { formatDelegatedTimelockedStake, isTimelockedObject } from '../timelock';

import {
    getVestingOverview,
    buildSupplyIncreaseVestingSchedule as buildVestingPortfolio,
    getLastSupplyIncreaseVestingPayout,
    getSupplyIncreaseVestingPayoutsCount,
    getSupplyIncreaseVestingUserType,
} from './vesting';

describe('get last supply increase vesting payout', () => {
    it('should get the object with highest expirationTimestampMs', () => {
        const timelockedObjects = MOCKED_SUPPLY_INCREASE_VESTING_TIMELOCKED_OBJECTS;

        // the last in the array is also the one with the latest expiration time
        const expectedObject =
            MOCKED_SUPPLY_INCREASE_VESTING_TIMELOCKED_OBJECTS[
                MOCKED_SUPPLY_INCREASE_VESTING_TIMELOCKED_OBJECTS.length - 1
            ];

        const lastPayout = getLastSupplyIncreaseVestingPayout(timelockedObjects);

        expect(lastPayout?.expirationTimestampMs).toEqual(expectedObject.expirationTimestampMs);
        expect(lastPayout?.amount).toEqual(expectedObject.locked.value);
    });
});

describe('get supply increase user type', () => {
    it('should return staker, if last payout is two years away from vesting starting year (2023)', () => {
        const vestingPayout: SupplyIncreaseVestingPayout = {
            amount: 1000,
            expirationTimestampMs: 1735689661000, // Wednesday, 1 January 2025 00:01:01
        };
        const userType = getSupplyIncreaseVestingUserType([vestingPayout]);
        expect(userType).toEqual(SupplyIncreaseUserType.Staker);
    });

    it('should return entity, if last payout is more than two years away from vesting starting year (2023)', () => {
        const vestingPayout: SupplyIncreaseVestingPayout = {
            amount: 1000,
            expirationTimestampMs: 1798761661000, // Friday, 1 January 2027 00:01:01
        };
        const userType = getSupplyIncreaseVestingUserType([vestingPayout]);
        expect(userType).toEqual(SupplyIncreaseUserType.Entity);
    });
});

describe('build supply increase staker vesting portfolio', () => {
    it('should build with mocked timelocked objects', () => {
        const timelockedObjects = MOCKED_SUPPLY_INCREASE_VESTING_TIMELOCKED_OBJECTS;

        const lastPayout = getLastSupplyIncreaseVestingPayout(timelockedObjects);

        expect(lastPayout).toBeDefined();

        const vestingPortfolio = buildVestingPortfolio(lastPayout!, Date.now());

        expect(vestingPortfolio.length).toEqual(
            getSupplyIncreaseVestingPayoutsCount(SupplyIncreaseUserType.Staker),
        );
    });

    it('should build properly with mocked timelocked staked objects', () => {
        const timelockedStakedObjects = MOCKED_VESTING_TIMELOCKED_STAKED_OBJECTS;
        const extendedTimelockedStakedObjects =
            formatDelegatedTimelockedStake(timelockedStakedObjects);
        const lastPayout = getLastSupplyIncreaseVestingPayout(extendedTimelockedStakedObjects);

        expect(lastPayout).toBeDefined();

        const vestingPortfolio = buildVestingPortfolio(lastPayout!, Date.now());

        expect(vestingPortfolio.length).toEqual(
            getSupplyIncreaseVestingPayoutsCount(SupplyIncreaseUserType.Staker),
        );
    });

    it('should build properly with mix of mocked timelocked and timelocked staked objects', () => {
        const timelockedObjects = MOCKED_SUPPLY_INCREASE_VESTING_TIMELOCKED_OBJECTS;
        const timelockedStakedObjects = MOCKED_VESTING_TIMELOCKED_STAKED_OBJECTS;
        const extendedTimelockedStakedObjects =
            formatDelegatedTimelockedStake(timelockedStakedObjects);
        const mixedObjects = [...timelockedObjects, ...extendedTimelockedStakedObjects];

        const lastPayout = getLastSupplyIncreaseVestingPayout(mixedObjects);
        expect(lastPayout).toBeDefined();

        const vestingPortfolio = buildVestingPortfolio(lastPayout!, Date.now());
        expect(vestingPortfolio.length).toEqual(
            getSupplyIncreaseVestingPayoutsCount(SupplyIncreaseUserType.Staker),
        );
    });
});

describe('vesting overview', () => {
    it('should get correct vesting overview data with timelocked objects', () => {
        const timelockedObjects = MOCKED_SUPPLY_INCREASE_VESTING_TIMELOCKED_OBJECTS;
        const lastPayout = timelockedObjects[timelockedObjects.length - 1];
        const totalAmount =
            (SUPPLY_INCREASE_STAKER_VESTING_DURATION *
                SUPPLY_INCREASE_VESTING_PAYOUTS_IN_1_YEAR *
                lastPayout.locked.value) /
            0.9;

        const vestingOverview = getVestingOverview(timelockedObjects, Date.now());
        expect(vestingOverview.totalVested).toEqual(totalAmount);

        const vestingPortfolio = buildVestingPortfolio(
            {
                amount: lastPayout.locked.value,
                expirationTimestampMs: lastPayout.expirationTimestampMs,
            },
            Date.now(),
        );

        const lockedAmount = vestingPortfolio.reduce(
            (acc, current) =>
                current.expirationTimestampMs > Date.now() ? acc + current.amount : acc,
            0,
        );

        expect(vestingOverview.totalLocked).toEqual(lockedAmount);
        expect(vestingOverview.totalUnlocked).toEqual(totalAmount - lockedAmount);

        // In this scenario there are no staked objects
        expect(vestingOverview.totalStaked).toEqual(0);

        const lockedObjectsAmount = timelockedObjects.reduce(
            (acc, current) =>
                current.expirationTimestampMs > Date.now() ? acc + current.locked.value : acc,
            0,
        );
        const unlockedObjectsAmount = timelockedObjects.reduce(
            (acc, current) =>
                current.expirationTimestampMs <= Date.now() ? acc + current.locked.value : acc,
            0,
        );

        expect(vestingOverview.availableClaiming).toEqual(unlockedObjectsAmount);
        expect(vestingOverview.availableStaking).toEqual(lockedObjectsAmount);
    });

    it('should get correct vesting overview data with timelocked staked objects', () => {
        const timelockedStakedObjects = MOCKED_VESTING_TIMELOCKED_STAKED_OBJECTS;
        const extendedTimelockedStakedObjects =
            formatDelegatedTimelockedStake(timelockedStakedObjects);
        const lastPayout =
            extendedTimelockedStakedObjects[extendedTimelockedStakedObjects.length - 1];
        const lastPayoutValue = Number(lastPayout.principal);
        const totalAmount =
            SUPPLY_INCREASE_STAKER_VESTING_DURATION *
            SUPPLY_INCREASE_VESTING_PAYOUTS_IN_1_YEAR *
            lastPayoutValue;
        const vestingOverview = getVestingOverview(extendedTimelockedStakedObjects, Date.now());
        expect(vestingOverview.totalVested).toEqual(totalAmount);

        const vestingPortfolio = buildVestingPortfolio(
            {
                amount: lastPayoutValue,
                expirationTimestampMs: Number(lastPayout.expirationTimestampMs),
            },
            Date.now(),
        );

        const lockedAmount = vestingPortfolio.reduce(
            (acc, current) =>
                current.expirationTimestampMs > Date.now() ? acc + current.amount : acc,
            0,
        );

        expect(vestingOverview.totalLocked).toEqual(lockedAmount);
        expect(vestingOverview.totalUnlocked).toEqual(totalAmount - lockedAmount);

        let totalStaked: number = 0;
        for (const timelockedStakedObject of timelockedStakedObjects) {
            const stakesAmount = timelockedStakedObject.stakes.reduce(
                (acc, current) => acc + Number(current.principal),
                0,
            );
            totalStaked += stakesAmount;
        }

        expect(vestingOverview.totalStaked).toEqual(totalStaked);

        // In this scenario there are no objects to stake or claim because they are all staked
        expect(vestingOverview.availableClaiming).toEqual(0);
        expect(vestingOverview.availableStaking).toEqual(0);
    });

    it('should get correct vesting overview data with mixed objects', () => {
        const timelockedObjects = MOCKED_SUPPLY_INCREASE_VESTING_TIMELOCKED_OBJECTS;
        const timelockedStakedObjects = MOCKED_VESTING_TIMELOCKED_STAKED_OBJECTS;
        const extendedTimelockedStakedObjects =
            formatDelegatedTimelockedStake(timelockedStakedObjects);
        const mixedObjects = [...timelockedObjects, ...extendedTimelockedStakedObjects];

        const lastPayout = getLastSupplyIncreaseVestingPayout(mixedObjects)!;
        const totalAmount =
            SUPPLY_INCREASE_STAKER_VESTING_DURATION *
            SUPPLY_INCREASE_VESTING_PAYOUTS_IN_1_YEAR *
            lastPayout.amount;

        const vestingOverview = getVestingOverview(mixedObjects, Date.now());
        expect(vestingOverview.totalVested).toEqual(totalAmount);

        const vestingPortfolio = buildVestingPortfolio(
            {
                amount: lastPayout.amount,
                expirationTimestampMs: lastPayout.expirationTimestampMs,
            },
            Date.now(),
        );

        const lockedAmount = vestingPortfolio.reduce(
            (acc, current) =>
                current.expirationTimestampMs > Date.now() ? acc + current.amount : acc,
            0,
        );

        expect(vestingOverview.totalLocked).toEqual(lockedAmount);
        expect(vestingOverview.totalUnlocked).toEqual(totalAmount - lockedAmount);

        const totalStaked = extendedTimelockedStakedObjects.reduce(
            (acc, current) => acc + Number(current.principal),
            0,
        );

        expect(vestingOverview.totalStaked).toEqual(totalStaked);

        const timelockObjects = mixedObjects.filter(isTimelockedObject);
        const availableClaiming = timelockObjects.reduce(
            (acc, current) =>
                current.expirationTimestampMs <= Date.now() ? acc + current.locked.value : acc,
            0,
        );
        const availableStaking = timelockObjects.reduce(
            (acc, current) =>
                current.expirationTimestampMs > Date.now() ? acc + current.locked.value : acc,
            0,
        );
        expect(vestingOverview.availableClaiming).toEqual(availableClaiming);
        expect(vestingOverview.availableStaking).toEqual(availableStaking);
    });
});
