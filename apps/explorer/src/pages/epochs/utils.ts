// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useTimeAgo } from '@iota/core';
import { useIotaClientQuery } from '@iota/dapp-kit';
import { useEffect, useRef } from 'react';

interface EpochProgress {
    epoch?: number;
    progress: number;
    label: string;
    end: number;
    start: number;
}

export function useEpochProgress(suffix: string = 'left'): EpochProgress {
    const { data, refetch } = useIotaClientQuery('getLatestIotaSystemState');
    const start = data?.epochStartTimestampMs ? Number(data.epochStartTimestampMs) : undefined;
    const duration = data?.epochDurationMs ? Number(data.epochDurationMs) : undefined;
    const end = start !== undefined && duration !== undefined ? start + duration : undefined;
    const lastEpoch = useRef<number | null>(null);
    const refetchEpochInterval = useRef<NodeJS.Timeout | null>(null);
    const currentEpoch = data?.epoch ? Number(data.epoch) : undefined;

    const time = useTimeAgo({
        timeFrom: end || null,
        shortedTimeLabel: true,
        shouldEnd: true,
    });

    // Effect to handle refetch logic
    useEffect(() => {
        // Clear any existing interval first
        if (refetchEpochInterval.current) {
            clearInterval(refetchEpochInterval.current);
            refetchEpochInterval.current = null;
        }

        if (!end) return;

        // Store current epoch if it exists
        if (currentEpoch !== undefined && lastEpoch.current !== currentEpoch) {
            lastEpoch.current = currentEpoch;
        }

        // Check if end time has expired
        const checkAndSetupInterval = () => {
            const now = Date.now();
            const isExpired = now >= end;

            if (isExpired && !refetchEpochInterval.current) {
                // End time expired, start refetching
                refetchEpochInterval.current = setInterval(() => {
                    refetch();
                }, 5000);
            }
        };

        checkAndSetupInterval();

        // Set up a timer to check again when end time is reached
        const timeToEnd = end - Date.now();
        if (timeToEnd > 0) {
            const timeoutId = setTimeout(() => {
                checkAndSetupInterval();
            }, timeToEnd);

            return () => {
                clearTimeout(timeoutId);
            };
        }

        return () => {
            if (refetchEpochInterval.current) {
                clearInterval(refetchEpochInterval.current);
                refetchEpochInterval.current = null;
            }
        };
    }, [end, refetch, currentEpoch]);

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

export function getElapsedTime(start: number, end: number): string {
    const diff = end - start;

    const seconds = Math.floor(diff / 1000);
    const minutes = Math.floor(seconds / 60);
    const hours = Math.floor(minutes / 60);
    const days = Math.floor(hours / 24);

    const displayHours = hours % 24;
    const displayMinutes = minutes % 60;
    const displaySeconds = seconds % 60;

    const renderTime: string[] = [];

    if (days > 0) {
        renderTime.push(`${days}d`);
    }
    if (displayHours > 0) {
        renderTime.push(`${displayHours}h`);
    }
    if (displayMinutes > 0) {
        renderTime.push(`${displayMinutes}m`);
    }
    if (displaySeconds > 0 || renderTime.length === 0) {
        // Ensure at least seconds are shown
        renderTime.push(`${displaySeconds}s`);
    }

    return renderTime.join(' ');
}
