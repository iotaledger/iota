// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ampli } from './analytics/ampli';

export type CopiedElementType =
    | 'address'
    | 'stardust-address'
    | 'transaction-digest'
    | 'kiosk-object-id'
    | 'stake-id'
    | 'digest';

export function trackElementCopied(elementType: CopiedElementType): void {
    ampli.copiedElement({ type: elementType });
}
