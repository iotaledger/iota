// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useGetObject } from '@mysten/core';
import { type SuiObjectData } from '@mysten/sui.js/client';

export function useOwnedNFT(nftObjectId: string | null) {
    const objectResult = useGetObject(nftObjectId);

    return {
        ...objectResult,
        data: objectResult.data?.data as SuiObjectData | null,
    };
}
