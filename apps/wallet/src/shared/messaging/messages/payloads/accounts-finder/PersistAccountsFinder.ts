// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { isBasePayload } from '_payloads';
import type { BasePayload, Payload } from '_payloads';
import type { AccountFinderConfigParams } from '_src/ui/app/accounts-finder';
import { type SourceStrategyToPersist } from './types';

export interface PersistAccountsFinder extends BasePayload {
    type: 'persist-accounts-finder';
    sourceStrategy: SourceStrategyToPersist;
}

export type PersistAccountsFinderPayload = PersistAccountsFinder & AccountFinderConfigParams;

export function isPersistAccountsFinder(
    payload: Payload,
): payload is PersistAccountsFinderPayload & AccountFinderConfigParams {
    return isBasePayload(payload) && payload.type === 'persist-accounts-finder';
}
