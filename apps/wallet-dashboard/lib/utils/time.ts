// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export default function formatTimestamp(timeStamp: number): string {
    const date = new Date(timeStamp);
    return new Intl.DateTimeFormat('en-US').format(date);
}

export function parseTimestamp(timestampMs: string): number {
    const timestamp = parseInt(timestampMs, 10);
    if (!Number.isFinite(timestamp)) {
        throw new Error('Invalid timestamp');
    }
    return timestamp;
}
