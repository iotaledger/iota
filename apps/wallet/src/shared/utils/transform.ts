// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { fromBase64, toBase64 } from '@iota/iota-sdk/utils';

export function toUtf8OrB64(message: string | Uint8Array) {
    const messageBytes = typeof message === 'string' ? fromBase64(message) : message;
    let messageToReturn: string = typeof message === 'string' ? message : toBase64(message);
    let type: 'utf8' | 'base64' = 'base64';
    try {
        messageToReturn = new TextDecoder('utf8', { fatal: true }).decode(messageBytes);
        type = 'utf8';
    } catch (e) {
        // do nothing
    }
    return {
        message: messageToReturn,
        type,
    };
}
