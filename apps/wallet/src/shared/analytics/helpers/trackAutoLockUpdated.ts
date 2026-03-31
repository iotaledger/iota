// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ampli } from '_src/shared/analytics/ampli';
import { autoLockDataToMinutes } from '_hooks';

type AutoLockData = {
    enabled: boolean;
    timer: number;
    interval: 'day' | 'hour' | 'minute';
};

export function trackAutoLockUpdated(autoLock: AutoLockData) {
    const minutes = autoLockDataToMinutes(autoLock);
    const lockTimeSeconds = autoLock.enabled && minutes !== null ? Math.round(minutes * 60) : 0;

    ampli.updatedAutoLock({
        autoLock: autoLock.enabled,
        lockTimeSeconds,
    });
}
