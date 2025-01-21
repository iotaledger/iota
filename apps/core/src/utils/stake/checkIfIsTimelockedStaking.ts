// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { IotaTransactionBlockResponse } from '@iota/iota-sdk/client';
import { STAKING_REQUEST_EVENT, UNSTAKING_REQUEST_EVENT } from '../../constants';

export function checkIfIsTimelockedStaking(events: IotaTransactionBlockResponse['events']) {
    const TIMELOCKED_STAKING_EVENT_MODULE = 'timelocked_staking';
    if (!events) return { isTimelockedStaking: false, isTimelockedUnstaking: false };

    const stakeTypeTransaction = events?.find(({ type }) => type === STAKING_REQUEST_EVENT);
    const unstakeTypeTransaction = events?.find(({ type }) => type === UNSTAKING_REQUEST_EVENT);

    const isTimelockedStaking =
        stakeTypeTransaction?.transactionModule === TIMELOCKED_STAKING_EVENT_MODULE;
    const isTimelockedUnstaking =
        unstakeTypeTransaction?.transactionModule === TIMELOCKED_STAKING_EVENT_MODULE;

    return {
        isTimelockedStaking,
        isTimelockedUnstaking,
        stakeTypeTransaction,
        unstakeTypeTransaction,
    };
}
