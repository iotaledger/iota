// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    MOCKED_VESTING_TIMELOCKED_AND_TIMELOCK_STAKED_OBJECTS,
    MOCKED_VESTING_TIMELOCKED_OBJECTS,
    MOCKED_VESTING_TIMELOCKED_STAKED_OBJECTS,
} from '../../constants';

import { SupplyIncreaseUserType, SupplyIncreaseVestingPayout } from '../../interfaces';

import {
    buildVestingSchedule as buildVestingPortfolio,
    getLastVestingPayout,
    getVestingUserType,
} from './vesting';

describe('get last vesting payout', () => {
    it('should get the object with highest expirationTimestampMs', () => {
        const timelockedObjects = MOCKED_VESTING_TIMELOCKED_OBJECTS;

        // the last in the array is also the one with the latest expiration time
        const expectedObject =
            MOCKED_VESTING_TIMELOCKED_OBJECTS[MOCKED_VESTING_TIMELOCKED_OBJECTS.length - 1];

        const lastPayout = getLastVestingPayout(timelockedObjects);

        expect(lastPayout?.expirationTimestampMs).toEqual(expectedObject.expirationTimestampMs);
        expect(lastPayout?.amount).toEqual(expectedObject.locked.value);
    });
});

describe('get user type', () => {
    it('should return staker, if last payout is two years away from vesting starting year (2023)', () => {
        const vestingPayout: SupplyIncreaseVestingPayout = {
            amount: 1000,
            expirationTimestampMs: 1735689661000, // Wednesday, 1 January 2025 00:01:01
        };
        const userType = getVestingUserType([vestingPayout]);
        expect(userType).toEqual(SupplyIncreaseUserType.Staker);
    });

    it('should return entity, if last payout is more than two years away from vesting starting year (2023)', () => {
        const vestingPayout: SupplyIncreaseVestingPayout = {
            amount: 1000,
            expirationTimestampMs: 1798761661000, // Friday, 1 January 2027 00:01:01
        };
        const userType = getVestingUserType([vestingPayout]);
        expect(userType).toEqual(SupplyIncreaseUserType.Entity);
    });
});

describe('build vesting portfolio', () => {
    it('should build propery with mocked timelocked objects', () => {
        const timelockedObjects = MOCKED_VESTING_TIMELOCKED_OBJECTS;

        const lastPayout = getLastVestingPayout(timelockedObjects);

        expect(lastPayout).toBeDefined();

        const vestingPortfolio = buildVestingPortfolio(lastPayout!);

        expect(vestingPortfolio.length).toEqual(52);
    });

    it('should build propery with mocked timelocked staked objects', () => {
        const timelockedStakedObjects = MOCKED_VESTING_TIMELOCKED_STAKED_OBJECTS;

        const lastPayout = getLastVestingPayout(timelockedStakedObjects);

        expect(lastPayout).toBeDefined();

        const vestingPortfolio = buildVestingPortfolio(lastPayout!);

        expect(vestingPortfolio.length).toEqual(52);
    });

    it('should build propery with mix of mocked timelocked and timelocked staked objects', () => {
        const mixedObjects = MOCKED_VESTING_TIMELOCKED_AND_TIMELOCK_STAKED_OBJECTS;

        const lastPayout = getLastVestingPayout(mixedObjects);

        expect(lastPayout).toBeDefined();

        const vestingPortfolio = buildVestingPortfolio(lastPayout!);

        expect(vestingPortfolio.length).toEqual(52);
    });
});
