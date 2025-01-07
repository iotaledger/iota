// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { MILLISECONDS_PER_HOUR } from '@iota/core/constants/time.constants';
import {
    mockedTimelockedStackedObjectsWithDynamicDate,
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
    getLatestOrEarliestSupplyIncreaseVestingPayout,
    getSupplyIncreaseVestingPayoutsCount,
    getSupplyIncreaseVestingUserType,
} from './vesting';

const MOCKED_CURRENT_EPOCH_TIMESTAMP = Date.now() + MILLISECONDS_PER_HOUR * 6; // 6 hours later

describe('get last supply increase vesting payout', () => {
    it('should get the object with highest expirationTimestampMs', () => {
        const timelockedObjects = MOCKED_SUPPLY_INCREASE_VESTING_TIMELOCKED_OBJECTS;

        // the last in the array is also the one with the latest expiration time
        const expectedObject =
            MOCKED_SUPPLY_INCREASE_VESTING_TIMELOCKED_OBJECTS[
                MOCKED_SUPPLY_INCREASE_VESTING_TIMELOCKED_OBJECTS.length - 1
            ];

        const lastPayout = getLatestOrEarliestSupplyIncreaseVestingPayout(
            timelockedObjects,
            MOCKED_CURRENT_EPOCH_TIMESTAMP,
        );

        expect(lastPayout?.expirationTimestampMs).toEqual(expectedObject.expirationTimestampMs);
        expect(lastPayout?.amount).toEqual(expectedObject.locked.value);
    });
});

describe('get supply increase user type', () => {
    it('should return staker, if last payout is two years away from vesting starting year (2023)', () => {
        const vestingPayout: SupplyIncreaseVestingPayout = {
            amount: 1000n,
            expirationTimestampMs: 1735689661000, // Wednesday, 1 January 2025 00:01:01
        };
        const userType = getSupplyIncreaseVestingUserType([vestingPayout]);
        expect(userType).toEqual(SupplyIncreaseUserType.Staker);
    });

    it('should return entity, if last payout is more than two years away from vesting starting year (2023)', () => {
        const vestingPayout: SupplyIncreaseVestingPayout = {
            amount: 1000n,
            expirationTimestampMs: 1798761661000, // Friday, 1 January 2027 00:01:01
        };
        const userType = getSupplyIncreaseVestingUserType([vestingPayout]);
        expect(userType).toEqual(SupplyIncreaseUserType.Entity);
    });
});

describe('build supply increase staker vesting portfolio', () => {
    xit('should build with mocked timelocked objects', () => {
        const timelockedObjects = MOCKED_SUPPLY_INCREASE_VESTING_TIMELOCKED_OBJECTS;

        const lastPayout = getLatestOrEarliestSupplyIncreaseVestingPayout(
            timelockedObjects,
            MOCKED_CURRENT_EPOCH_TIMESTAMP,
        );

        expect(lastPayout).toBeDefined();

        const vestingPortfolio = buildVestingPortfolio(lastPayout!, Date.now());

        expect(vestingPortfolio.length).toEqual(
            getSupplyIncreaseVestingPayoutsCount(SupplyIncreaseUserType.Staker),
        );
    });

    it('should build properly with mocked timelocked staked objects', () => {
        const timelockedStakedObjects = mockedTimelockedStackedObjectsWithDynamicDate;
        const extendedTimelockedStakedObjects =
            formatDelegatedTimelockedStake(timelockedStakedObjects);
        const lastPayout = getLatestOrEarliestSupplyIncreaseVestingPayout(
            extendedTimelockedStakedObjects,
            MOCKED_CURRENT_EPOCH_TIMESTAMP,
        );

        expect(lastPayout).toBeDefined();

        const vestingPortfolio = buildVestingPortfolio(lastPayout!, Date.now());

        expect(vestingPortfolio.length).toEqual(
            getSupplyIncreaseVestingPayoutsCount(SupplyIncreaseUserType.Staker),
        );
    });

    xit('should build properly with mix of mocked timelocked and timelocked staked objects', () => {
        const timelockedObjects = MOCKED_SUPPLY_INCREASE_VESTING_TIMELOCKED_OBJECTS;
        const timelockedStakedObjects = MOCKED_VESTING_TIMELOCKED_STAKED_OBJECTS;
        const extendedTimelockedStakedObjects =
            formatDelegatedTimelockedStake(timelockedStakedObjects);
        const mixedObjects = [...timelockedObjects, ...extendedTimelockedStakedObjects];

        const lastPayout = getLatestOrEarliestSupplyIncreaseVestingPayout(
            mixedObjects,
            MOCKED_CURRENT_EPOCH_TIMESTAMP,
        );
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
            (BigInt(SUPPLY_INCREASE_STAKER_VESTING_DURATION) *
                BigInt(SUPPLY_INCREASE_VESTING_PAYOUTS_IN_1_YEAR) *
                10n *
                lastPayout.locked.value) /
            9n;

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
            0n,
        );

        expect(vestingOverview.totalLocked).toEqual(lockedAmount);
        expect(vestingOverview.totalUnlocked).toEqual(totalAmount - lockedAmount);

        // In this scenario there are no staked objects
        expect(vestingOverview.totalStaked).toEqual(0n);

        const lockedObjectsAmount = timelockedObjects.reduce(
            (acc, current) =>
                current.expirationTimestampMs > Date.now() ? acc + current.locked.value : acc,
            0n,
        );
        const unlockedObjectsAmount = timelockedObjects.reduce(
            (acc, current) =>
                current.expirationTimestampMs <= Date.now() ? acc + current.locked.value : acc,
            0n,
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
        const lastPayoutValue = BigInt(lastPayout.principal);
        const totalAmount =
            (BigInt(SUPPLY_INCREASE_STAKER_VESTING_DURATION) *
                BigInt(SUPPLY_INCREASE_VESTING_PAYOUTS_IN_1_YEAR) *
                lastPayoutValue *
                10n) /
            9n;

        const vestingOverview = getVestingOverview(extendedTimelockedStakedObjects, Date.now());
        expect(vestingOverview.totalVested).toEqual(totalAmount);

        const vestingPortfolio = buildVestingPortfolio(
            {
                amount: BigInt(lastPayoutValue),
                expirationTimestampMs: Number(lastPayout.expirationTimestampMs),
            },
            Date.now(),
        );

        const lockedAmount = vestingPortfolio.reduce(
            (acc, current) =>
                current.expirationTimestampMs > Date.now() ? acc + current.amount : acc,
            0n,
        );

        expect(vestingOverview.totalLocked).toEqual(lockedAmount);
        expect(vestingOverview.totalUnlocked).toEqual(totalAmount - lockedAmount);

        let totalStaked = 0n;
        for (const timelockedStakedObject of timelockedStakedObjects) {
            const stakesAmount = timelockedStakedObject.stakes.reduce(
                (acc, current) => acc + BigInt(current.principal),
                0n,
            );
            totalStaked += stakesAmount;
        }

        expect(vestingOverview.totalStaked).toEqual(totalStaked);

        // In this scenario there are no objects to stake or claim because they are all staked
        expect(vestingOverview.availableClaiming).toEqual(0n);
        expect(vestingOverview.availableStaking).toEqual(0n);
    });

    it('should get correct vesting overview data with mixed objects', () => {
        const timelockedObjects = MOCKED_SUPPLY_INCREASE_VESTING_TIMELOCKED_OBJECTS;
        const timelockedStakedObjects = MOCKED_VESTING_TIMELOCKED_STAKED_OBJECTS;
        const extendedTimelockedStakedObjects =
            formatDelegatedTimelockedStake(timelockedStakedObjects);
        const mixedObjects = [...timelockedObjects, ...extendedTimelockedStakedObjects];

        const lastPayout = getLatestOrEarliestSupplyIncreaseVestingPayout(
            mixedObjects,
            MOCKED_CURRENT_EPOCH_TIMESTAMP,
        )!;
        const totalAmount =
            (BigInt(SUPPLY_INCREASE_STAKER_VESTING_DURATION) *
                BigInt(SUPPLY_INCREASE_VESTING_PAYOUTS_IN_1_YEAR) *
                10n *
                lastPayout.amount) /
            9n;

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
            0n,
        );

        expect(vestingOverview.totalLocked).toEqual(lockedAmount);
        expect(vestingOverview.totalUnlocked).toEqual(totalAmount - lockedAmount);

        const totalStaked = extendedTimelockedStakedObjects.reduce(
            (acc, current) => acc + BigInt(current.principal),
            0n,
        );

        expect(vestingOverview.totalStaked).toEqual(totalStaked);

        const timelockObjects = mixedObjects.filter(isTimelockedObject);
        const availableClaiming = timelockObjects.reduce(
            (acc, current) =>
                current.expirationTimestampMs <= Date.now() ? acc + current.locked.value : acc,
            0n,
        );
        const availableStaking = timelockObjects.reduce(
            (acc, current) =>
                current.expirationTimestampMs > Date.now() ? acc + current.locked.value : acc,
            0n,
        );
        expect(vestingOverview.availableClaiming).toEqual(availableClaiming);
        expect(vestingOverview.availableStaking).toEqual(availableStaking);
    });
});
