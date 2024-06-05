// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type EndOfEpochInfo } from '@mysten/sui.js/client';

interface StorageFundFlow {
    netInflow: bigint | null;
    fundInflow: bigint | null;
    fundOutflow: bigint | null;
}

export function getEpochStorageFundFlow(endOfEpochInfo: EndOfEpochInfo | null): StorageFundFlow {
    const fundInflow = endOfEpochInfo
        ? BigInt(endOfEpochInfo.storageFundReinvestment) +
          BigInt(endOfEpochInfo.storageCharge) +
          BigInt(endOfEpochInfo.leftoverStorageFundInflow)
        : null;

    const fundOutflow = endOfEpochInfo ? BigInt(endOfEpochInfo.storageRebate) : null;

    const netInflow = fundInflow !== null && fundOutflow !== null ? fundInflow - fundOutflow : null;

    return { netInflow, fundInflow, fundOutflow };
}
