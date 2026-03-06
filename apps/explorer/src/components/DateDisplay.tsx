// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { formatDate, useTimeAgo } from '@iota/core';
import { type DateType, useDateFormat } from '~/contexts/dateFormatContext';

interface DateDisplayProps {
    timestamp: number | string;
    type: DateType;
}

export function DateDisplay({ timestamp, type }: DateDisplayProps): JSX.Element {
    const { format, toggle } = useDateFormat(type);
    const timestampMs = Number(timestamp);

    const relativeText = useTimeAgo({ timeFrom: timestampMs, shortedTimeLabel: false });
    const absoluteText = formatDate(timestampMs);

    const displayed = format === 'relative' ? relativeText : absoluteText;
    const tooltip = format === 'relative' ? absoluteText : relativeText;

    return (
        <time
            dateTime={new Date(timestampMs).toISOString()}
            title={tooltip}
            onClick={toggle}
            className="cursor-pointer select-none"
        >
            {displayed || '--'}
        </time>
    );
}
