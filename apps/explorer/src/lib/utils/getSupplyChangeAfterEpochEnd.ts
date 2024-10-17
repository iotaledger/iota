// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type EndOfEpochInfo } from '@iota/iota-sdk/src/client';

export function getSupplyChangeAfterEpochEnd(
    endOfEpochInfo?: EndOfEpochInfo | null,
): bigint | null {
    if (endOfEpochInfo?.mintedTokensAmount == null || endOfEpochInfo?.burntTokensAmount == null)
        return null;

    return BigInt(endOfEpochInfo.mintedTokensAmount) - BigInt(endOfEpochInfo.burntTokensAmount);
}
