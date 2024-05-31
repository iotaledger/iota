// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export default function formatTimestamp(timeStamp: number): string {
    const date = new Date(timeStamp);
    return new Intl.DateTimeFormat('en-US').format(date);
}

export const parseTimestamp = (timestampMs?: string | null): number | undefined => {
    if (!timestampMs) {
        return;
    }

    const timestamp = parseInt(timestampMs);
    return Number.isInteger(timestamp) ? timestamp : undefined;
};
