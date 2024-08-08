// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type EndOfEpochInfo } from '@iota/iota.js/src/client';

export function getSupplyChangeAfterEpochEnd(endOfEpochInfo: EndOfEpochInfo | null): bigint | null {
    if (endOfEpochInfo?.mintedTokensAmount == null || endOfEpochInfo.burnTokensAmount == null) return null;

    return BigInt(endOfEpochInfo.mintedTokensAmount) - BigInt(endOfEpochInfo.burnTokensAmount);
}
