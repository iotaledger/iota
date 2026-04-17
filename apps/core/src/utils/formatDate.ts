// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export type Format = 'year' | 'month' | 'day' | 'hour' | 'minute' | 'second' | 'weekday';
export type SupportedTimeZone = 'UTC';

export function formatDate(
    date: Date | number,
    format: Format[] = ['day', 'month', 'year', 'hour', 'minute'],
    timeZone?: SupportedTimeZone,
): string {
    const dateTime = new Date(date);
    if (!(dateTime instanceof Date)) return '';

    const options = {
        year: 'numeric',
        month: 'short',
        day: 'numeric',
        hour: 'numeric',
        weekday: 'short',
        minute: 'numeric',
        second: 'numeric',
    };

    const formatOptions = format.reduce((accumulator, current: Format) => {
        const responseObj = {
            ...accumulator,
            ...{ [current]: options[current] },
        };
        return responseObj;
    }, {});

    return new Intl.DateTimeFormat('en-GB', { ...formatOptions, timeZone }).format(dateTime);
}
