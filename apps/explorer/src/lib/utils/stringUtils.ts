// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { extractMediaFileType } from '@iota/core';

export function hexToAscii(hex: string): string | undefined {
    if (!hex || typeof hex != 'string') return;
    hex = hex.replace(/^0x/, '');

    let str = '';
    for (let n = 0; n < hex.length; n += 2)
        str += String.fromCharCode(parseInt(hex.substring(n, 2), 16));

    return str;
}

export const trimStdLibPrefix = (str: string): string => str.replace(/^0x2::/, '');

export async function genFileTypeMsg(displayString: string, signal: AbortSignal): Promise<string> {
    return extractMediaFileType(displayString, signal)
        .then((result) => (result === 'Image' ? result : result.toUpperCase()))
        .then((result) => `1 ${result} File`)
        .catch((err) => {
            console.error(err);
            return `1 Image File`;
        });
}

export function isString(value: unknown): boolean {
    return typeof value === 'string';
}
