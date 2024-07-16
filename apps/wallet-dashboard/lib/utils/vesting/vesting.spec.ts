// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    MOCKED_VESTING_TIMELOCKED_AND_TIMELOCK_STAKED_OBJECTS,
    MOCKED_SUPPLY_INCREASE_VESTING_TIMELOCKED_OBJECTS,
    MOCKED_VESTING_TIMELOCKED_STAKED_OBJECTS,
} from '../../constants';

import { SupplyIncreaseUserType, SupplyIncreaseVestingPayout } from '../../interfaces';

import {
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

        const vestingPortfolio = buildVestingPortfolio(lastPayout!);

        expect(vestingPortfolio.length).toEqual(
            getSupplyIncreaseVestingPayoutsCount(SupplyIncreaseUserType.Staker),
        );
    });

    it('should build properly with mocked timelocked staked objects', () => {
        const timelockedStakedObjects = MOCKED_VESTING_TIMELOCKED_STAKED_OBJECTS;

        const lastPayout = getLastSupplyIncreaseVestingPayout(timelockedStakedObjects);

        expect(lastPayout).toBeDefined();

        const vestingPortfolio = buildVestingPortfolio(lastPayout!);

        expect(vestingPortfolio.length).toEqual(
            getSupplyIncreaseVestingPayoutsCount(SupplyIncreaseUserType.Staker),
        );
    });

    it('should build properly with mix of mocked timelocked and timelocked staked objects', () => {
        const mixedObjects = MOCKED_VESTING_TIMELOCKED_AND_TIMELOCK_STAKED_OBJECTS;

        const lastPayout = getLastSupplyIncreaseVestingPayout(mixedObjects);

        expect(lastPayout).toBeDefined();

        const vestingPortfolio = buildVestingPortfolio(lastPayout!);

        expect(vestingPortfolio.length).toEqual(
            getSupplyIncreaseVestingPayoutsCount(SupplyIncreaseUserType.Staker),
        );
    });
});
