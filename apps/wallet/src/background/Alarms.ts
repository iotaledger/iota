// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import Browser from 'webextension-polyfill';

export const AUTO_LOCK_ALARM_NAME = 'auto-lock-alarm';
export const CLEAN_UP_ALARM_NAME = 'clean-up-storage-alarm';

class Alarms {
    public async setAutoLockAlarm(minutes: number) {
        Browser.alarms.create(AUTO_LOCK_ALARM_NAME, { delayInMinutes: minutes });
    }

    public clearAutoLockAlarm() {
        return Browser.alarms.clear(AUTO_LOCK_ALARM_NAME);
    }

    public async setCleanUpAlarm() {
        Browser.alarms.create(CLEAN_UP_ALARM_NAME, { periodInMinutes: 60 * 6 }); //  every 6 hours
    }
}

const alarms = new Alarms();
export default alarms;
