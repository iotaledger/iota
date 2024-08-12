// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useQuery } from '@tanstack/react-query';
import { type z } from 'zod';

import { type zodSchema } from '_components/accounts/AutoLockSelector';
import { useBackgroundClient } from './useBackgroundClient';

export type AutoLockInterval = z.infer<typeof zodSchema>['autoLock']['interval'];
export const AUTO_LOCK_MINUTES_QUERY_KEY = ['get auto-lock minutes'];

export function useAutoLockMinutes() {
    const backgroundClient = useBackgroundClient();
    return useQuery({
        queryKey: AUTO_LOCK_MINUTES_QUERY_KEY,
        queryFn: () => backgroundClient.getAutoLockMinutes(),
        refetchInterval: 15 * 1000,
        meta: {
            skipPersistedCache: true,
        },
    });
}

const MINUTES_ONE_DAY = 60 * 24;
const MINUTES_ONE_HOUR = 60;

export function formatAutoLock(minutes: number | null) {
    const { enabled, timer, interval } = parseAutoLock(minutes);
    if (!enabled) {
        return '';
    }
    return `${timer} ${interval}${timer === 1 ? '' : 's'}`;
}

export function parseAutoLock(minutes: number | null) {
    let timer = minutes || 1;
    const enabled = !!minutes;
    let interval: AutoLockInterval = 'hour';
    if (enabled) {
        if (minutes % MINUTES_ONE_DAY === 0) {
            timer = Math.floor(minutes / MINUTES_ONE_DAY);
            interval = 'day';
        } else if (minutes % MINUTES_ONE_HOUR === 0) {
            timer = Math.floor(minutes / MINUTES_ONE_HOUR);
            interval = 'hour';
        } else {
            interval = 'minute';
        }
    }
    return {
        enabled,
        timer,
        interval,
    };
}

const intervalToMinutesMultiplier: Record<AutoLockInterval, number> = {
    minute: 1,
    hour: MINUTES_ONE_HOUR,
    day: MINUTES_ONE_DAY,
};

export function autoLockDataToMinutes({
    enabled,
    timer,
    interval,
}: {
    enabled: boolean;
    timer: number;
    interval: AutoLockInterval;
}) {
    if (!enabled) {
        return null;
    }
    return intervalToMinutesMultiplier[interval] * (Number(timer) || 1);
}
