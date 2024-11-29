// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useState, useEffect } from 'react';
import {
    MILLISECONDS_PER_DAY,
    MILLISECONDS_PER_HOUR,
    MILLISECONDS_PER_MINUTE,
    MILLISECONDS_PER_SECOND,
} from '../constants';

export function useCountdown(initialTimestamp: number | null): string {
    const [timeRemainingMs, setTimeRemainingMs] = useState<number>(0);

    useEffect(() => {
        if (timeRemainingMs <= 0 && initialTimestamp) {
            setTimeRemainingMs(initialTimestamp && Date.now() - initialTimestamp);
        }
        const interval = setInterval(() => {
            setTimeRemainingMs((prev) => prev - MILLISECONDS_PER_SECOND);
        }, MILLISECONDS_PER_SECOND);

        return () => clearInterval(interval);
    }, [timeRemainingMs, initialTimestamp]);
    const formattedCountdown = formatCountdown(timeRemainingMs);
    return formattedCountdown;
}

function formatCountdown(totalMilliseconds: number) {
    const days = Math.floor(totalMilliseconds / MILLISECONDS_PER_DAY);
    const hours = Math.floor((totalMilliseconds % MILLISECONDS_PER_DAY) / MILLISECONDS_PER_HOUR);
    const minutes = Math.floor(
        (totalMilliseconds % MILLISECONDS_PER_HOUR) / MILLISECONDS_PER_MINUTE,
    );
    const seconds = Math.floor(
        (totalMilliseconds % MILLISECONDS_PER_MINUTE) / MILLISECONDS_PER_SECOND,
    );

    const timeUnits = [];
    if (days > 0) timeUnits.push(`${days}d`);
    if (hours > 0) timeUnits.push(`${hours}h`);
    if (minutes > 0) timeUnits.push(`${minutes}m`);
    if (seconds > 0 || timeUnits.length === 0) timeUnits.push(`${seconds}s`);

    return timeUnits.join(' ');
};
