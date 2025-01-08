// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { IotaEvent } from '@iota/iota-sdk/client';
import { StakeEventJson, UnstakeEventJson } from '../../interfaces';
import { getStakeDetails, getUnstakeDetails, checkIfIsTimelockedStaking, parseEventJson } from '.';

export function getTransactionAmountForTimelocked(
    events: IotaEvent[],
): bigint | undefined | string {
    if (!events) return;
    const { isTimelockedStaking, isTimelockedUnstaking } = checkIfIsTimelockedStaking(events);

    if (isTimelockedStaking) {
        const eventJson = parseEventJson<StakeEventJson>(events[0]);
        const { stakedAmount } = getStakeDetails(eventJson);
        return stakedAmount;
    } else if (isTimelockedUnstaking) {
        const eventJson = parseEventJson<UnstakeEventJson>(events[0]);
        const { totalAmount } = getUnstakeDetails(eventJson);
        return totalAmount;
    }
}
