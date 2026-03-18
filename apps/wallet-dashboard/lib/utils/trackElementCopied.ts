// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ampli } from './analytics/ampli';

export function trackElementCopied(elementType: string): void {
    ampli.elementCopied({ type: elementType });
}
