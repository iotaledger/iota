// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// TODO - handle multiple date formats
// Wed Aug 05
//
type Show = 'year' | 'month' | 'day' | 'hour' | 'minute' | 'second' | 'weekday';
export default function formatDate(timeStamp: number, show: Show[]): string {
    const date = new Date(timeStamp);
    if (!(date instanceof Date) || !show.length) return '';

    const OPTIONS = {
        year: 'numeric',
        month: 'short',
        day: 'numeric',
        hour: 'numeric',
        weekday: 'short',
        minute: 'numeric',
        second: 'numeric',
    };

    const formatOptions = show.reduce((accumulator, current: Show) => {
        const responseObj = {
            ...accumulator,
            ...{ [current]: OPTIONS[current] },
        };
        return responseObj;
    }, {});

    return new Intl.DateTimeFormat('en-US', formatOptions).format(date);
}
