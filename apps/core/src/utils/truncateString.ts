// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export function truncateString(value: string, length: number, truncateLength?: number): string {
    if (value.length <= length) {
        return value;
    }
    if (truncateLength) {
        const startSegment = value.slice(0, truncateLength);
        const endSegment = value.slice(-truncateLength);
        return `${startSegment}...${endSegment}`;
    } else {
        const startSegment = value.slice(0, length);
        return `${startSegment}...`;
    }
}
