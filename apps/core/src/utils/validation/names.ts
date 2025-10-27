// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export function shouldResolveInputAsName(value: string): boolean {
    const isDotNotation = value.includes('.') && value.endsWith('.iota');
    const isAtNotation = value.includes('@');

    return !value.startsWith('0x') && (isAtNotation || isDotNotation);
}
