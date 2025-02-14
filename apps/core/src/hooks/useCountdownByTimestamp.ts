// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useState, useEffect } from 'react';
import {
    MILLISECONDS_PER_DAY,
    MILLISECONDS_PER_HOUR,
    MILLISECONDS_PER_MINUTE,
    MILLISECONDS_PER_SECOND,
} from '../constants';

interface FormatCountdownOptions {
    showSeconds?: boolean;
    showMinutes?: boolean;
    showHours?: boolean;
    showDays?: boolean;
    hideZeroUnits?: boolean;
}

export function useCountdownByTimestamp(
    initialTimestamp: number | null,
    options?: FormatCountdownOptions,
): string {
    const [timeRemainingMs, setTimeRemainingMs] = useState<number>(0);

    useEffect(() => {
        if (timeRemainingMs <= 0 && initialTimestamp) {
            setTimeRemainingMs(initialTimestamp - Date.now());
        }
        const interval = setInterval(() => {
            setTimeRemainingMs((prev) => prev - MILLISECONDS_PER_SECOND);
        }, MILLISECONDS_PER_SECOND);

        return () => clearInterval(interval);
    }, [initialTimestamp]);
    const formattedCountdown = formatCountdown(timeRemainingMs, options);
    return formattedCountdown;
}

function formatCountdown(
    totalMilliseconds: number,
    {
        showSeconds = true,
        showMinutes = true,
        showHours = true,
        showDays = true,
        hideZeroUnits = false,
    }: FormatCountdownOptions = {},
) {
    const days = Math.floor(totalMilliseconds / MILLISECONDS_PER_DAY);
    const hours = Math.floor((totalMilliseconds % MILLISECONDS_PER_DAY) / MILLISECONDS_PER_HOUR);
    const minutes = Math.floor(
        (totalMilliseconds % MILLISECONDS_PER_HOUR) / MILLISECONDS_PER_MINUTE,
    );
    const seconds = Math.floor(
        (totalMilliseconds % MILLISECONDS_PER_MINUTE) / MILLISECONDS_PER_SECOND,
    );

    const timeUnits: string[] = [];
    if (showDays && days > 0) timeUnits.push(`${days}d`);
    if (showHours && hours > 0) timeUnits.push(`${hours}h`);
    if (showMinutes && minutes > 0) timeUnits.push(`${minutes}m`);

    // If the total time is less than 1 second, show "<1s" instead of -1s
    if (showSeconds && (seconds > 0 || timeUnits.length === 0))
        timeUnits.push(hideZeroUnits && seconds < 1 ? '<1s' : `${seconds}s`);

    return timeUnits.join(' ');
}
