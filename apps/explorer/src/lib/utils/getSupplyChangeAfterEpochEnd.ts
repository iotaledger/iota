// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type EndOfEpochInfo } from '@iota/iota.js/src/client';

export function getSupplyChangeAfterEpochEnd(endOfEpochInfo: EndOfEpochInfo | null): bigint {
    return (
        BigInt(endOfEpochInfo?.mintedTokensAmount ?? 0) -
        BigInt(endOfEpochInfo?.burnTokensAmount ?? 0)
    );
}
