// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useTimeAgo } from '@iota/core';
import { useIotaClientQuery } from '@iota/dapp-kit';

interface EpochProgress {
    epoch?: number;
    progress: number;
    label: string;
    end: number;
    start: number;
}

export function useEpochProgress(suffix: string = 'left'): EpochProgress {
    const { data } = useIotaClientQuery('getLatestIotaSystemState');
    const start = data?.epochStartTimestampMs ? Number(data.epochStartTimestampMs) : undefined;
    const duration = data?.epochDurationMs ? Number(data.epochDurationMs) : undefined;
    const end = start !== undefined && duration !== undefined ? start + duration : undefined;
    const time = useTimeAgo({
        timeFrom: end || null,
        shortedTimeLabel: true,
        shouldEnd: true,
    });

    if (!start || !end) {
        return {
            progress: 0,
            label: '',
            end: 0,
            start: 0,
        };
    }

    const progress =
        start && duration ? Math.min(((Date.now() - start) / (end - start)) * 100, 100) : 0;

    const timeLeftMs = Date.now() - end;
    const timeLeftMin = Math.floor(timeLeftMs / 60000);

    let label;
    if (timeLeftMs >= 0) {
        label = 'Ending soon';
    } else if (timeLeftMin >= -1) {
        label = 'About a min left';
    } else {
        label = `${time} ${suffix}`;
    }

    return {
        epoch: Number(data?.epoch),
        progress,
        label,
        end,
        start,
    };
}

export function getElapsedTime(start: number, end: number) {
    const diff = end - start;

    const seconds = Math.floor(diff / 1000);
    const minutes = Math.floor(seconds / 60);
    const hours = Math.floor(minutes / 60);

    const displayMinutes = minutes - hours * 60;
    const displaySeconds = seconds - minutes * 60;

    const renderTime = [];

    if (hours > 0) {
        renderTime.push(`${hours}h`);
    }
    if (displayMinutes > 0) {
        renderTime.push(`${displayMinutes}m`);
    }
    if (displaySeconds > 0) {
        renderTime.push(`${displaySeconds}s`);
    }

    return renderTime.join(' ');
}
