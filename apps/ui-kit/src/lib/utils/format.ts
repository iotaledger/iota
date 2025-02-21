// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export function truncate(value: string, startLength: number = 6, endLength: number = 6): string {
    return value.length > startLength + endLength
        ? `${value.slice(0, startLength)}...${value.slice(-endLength)}`
        : value;
}
