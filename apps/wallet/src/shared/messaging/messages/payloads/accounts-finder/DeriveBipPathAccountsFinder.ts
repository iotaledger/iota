// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { isBasePayload } from '_payloads';
import type { BasePayload, Payload } from '_payloads';
import { type MakeDerivationOptions } from '_src/background/account-sources/bip44Path';

export interface DeriveBipPathAccountsFinder extends BasePayload {
    type: 'derive-bip-path-accounts-finder';
    sourceID: string;
    derivationOptions: MakeDerivationOptions;
}

export type DeriveBipPathAccountsFinderPayload = DeriveBipPathAccountsFinder;

export function isDeriveBipPathAccountsFinder(
    payload: Payload,
): payload is DeriveBipPathAccountsFinderPayload {
    return isBasePayload(payload) && payload.type === 'derive-bip-path-accounts-finder';
}

export interface DeriveBipPathAccountsFindeResponsePayload extends BasePayload {
    publicKey: string;
}

export function isDeriveBipPathAccountsFinderResponse(
    payload: Payload,
): payload is DeriveBipPathAccountsFindeResponsePayload {
    return isBasePayload(payload) && payload.type === 'derive-bip-path-accounts-finder-response';
}
