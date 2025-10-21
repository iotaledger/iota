// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    MOCKED_SUPPLY_INCREASE_VESTING_TIMELOCKED_OBJECTS,
    MOCKED_VESTING_TIMELOCKED_STAKED_OBJECTS,
} from '../../constants/vesting.constants';
import {
    getMockedSupplyIncreaseVestingTimelockedObjectsWithDynamicDate,
    getMockedVestingTimelockedStakedObjectsWithDynamicDate,
} from './buildMockedObjectsWithDynamicDateUtils';

export const mockedSupplyIncreaseVestingTimelockedObjectsWithDynamicDate =
    getMockedSupplyIncreaseVestingTimelockedObjectsWithDynamicDate(
        MOCKED_SUPPLY_INCREASE_VESTING_TIMELOCKED_OBJECTS,
    );

export const mockedVestingTimelockedStakedObjectsWithDynamicDate =
    getMockedVestingTimelockedStakedObjectsWithDynamicDate(
        MOCKED_VESTING_TIMELOCKED_STAKED_OBJECTS,
    );
