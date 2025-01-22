// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type IotaSignPersonalMessageOutput } from '@iota/wallet-standard';

import { isBasePayload, type BasePayload } from '../basePayload';
import { type Payload } from '../payload';

export interface SignMessageRequest extends BasePayload {
    type: 'sign-personal-message-request';
    args?: {
        message: string; // base64
        accountAddress: string;
    };
    return?: IotaSignPersonalMessageOutput;
}

export function isSignMessageRequest(payload: Payload): payload is SignMessageRequest {
    return isBasePayload(payload) && payload.type === 'sign-personal-message-request';
}
