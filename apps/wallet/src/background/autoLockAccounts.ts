// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { throttle } from 'throttle-debounce';

import Alarms from './alarms';
import { getDB, SETTINGS_KEYS } from './db';

export async function getAutoLockMinutes() {
    const minutesStored =
        (await (await getDB()).settings.get(SETTINGS_KEYS.autoLockMinutes))?.value || null;
    return typeof minutesStored === 'number' ? minutesStored : null;
}

/**
 * minutes as null disables auto-lock
 * updates the alarm to the new timeout
 */
export async function setAutoLockMinutes(minutes: number | null) {
    await (
        await getDB()
    ).settings.put({
        setting: SETTINGS_KEYS.autoLockMinutes,
        value: minutes,
    });
    await setupAutoLockAlarm();
}

export async function setupAutoLockAlarm() {
    const minutes = await getAutoLockMinutes();
    if (!minutes) {
        Alarms.clearAutoLockAlarm();
    } else {
        Alarms.setAutoLockAlarm(minutes);
    }
}

export const notifyUserActive = throttle(
    5000,
    () => {
        setupAutoLockAlarm();
    },
    { noTrailing: true },
);
