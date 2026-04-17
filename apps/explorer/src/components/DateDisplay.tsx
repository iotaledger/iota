// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { formatDate, type Format, useTimeAgo } from '@iota/core';
import { useEffect, useRef, useState } from 'react';
import {
    GLOBAL_DATE_TYPE,
    type DateFormat,
    type DateType,
    useDateFormat,
} from '~/contexts/dateFormatContext';

const ABSOLUTE_FORMAT: Format[] = ['day', 'month', 'year', 'hour', 'minute', 'second'];

const FORMAT_LABEL: Record<DateFormat, string> = {
    default: 'Relative time',
    local: 'Local time',
    utc: 'UTC',
};

interface DateDisplayProps {
    timestamp: number | string;
    type?: DateType;
    showTimeAgo?: boolean;
    showTooltip?: boolean;
    showHoverStyle?: boolean;
}

export function DateDisplay({
    timestamp,
    type,
    showTimeAgo,
    showTooltip = true,
    showHoverStyle = true,
}: DateDisplayProps): JSX.Element {
    const effectiveType = type ?? GLOBAL_DATE_TYPE;
    const { format, cycle } = useDateFormat(effectiveType);
    const timestampMs = Number(timestamp);

    const relativeText = useTimeAgo({ timeFrom: timestampMs, shortedTimeLabel: false });
    const [showMessage, setShowMessage] = useState(false);
    const timeoutRef = useRef<ReturnType<typeof setTimeout>>();

    useEffect(() => () => clearTimeout(timeoutRef.current), []);

    function handleClick() {
        cycle();
        clearTimeout(timeoutRef.current);

        if (showTooltip) {
            setShowMessage(true);
            timeoutRef.current = setTimeout(() => setShowMessage(false), 1000);
        }
    }

    let displayed: string;
    if (format === 'default') {
        displayed = relativeText || '--';
    } else {
        const timeZone = format === 'utc' ? 'UTC' : undefined;
        const absolute = formatDate(timestampMs, ABSOLUTE_FORMAT, timeZone);
        displayed = showTimeAgo && relativeText ? `${absolute} (${relativeText})` : absolute;
    }

    return (
        <span className="relative inline-block">
            {showMessage && (
                <span className="tooltip-bg tooltip-text-color absolute bottom-full left-1/2 mb-1 -translate-x-1/2 transform whitespace-nowrap rounded p-xs text-label-sm">
                    {`${FORMAT_LABEL[format]}`}
                </span>
            )}
            <time
                dateTime={new Date(timestampMs).toISOString()}
                onClick={handleClick}
                className={
                    showHoverStyle
                        ? 'cursor-pointer select-none text-nowrap rounded-md p-1 hover:bg-iota-neutral-96 dark:hover:bg-iota-neutral-12'
                        : 'cursor-pointer select-none'
                }
            >
                {displayed || '--'}
            </time>
        </span>
    );
}
